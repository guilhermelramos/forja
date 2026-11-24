//! Espera não bloqueante por sinal de prontidão do ajudante e aviso sem matar ao estourar.

use crate::context;
use crate::Shared;
use forja_contracts::HelperStatus;
use forja_kernel::logging::{self, LogLevel};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Estado de prontidão de uma execução no terminal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadyState {
    /// Ajudante respondeu ao sinal de prontidão esperado.
    pub ready: bool,
    /// O prazo `helper.launch.ready_timeout_s` expirou antes de ficar pronto.
    pub timed_out: bool,
    /// `started_at` da execução que sinalizou prontidão (identidade do processo).
    pub identity: Option<String>,
}

/// Critério verificável para saber quando a TUI/CLI está pronta para receber contexto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadyMatcher {
    patterns: Vec<String>,
}

impl ReadyMatcher {
    /// Constrói a partir dos padrões declarados pelo perfil (`[ready].prompt` e `[ready].patterns`).
    pub fn from_patterns(patterns: Vec<String>) -> Self {
        let patterns = patterns
            .into_iter()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .collect();
        Self { patterns }
    }

    /// Verdadeiro apenas quando `title` ou `output` traz um sinal que o perfil
    /// declarou. Um banner de carregamento ou pedido de permissão não conta.
    pub fn matches(&self, title: &str, output: &str) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let title_lower = title.to_ascii_lowercase();
        let output_lower = output.to_ascii_lowercase();
        self.patterns.iter().any(|pattern| {
            let needle = pattern.to_ascii_lowercase();
            output_lower.contains(&needle) || title_lower.contains(&needle)
        })
    }

    /// Hook específico que sinaliza prontidão; qualquer evento de status não serve.
    pub fn accepts_hook(event: Option<&str>) -> bool {
        event
            .map(|value| {
                let lower = value.to_ascii_lowercase();
                lower.contains("ready") || lower == "sessionstart"
            })
            .unwrap_or(false)
    }
}

/// Prazo configurado em `helper.launch.ready_timeout_s` (5..=120s, padrão 30s).
pub fn timeout_from_settings(cx: &Ctx<'_>) -> Duration {
    let seconds = cx
        .setting("helper.launch.ready_timeout_s")
        .and_then(|value| value.as_i64())
        .unwrap_or(30)
        .clamp(5, 120) as u64;
    Duration::from_secs(seconds)
}

/// Entrega o texto pendente somente ao ajudante ativo que sinalizou prontidão.
pub(crate) fn flush_deferred_sends(cx: &mut Ctx<'_>, core: &Shared, terminal_id: &str) {
    let payload = core
        .with_state(|state| {
            let ready = state.launch.ready_states.get(terminal_id);
            let valid = state
                .runs
                .get(terminal_id)
                .filter(|run| !matches!(run.status, HelperStatus::Finished))
                .is_some_and(|run| {
                    ready.is_some_and(|entry| entry.ready && entry.identity.as_deref() == Some(run.started_at.as_str()))
                });
            let payload = state.launch.deferred_sends.remove(terminal_id);
            if valid {
                payload
            } else {
                None
            }
        })
        .ok()
        .flatten();
    let Some(parts) = payload else {
        return;
    };
    let mut read_args = Args::new();
    read_args.insert("terminal_id", json!(terminal_id));
    read_args.insert("lines", json!(1));
    let alive = cx
        .run_command("terminal.read", read_args)
        .ok()
        .and_then(|value| value.get("alive").and_then(Value::as_bool))
        .unwrap_or(false);
    if !alive {
        return;
    }
    let text = parts
        .into_iter()
        .map(|part| part.trim().to_owned())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if text.is_empty() {
        return;
    }
    let mut send = Args::new();
    send.insert("terminal_id", json!(terminal_id));
    send.insert("text", json!(text));
    send.insert("enter", json!(true));
    let _ = cx.run_command("terminal.send", send);
}

/// Inicia a espera assíncrona de prontidão sem bloquear a UI nem matar o processo ao vencer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn start_watch(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: String,
    bench_path: String,
    profile_name: String,
    matcher: ReadyMatcher,
    timeout: Duration,
    delayed_prompt: Option<String>,
) {
    let _ = core.with_state(|state| {
        state
            .launch.ready_states
            .insert(terminal_id.clone(), ReadyState::default());
        if let Some(prompt) = delayed_prompt
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
        {
            state
                .launch.deferred_sends
                .entry(terminal_id.clone())
                .or_default()
                .push(prompt);
        }
    });
    let deadline = Instant::now() + timeout;
    schedule_poll(
        cx,
        core,
        terminal_id,
        bench_path,
        profile_name,
        matcher,
        deadline,
    );
}

