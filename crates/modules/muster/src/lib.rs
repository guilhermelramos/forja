//! Módulo `muster` da Forja: Mutirão, tarefas, recados e entregas.
//!
//! O estado persistido fica em `muster.json`; a página é uma Rail. O módulo
//! fala com bancadas, ajudantes e terminais por comandos e eventos.

mod commands;
mod cost;
mod delivery;
pub mod demo;
mod model;
mod plan;
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
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.shared);
        commands::subscribe_events(r, &self.shared);

        for key in view::USED_KEYS.iter().chain(commands::USED_KEYS).copied() {
            r.uses_i18n(key);
        }

        let shared = self.shared.clone();
        let order = if std::env::var_os("FORJA_MUSTER_PREVIEW").is_some() {
            0
        } else {
            50
        };
        r.view(
            Slot::Rail,
            "muster.page",
            "muster.page.title",
            order,
            move || Box::new(view::MusterView::new(shared.clone())),
        );
        r.view_glyph("muster.page", forja_ui_kit::Icon::Muster.id());

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
        "muster.coordinator.profile",
        // Espelha `helper.default.profile`; o helper ainda não expõe uma API
        // pública com a lista de perfis descobertos.
        SettingKind::Choice {
            options: [
                "auto",
                "aider",
                "amp",
                "antigravity",
                "claude-code",
                "cline",
                "codex",
                "continue",
                "copilot",
                "crush",
                "cursor-agent",
                "gemini",
                "goose",
                "kimi-code",
                "kiro",
                "opencode",
                "qwen-code",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        },
        SettingValue::Text("auto".to_owned()),
        "setting-muster-coordinator-profile-label",
        "setting-muster-coordinator-profile-help",
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
        "muster.dispatch.target",
        SettingKind::Choice {
            options: ["ask", "current", "new"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        "setting-muster-dispatch-target-label",
        "setting-muster-dispatch-target-help",
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
        "muster.escalation.max_retries",
        SettingKind::Integer {
            min: Some(0),
            max: Some(10),
        },
        SettingValue::Integer(2),
        "setting-muster-escalation-max_retries-label",
        "setting-muster-escalation-max_retries-help",
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
    use forja_contracts::{BenchCreateFailed, BenchCreated, BenchRef, TerminalExited, TerminalRef};
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::PathBuf;
    use std::sync::Arc;

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-muster-module-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
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

    struct StubHelper {
        profiles: Arc<std::sync::Mutex<Vec<String>>>,
        paths: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Module for StubHelper {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("helper", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let profiles = Arc::clone(&self.profiles);
            let paths = Arc::clone(&self.paths);
            r.command("helper.launch", "muster.command.start", move |_, args| {
                let profile = args.str("profile_id").unwrap_or("auto").to_owned();
                profiles.lock().unwrap().push(profile.clone());
                paths
                    .lock()
                    .unwrap()
                    .push(args.str("bench_path").unwrap_or("").to_owned());
                Ok(json!({
                    "profile_id": profile,
                    "terminal_id": "9",
                    "mode": "tui"
                }))
            });
        }
    }

    struct StubBench {
        current: String,
        created: Arc<std::sync::Mutex<Vec<String>>>,
        stored: Arc<std::sync::Mutex<Vec<String>>>,
        dirty: Arc<std::sync::Mutex<std::collections::BTreeMap<String, bool>>>,
    }

    impl Module for StubBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let current = self.current.clone();
            r.command("bench.current", "muster.command.status", move |_, _| {
                Ok(json!({
                    "project_id": "p1",
                    "bench_id": "bench-current",
                    "name": "atual",
                    "path": current
                }))
            });

            let created = Arc::clone(&self.created);
            r.command("bench.create", "muster.command.start", move |_, args| {
                let name = args.required_str("name")?.to_owned();
                created.lock().unwrap().push(name.clone());
                Ok(json!({
                    "project_id": "p1",
                    "bench_id": "bench-new",
                    "name": name,
                    "path": format!("C:/bancadas/{name}")
                }))
            });

            let stored = Arc::clone(&self.stored);
            r.command("bench.store", "muster.command.start", move |_, args| {
                stored
                    .lock()
                    .unwrap()
                    .push(args.required_str("bench_id")?.to_owned());
                Ok(json!({ "pending": true }))
            });

            let dirty = Arc::clone(&self.dirty);
            r.command("bench.is_dirty", "muster.command.status", move |_, args| {
                let id = args.required_str("bench_id")?;
                let value = dirty.lock().unwrap().get(id).copied().unwrap_or(false);
                Ok(json!({ "dirty": value }))
            });
        }
    }

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
    fn plan_approval_and_budget_gate_starting() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir.0);
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
                    ("bench_path".to_owned(), json!(dir.0.display().to_string())),
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
                    ("bench_path".to_owned(), json!(dir.0.display().to_string())),
                ]),
            )
            .expect_err("budget is exhausted");
        assert!(error.message.contains("teto"), "{}", error.message);
    }

    struct StubUsage {
        usd: f64,
    }

    impl Module for StubUsage {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("usage", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let usd = self.usd;
            r.command("usage.spend", "muster.command.spend", move |_, _| {
                Ok(json!({
                    "period": "all",
                    "project": "C:/bench",
                    "matched_projects": 1,
                    "usd": usd,
                    "tokens": 100,
                    "unpriced_tokens": 0,
                    "priced": true,
                    "pending": false
                }))
            });
        }
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

    fn bench_runtime(dir: &std::path::Path) -> (Runtime, BenchStubs) {
        let stubs = BenchStubs::default();
        let runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubHelper {
                    profiles: Arc::clone(&stubs.profiles),
                    paths: Arc::clone(&stubs.paths),
                }),
                Box::new(StubBench {
                    current: "C:/atual".to_owned(),
                    created: Arc::clone(&stubs.created),
                    stored: Arc::clone(&stubs.stored),
                    dirty: Arc::clone(&stubs.dirty),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster, helper and bench register");
        (runtime, stubs)
    }

    /// Drena as tarefas em segundo plano até não haver trabalho pendente.
    fn settle(runtime: &mut Runtime) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || {
            let _ = sender.send(());
        }));
        loop {
            runtime.poll_tasks();
            if runtime.tasks().inflight() == 0 {
                runtime.poll_tasks();
                return;
            }
            let _ = receiver.recv_timeout(std::time::Duration::from_secs(10));
        }
    }

    #[derive(Default)]
    struct BenchStubs {
        profiles: Arc<std::sync::Mutex<Vec<String>>>,
        paths: Arc<std::sync::Mutex<Vec<String>>>,
        created: Arc<std::sync::Mutex<Vec<String>>>,
        stored: Arc<std::sync::Mutex<Vec<String>>>,
        dirty: Arc<std::sync::Mutex<std::collections::BTreeMap<String, bool>>>,
    }

    fn new_run_and_task(runtime: &mut Runtime) -> (String, String) {
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Despacho"))]),
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
        (
            run["id"].as_str().unwrap().to_owned(),
            task["id"].as_str().unwrap().to_owned(),
        )
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
}
