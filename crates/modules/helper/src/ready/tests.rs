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

        // Inicialização lenta: nenhum byte vai para o PTY antes do sinal.
        for _ in 0..6 {
            runtime.poll_tasks();
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(
            sends.lock().unwrap().is_empty(),
            "timer must never deliver context"
        );
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
            .with_state(|s| s.launch.ready_states.get("term-r1").cloned().unwrap())
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
            .with_state(|s| s.launch.ready_states.get("term-timeout").cloned().unwrap())
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
