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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadyState {
    /// Ajudante respondeu ao sinal de prontidão esperado.
    pub ready: bool,
    /// O prazo `helper.launch.ready_timeout_s` expirou antes de ficar pronto.
    pub timed_out: bool,
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

/// Entrega pendente após um curto prazo quando o perfil não declara sinal de prontidão.
pub(crate) fn schedule_fallback_send(
    cx: &mut Ctx<'_>,
    core: &Shared,
    terminal_id: String,
    delay: Duration,
) {
    let owned = core.clone();
    cx.spawn(
        move || std::thread::sleep(delay),
        move |cx, ()| flush_deferred_sends(cx, &owned, &terminal_id),
    );
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

/// Entrega qualquer texto pendente aguardando prontidão para `terminal_id`.
pub(crate) fn flush_deferred_sends(cx: &mut Ctx<'_>, core: &Shared, terminal_id: &str) {
    let payload = core
        .with_state(|state| state.launch.deferred_sends.remove(terminal_id))
        .ok()
        .flatten();
    let Some(parts) = payload else {
        return;
    };
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
mod tests {
    use super::*;
    use crate::{status::Run, Core, HelperModule};
    use forja_kernel::{Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-ready-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    struct TerminalStub {
        output: Arc<Mutex<String>>,
        sends: Arc<Mutex<Vec<Value>>>,
        closes: Arc<Mutex<Vec<Value>>>,
    }

    impl Module for TerminalStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("terminal", "helper.name", "0.0.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            let out = Arc::clone(&self.output);
            r.command("terminal.read", "helper.name", move |_, _| {
                let text = out.lock().unwrap().clone();
                Ok(json!({ "text": text, "alive": true }))
            });
            let sends = Arc::clone(&self.sends);
            r.command("terminal.send", "helper.name", move |_, args| {
                sends.lock().unwrap().push(args.into_json());
                Ok(json!({}))
            });
            let closes = Arc::clone(&self.closes);
            r.command("terminal.close", "helper.name", move |_, args| {
                closes.lock().unwrap().push(args.into_json());
                Ok(json!({}))
            });
        }
    }

    struct NotifyStub(Arc<Mutex<Vec<Value>>>);

    impl Module for NotifyStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("notify", "helper.name", "0.0.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            let notifies = Arc::clone(&self.0);
            r.command("notify.show", "helper.name", move |_, args| {
                notifies.lock().unwrap().push(args.into_json());
                Ok(json!({}))
            });
        }
    }

    struct WatchStub {
        core: Arc<Core>,
        watch_timeout: Duration,
        delayed_prompt: Option<String>,
        bench_path: String,
    }

    impl Module for WatchStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("test", "helper.name", "0.0.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            let core = Arc::clone(&self.core);
            let timeout = self.watch_timeout;
            let delayed = self.delayed_prompt.clone();
            let bench = self.bench_path.clone();
            r.command("test.watch", "helper.name", move |cx, args| {
                let tid = args.required_str("terminal_id")?.to_owned();
                let name = args.str("name").unwrap_or("Fake Helper").to_owned();
                start_watch(
                    cx,
                    &core,
                    tid,
                    bench.clone(),
                    name,
                    ReadyMatcher::from_patterns(vec!["READY>".into()]),
                    timeout,
                    delayed.clone(),
                );
                Ok(json!({}))
            });
        }
    }

    #[test]
    fn matcher_uses_profile_prompt_and_patterns_before_fallback() {
        let matcher = ReadyMatcher::from_patterns(vec!["READY>".into(), "Type message".into()]);
        assert!(!matcher.matches("", "Starting engine..."));
        assert!(matcher.matches("", "Welcome!\nready> "));
        assert!(matcher.matches("", "Type message to begin"));

        let fallback = ReadyMatcher::from_patterns(Vec::new());
        assert!(!fallback.matches("", "   "));
        assert!(!fallback.matches("", "Loading model..."));
        assert!(!fallback.matches("", "Claude ready"));
        assert!(!ReadyMatcher::accepts_hook(Some("status")));
        assert!(ReadyMatcher::accepts_hook(Some("ready")));
    }

    #[test]
    fn ready_signal_delivers_deferred_prompt_without_timeout_warning() {
        let dir = temp_dir("ready-ok");
        let core = Arc::new(Core::new());
        let output = Arc::new(Mutex::new("Loading...".to_owned()));
        let sends = Arc::new(Mutex::new(Vec::new()));
        let closes = Arc::new(Mutex::new(Vec::new()));
        let notifies = Arc::new(Mutex::new(Vec::new()));

        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(TerminalStub {
                    output: Arc::clone(&output),
                    sends: Arc::clone(&sends),
                    closes: Arc::clone(&closes),
                }),
                Box::new(NotifyStub(Arc::clone(&notifies))),
                Box::new(WatchStub {
                    core: Arc::clone(&core),
                    watch_timeout: Duration::from_secs(2),
                    delayed_prompt: Some("hello world".into()),
                    bench_path: dir.0.display().to_string(),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .unwrap();

        core.with_state(|state| {
            state.runs.insert(
                "term-r1".into(),
                Run {
                    terminal_id: "term-r1".into(),
                    profile_id: "fake".into(),
                    bench_path: dir.0.display().to_string(),
                    started_at: String::new(),
                    hook_token: String::new(),
                    status: HelperStatus::Working,
                    last_event: Some("launch".into()),
                },
            );
        })
        .unwrap();

        let mut watch_args = Args::new();
        watch_args.insert("terminal_id", json!("term-r1"));
        watch_args.insert("name", json!("Fake Helper"));
        runtime.run_command("test.watch", watch_args).unwrap();

        std::thread::sleep(Duration::from_millis(60));
        *output.lock().unwrap() = "Boot done\nREADY> ".to_owned();
        for _ in 0..10 {
            runtime.poll_tasks();
            if core
                .with_state(|s| s.launch.ready_states.get("term-r1").is_some_and(|r| r.ready))
                .unwrap()
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let ready = core
            .with_state(|s| s.launch.ready_states.get("term-r1").copied().unwrap())
            .unwrap();
        assert!(ready.ready);
        assert!(!ready.timed_out);
        assert_eq!(sends.lock().unwrap().len(), 1);
        assert_eq!(sends.lock().unwrap()[0]["text"], "hello world");
        assert!(notifies.lock().unwrap().is_empty());
        assert!(closes.lock().unwrap().is_empty());
    }

    #[test]
    fn timeout_warns_on_bench_and_log_without_killing_process() {
        let dir = temp_dir("timeout");
        let core = Arc::new(Core::new());
        let output = Arc::new(Mutex::new("Still loading model...".to_owned()));
        let sends = Arc::new(Mutex::new(Vec::new()));
        let closes = Arc::new(Mutex::new(Vec::new()));
        let notifies = Arc::new(Mutex::new(Vec::new()));

        let bench = dir.0.display().to_string();
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(TerminalStub {
                    output,
                    sends: Arc::clone(&sends),
                    closes: Arc::clone(&closes),
                }),
                Box::new(NotifyStub(Arc::clone(&notifies))),
                Box::new(WatchStub {
                    core: Arc::clone(&core),
                    watch_timeout: Duration::from_millis(40),
                    delayed_prompt: Some("never sent before timeout".into()),
                    bench_path: bench.clone(),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .unwrap();

        core.with_state(|state| {
            state.runs.insert(
                "term-timeout".into(),
                Run {
                    terminal_id: "term-timeout".into(),
                    profile_id: "fake".into(),
                    bench_path: bench.clone(),
                    started_at: String::new(),
                    hook_token: String::new(),
                    status: HelperStatus::Working,
                    last_event: Some("launch".into()),
                },
            );
        })
        .unwrap();

        let mut watch_args = Args::new();
        watch_args.insert("terminal_id", json!("term-timeout"));
        watch_args.insert("name", json!("Slow Helper"));
        runtime.run_command("test.watch", watch_args).unwrap();

        for _ in 0..15 {
            std::thread::sleep(Duration::from_millis(25));
            runtime.poll_tasks();
            if core
                .with_state(|s| {
                    s.launch.ready_states
                        .get("term-timeout")
                        .is_some_and(|r| r.timed_out)
                })
                .unwrap()
            {
                break;
            }
        }

        let ready = core
            .with_state(|s| s.launch.ready_states.get("term-timeout").copied().unwrap())
            .unwrap();
        assert!(!ready.ready);
        assert!(ready.timed_out);
        assert!(closes.lock().unwrap().is_empty(), "process must NOT be killed");
        let notices = notifies.lock().unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0]["level"], "warn");
        assert_eq!(notices[0]["bench_path"], bench);
        assert!(sends.lock().unwrap().is_empty());
        assert!(core
            .with_state(|s| s.launch.bench_notices.contains_key(&context::path_key(&dir.0)))
            .unwrap());
    }
}
