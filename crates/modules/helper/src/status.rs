//! Status dos ajudantes: inferência pela saída e correlação de hooks.

use crate::Shared;
use forja_contracts::{HelperStatus, HelperStatusChanged, TerminalRef};
use forja_kernel::Ctx;
use std::collections::BTreeMap;

/// Um ajudante ativo, ligado a um terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// Terminal dono.
    pub terminal_id: String,
    /// Perfil usado.
    pub profile_id: String,
    /// Bancada.
    pub bench_path: String,
    /// Início em RFC 3339.
    pub started_at: String,
    /// Segredo volátil que autoriza hooks desta execução.
    pub hook_token: String,
    /// Estado atual.
    pub status: HelperStatus,
    /// Último evento recebido.
    pub last_event: Option<String>,
}

/// Deduz o estado pelo título e pela saída recente.
pub fn infer_status(title: &str, output: &str) -> Option<HelperStatus> {
    let text = format!("{title}\n{output}").to_lowercase();
    if [
        "needs input",
        "permission required",
        "approve this",
        "would you like",
        "allow this",
        "[y/n]",
        "press enter to continue",
    ]
    .iter()
    .any(|pattern| text.contains(pattern))
    {
        Some(HelperStatus::NeedsYou)
    } else if [
        "working",
        "thinking",
        "running tool",
        "executing",
        "generating",
        "esc to interrupt",
    ]
    .iter()
    .any(|pattern| text.contains(pattern))
    {
        Some(HelperStatus::Working)
    } else if ["completed", "task complete", "ready for", "idle"]
        .iter()
        .any(|pattern| text.contains(pattern))
    {
        Some(HelperStatus::Idle)
    } else {
        None
    }
}

/// Correlaciona um payload de hook a um run.
/// Correlaciona somente com a execução que recebeu o segredo no ambiente do processo.
pub fn match_hook_run<'a>(
    runs: &'a BTreeMap<String, Run>,
    profile_id: &str,
    hook_token: &str,
    cwd: &str,
) -> Option<&'a Run> {
    runs.values().find(|run| {
        run.profile_id == profile_id
            && run.hook_token == hook_token
            && same_path(&run.bench_path, cwd)
    })
}

fn same_path(left: &str, right: &str) -> bool {
    left.replace('/', "\\")
        .eq_ignore_ascii_case(&right.replace('/', "\\"))
}

/// Atualiza um run pelo terminal e publica o evento quando o estado muda.
pub(crate) fn update_run(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal: &TerminalRef,
    status: HelperStatus,
    event: Option<String>,
) {
    update_run_by_id(cx, core, &terminal.terminal_id, status, event);
}

/// Atualiza um run pelo id do terminal.
pub(crate) fn update_run_by_id(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: &str,
    status: HelperStatus,
    event: Option<String>,
) {
    update_run_inner(cx, core, terminal_id, status, event, false);
}

pub(crate) fn update_exited_run(cx: &mut Ctx<'_>, core: &Shared, terminal: &TerminalRef, code: Option<i32>) {
    update_run_inner(cx, core, &terminal.terminal_id, HelperStatus::Finished,
        Some("process exited".to_owned()), code.is_some_and(|code| code != 0));
}

fn update_run_inner(
    cx: &mut Ctx<'_>, core: &Shared, terminal_id: &str, status: HelperStatus,
    event: Option<String>, failed: bool,
) {
    let changed = {
        let Ok(mut state) = core.state.lock() else {
            return;
        };
        let Some(run) = state.runs.get_mut(terminal_id) else {
            return;
        };
        if run.status == status && event.is_none() {
            return;
        }
        run.status = status;
        if let Some(event) = event {
            run.last_event = Some(event);
        }
        let snapshot = run.clone();
        state.refresh_awake();
        snapshot
    };
    let _ = cx.emit(&HelperStatusChanged {
        bench_path: changed.bench_path.clone(),
        terminal_id: Some(changed.terminal_id.clone()),
        profile_id: changed.profile_id.clone(),
        status,
    });
    notify(cx, &changed, status, failed);
}

