//! Exploration loop tying together app lifecycle, coverage strategy, and invariant checks.

use crate::client::{
    call_ui, load_catalog_keybinds, resolve_pipe_name, AppSession, PipeError, CALL_TIMEOUT,
};
use crate::ftl::TranslationDict;
use crate::invariants::{
    check_escape_window, check_snapshot, crash_or_hang_violation, FindingLog,
};
use crate::snapshot::Snapshot;
use crate::strategy::Strategy;
use serde_json::json;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const RECENT_STEP_LIMIT: usize = 15;

/// What to do after the app closed and was asked to reopen.
enum Restart {
    Continue(Snapshot),
    Stop,
}

/// Reopens the app after it closed. A failed restart ends the run with the
/// findings collected so far instead of turning them into a process error.
fn attempt_restart(spawn: impl FnOnce() -> Result<Snapshot, String>) -> Restart {
    match spawn() {
        Ok(snapshot) => Restart::Continue(snapshot),
        Err(error) => {
            eprintln!(
                "forja-explore: could not reopen forja-app ({error}); \
                 finishing with the findings collected so far"
            );
            Restart::Stop
        }
    }
}

/// Configuration for an exploration run.
#[derive(Debug, Clone)]
pub struct ExploreConfig {
    pub app: Option<PathBuf>,
    pub pipe: Option<String>,
    pub minutes: Option<u64>,
    pub actions: Option<usize>,
    pub seed: u64,
    pub locale: String,
    pub out_dir: PathBuf,
    pub findings_path: Option<PathBuf>,
    pub trail_path: Option<PathBuf>,
    pub pick_dir: Option<PathBuf>,
    pub repo_root: Option<PathBuf>,
}

/// Summary returned after an exploration run finishes.
#[derive(Debug, Clone)]
pub struct ExploreSummary {
    pub actions: usize,
    pub screens: usize,
    pub elements: usize,
    pub keybinds: usize,
    pub findings: usize,
    pub crashes_or_hangs: usize,
    pub restarts: usize,
}

