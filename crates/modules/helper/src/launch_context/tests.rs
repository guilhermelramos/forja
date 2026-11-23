    use super::*;
    use crate::profile::{Layer, Profile};
    use crate::{detect, Core, HelperModule};
    use forja_kernel::{Module, ModuleManifest, Runtime, RuntimeOptions, SettingValue};
    use serde_json::{json, Map, Value};

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
        struct TerminalStub {
            calls: Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
            core: Arc<Core>,
        }
        impl Module for TerminalStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("terminal", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let flush_core = Arc::clone(&self.core);
                r.command("terminal.flush-test", "helper.name", move |cx, args| {
                    let tid = args.required_str("terminal_id")?.to_owned();
                    crate::ready::flush_deferred_sends(cx, &flush_core, &tid);
                    Ok(json!({}))
                });
                let open_calls = Arc::clone(&self.calls);
                r.command("terminal.open", "helper.name", move |_, args| {
                    open_calls
                        .lock()
                        .unwrap()
                        .push(("terminal.open".to_owned(), args.into_json()));
                    Ok(json!({ "terminal_id": "term-ctx" }))
                });
                let send_calls = Arc::clone(&self.calls);
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
                Box::new(TerminalStub {
                    calls: Arc::clone(&calls),
                    core: Arc::clone(&core),
                }),
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
            if let Some(profile) = state.profiles.iter_mut().find(|p| p.id == "claude-code") {
                profile.ready.patterns = vec!["READY>".to_owned()];
            }
            state.loaded_for = Some(bench_dir.clone());
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
        // Confirma a prontidão pelo sinal do ajudante (a detecção do sinal é
        // provada em ready.rs); a entrega das regras depende dela.
        core.with_state(|state| {
            let started = state
                .runs
                .get("term-ctx")
                .map(|run| run.started_at.clone())
                .unwrap_or_default();
            let entry = state
                .launch
                .ready_states
                .entry("term-ctx".to_owned())
                .or_default();
            entry.ready = true;
            entry.identity = Some(started);
        })
        .unwrap();
        let mut flush = Args::new();
        flush.insert("terminal_id", json!("term-ctx"));
        runtime.run_command("terminal.flush-test", flush).unwrap();
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

    #[test]
    fn failed_helper_never_writes_the_request_to_the_pty() {
        struct PtyStub(Arc<std::sync::Mutex<Vec<(String, Value)>>>);
        impl Module for PtyStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("terminal", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                r.command("terminal.open", "helper.name", move |_, _| {
                    Ok(json!({ "terminal_id": "term-failed" }))
                });
                let sends = Arc::clone(&self.0);
                r.command("terminal.send", "helper.name", move |_, args| {
                    sends
                        .lock()
                        .unwrap()
                        .push(("terminal.send".to_owned(), args.into_json()));
                    Ok(json!({}))
                });
                let reads = Arc::clone(&self.0);
                r.command("terminal.read", "helper.name", move |_, _| {
                    reads
                        .lock()
                        .unwrap()
                        .push(("terminal.read".to_owned(), json!({})));
                    Ok(json!({ "text": "", "alive": true }))
                });
            }
        }
        struct NotifyStub(Arc<std::sync::Mutex<Vec<Value>>>);
        impl Module for NotifyStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("notify", "helper.name", "0.0.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let notices = Arc::clone(&self.0);
                r.command("notify.show", "helper.name", move |_, args| {
                    notices.lock().unwrap().push(args.into_json());
                    Ok(json!({}))
                });
            }
        }

        let dir = temp_guard("failed-helper");
        let bench_dir = dir.0.join("bench");
        std::fs::create_dir_all(&bench_dir).unwrap();
        let pwned = dir.0.join("pwned.txt");
        let core = Arc::new(Core::new());
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let notices = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(PtyStub(Arc::clone(&calls))),
                Box::new(NotifyStub(Arc::clone(&notices))),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let mut missing = Profile::parse(
            "id = \"missing-cli\"\nname = \"Missing CLI\"\nmaker = \"Test\"\nsite = \"https://example.test\"\n[detect]\nexecutables=[\"definitely-not-a-real-executable-xyz\"]\n[start]\ncommand=\"definitely-not-a-real-executable-xyz\"\n",
            PathBuf::from("missing-cli.toml"),
            Layer::User,
        )
        .unwrap();
        missing.trusted = true;
        core.with_state(|state| {
            state.profiles = vec![missing];
            state.detection.clear();
            state.probed.clear();
            state.loaded_for = Some(bench_dir.clone());
        })
        .unwrap();

        let mut args = Args::new();
        args.insert("profile_id", json!("missing-cli"));
        args.insert("bench_path", json!(bench_dir.display().to_string()));
        args.insert(
            "prompt",
            json!(format!("& echo PWNED > \"{}\"", pwned.display())),
        );
        let result = runtime.run_command("helper.launch", args);
        assert!(result.is_err(), "uninstalled helper must fail to launch");
        for _ in 0..60 {
            runtime.poll_tasks();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            calls
                .lock()
                .unwrap()
                .iter()
                .all(|(command, _)| command != "terminal.send"),
            "failed launch must never write to the PTY"
        );
        assert!(!pwned.exists(), "untrusted text executed");
        assert!(!notices.lock().unwrap().is_empty(), "failure must warn");
    }

    struct StubChat {
        calls: Arc<std::sync::Mutex<Vec<(String, serde_json::Value)>>>,
        bench: Arc<std::sync::Mutex<Option<String>>>,
    }

    impl Module for StubChat {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("chat", "chat.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let open_calls = Arc::clone(&self.calls);
            let open_bench = Arc::clone(&self.bench);
            r.command("chat.open", "chat.command.open", move |_, args| {
                *open_bench.lock().unwrap() = args
                    .str("bench_path")
                    .map(str::to_owned);
                open_calls
                    .lock()
                    .unwrap()
                    .push(("chat.open".to_owned(), args.into_json()));
                Ok(json!({ "session_id": serde_json::Value::Null }))
            });
            let send_calls = Arc::clone(&self.calls);
            r.command("chat.send", "chat.command.send", move |_, args| {
                send_calls
                    .lock()
                    .unwrap()
                    .push(("chat.send".to_owned(), args.into_json()));
                Ok(json!({ "queued": false, "pending": 0 }))
            });
            let status_bench = Arc::clone(&self.bench);
            r.command("chat.status", "chat.command.status", move |_, _| {
                Ok(json!({
                    "session_id": serde_json::Value::Null,
                    "bench_path": status_bench.lock().unwrap().clone().unwrap_or_default(),
                }))
            });
        }
    }

    #[test]
    fn chat_mode_calls_chat_open_and_send() {
        let dir = temp_guard("chat-mode");
        let dir = &dir.0;
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::new(Core::new()),
                }),
                Box::new(StubChat {
                    calls: Arc::clone(&calls),
                    bench: Arc::new(std::sync::Mutex::new(None)),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("helper and stub chat register");
        runtime
            .set_setting("helper.launch.mode", SettingValue::Text("chat".to_owned()))
            .unwrap();
        let value = runtime
            .call_method(
                "helper.launch",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("prompt".to_owned(), json!("oi")),
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
                ]),
            )
            .expect("chat launch");
        assert_eq!(value["mode"], "chat");
        for _ in 0..60 {
            runtime.poll_tasks();
            if calls.lock().unwrap().len() >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].0, "chat.open");
        assert_eq!(calls[0].1["profile_id"], "claude-code");
        assert_eq!(calls[1].0, "chat.send");
        assert_eq!(calls[1].1["prompt"], "oi");
        drop(calls);
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