fn schedule_poll(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: String,
    bench_path: String,
    profile_name: String,
    matcher: ReadyMatcher,
    deadline: Instant,
) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let wait = remaining.min(POLL_INTERVAL);
    let owned = Arc::clone(core);
    cx.spawn(
        move || {
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        },
        move |cx, ()| {
            on_poll_tick(
                cx,
                &owned,
                terminal_id,
                bench_path,
                profile_name,
                matcher,
                deadline,
            );
        },
    );
}

fn on_poll_tick(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: String,
    bench_path: String,
    profile_name: String,
    matcher: ReadyMatcher,
    deadline: Instant,
) {
    let state_snapshot = core
        .with_state(|state| {
            let run = state.runs.get(&terminal_id)?;
            let finished = matches!(run.status, HelperStatus::Finished);
            let ready = state
                .launch.ready_states
                .get(&terminal_id)
                .is_some_and(|entry| entry.ready);
            Some((finished, ready))
        })
        .ok()
        .flatten();
    let Some((finished, already_ready)) = state_snapshot else {
        return;
    };
    if finished {
        return;
    }
    if already_ready {
        flush_deferred_sends(cx, core, &terminal_id);
        return;
    }

    let mut read_args = Args::new();
    read_args.insert("terminal_id", json!(terminal_id.clone()));
    read_args.insert("lines", json!(40));
    if let Ok(read) = cx.run_command("terminal.read", read_args) {
        let alive = read.get("alive").and_then(Value::as_bool);
        let read_error = read
            .get("error")
            .and_then(Value::as_str)
            .is_some_and(|err| !err.trim().is_empty());
        if alive == Some(false) || read_error {
            return;
        }
        let text = read.get("text").and_then(Value::as_str).unwrap_or("");
        if matcher.matches("", text) {
            let _ = core.with_state(|state| {
                let entry = state.launch.ready_states.entry(terminal_id.clone()).or_default();
                entry.ready = true;
                entry.timed_out = false;
                if let Some(run) = state.runs.get_mut(&terminal_id) {
                    entry.identity = Some(run.started_at.clone());
                    if run.last_event.as_deref() == Some("launch") {
                        run.last_event = Some("ready".to_owned());
                    }
                }
            });
            flush_deferred_sends(cx, core, &terminal_id);
            return;
        }
    }

    if Instant::now() >= deadline {
        handle_timeout(cx, core, &terminal_id, &bench_path, &profile_name);
        return;
    }

    schedule_poll(
        cx,
        core,
        terminal_id,
        bench_path,
        profile_name,
        matcher,
        deadline,
    );
}

fn handle_timeout(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: &str,
    bench_path: &str,
    profile_name: &str,
) {
    let secs = cx
        .setting("helper.launch.ready_timeout_s")
        .and_then(|value| value.as_i64())
        .unwrap_or(30);
    let secs_text = secs.to_string();
    let title = cx.t("helper.ready.timeout-title");
    let body = cx.t_args(
        "helper.ready.timeout-body",
        &[("name", profile_name), ("seconds", &secs_text)],
    );
    let _ = core.with_state(|state| {
        let entry = state
            .launch.ready_states
            .entry(terminal_id.to_owned())
            .or_default();
        entry.timed_out = true;
        if let Some(run) = state.runs.get_mut(terminal_id) {
            run.last_event = Some("ready timeout".to_owned());
        }
        state
            .launch.bench_notices
            .insert(context::path_key(Path::new(bench_path)), body.clone());
    });
    logging::log(
        LogLevel::Warn,
        format!(
            "forja: helper `{profile_name}` on `{bench_path}` did not become ready within {secs}s (process kept running)"
        ),
    );
    let mut notify = Args::new();
    notify.insert("title", json!(title));
    notify.insert("body", json!(body));
    notify.insert("level", json!("warn"));
    notify.insert("bench_path", json!(bench_path));
    let _ = cx.run_command("notify.show", notify);
}

#[cfg(test)]
mod tests;