/// Runs the coverage-guided UI exploration loop.
pub fn run(config: &ExploreConfig) -> Result<ExploreSummary, String> {
    std::fs::create_dir_all(&config.out_dir).map_err(|e| e.to_string())?;
    let findings_path = config
        .findings_path
        .clone()
        .unwrap_or_else(|| config.out_dir.join("findings.jsonl"));
    let trail_path = config
        .trail_path
        .clone()
        .unwrap_or_else(|| config.out_dir.join("trail.jsonl"));

    let mut trail_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&trail_path)
        .map_err(|e| format!("could not open trail {}: {e}", trail_path.display()))?;

    let dict = TranslationDict::discover_and_load(config.repo_root.as_deref());
    let keybinds = config
        .app
        .as_deref()
        .map(|exe| load_catalog_keybinds(exe, &config.locale))
        .unwrap_or_default();

    let mut session = if let Some(app_exe) = &config.app {
        let pipe = config
            .pipe
            .as_deref()
            .map(|p| resolve_pipe_name(Some(p)))
            .unwrap_or_else(|| {
                format!(
                    r"\\.\pipe\forja-explore-{}-{}",
                    std::process::id(),
                    config.seed
                )
            });
        let pick_dir = config
            .pick_dir
            .clone()
            .unwrap_or_else(|| config.out_dir.join("sample"));
        Some(AppSession::start(
            app_exe.clone(),
            pipe,
            config.locale.clone(),
            config.out_dir.clone(),
            pick_dir,
        )?)
    } else {
        None
    };

    let external_pipe = resolve_pipe_name(config.pipe.as_deref());
    let active_pipe = |sess: &Option<AppSession>| -> String {
        sess.as_ref()
            .map(|s| s.pipe().to_owned())
            .unwrap_or_else(|| external_pipe.clone())
    };

    let mut current_snap = {
        let pipe = active_pipe(&session);
        let raw = call_ui(&pipe, "ui.snapshot", json!({}), CALL_TIMEOUT)
            .map_err(|err| format!("initial snapshot failed: {err}"))?;
        Snapshot::from_rpc_result(&raw)?
    };

    let mut findings = FindingLog::new(Some(findings_path));
    let mut strategy = Strategy::new(config.seed, keybinds);
    let mut recent_steps: VecDeque<String> = VecDeque::with_capacity(RECENT_STEP_LIMIT);
    let mut actions_done = 0usize;
    let mut crashes_or_hangs = 0usize;
    let mut restarts = 0usize;

    let deadline = config
        .minutes
        .map(|m| Instant::now() + Duration::from_secs(m.saturating_mul(60)));
    let max_actions = match (config.actions, config.minutes) {
        (Some(n), _) => Some(n),
        (None, Some(_)) => None,
        (None, None) => Some(200),
    };

    for violation in check_snapshot(&current_snap, &config.locale, &dict, &[]) {
        if findings.record(violation).map_err(|e| e.to_string())? {
            if let Some(last) = findings.findings().last() {
                println!("finding [{}]: {}", last.severity, last.title);
            }
        }
    }

    loop {
        if max_actions.is_some_and(|limit| actions_done >= limit) {
            break;
        }
        if deadline.is_some_and(|end| Instant::now() >= end) {
            break;
        }

        if let Some(sess) = session.as_mut() {
            if let Some(code) = sess.poll_exit() {
                let steps: Vec<String> = recent_steps.iter().cloned().collect();
                if code != Some(0) {
                    crashes_or_hangs += 1;
                    let detail = format!(
                        "Processo terminou com código {code:?}. stderr: {}",
                        sess.stderr_tail(8)
                    );
                    let v = crash_or_hang_violation(
                        false,
                        &current_snap.screen_label(),
                        &detail,
                        &steps,
                    );
                    let _ = findings.record(v);
                }
                restarts += 1;
                recent_steps.clear();
                match attempt_restart(|| sess.spawn_and_wait()) {
                    Restart::Continue(snapshot) => current_snap = snapshot,
                    Restart::Stop => break,
                }
                continue;
            }
        }

        let action = strategy.next_action(&current_snap);
        let step_json = action.to_drive_json();
        if recent_steps.len() == RECENT_STEP_LIMIT {
            recent_steps.pop_front();
        }
        recent_steps.push_back(step_json.clone());
        let _ = writeln!(trail_file, "{step_json}");
        let _ = trail_file.flush();

        let (method, params) = action.to_rpc();
        let pipe = active_pipe(&session);
        let steps_vec: Vec<String> = recent_steps.iter().cloned().collect();

        match call_ui(&pipe, method, params, CALL_TIMEOUT) {
            Ok(raw) => {
                let next_snap = Snapshot::from_rpc_result(&raw)?;
                if action.is_escape() {
                    if let Some(v) = check_escape_window(&current_snap, &next_snap, &steps_vec) {
                        if findings.record(v).map_err(|e| e.to_string())? {
                            if let Some(last) = findings.findings().last() {
                                println!("finding [{}]: {}", last.severity, last.title);
                            }
                        }
                    }
                }
                for v in check_snapshot(&next_snap, &config.locale, &dict, &steps_vec) {
                    if findings.record(v).map_err(|e| e.to_string())? {
                        if let Some(last) = findings.findings().last() {
                            println!("finding [{}]: {}", last.severity, last.title);
                        }
                    }
                }
                current_snap = next_snap;
                actions_done += 1;
            }
            Err(PipeError::Rpc(_)) => {
                // Element vanished or transient mismatch; refresh snapshot if possible.
                if let Ok(raw) = call_ui(&pipe, "ui.snapshot", json!({}), CALL_TIMEOUT) {
                    if let Ok(snap) = Snapshot::from_rpc_result(&raw) {
                        current_snap = snap;
                    }
                }
                actions_done += 1;
            }
            Err(PipeError::Timeout) => {
                crashes_or_hangs += 1;
                let v = crash_or_hang_violation(
                    true,
                    &current_snap.screen_label(),
                    "Sem resposta em 10 s pelo pipe RPC.",
                    &steps_vec,
                );
                if findings.record(v).map_err(|e| e.to_string())? {
                    if let Some(last) = findings.findings().last() {
                        println!("finding [{}]: {}", last.severity, last.title);
                    }
                }
                if let Some(sess) = session.as_mut() {
                    restarts += 1;
                    recent_steps.clear();
                    match attempt_restart(|| sess.spawn_and_wait()) {
                        Restart::Continue(snapshot) => current_snap = snapshot,
                        Restart::Stop => break,
                    }
                } else {
                    break;
                }
            }
            Err(PipeError::NotRunning | PipeError::Io(_)) => {
                if let Some(sess) = session.as_mut() {
                    std::thread::sleep(Duration::from_millis(200));
                    let code = sess.poll_exit().flatten();
                    if code != Some(0) {
                        crashes_or_hangs += 1;
                        let detail = format!(
                            "Pipe fechou (código {code:?}). stderr: {}",
                            sess.stderr_tail(8)
                        );
                        let v = crash_or_hang_violation(
                            false,
                            &current_snap.screen_label(),
                            &detail,
                            &steps_vec,
                        );
                        if findings.record(v).map_err(|e| e.to_string())? {
                            if let Some(last) = findings.findings().last() {
                                println!("finding [{}]: {}", last.severity, last.title);
                            }
                        }
                    }
                    restarts += 1;
                    recent_steps.clear();
                    match attempt_restart(|| sess.spawn_and_wait()) {
                        Restart::Continue(snapshot) => current_snap = snapshot,
                        Restart::Stop => break,
                    }
                } else {
                    crashes_or_hangs += 1;
                    let v = crash_or_hang_violation(
                        false,
                        &current_snap.screen_label(),
                        "A conexão com o pipe foi encerrada.",
                        &steps_vec,
                    );
                    let _ = findings.record(v);
                    break;
                }
            }
        }
    }

    if let Some(sess) = session.as_mut() {
        sess.kill();
    }

    Ok(ExploreSummary {
        actions: actions_done,
        screens: strategy.screens_covered(),
        elements: strategy.elements_covered(),
        keybinds: strategy.keybinds_covered(),
        findings: findings.count(),
        crashes_or_hangs,
        restarts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Element, WindowSize};

    fn snapshot(label: &str) -> Snapshot {
        Snapshot {
            window: WindowSize {
                width: 1280.0,
                height: 800.0,
            },
            changes: Default::default(),
            view: String::new(),
            elements: vec![Element {
                id: 1,
                role: "Window".to_owned(),
                name: label.to_owned(),
                rect: [10.0, 10.0, 400.0, 300.0],
                clickable: false,
                editable: false,
                state: Vec::new(),
            }],
        }
    }

    #[test]
    fn a_failed_restart_ends_the_run_without_an_error() {
        let outcome = attempt_restart(|| {
            Err("timed out waiting for forja-app --test-driver pipe".to_owned())
        });
        assert!(matches!(outcome, Restart::Stop));
    }

    #[test]
    fn a_successful_restart_resumes_the_run() {
        let restored = snapshot("Projetos");
        match attempt_restart(|| Ok(restored)) {
            Restart::Continue(snapshot) => assert_eq!(snapshot.screen_label(), "Projetos"),
            Restart::Stop => panic!("a successful restart must resume the run"),
        }
    }
}
