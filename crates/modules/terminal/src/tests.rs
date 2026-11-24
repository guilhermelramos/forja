    use super::*;
    use forja_kernel::ui_contract::{egui, View};
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..40 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 39 {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
        }
    }

    fn temp_guard(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-terminal-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(TerminalModule {
                core: Arc::new(Core::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("terminal module registers")
    }

    struct TestTrust {
        approved: Arc<AtomicBool>,
        requests: Arc<AtomicUsize>,
    }

    impl Module for TestTrust {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("trust", "terminal.name", "0.1.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let approved = Arc::clone(&self.approved);
            r.command("trust.check", "terminal.command.open", move |_, _| {
                Ok(json!({"approved": approved.load(Ordering::SeqCst)}))
            });
            let requests = Arc::clone(&self.requests);
            r.command("trust.request", "terminal.command.open", move |_, _| {
                requests.fetch_add(1, Ordering::SeqCst);
                Ok(json!({"approved": false}))
            });
        }
    }

    #[test]
    fn anvil_recipe_does_not_execute_without_trust() {
        let guard = temp_guard("anvil-trust");
        let dir = &guard.0;
        let core = Arc::new(Core::new());
        let approved = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let recipe = recipes::Recipe {
            id: "example".into(), title_pt: "Compilar exemplo".into(), title_en: "Build example".into(),
            command: "echo forja_recipe_marker".into(), scope: "project".into(), enter: true,
            directory: String::new(), manifest: "Cargo.toml".into(), source_path: dir.join("recipe.toml").display().to_string(),
            content: "example".into(), content_hash: "hash-example".into(),
        };
        core.with_hub(|hub| hub.set_contract_content(Vec::new(), vec![recipe])).unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(TerminalModule { core: Arc::clone(&core) }), Box::new(TestTrust {
                approved: Arc::clone(&approved), requests: Arc::clone(&requests),
            })],
            RuntimeOptions { data_dir: Some(dir.clone()), user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()), wake: None },
        ).unwrap();
        let mut args = Args::new();
        args.insert("cwd", json!(dir.display().to_string()));
        args.insert("command", json!("cmd.exe"));
        let terminal = runtime.run_command("terminal.open", args).unwrap();
        let terminal_id = terminal["terminal_id"].as_str().unwrap();
        let mut view = RecipeView::new(Arc::clone(&core));
        let hits = view.anvil_hits("compilar", &mut runtime.ctx("terminal"));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Compilar exemplo");
        let run = |runtime: &mut Runtime| runtime.run_command(
            &hits[0].command, Args::from_map(hits[0].args.clone()));
        let error = run(&mut runtime).expect_err("unapproved recipe must be blocked");
        assert!(error.to_string().contains("aprovação"));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        let mut stale = hits[0].args.clone();
        stale.insert("content_hash".into(), json!("outdated"));
        assert!(runtime.run_command(&hits[0].command, Args::from_map(stale)).is_err());
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        let mut read_args = Args::new();
        read_args.insert("terminal_id", json!(terminal_id));
        std::thread::sleep(Duration::from_millis(250));
        let output = runtime.run_command("terminal.read", read_args.clone()).unwrap();
        assert!(!output["text"].as_str().unwrap_or_default().contains("forja_recipe_marker"));
        approved.store(true, Ordering::SeqCst);
        run(&mut runtime).expect("approved recipe runs");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = false;
        while Instant::now() < deadline {
            let output = runtime.run_command("terminal.read", read_args.clone()).unwrap();
            if output["text"].as_str().unwrap_or_default().contains("forja_recipe_marker") {
                seen = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        assert!(seen, "approved recipe did not reach the terminal");
    }

    #[test]
    fn helper_status_events_update_close_guard() {
        let guard = temp_guard("helper-events");
        let dir = &guard.0;
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(vec![Box::new(TerminalModule { core: Arc::clone(&core) })],
            RuntimeOptions { data_dir: Some(dir.clone()), user_dir: Some(dir.join("user")), locale: None, wake: None }).unwrap();
        let event = |status| HelperStatusChanged { bench_path: dir.display().to_string(),
            terminal_id: Some("17".into()), profile_id: "test".into(), status };
        runtime.emit(&event(HelperStatus::Working));
        assert!(core.with_hub(|hub| hub.helper_terminals.contains(&17)).unwrap());
        runtime.emit(&event(HelperStatus::Finished));
        assert!(!core.with_hub(|hub| hub.helper_terminals.contains(&17)).unwrap());
    }

    #[test]
    fn contract_and_rpc_open_send_read() {
        let guard = temp_guard("rpc");
        let dir = &guard.0;
        let mut runtime = runtime(dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let open = runtime
            .call_method(
                "terminal.open",
                Map::from_iter([
                    ("cwd".to_owned(), json!(dir.display().to_string())),
                    ("command".to_owned(), json!("cmd.exe")),
                    ("args".to_owned(), json!("/d")),
                    ("title".to_owned(), json!("RPC test")),
                ]),
            )
            .expect("open terminal");
        let terminal_id = open["terminal_id"]
            .as_str()
            .expect("terminal id")
            .to_owned();
        runtime
            .call_method(
                "terminal.send",
                Map::from_iter([
                    ("terminal_id".to_owned(), json!(terminal_id)),
                    ("text".to_owned(), json!("echo forja_rpc_marker")),
                    ("enter".to_owned(), json!(true)),
                ]),
            )
            .expect("send");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut text = String::new();
        while Instant::now() < deadline {
            let read = runtime
                .call_method(
                    "terminal.read",
                    Map::from_iter([("terminal_id".to_owned(), json!(terminal_id))]),
                )
                .expect("read");
            text = read["text"].as_str().unwrap_or_default().to_owned();
            if text.contains("forja_rpc_marker") {
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        assert!(text.contains("forja_rpc_marker"), "{text}");
        let list = runtime
            .call_method("terminal.list", Map::new())
            .expect("list");
        assert_eq!(list["terminals"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn trust_decided_runs_pending_project_recipe() {
        let guard = temp_guard("recipe-trust");
        let dir = &guard.0;
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(
            vec![Box::new(TerminalModule {
                core: Arc::clone(&core),
            })],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        runtime
            .set_setting("terminal.recipes.confirm", forja_kernel::SettingValue::Bool(false))
            .unwrap();
        let recipe = crate::recipes::Recipe {
            id: "check".to_owned(),
            title_en: "Check".to_owned(),
            title_pt: "Verificar".to_owned(),
            command: "echo recipe_ran_ok".to_owned(),
            directory: String::new(),
            manifest: "Cargo.toml".to_owned(),
            enter: true,
            scope: "project".to_owned(),
            source_path: dir.join("check.toml").display().to_string(),
            content: "command = \"echo ok\"".to_owned(),
            content_hash: "abc123".to_owned(),
        };
        core.with_hub(|hub| {
            hub.bench_path = Some(dir.clone());
            hub.notice = Some("A receita precisa de aprovação".to_owned());
            hub.pending_trust_recipe = Some(recipe.clone());
        })
        .unwrap();
        runtime.emit(&TrustDecided {
            request_id: "req-1".to_owned(),
            kind: TrustKind::Recipe,
            source_path: recipe.source_path.clone(),
            content_hash: recipe.content_hash.clone(),
            approved: true,
            decided_at: "2026-01-01T00:00:00Z".to_owned(),
        });
        let state = core
            .with_hub(|hub| (hub.notice.clone(), hub.pending_trust_recipe.clone(), hub.terminals.len()))
            .unwrap();
        assert!(state.0.is_none());
        assert!(state.1.is_none());
        assert_eq!(state.2, 1);
    }

    #[test]
    fn trust_decided_from_another_recipe_keeps_pending_and_runs_nothing() {
        let guard = temp_guard("recipe-trust-identity");
        let dir = &guard.0;
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(
            vec![Box::new(TerminalModule {
                core: Arc::clone(&core),
            })],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        runtime
            .set_setting("terminal.recipes.confirm", forja_kernel::SettingValue::Bool(false))
            .unwrap();
        let recipe = crate::recipes::Recipe {
            id: "check".to_owned(),
            title_en: "Check".to_owned(),
            title_pt: "Verificar".to_owned(),
            command: "echo recipe_ran_ok".to_owned(),
            directory: String::new(),
            manifest: "Cargo.toml".to_owned(),
            enter: true,
            scope: "project".to_owned(),
            source_path: dir.join("check.toml").display().to_string(),
            content: "command = \"echo ok\"".to_owned(),
            content_hash: "abc123".to_owned(),
        };
        core.with_hub(|hub| {
            hub.bench_path = Some(dir.clone());
            hub.pending_trust_recipe = Some(recipe.clone());
        })
        .unwrap();

        // A decisão de outra receita (outro caminho e hash) não dispara a pendente.
        runtime.emit(&TrustDecided {
            request_id: "req-other".to_owned(),
            kind: TrustKind::Recipe,
            source_path: dir.join("other.toml").display().to_string(),
            content_hash: "different-hash".to_owned(),
            approved: true,
            decided_at: "2026-01-01T00:00:00Z".to_owned(),
        });
        let state = core
            .with_hub(|hub| (hub.pending_trust_recipe.clone(), hub.terminals.len()))
            .unwrap();
        assert!(state.0.is_some(), "a decisão alheia não pode consumir a pendente");
        assert_eq!(state.1, 0, "a decisão alheia não pode rodar a receita");

        // Só a decisão com a mesma identidade roda, e exatamente uma vez.
        runtime.emit(&TrustDecided {
            request_id: "req-1".to_owned(),
            kind: TrustKind::Recipe,
            source_path: recipe.source_path.clone(),
            content_hash: recipe.content_hash.clone(),
            approved: true,
            decided_at: "2026-01-01T00:00:00Z".to_owned(),
        });
        let state = core
            .with_hub(|hub| (hub.pending_trust_recipe.clone(), hub.terminals.len()))
            .unwrap();
        assert!(state.0.is_none());
        assert_eq!(state.1, 1, "a receita própria deve rodar uma única vez");
    }

    #[test]
    fn spawn_failure_emits_exited_event_and_marks_not_alive() {
        let guard = temp_guard("spawn-fail");
        let dir = &guard.0;
        let core = Arc::new(Core::new());
        let settings = TermSettings::default();
        let opened = core
            .with_hub(|hub| {
                hub.open(
                    &settings,
                    crate::core::OpenRequest {
                        cwd: Some(dir.clone()),
                        command: Some("definitely-missing-forja-binary-xyz".to_owned()),
                        args: Some(String::new()),
                        title: Some("Missing".to_owned()),
                        ..crate::core::OpenRequest::default()
                    },
                )
            })
            .unwrap()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_exit = false;
        let mut error_text = None;
        while Instant::now() < deadline {
            let (events, err, alive) = core
                .with_hub(|hub| {
                    let events = hub.poll(Instant::now(), &settings, None);
                    let terminal = hub.terminal(opened).unwrap();
                    (events, terminal.error.clone(), terminal.alive())
                })
                .unwrap();
            if events
                .iter()
                .any(|event| event.kind == crate::core::HubEventKind::Exited && event.exit_code == Some(1))
            {
                saw_exit = true;
                error_text = err;
                assert!(!alive);
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(saw_exit, "expected Exited(Some(1)) on spawn failure");
        assert!(error_text.is_some());
    }

    #[test]
    fn legacy_gpu_mode_migrates_to_images_enabled_without_clobbering_new_key() {
        let guard = temp_guard("legacy-gpu-mode");
        let dir = &guard.0;
        std::fs::write(dir.join("settings.toml"), "terminal.gpu.mode = \"off\"\n").unwrap();
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(
            vec![Box::new(TerminalModule {
                core: Arc::clone(&core),
            })],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let mut view = TerminalView::new(Arc::clone(&core));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.ui(ui, &mut runtime.ctx("terminal"));
            });
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            runtime.poll_tasks();
            if !TermSettings::from_ctx(&runtime.ctx("terminal")).images_enabled {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!TermSettings::from_ctx(&runtime.ctx("terminal")).images_enabled);
        assert_eq!(
            runtime.settings().get("terminal.images.enabled"),
            Some(&forja_kernel::SettingValue::Bool(false))
        );
    }
