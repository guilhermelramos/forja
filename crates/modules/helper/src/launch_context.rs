//! Registro e estado das opções de contexto, prontidão e retomada do ajudante.
//!
//! Mantém fora de `lib.rs` o registro das quatro chaves ligadas em #204, os
//! eventos de ativação de bancada/prontidão e a estado correspondente.

use crate::{context, ready, resume, Shared};
use forja_contracts::{names, BenchActivated, TerminalCommandFinished};
use forja_kernel::{
    Registrar, SettingKind, SettingSpec, SettingValue,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

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
        let _ = finished_core.with_state(|state| {
            let entry = state
                .launch
                .ready_states
                .entry(event.terminal.terminal_id.clone())
                .or_default();
            entry.ready = true;
            entry.timed_out = false;
        });
        ready::flush_deferred_sends(cx, &finished_core, &event.terminal.terminal_id);
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
    use forja_kernel::{Module, ModuleManifest, Runtime, RuntimeOptions};
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
        assert!(cmdline.contains("Fix initial context"), "{cmdline}");
        assert!(cmdline.contains("Pass request title, description and links."), "{cmdline}");
        assert!(cmdline.contains("https://docs.example.test/r12"), "{cmdline}");

        // Async task reads AGENTS.md and delivers rules once ready without blocking launch.
        for _ in 0..20 {
            runtime.poll_tasks();
            let sent = calls
                .lock()
                .unwrap()
                .iter()
                .any(|(cmd, payload)| {
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
        assert!(
            calls.lock().unwrap().iter().any(|(cmd, payload)| {
                cmd == "terminal.send"
                    && payload["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("Follow strict module boundaries."))
            }),
            "expected async rule delivery via terminal.send"
        );
    }
}