fn notification_keys(status: HelperStatus, failed: bool) -> Option<(&'static str, &'static str, &'static str)> {
    if failed {
        Some(("helper.notifications.error", "helper.notify.error-title", "helper.notify.error-body"))
    } else {
        match status {
            HelperStatus::NeedsYou => Some(("helper.notifications.waiting", "helper.notify.waiting-title", "helper.notify.waiting-body")),
            HelperStatus::Finished => Some(("helper.notifications.done", "helper.notify.done-title", "helper.notify.done-body")),
            _ => None,
        }
    }
}

fn notify(cx: &mut Ctx<'_>, run: &Run, status: HelperStatus, failed: bool) {
    let Some((enabled_key, title_key, body_key)) = notification_keys(status, failed) else { return; };
    if !cx
        .setting(enabled_key)
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
    {
        return;
    }
    let mut args = forja_kernel::Args::new();
    args.insert("title", serde_json::json!(cx.t(title_key)));
    args.insert(
        "body",
        serde_json::json!(cx.t_args(
            body_key,
            &[
                ("profile", run.profile_id.as_str()),
                ("bench", run.bench_path.as_str())
            ]
        )),
    );
    args.insert("bench_path", serde_json::json!(run.bench_path));
    let _ = cx.run_command("notify.show", args);
}

/// Lê o terminal e infere o status pelo título e pela saída.
pub(crate) fn infer_from_terminal(cx: &mut Ctx<'_>, core: &Shared, terminal: &TerminalRef) {
    let Some(text) = crate::read_output(cx, &terminal.terminal_id) else {
        return;
    };
    if let Some(status) = infer_status(&terminal.title, &text) {
        update_run(
            cx,
            core,
            terminal,
            status,
            Some("terminal output".to_owned()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(terminal: &str, bench: &str) -> Run {
        Run {
            terminal_id: terminal.to_owned(),
            profile_id: "codex".to_owned(),
            bench_path: bench.to_owned(),
            started_at: "2026-11-12T12:00:00Z".to_owned(),
            hook_token: format!("token-{terminal}"),
            status: HelperStatus::Working,
            last_event: None,
        }
    }

    #[test]
    fn inference_prefers_waiting_then_working_then_idle() {
        assert_eq!(
            infer_status("claude", "Permission required"),
            Some(HelperStatus::NeedsYou)
        );
        assert_eq!(
            infer_status("codex", "Running tool Bash"),
            Some(HelperStatus::Working)
        );
        assert_eq!(
            infer_status("gemini", "Task complete"),
            Some(HelperStatus::Idle)
        );
        assert_eq!(infer_status("x", "nothing useful"), None);
    }

    #[test]
    fn failed_exit_uses_error_notification_instead_of_done() {
        assert_eq!(notification_keys(HelperStatus::Finished, true).unwrap().0, "helper.notifications.error");
        assert_eq!(notification_keys(HelperStatus::Finished, false).unwrap().0, "helper.notifications.done");
    }

    #[test]
    fn hooks_require_matching_profile_token_and_bench() {
        let mut runs = BTreeMap::new();
        runs.insert("t1".to_owned(), run("t1", "C:/a"));
        assert_eq!(
            match_hook_run(&runs, "codex", "token-t1", "C:/A").map(|run| run.terminal_id.as_str()),
            Some("t1")
        );
        assert!(match_hook_run(&runs, "codex", "wrong", "C:/a").is_none());
        assert!(match_hook_run(&runs, "another-profile", "token-t1", "C:/a").is_none());
        assert!(match_hook_run(&runs, "codex", "token-t1", "C:/other").is_none());
        runs.insert("t2".to_owned(), run("t2", "C:/b"));
        assert!(match_hook_run(&runs, "codex", "missing", "C:/b").is_none());
    }
}
