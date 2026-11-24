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

pub(crate) fn same_path(left: &str, right: &str) -> bool {
    left.replace('/', "\\")
        .trim_end_matches('\\')
        .eq_ignore_ascii_case(right.replace('/', "\\").trim_end_matches('\\'))
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

/// Verdadeiro quando o run encerrou com falha de processo.
pub(crate) fn is_failed(run: &Run) -> bool {
    run.last_event.as_deref() == Some("process failed")
}

pub(crate) fn update_exited_run(cx: &mut Ctx<'_>, core: &Shared, terminal: &TerminalRef, code: Option<i32>) {
    let failed = code.is_some_and(|code| code != 0);
    let event = if failed { "process failed" } else { "process exited" };
    let status = if failed {
        HelperStatus::Error
    } else {
        HelperStatus::Finished
    };
    update_run_inner(
        cx,
        core,
        &terminal.terminal_id,
        status,
        Some(event.to_owned()),
        failed,
    );
}

/// Registra uma falha de lançamento como execução em estado de erro.
///
/// Usado quando o processo nem chega a abrir (por exemplo, executável ausente):
/// não há terminal para associar, mas a interface ainda precisa mostrar `Erro`
/// em vez de `Ocioso`/`Parado`.
pub(crate) fn record_launch_failure(
    cx: &mut Ctx<'_>,
    core: &Shared,
    bench_path: &str,
    profile_id: Option<&str>,
    raw_error: &str,
) {
    let profile_id = profile_id.unwrap_or("unknown").to_owned();
    let run = Run {
        terminal_id: format!("launch-error:{profile_id}"),
        profile_id: profile_id.clone(),
        bench_path: bench_path.to_owned(),
        started_at: chrono::Utc::now().to_rfc3339(),
        hook_token: String::new(),
        status: HelperStatus::Error,
        last_event: Some("launch failed".to_owned()),
    };
    {
        let Ok(mut state) = core.state.lock() else {
            return;
        };
        state.runs.insert(run.terminal_id.clone(), run.clone());
        state.last_launch_error = Some(raw_error.to_owned());
        state.refresh_awake();
    }
    let _ = cx.emit(&HelperStatusChanged {
        bench_path: run.bench_path.clone(),
        terminal_id: Some(run.terminal_id.clone()),
        profile_id: run.profile_id.clone(),
        status: HelperStatus::Error,
    });
}

/// Remove a execução de erro de lançamento quando o mesmo perfil e bancada
/// voltam a funcionar, para que a lista não mostre um `Erro` fantasma.
pub(crate) fn clear_launch_failure_run(
    state: &mut crate::HelperState,
    profile_id: &str,
    bench_path: &str,
) -> bool {
    let error_id = format!("launch-error:{profile_id}");
    let matches = state
        .runs
        .get(&error_id)
        .is_some_and(|run| same_path(&run.bench_path, bench_path));
    if matches {
        state.runs.remove(&error_id);
    }
    matches
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
    if failed || status == HelperStatus::Error {
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
