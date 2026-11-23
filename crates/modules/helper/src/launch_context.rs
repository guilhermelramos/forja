//! Registro e estado das opções de contexto, prontidão e retomada do ajudante.
//!
//! Mantém fora de `lib.rs` o registro das quatro chaves ligadas em #204, os
//! eventos de ativação de bancada/prontidão e a estado correspondente.

use crate::{context, ready, resume, Shared};
use forja_contracts::{names, BenchActivated, RequestRef, TerminalCommandFinished};
use forja_kernel::{Args, Ctx, Registrar, SettingKind, SettingSpec, SettingValue};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Estado em memória do contexto inicial, prontidão e retomada.
#[derive(Default)]
pub struct State {
    /// Cache assíncrono dos arquivos de regras lidos por `(bancada, candidatos)`.
    pub rules_cache: BTreeMap<context::RulesCacheKey, Vec<context::RuleSnippet>>,
    /// Leituras assíncronas de regras em andamento.
    pub rules_loading: BTreeSet<context::RulesCacheKey>,
    /// Estado de prontidão de cada execução por `terminal_id`.
    pub ready_states: BTreeMap<String, ready::ReadyState>,
    /// Textos aguardando o ajudante sinalizar prontidão.
    pub deferred_sends: BTreeMap<String, Vec<String>>,
    /// Último aviso registrado para uma bancada.
    pub bench_notices: BTreeMap<String, String>,
    /// Retomadas pendentes (`resume_policy = "ask"`).
    pub pending_resumes: BTreeMap<String, resume::PendingResume>,
    /// Avisos quando `auto` cai para `never`.
    pub resume_notices: BTreeMap<String, String>,
}

/// Identidade e texto necessários para entregar o contexto inicial pelo destino certo.
pub(crate) struct ContextDelivery {
    pub bench: PathBuf,
    pub candidates: Vec<String>,
    pub context_mode: String,
    pub request_ref: Option<RequestRef>,
    pub bench_ctx: Option<context::BenchContext>,
    pub explicit_prompt: Option<String>,
    pub chat_opened: bool,
    pub chat_session: Option<String>,
    pub terminal_id: Option<String>,
}

/// Carrega as regras (em tarefa do kernel quando necessário) e só depois monta
/// e entrega o primeiro prompt, na mesma ordem para chat e terminal.
pub(crate) fn deliver_context(cx: &mut Ctx<'_>, core: &Shared, delivery: ContextDelivery) {
    let key = context::cache_key(&delivery.bench, &delivery.candidates);
    let cached = core
        .with_state(|state| state.launch.rules_cache.get(&key).cloned())
        .ok()
        .flatten();
    if let Some(rules) = cached {
        let text = compose(&delivery, Some(&rules));
        send(cx, core, &delivery, text);
        return;
    }
    let owned = Arc::clone(core);
    let bench_bg = delivery.bench.clone();
    let candidates_bg = delivery.candidates.clone();
    cx.spawn(
        move || context::read_rules_files(&bench_bg, &candidates_bg),
        move |cx, rules| {
            let _ = owned.with_state(|state| {
                state.launch.rules_cache.insert(key, rules.clone());
            });
            let text = compose(&delivery, Some(&rules));
            send(cx, &owned, &delivery, text);
        },
    );
}

fn compose(delivery: &ContextDelivery, rules: Option<&[context::RuleSnippet]>) -> Option<String> {
    context::compose_initial_prompt(
        &delivery.context_mode,
        delivery.bench_ctx.as_ref(),
        delivery.request_ref.as_ref(),
        rules,
        delivery.explicit_prompt.as_deref(),
    )
}

fn send(cx: &mut Ctx<'_>, core: &Shared, delivery: &ContextDelivery, text: Option<String>) {
    let Some(text) = text else {
        return;
    };
    if delivery.chat_opened {
        // `chat.send` usa a sessão ativa: confirme a identidade antes de enviar.
        if let Ok(status) = cx.run_command("chat.status", Args::new()) {
            let same_session = delivery
                .chat_session
                .as_deref()
                .is_none_or(|expected| status.get("session_id").and_then(Value::as_str) == Some(expected));
            let same_bench = status
                .get("bench_path")
                .and_then(Value::as_str)
                .is_some_and(|path| crate::status::same_path(path, &delivery.bench.display().to_string()));
            if same_session && same_bench {
                let mut args = Args::new();
                args.insert("prompt", json!(text));
                let _ = cx.run_command("chat.send", args);
            }
        }
        return;
    }
    if let Some(terminal_id) = &delivery.terminal_id {
        let _ = core.with_state(|state| {
            state
                .launch
                .deferred_sends
                .entry(terminal_id.clone())
                .or_default()
                .push(text);
        });
        ready::schedule_fallback_send(cx, core, terminal_id.clone(), Duration::from_millis(400));
    }
}

