//! Módulo `muster` da Forja: Mutirão, tarefas, recados e entregas.
//!
//! O estado persistido fica em `muster.json`; a página é uma Rail. O módulo
//! fala com bancadas, ajudantes e terminais por comandos e eventos.

mod commands;
mod delivery;
mod model;
mod preamble;
mod state;
mod view;

use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use state::Shared;

#[forja_kernel::module]
static MUSTER: fn() -> Box<dyn Module> = || Box::new(MusterModule::new());

/// Módulo Mutirão.
pub struct MusterModule {
    shared: Shared,
}

impl MusterModule {
    fn new() -> Self {
        Self {
            shared: state::new_shared(),
        }
    }
}

impl Default for MusterModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MusterModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("muster", "muster.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.shared);
        commands::subscribe_events(r, &self.shared);

        for key in view::USED_KEYS
            .iter()
            .chain(commands::USED_KEYS)
            .copied()
        {
            r.uses_i18n(key);
        }

        let shared = self.shared.clone();
        let order = if std::env::var_os("FORJA_MUSTER_PREVIEW").is_some() {
            0
        } else {
            50
        };
        r.view(Slot::Rail, "muster.page", "muster.page.title", order, move || {
            Box::new(view::MusterView::new(shared.clone()))
        });
        r.view_glyph("muster.page", forja_ui_kit::HELPERS);

        let tick_shared = self.shared.clone();
        r.view(
            Slot::StatusBar,
            "muster.tick",
            "muster.page.title",
            70,
            move || Box::new(view::MusterTick::new(tick_shared.clone())),
        );
    }
}


fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "muster.mode.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-mode-enabled-label",
        "setting-muster-mode-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "muster.plan.max_tasks",
        SettingKind::Integer {
            min: Some(1),
            max: Some(100),
        },
        SettingValue::Integer(20),
        "setting-muster-plan-max_tasks-label",
        "setting-muster-plan-max_tasks-help",
    ));
    r.setting(SettingSpec::new(
        "muster.plan.require_approval",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-plan-require_approval-label",
        "setting-muster-plan-require_approval-help",
    ));
    r.setting(SettingSpec::new(
        "muster.dispatch.max_parallel",
        SettingKind::Integer {
            min: Some(1),
            max: Some(16),
        },
        SettingValue::Integer(4),
        "setting-muster-dispatch-max_parallel-label",
        "setting-muster-dispatch-max_parallel-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notes.poll_s",
        SettingKind::Integer {
            min: Some(1),
            max: Some(60),
        },
        SettingValue::Integer(3),
        "setting-muster-notes-poll_s-label",
        "setting-muster-notes-poll_s-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notes.retention_days",
        SettingKind::Integer {
            min: Some(1),
            max: Some(365),
        },
        SettingValue::Integer(30),
        "setting-muster-notes-retention_days-label",
        "setting-muster-notes-retention_days-help",
    ));
    r.setting(SettingSpec::new(
        "muster.handoff.require_evidence",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-handoff-require_evidence-label",
        "setting-muster-handoff-require_evidence-help",
    ));
    r.setting(SettingSpec::new(
        "muster.budget.max_usd",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(10000.0),
        },
        SettingValue::Number(0.0),
        "setting-muster-budget-max_usd-label",
        "setting-muster-budget-max_usd-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notifications.task_failed",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-notifications-task_failed-label",
        "setting-muster-notifications-task_failed-help",
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-muster-module-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime_with(dir: &std::path::Path, shared: Shared) -> Runtime {
        Runtime::new(
            vec![Box::new(MusterModule { shared })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster module registers")
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        runtime_with(dir, state::new_shared())
    }

    #[test]
    fn run_rejects_whitespace_titles() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let error = runtime.call_method("muster.run", Map::from_iter([("title".into(), json!("  \t  "))])).unwrap_err();
        assert!(error.message.contains("Digite um título"));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contract_and_rpc_flow_degrades_without_helper() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        runtime
            .set_setting(
                "muster.plan.require_approval",
                SettingValue::Bool(false),
            )
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
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
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
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    struct StubShell {
        calls: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Module for StubShell {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("shell", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let calls = Arc::clone(&self.calls);
            r.command("shell.rail.open", "muster.command.open", move |_, args| {
                if let Some(view) = args.str("view_id") {
                    calls.lock().unwrap().push(view.to_owned());
                }
                Ok(json!({ "opening": true }))
            });
        }
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
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster and shell register");
        runtime
            .run_command("muster.open", forja_kernel::Args::new())
            .expect("open");
        assert_eq!(calls.lock().unwrap().as_slice(), ["muster.page"]);
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plan_approval_and_budget_gate_starting() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([
                    ("title".to_owned(), json!("Plano")),
                    ("budget_usd".to_owned(), json!(10.0)),
                ]),
            )
            .unwrap();
        let run_id = run["id"].as_str().unwrap().to_owned();
        assert_eq!(run["plan_approved"], false);
        assert_eq!(run["budget_usd"], 10.0);
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
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
                ]),
            )
            .expect_err("plan is pending");
        assert!(error.message.contains("aprovado"), "{}", error.message);
        let approved = runtime
            .call_method(
                "muster.plan.approve",
                Map::from_iter([("run".to_owned(), json!(run_id))]),
            )
            .unwrap();
        assert_eq!(approved["plan_approved"], true);
        let spent = runtime
            .call_method(
                "muster.spend",
                Map::from_iter([
                    ("run".to_owned(), json!(run_id)),
                    ("usd".to_owned(), json!(10.0)),
                ]),
            )
            .unwrap();
        assert_eq!(spent["spent_usd"], 10.0);
        let error = runtime
            .call_method(
                "muster.start",
                Map::from_iter([
                    ("task".to_owned(), json!(task_id)),
                    ("bench_path".to_owned(), json!(dir.display().to_string())),
                ]),
            )
            .expect_err("budget is exhausted");
        assert!(error.message.contains("teto"), "{}", error.message);
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn disabled_muster_rejects_new_runs() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        runtime.set_setting("muster.mode.enabled", SettingValue::Bool(false)).unwrap();
        assert!(runtime.call_method("muster.run", Map::from_iter([
            ("title".to_owned(), json!("Run")),
        ])).is_err());
        runtime.set_setting("muster.mode.enabled", SettingValue::Bool(true)).unwrap();
        assert!(runtime.call_method("muster.run", Map::from_iter([
            ("title".to_owned(), json!("Run")),
        ])).is_ok());
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dispatch_rejects_new_bench_until_creation_can_be_awaited() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let result = runtime.call_method("muster.start", Map::from_iter([
            ("task".to_owned(), json!("task-1")),
            ("new_bench".to_owned(), json!(true)),
        ])).unwrap_err();
        assert!(result.message.contains("bancada"), "{}", result.message);
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn max_tasks_limits_the_plan() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
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
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn handoff_with_evidence_marks_task_delivered() {
        let dir = temp_dir();
        let shared = state::new_shared();
        let mut runtime = runtime_with(&dir, shared.clone());
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
        let status = runtime
            .call_method("muster.status", Map::new())
            .unwrap();
        assert_eq!(status["tasks"][0]["state"], "delivered");
        assert_eq!(status["runs"][0]["state"], "done");
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn raw_muster_errors_are_humanized() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let cx = runtime.ctx("muster");
        let disabled = view::humanize_error(&cx, "muster is disabled");
        assert!(disabled.contains("desativado"), "{disabled}");
        let cmd = view::humanize_error(&cx, "command `muster.run` failed");
        assert!(cmd.contains("Não foi possível concluir"), "{cmd}");
        assert!(cmd.contains("muster.run"), "{cmd}");
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
