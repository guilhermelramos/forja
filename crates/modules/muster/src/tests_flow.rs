//! Testes do Mutirão (extraídos de `lib.rs`).

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::tests_support::*;

    #[test]
    fn run_rejects_whitespace_titles() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        let error = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".into(), json!("  \t  "))]),
            )
            .unwrap_err();
        assert!(error.message.contains("Digite um título"));
    }

    #[test]
    fn contract_and_rpc_flow_degrades_without_helper() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([
                    ("title".to_owned(), json!("Mutirão de teste")),
                    ("objective".to_owned(), json!("Validar o fluxo")),
                ]),
            )
            .expect("run");
        let run_id = run["id"].as_str().unwrap().to_owned();
        let task = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Primeira tarefa")),
                    ("spec".to_owned(), json!("Fazer o teste")),
                ]),
            )
            .expect("task");
        let task_id = task["id"].as_str().unwrap().to_owned();
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("bench_path".to_owned(), json!(dir.0.display().to_string())),
                ]),
            )
            .expect_err("helper is absent");
        assert!(error.message.contains("ajudante"), "{}", error.message);
        let status = runtime
            .call_method("muster.status", Map::new())
            .expect("status");
        assert_eq!(status["tasks"][0]["state"], "failed");
        assert_eq!(status["assignments"][0]["state"], "failed");
        let inbox = runtime
            .call_method("muster.inbox", Map::new())
            .expect("inbox");
        assert_eq!(inbox["notes"], json!([]));
    }

    #[test]
    fn muster_open_queues_the_rail_page() {
        let dir = temp_dir();
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubShell {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster and shell register");
        runtime
            .run_command("muster.open", forja_kernel::Args::new())
            .expect("open");
        assert_eq!(calls.lock().unwrap().as_slice(), ["muster.page"]);
    }

    #[test]
    fn escalation_resends_after_a_confirmed_exit_without_overlapping() {
        let dir = temp_dir();
        let (mut runtime, stubs) = bench_runtime(&dir.0);
        runtime
            .set_setting("muster.plan.require_approval", SettingValue::Bool(false))
            .unwrap();
        runtime
            .set_setting("muster.escalation.max_retries", SettingValue::Integer(1))
            .unwrap();
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id.clone())),
                    ("bench_path".to_owned(), json!("C:/atual")),
                ]),
            )
            .unwrap();
        assert_eq!(stubs.profiles.lock().unwrap().len(), 1);

        let exit = TerminalExited {
            terminal: TerminalRef {
                terminal_id: "9".to_owned(),
                bench_path: "C:/atual".to_owned(),
                title: "tarefa".to_owned(),
            },
            exit_code: Some(1),
        };
        runtime.emit(&exit);
        settle(&mut runtime);
        assert_eq!(stubs.profiles.lock().unwrap().len(), 2);
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        let assignments = status["assignments"].as_array().unwrap();
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0]["task"], task_id);
        assert_eq!(assignments[1]["task"], task_id);
        assert_eq!(assignments[1]["retries"], 1);

        runtime.emit(&exit);
        settle(&mut runtime);
        assert_eq!(stubs.profiles.lock().unwrap().len(), 2, "sem terceira execução");
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["tasks"][0]["state"], "blocked");
    }

    #[test]
    fn max_tasks_limits_the_plan() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        runtime
            .set_setting("muster.plan.max_tasks", SettingValue::Integer(1))
            .unwrap();
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Limite"))]),
            )
            .unwrap();
        let run_id = run["id"].as_str().unwrap().to_owned();
        runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Primeira")),
                ]),
            )
            .unwrap();
        let error = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Segunda")),
                ]),
            )
            .expect_err("max tasks reached");
        assert!(error.message.contains("limite"), "{}", error.message);
    }

    #[test]
    fn handoff_with_evidence_marks_task_delivered() {
        let dir = temp_dir();
        let shared = state::new_shared();
        let mut runtime = runtime_with(&dir.0, shared.clone());
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Run"))]),
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
        let task_id = task["id"].as_str().unwrap().to_owned();
        state::with(&shared, |state| {
            let assignment = state
                .store
                .start(&task_id, "claude-code", "C:/bench")
                .expect("assignment");
            state
                .store
                .launched(
                    &assignment.id,
                    "claude-code",
                    Some("1".to_owned()),
                    None,
                    "tui",
                )
                .expect("launched");
        })
        .expect("state");
        let missing = runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                ]),
            )
            .expect_err("evidence is required");
        assert!(missing.message.contains("evidence"));
        let delivery = runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("cargo test")),
                ]),
            )
            .expect("handoff");
        assert_eq!(delivery["result"], "feito");
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["tasks"][0]["state"], "delivered");
        assert_eq!(status["runs"][0]["state"], "done");
    }

    #[test]
    fn raw_muster_errors_are_humanized() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        let cx = runtime.ctx("muster");
        let disabled = view::humanize_error(&cx, "muster is disabled");
        assert!(disabled.contains("desativado"), "{disabled}");
        let cmd = view::humanize_error(&cx, "command `muster.run` failed");
        assert!(cmd.contains("Não foi possível concluir"), "{cmd}");
        assert!(cmd.contains("muster.run"), "{cmd}");
    }

    #[test]
    fn strips_internal_command_prefixes_from_errors() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
        let cx = runtime.ctx("muster");
        let msg = view::humanize_error(
            &cx,
            "command `muster.start` failed: não foi possível abrir o ajudante: command `helper.launch` failed: O ajudante Claude Code não está instalado.",
        );
        assert!(!msg.contains("muster.start"), "{msg}");
        assert!(!msg.contains("helper.launch"), "{msg}");
        assert!(msg.contains("não está instalado"), "{msg}");
    }

    #[test]
    fn coordinator_note_keeps_state_and_uses_task_title_in_notifications() {
        let dir = temp_dir();
        let shared = state::new_shared();
        let mut runtime = runtime_with(&dir.0, shared.clone());
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Revisão"))]),
            )
            .unwrap();
        let run_id = run["id"].as_str().unwrap().to_owned();
        let task = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("title".to_owned(), json!("Validar e-mail")),
                ]),
            )
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_owned();

        let no_helper_res = runtime
            .call_method(
                "muster.note",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("body".to_owned(), json!("Conferir regex")),
                    ("from".to_owned(), json!("coordinator")),
                ]),
            )
            .unwrap();
        assert_eq!(no_helper_res["no_helper"], true);
        assert_eq!(no_helper_res["delivered"], false);
        assert_eq!(no_helper_res["note"]["no_helper"], true);

        state::with(&shared, |state| {
            let assignment = state.store.start(&task_id, "opencode", "C:/bench").unwrap();
            state
                .store
                .launched(
                    &assignment.id,
                    "opencode",
                    Some("term-1".into()),
                    None,
                    "tui",
                )
                .unwrap();
            state
                .store
                .update_status(&assignment.id, forja_contracts::HelperStatus::NeedsYou, "");
        })
        .unwrap();

        let queued_res = runtime
            .call_method(
                "muster.note",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("body".to_owned(), json!("Use RFC 5322")),
                    ("from".to_owned(), json!("coordinator")),
                ]),
            )
            .unwrap();
        assert_eq!(queued_res["queued"], true);
        assert_eq!(queued_res["no_helper"], false);
        assert_eq!(queued_res["note"]["queued"], true);

        let conversation =
            state::with(&shared, |state| state.store.conversation(Some(&run_id))).unwrap();
        assert_eq!(conversation.len(), 2);
}
