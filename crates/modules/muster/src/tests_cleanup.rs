//! Testes do Mutirão (extraídos de `lib.rs`).

#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use crate::tests_support::*;

    #[test]
    fn cleanup_keeps_dirty_bench_and_warns() {
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
        runtime
            .set_setting(
                "muster.cleanup.on_finish",
                SettingValue::Text("remove".to_owned()),
            )
            .unwrap();
        stubs
            .dirty
            .lock()
            .unwrap()
            .insert("bench-new".to_owned(), true);
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        finish_new_bench_dispatch(&mut runtime, &task_id);
        runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("testes ok")),
                ]),
            )
            .unwrap();
        settle(&mut runtime);
        assert!(stubs.stored.lock().unwrap().is_empty());
        assert_eq!(
            *stubs.dirty_calls.lock().unwrap(),
            0,
            "o done da limpeza não pode consultar git"
        );
        assert!(stubs.benches.lock().unwrap().contains(&"bench-new".to_owned()));
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["notice"], "muster-notice-cleanup-dirty");
        assert_eq!(status["runs"][0]["cleanup_done"], true);
    }

    #[test]
    fn cleanup_removes_only_the_owned_clean_bench() {
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
        runtime
            .set_setting(
                "muster.cleanup.on_finish",
                SettingValue::Text("remove".to_owned()),
            )
            .unwrap();
        // Uma bancada real do mesmo projeto, criada fora do mutirão.
        stubs
            .benches
            .lock()
            .unwrap()
            .push("bench-foreign".to_owned());
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        finish_new_bench_dispatch(&mut runtime, &task_id);
        runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("testes ok")),
                ]),
            )
            .unwrap();
        settle(&mut runtime);
        assert_eq!(stubs.stored.lock().unwrap().as_slice(), ["bench-new"]);
        assert_eq!(
            *stubs.dirty_calls.lock().unwrap(),
            0,
            "o done da limpeza não pode consultar git"
        );
        assert!(
            stubs.benches.lock().unwrap().contains(&"bench-foreign".to_owned()),
            "bancada alheia continua no serviço"
        );
        assert!(!stubs.benches.lock().unwrap().contains(&"bench-new".to_owned()));
    }

    /// O serviço falso guarda a lista de bancadas do mesmo projeto (`p1`, o
    /// mesmo de `bench.current` e `bench.create`) para que a prova de que a
    /// alheia sobrevive não dependa só de um mapa de sujeira solto.
    #[test]
    fn cleanup_keeps_foreign_and_dirty_benches_from_the_same_project() {
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
        runtime
            .set_setting(
                "muster.cleanup.on_finish",
                SettingValue::Text("remove".to_owned()),
            )
            .unwrap();
        stubs
            .benches
            .lock()
            .unwrap()
            .push("bench-de-outro".to_owned());
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        finish_new_bench_dispatch(&mut runtime, &task_id);
        stubs
            .dirty
            .lock()
            .unwrap()
            .insert("bench-new".to_owned(), true);
        runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("testes ok")),
                ]),
            )
            .unwrap();
        settle(&mut runtime);
        let benches = stubs.benches.lock().unwrap();
        assert!(
            benches.contains(&"bench-de-outro".to_owned()),
            "bancada alheia do mesmo projeto continua lá"
        );
        assert!(
            benches.contains(&"bench-new".to_owned()),
            "bancada do mutirão com mudança não comitada continua lá"
        );
        assert!(stubs.stored.lock().unwrap().is_empty());
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["notice"], "muster-notice-cleanup-dirty");
    }

    #[test]
    fn cleanup_done_never_calls_git() {
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
        runtime
            .set_setting(
                "muster.cleanup.on_finish",
                SettingValue::Text("remove".to_owned()),
            )
            .unwrap();
        let (_, task_id) = new_run_and_task(&mut runtime);
        runtime
            .call_method(
                "muster.start",
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        finish_new_bench_dispatch(&mut runtime, &task_id);
        runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("testes ok")),
                ]),
            )
            .unwrap();
        settle(&mut runtime);
        assert_eq!(
            *stubs.dirty_calls.lock().unwrap(),
            0,
            "a função chamada no done da Task não pode consultar git"
        );
        assert_eq!(stubs.stored.lock().unwrap().as_slice(), ["bench-new"]);
    }

    #[test]
    fn cleanup_ask_only_warns() {
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
                Map::from_iter([("task".to_owned(), json!(task_id.clone()))]),
            )
            .unwrap();
        finish_new_bench_dispatch(&mut runtime, &task_id);
        runtime
            .call_method(
                "muster.handoff",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("summary".to_owned(), json!("feito")),
                    ("evidence".to_owned(), json!("testes ok")),
                ]),
            )
            .unwrap();
        settle(&mut runtime);
        assert!(stubs.stored.lock().unwrap().is_empty());
        let status = runtime.call_method("muster.status", Map::new()).unwrap();
        assert_eq!(status["notice"], "muster-notice-cleanup-ask");
    }