const I18N_KEYS: &[&str] = &[
    "helper.ready.timeout-title",
    "helper.ready.timeout-body",
    "helper.resume.ask-title",
    "helper.resume.ask-body",
    "helper.resume.unsupported-title",
    "helper.resume.unsupported-body",
    "helper.resume.none-found",
    "setting.helper.launch.initial_context.option.request",
    "setting.helper.launch.initial_context.option.bench",
    "setting.helper.launch.initial_context.option.none",
    "setting.helper.sessions.resume_policy.option.ask",
    "setting.helper.sessions.resume_policy.option.auto",
    "setting.helper.sessions.resume_policy.option.never",
];

/// Registra chaves, eventos, comandos de retomada e a prontidão por terminal.
pub(crate) fn register(r: &mut Registrar<'_>, core: &Shared) {
    for key in I18N_KEYS {
        r.uses_i18n(key);
    }
    let mut add = |key: &str, kind: SettingKind, default: SettingValue| {
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{}-label", key.replace('.', "-")),
            format!("setting-{}-help", key.replace('.', "-")),
        ));
    };
    add(
        "helper.launch.initial_context",
        SettingKind::Choice {
            options: ["request", "bench", "none"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("request".to_owned()),
    );
    add(
        "helper.launch.include_rules",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.launch.ready_timeout_s",
        SettingKind::Integer {
            min: Some(5),
            max: Some(120),
        },
        SettingValue::Integer(30),
    );
    add(
        "helper.sessions.resume_policy",
        SettingKind::Choice {
            options: ["ask", "auto", "never"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
    );

    r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
    r.contract_event::<TerminalCommandFinished>(names::TERMINAL_COMMAND_FINISHED);
    let bench_core = Arc::clone(core);
    r.subscribe(move |cx, event: &BenchActivated| {
        context::schedule_rules_preload(
            cx,
            &bench_core,
            PathBuf::from(&event.0.path),
            vec!["AGENTS.md".to_owned(), "CLAUDE.md".to_owned()],
        );
        resume::on_bench_activated(cx, &bench_core, &event.0);
    });

    resume::register(r, core);

    let finished_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TerminalCommandFinished| {
        let _ = crate::commands::ensure_current(cx, &finished_core);
        crate::status::update_run(
            cx,
            &finished_core,
            &event.terminal,
            forja_contracts::HelperStatus::Idle,
            Some("command finished".to_owned()),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{detect, Core, HelperModule};
    use forja_kernel::{Module, ModuleManifest, Runtime, RuntimeOptions, SettingValue};
    use serde_json::{json, Map};

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn temp_guard(label: &str) -> TempDirGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-context-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    #[test]
    fn launch_includes_rules_via_async_task_and_request_context_from_bench_contract() {
        struct TerminalStub(Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>);
        impl Module for TerminalStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("terminal", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let open_calls = Arc::clone(&self.0);
                r.command("terminal.open", "helper.name", move |_, args| {
                    open_calls
                        .lock()
                        .unwrap()
                        .push(("terminal.open".to_owned(), args.into_json()));
                    Ok(json!({ "terminal_id": "term-ctx" }))
                });
                let send_calls = Arc::clone(&self.0);
                r.command("terminal.send", "helper.name", move |_, args| {
                    send_calls
                        .lock()
                        .unwrap()
                        .push(("terminal.send".to_owned(), args.into_json()));
                    Ok(json!({}))
                });
                r.command("terminal.read", "helper.name", move |_, _| {
                    Ok(json!({ "text": "READY>", "alive": true }))
                });
            }
        }
        struct BenchStub(PathBuf);
        impl Module for BenchStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("bench", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let bench_path = self.0.display().to_string();
                r.command("bench.request", "helper.name", move |_, _| {
                    Ok(json!({
                        "bench_id": "b-req",
                        "bench_path": bench_path,
                        "name": "pedido-12-fix",
                        "branch": "forja/pedido-12-fix",
                        "request_key": "#12",
                        "request_ref": {
                            "counter": "github",
                            "key": "#12",
                            "title": "Fix initial context",
                            "url": "https://github.com/org/repo/issues/12",
                            "description": "Pass request title, description and links.",
                            "links": ["https://github.com/org/repo/issues/12", "https://docs.example.test/r12"]
                        }
                    }))
                });
            }
        }

        let dir = temp_guard("launch-rules-context");
        let bench_dir = dir.0.join("bench");
        std::fs::create_dir_all(&bench_dir).unwrap();
        std::fs::write(bench_dir.join("AGENTS.md"), "Follow strict module boundaries.").unwrap();

        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(TerminalStub(Arc::clone(&calls))),
                Box::new(BenchStub(bench_dir.clone())),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();

        core.with_state(|state| {
            state.detection.insert(
                "claude-code".into(),
                detect::Detection {
                    executable: Some(dir.0.join("claude.exe")),
                    ..detect::Detection::default()
                },
            );
        })
        .unwrap();

        let result = runtime
            .call_method(
                "helper.launch",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("bench_path".to_owned(), json!(bench_dir.display().to_string())),
                ]),
            )
            .expect("launch with bench request context succeeds");
        assert_eq!(result["terminal_id"], "term-ctx");
        let cmdline = result["command_line"].as_str().unwrap().to_owned();
        assert!(!cmdline.contains("Fix initial context"), "{cmdline}");
        assert!(!cmdline.contains("PWNED"), "{cmdline}");

        // Async task reads AGENTS.md, then delivers rules + request context by PTY.
        for _ in 0..60 {
            runtime.poll_tasks();
            let sent = calls.lock().unwrap().iter().any(|(cmd, payload)| {
                cmd == "terminal.send"
                    && payload["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("Follow strict module boundaries."))
            });
            if sent {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        let sent = calls
            .lock()
            .unwrap()
            .iter()
            .find(|(cmd, payload)| {
                cmd == "terminal.send"
                    && payload["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("Follow strict module boundaries."))
            })
            .map(|(_, payload)| payload["text"].as_str().unwrap().to_owned())
            .expect("expected async rule delivery via terminal.send");
        assert!(sent.contains("Fix initial context"), "{sent}");
        assert!(sent.contains("Pass request title, description and links."), "{sent}");
        assert!(sent.contains("https://docs.example.test/r12"), "{sent}");
        let rules_at = sent.find("Follow strict module boundaries.").unwrap();
        let request_at = sent.find("Fix initial context").unwrap();
        assert!(rules_at < request_at, "rules must come before the request: {sent}");
    }

    #[test]
    fn chat_delivery_does_not_land_on_another_bench() {
        struct WrongBenchChat {
            sends: Arc<std::sync::Mutex<Vec<String>>>,
        }
        impl Module for WrongBenchChat {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("chat", "chat.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                r.command("chat.open", "chat.command.open", |_, _| {
                    Ok(json!({ "session_id": "sess-1" }))
                });
                r.command("chat.status", "chat.command.status", |_, _| {
                    Ok(json!({
                        "session_id": "sess-2",
                        "bench_path": "C:/another/bench",
                    }))
                });
                let sends = Arc::clone(&self.sends);
                r.command("chat.send", "chat.command.send", move |_, args| {
                    sends
                        .lock()
                        .unwrap()
                        .push(args.str("prompt").unwrap_or("").to_owned());
                    Ok(json!({ "queued": true }))
                });
            }
        }

        let dir = temp_guard("chat-wrong-bench");
        let bench_dir = dir.0.join("bench");
        std::fs::create_dir_all(&bench_dir).unwrap();
        std::fs::write(bench_dir.join("AGENTS.md"), "Bench rules.").unwrap();
        let core = Arc::new(Core::new());
        let sends = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(WrongBenchChat {
                    sends: Arc::clone(&sends),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        runtime
            .set_setting("helper.launch.mode", SettingValue::Text("chat".to_owned()))
            .unwrap();
        runtime
            .call_method(
                "helper.launch",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("bench_path".to_owned(), json!(bench_dir.display().to_string())),
                    ("prompt".to_owned(), json!("only for this bench")),
                ]),
            )
            .expect("chat launch");
        for _ in 0..60 {
            runtime.poll_tasks();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            sends.lock().unwrap().is_empty(),
            "chat.send must not target another bench/session"
        );
    }
}
