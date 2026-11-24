//! Testes do Mutirão (extraídos de `lib.rs`).

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::tests_support::*;

    #[test]
    fn coordinator_profile_setting_drives_task_launch() {
        let dir = temp_dir();
        let profiles = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubHelper {
                    profiles: Arc::clone(&profiles),
                    paths: Arc::new(std::sync::Mutex::new(Vec::new())),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster and helper register");
        runtime
            .set_setting(
                "muster.coordinator.profile",
                SettingValue::Text("codex".to_owned()),
            )
            .unwrap();
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Perfil"))]),
            )
            .unwrap();
        let task = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), run["id"].clone()),
                    ("title".to_owned(), json!("Tarefa")),
                ]),
            )
            .unwrap();
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), task["id"].clone()),
                    (
                        "bench_path".to_owned(),
                        json!(dir.0.display().to_string()),
                    ),
                ]),
            )
            .expect("helper launches");
        assert_eq!(profiles.lock().unwrap().as_slice(), ["codex"]);
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["runs"][0]["coordinator_profile"], "codex");
    }

    #[test]
    fn budget_gate_reads_usage_spend_when_available() {
        let dir = temp_dir();
        let mut runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubUsage { usd: 10.0 }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster and usage register");
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([
                    ("title".to_owned(), json!("Uso")),
                    ("bench_path".to_owned(), json!("C:/bench")),
                    ("budget_usd".to_owned(), json!(5.0)),
                ]),
            )
            .unwrap();
        let run_id = run["id"].as_str().unwrap().to_owned();
        let task = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Task")),
                ]),
            )
            .unwrap();
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), task["id"].clone()),
                    ("bench_path".to_owned(), json!("C:/bench")),
                ]),
            )
            .expect_err("usage spend exceeds the ceiling");
        assert!(error.message.contains("teto"), "{}", error.message);
    }

    #[test]
    fn coordinator_profile_ids_match_helper_default_profile() {
        let dir = temp_dir();
        let runtime = runtime(&dir.0);
        let spec = runtime
            .registry()
            .settings()
            .iter()
            .find(|spec| spec.key == "muster.coordinator.profile")
            .expect("muster.coordinator.profile");
        let SettingKind::Choice { options } = &spec.kind else {
            panic!("muster.coordinator.profile deve ser Choice");
        };
        assert_eq!(
            options,
            &helper_profile_ids(),
            "a lista de perfis do Mutirão saiu de sincronia com helper.default.profile"
        );
    }

    #[test]
    fn disabled_muster_rejects_new_runs() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        runtime
            .set_setting("muster.mode.enabled", SettingValue::Bool(false))
            .unwrap();
        assert!(runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Run")),])
            )
            .is_err());
        runtime
            .set_setting("muster.mode.enabled", SettingValue::Bool(true))
            .unwrap();
        assert!(runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Run")),])
            )
            .is_ok());
    }

    #[test]
    fn dispatch_new_bench_waits_for_its_identity_then_launches() {
        let dir = temp_dir();
        let (mut runtime, stubs) = bench_runtime(&dir.0);
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        runtime
            .set_setting(
                "muster.dispatch.target",
                SettingValue::Text("new".to_owned()),
            )
            .unwrap();
        let (_, task_id) = new_run_and_task(&mut runtime);
        let pending = runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        assert_eq!(pending["pending"], true);
        assert_eq!(pending["bench_id"], "bench-new");
        assert_eq!(
            stubs.created.lock().unwrap().as_slice(),
            [format!("muster-{task_id}")]
        );
        assert!(stubs.paths.lock().unwrap().is_empty());
        let before = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(before["pending_dispatches"][0]["bench_id"], "bench-new", "{before}");

        let wrong = BenchCreated(BenchRef {
            project_id: "p1".to_owned(),
            bench_id: "outra".to_owned(),
            name: "outra".to_owned(),
            path: "C:/errada".to_owned(),
            branch: None,
        });
        runtime.emit(&wrong);
        settle(&mut runtime);
        assert!(stubs.paths.lock().unwrap().is_empty());

        let ready = BenchCreated(BenchRef {
            project_id: "p1".to_owned(),
            bench_id: "bench-new".to_owned(),
            name: format!("muster-{task_id}"),
            path: "C:/bancadas/pronta".to_owned(),
            branch: None,
        });
        assert!(runtime.emit(&ready));
        settle(&mut runtime);
        let after = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(
            stubs.paths.lock().unwrap().as_slice(),
            ["C:/bancadas/pronta"],
            "{after}"
        );
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["pending_dispatches"], json!([]));
        assert_eq!(status["owned_benches"][0]["bench_id"], "bench-new");
        assert_eq!(status["assignments"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn dispatch_failure_closes_the_pending_dispatch_and_marks_the_task_failed() {
        let dir = temp_dir();
        let (mut runtime, stubs) = bench_runtime(&dir.0);
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        runtime
            .set_setting(
                "muster.dispatch.target",
                SettingValue::Text("new".to_owned()),
            )
            .unwrap();
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id))]),
            )
            .unwrap();
        runtime.emit(&BenchCreateFailed {
            bench_id: "bench-new".to_owned(),
            name: "muster-task-1".to_owned(),
            reason: "git falhou".to_owned(),
        });
        settle(&mut runtime);
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["pending_dispatches"], json!([]));
        assert_eq!(status["tasks"][0]["state"], "failed");
        assert!(stubs.paths.lock().unwrap().is_empty());
    }

    #[test]
    fn dispatch_target_decides_between_current_and_ask() {
        let dir = temp_dir();
        let (mut runtime, stubs) = bench_runtime(&dir.0);
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        runtime
            .set_setting(
                "muster.dispatch.target",
                SettingValue::Text("current".to_owned()),
            )
            .unwrap();
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id))]),
            )
            .unwrap();
        assert_eq!(stubs.paths.lock().unwrap().as_slice(), ["C:/atual"]);

        runtime
            .set_setting(
                "muster.dispatch.target",
                SettingValue::Text("ask".to_owned()),
            )
            .unwrap();
        let (_, other) = new_run_and_task(&mut runtime);
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(other))]),
            )
            .unwrap_err();
        assert!(error.message.contains("bancada"), "{}", error.message);
    }

    #[test]
    fn isolation_shared_allows_one_bench_but_worktree_refuses_reuse() {
        let dir = temp_dir();
        let (mut runtime, stubs) = bench_runtime(&dir.0);
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        runtime
            .set_setting(
                "muster.dispatch.target",
                SettingValue::Text("current".to_owned()),
            )
            .unwrap();
        runtime
            .set_setting(
                "muster.workspace.isolation",
                SettingValue::Text("shared".to_owned()),
            )
            .unwrap();
        let (run_id, first) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(first))]),
            )
            .unwrap();
        let second = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id.clone())),
                    ("title".to_owned(), json!("Segunda")),
                ]),
            )
            .unwrap();
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), second["id"].clone())]),
            )
            .unwrap();
        assert_eq!(
            stubs.paths.lock().unwrap().as_slice(),
            ["C:/atual", "C:/atual"]
        );

        runtime
            .set_setting(
                "muster.workspace.isolation",
                SettingValue::Text("worktree".to_owned()),
            )
            .unwrap();
        let third = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Terceira")),
                ]),
            )
            .unwrap();
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), third["id"].clone())]),
            )
            .unwrap_err();
        assert!(error.message.contains("bancada própria"), "{}", error.message);
        assert_eq!(stubs.paths.lock().unwrap().len(), 2);
    }

