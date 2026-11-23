    use super::*;
    use crate::{detect, Core, HelperModule};
    use forja_contracts::BenchActivated;
    use forja_kernel::{Module, ModuleManifest, Registrar, Runtime, RuntimeOptions, SettingValue};
    use std::sync::Mutex;

    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-resume-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    struct TerminalStub(Arc<Mutex<Vec<Value>>>);

    impl Module for TerminalStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("terminal", "helper.name", "0.0.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            let opens = Arc::clone(&self.0);
            r.command("terminal.open", "helper.name", move |_, args| {
                opens.lock().unwrap().push(args.into_json());
                Ok(json!({ "terminal_id": "term-resumed" }))
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

    struct RecordStub(Arc<Core>);

    impl Module for RecordStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("test", "helper.name", "0.0.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            let core = Arc::clone(&self.0);
            r.command("test.record_session", "helper.name", move |cx, args| {
                let bench_path = args.required_str("bench_path")?.to_owned();
                let profile_id = args.required_str("profile_id")?.to_owned();
                let session_id = args.str("session_id").map(str::to_owned);
                let _ = ensure(cx, &core, Some(PathBuf::from(&bench_path)));
                record_bench_session(cx, &core, &bench_path, &profile_id, session_id);
                Ok(json!({}))
            });
        }
    }

    type StubLog = Arc<Mutex<Vec<Value>>>;

    fn setup_runtime(dir: &Path) -> (Runtime, Arc<Core>, StubLog, StubLog) {
        let core = Arc::new(Core::new());
        let opens = Arc::new(Mutex::new(Vec::new()));
        let notifies = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(HelperModule {
                    core: Arc::clone(&core),
                }),
                Box::new(TerminalStub(Arc::clone(&opens))),
                Box::new(NotifyStub(Arc::clone(&notifies))),
                Box::new(RecordStub(Arc::clone(&core))),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .unwrap();
        let bench_dir = dir.join("bench");
        std::fs::create_dir_all(&bench_dir).unwrap();
        let mut list_args = Args::new();
        list_args.insert("bench_path", json!(bench_dir.display().to_string()));
        runtime.run_command("helper.list", list_args).unwrap();
        core.with_state(|state| {
            state.detection.insert(
                "claude-code".into(),
                detect::Detection {
                    executable: Some(dir.join("claude.exe")),
                    ..detect::Detection::default()
                },
            );
        })
        .unwrap();
        (runtime, core, opens, notifies)
    }

    #[test]
    fn resume_policy_handles_never_ask_and_auto_with_supported_profile() {
        let dir = temp_dir("policies");
        let (mut runtime, core, opens, notifies) = setup_runtime(&dir.0);
        let bench_dir = dir.0.join("bench");
        let bench_ref = BenchRef {
            project_id: "p1".into(),
            bench_id: "b1".into(),
            name: "bench".into(),
            path: bench_dir.display().to_string(),
            branch: Some("main".into()),
        };

        let mut rec = Args::new();
        rec.insert("bench_path", json!(bench_ref.path));
        rec.insert("profile_id", json!("claude-code"));
        rec.insert("session_id", json!("sess-9"));
        runtime.run_command("test.record_session", rec).unwrap();

        // 1) never: neither resumes nor prompts
        runtime
            .set_setting("helper.sessions.resume_policy", SettingValue::Text("never".into()))
            .unwrap();
        runtime.emit(&BenchActivated(bench_ref.clone()));
        assert!(opens.lock().unwrap().is_empty());
        assert!(notifies.lock().unwrap().is_empty());

        // 2) ask: stores pending prompt + notifies, does not auto-launch
        runtime
            .set_setting("helper.sessions.resume_policy", SettingValue::Text("ask".into()))
            .unwrap();
        runtime.emit(&BenchActivated(bench_ref.clone()));
        assert!(opens.lock().unwrap().is_empty());
        assert_eq!(notifies.lock().unwrap().len(), 1);
        assert!(core
            .with_state(|s| s
                .launch.pending_resumes
                .contains_key(&context::path_key(&bench_dir)))
            .unwrap());

        // 3) auto with supported profile: opens terminal with `--resume sess-9`
        runtime
            .set_setting("helper.sessions.resume_policy", SettingValue::Text("auto".into()))
            .unwrap();
        runtime.emit(&BenchActivated(bench_ref));
        let opened = opens.lock().unwrap();
        assert_eq!(opened.len(), 1);
        assert!(opened[0]["args"].as_str().unwrap().contains("--resume sess-9"));
    }

    #[test]
    fn resume_policy_auto_falls_back_to_never_and_warns_when_profile_cannot_resume() {
        let dir = temp_dir("unsupported");
        let (mut runtime, core, opens, notifies) = setup_runtime(&dir.0);
        let bench_dir = dir.0.join("bench");
        let bench_ref = BenchRef {
            project_id: "p1".into(),
            bench_id: "b2".into(),
            name: "bench".into(),
            path: bench_dir.display().to_string(),
            branch: Some("main".into()),
        };

        // Simulate a profile that declares no [resume] flags in state only.
        core.with_state(|state| {
            if let Some(profile) = state.profiles.iter_mut().find(|p| p.id == "claude-code") {
                profile.resume = Default::default();
            }
        })
        .unwrap();

        let mut rec = Args::new();
        rec.insert("bench_path", json!(bench_ref.path));
        rec.insert("profile_id", json!("claude-code"));
        runtime.run_command("test.record_session", rec).unwrap();
        runtime
            .set_setting("helper.sessions.resume_policy", SettingValue::Text("auto".into()))
            .unwrap();
        runtime.emit(&BenchActivated(bench_ref));

        assert!(opens.lock().unwrap().is_empty());
        let notices = notifies.lock().unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0]["level"], "warn");
        assert!(core
            .with_state(|s| s
                .launch.resume_notices
                .contains_key(&context::path_key(&bench_dir)))
            .unwrap());
    }
