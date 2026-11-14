use super::*;
use forja_kernel::{contract, Runtime, RuntimeOptions};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "forja-benches-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn git(path: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn repo(root: &std::path::Path) -> PathBuf {
    let path = root.join("projeto");
    std::fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-b", "main"]);
    git(&path, &["config", "user.name", "Forja Test"]);
    git(&path, &["config", "user.email", "forja@example.invalid"]);
    std::fs::write(path.join("README.md"), "# projeto\n").unwrap();
    git(&path, &["add", "README.md"]);
    git(&path, &["commit", "-m", "inicio"]);
    path
}

struct TestProjects {
    project: forja_contracts::ProjectRef,
}

impl Module for TestProjects {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("project", "test.project.name", "0.1.0")
    }
    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            "test-project-name = Projects\n",
            "test-project-name = Projetos\n",
        );
        let project = self.project.clone();
        r.command("project.list", "test.project.name", move |_cx, _| {
            Ok(json!({ "projects": [project.clone()] }))
        });
    }
}

fn runtime(root: &std::path::Path, project: Option<forja_contracts::ProjectRef>) -> Runtime {
    let mut modules: Vec<Box<dyn Module>> = vec![Box::new(BenchModule {
        state: Shared::new(),
    })];
    if let Some(project) = project {
        modules.push(Box::new(TestProjects { project }));
    }
    Runtime::new(
        modules,
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("modules register cleanly")
}

fn wait(runtime: &mut Runtime, mut ready: impl FnMut(&mut Runtime) -> bool) {
    let (sender, receiver) = std::sync::mpsc::channel();
    runtime.set_wake(std::sync::Arc::new(move || { let _ = sender.send(()); }));
    loop {
        runtime.poll_tasks();
        if ready(runtime) { return; }
        receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("task did not signal completion");
    }
}

#[test]
fn contract_passes_without_other_modules() {
    let root = temp_dir("contract");
    let runtime = runtime(&root, None);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn commands_that_need_context_declare_what_the_anvil_asks() {
    let root = temp_dir("ask-fields");
    let runtime = runtime(&root, None);
    let fields = |id: &str| {
        runtime
            .registry()
            .command(id)
            .unwrap_or_else(|| panic!("command {id} is registered"))
            .fields
            .iter()
            .map(|field| field.arg)
            .collect::<Vec<_>>()
    };
    assert_eq!(fields("bench.activate"), ["bench_id"]);
    assert_eq!(fields("bench.create"), ["project_id", "name"]);
    assert_eq!(fields("bench.stage.set"), ["bench_id", "stage"]);
    assert_eq!(fields("bench.store"), ["bench_id"]);
    assert_eq!(fields("bench.script.approve"), ["project_id"]);
    assert!(fields("bench.refresh").is_empty());
    let activate = &runtime.registry().command("bench.activate").unwrap().fields[0];
    assert!(matches!(
        activate.kind,
        forja_kernel::CommandFieldKind::Choice {
            command: "bench.list",
            value: "bench_id",
            ..
        }
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn create_store_and_stage_transitions() {
    let root = temp_dir("lifecycle");
    std::fs::create_dir_all(&root).unwrap();
    let project_path = repo(&root);
    let project = forja_contracts::ProjectRef {
        id: "p1".to_owned(),
        name: "Projeto".to_owned(),
        path: project_path.to_string_lossy().into_owned(),
    };
    let mut runtime = runtime(&root, Some(project.clone()));
    contract::assert_ok(runtime.registry(), runtime.i18n());

    let created = runtime
        .call_method(
            "bench.create",
            Map::from_iter([
                ("project_id".to_owned(), json!("p1")),
                ("name".to_owned(), json!("Primeira bancada")),
            ]),
        )
        .unwrap();
    assert!(created["path"]
        .as_str()
        .unwrap()
        .contains("primeira-bancada"));
    wait(&mut runtime, |runtime| {
        let listed = runtime.call_method("bench.list", Map::new()).unwrap();
        listed["benches"].as_array().unwrap().len() >= 2
    });
    let listed = runtime.call_method("bench.list", Map::new()).unwrap();
    let benches = listed["benches"].as_array().unwrap();
    let bench = benches
        .iter()
        .find(|bench| bench["name"] == "primeira-bancada")
        .expect("new bench");
    let bench_id = bench["bench_id"].as_str().unwrap().to_owned();
    let bench_path = PathBuf::from(bench["path"].as_str().unwrap());
    assert!(bench_path.exists());
    assert!(bench_path.join("README.md").exists());

    let forged = runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id)),
                ("stage".to_owned(), json!("forging")),
            ]),
        )
        .unwrap();
    assert_eq!(forged["stage"], "forging");
    let backward = runtime.call_method(
        "bench.stage.set",
        Map::from_iter([
            ("bench_id".to_owned(), json!(bench_id)),
            ("stage".to_owned(), json!("draft")),
        ]),
    );
    assert!(backward.is_err(), "backward needs confirmation");
    runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id)),
                ("stage".to_owned(), json!("draft")),
                ("confirm_backward".to_owned(), json!(true)),
            ]),
        )
        .unwrap();
    assert!(runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id)),
                ("stage".to_owned(), json!("ready")),
            ]),
        )
        .is_err());

    runtime
        .call_method(
            "bench.store",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id)),
                ("delete_branch".to_owned(), json!(false)),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |_| !bench_path.exists());
    let archived = runtime
        .call_method(
            "bench.list",
            Map::from_iter([("include_archived".to_owned(), json!(true))]),
        )
        .unwrap();
    let bench = archived["benches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bench| bench["bench_id"] == bench_id.as_str())
        .unwrap();
    assert_eq!(bench["branch"], "forja/primeira-bancada");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn setup_runs_only_after_the_exact_hash_is_approved() {
    let root = temp_dir("approval");
    std::fs::create_dir_all(&root).unwrap();
    let project_path = repo(&root);
    std::fs::write(
        project_path.join("forja.toml"),
        "setup = \"Set-Content -Path forja-setup-marker.txt -Value setup\"\ntabs = []\n",
    )
    .unwrap();
    let project = forja_contracts::ProjectRef {
        id: "p1".to_owned(),
        name: "Projeto".to_owned(),
        path: project_path.to_string_lossy().into_owned(),
    };
    let mut runtime = runtime(&root, Some(project));
    runtime
        .call_method(
            "bench.create",
            Map::from_iter([
                ("project_id".to_owned(), json!("p1")),
                ("name".to_owned(), json!("sem aprovacao")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |runtime| {
        let listed = runtime.call_method("bench.list", Map::new()).unwrap();
        listed["benches"].as_array().unwrap().len() >= 2
    });
    let listed = runtime.call_method("bench.list", Map::new()).unwrap();
    let path = listed["benches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bench| bench["name"] == "sem-aprovacao")
        .and_then(|bench| bench["path"].as_str())
        .unwrap()
        .to_owned();
    assert!(!PathBuf::from(&path).join("forja-setup-marker.txt").exists());

    runtime
        .call_method(
            "bench.script.approve",
            Map::from_iter([("project_id".to_owned(), json!("p1"))]),
        )
        .unwrap();
    runtime
        .call_method(
            "bench.create",
            Map::from_iter([
                ("project_id".to_owned(), json!("p1")),
                ("name".to_owned(), json!("com aprovacao")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |runtime| {
        let listed = runtime.call_method("bench.list", Map::new()).unwrap();
        listed["benches"].as_array().unwrap().len() >= 3
    });
    let listed = runtime.call_method("bench.list", Map::new()).unwrap();
    let path = listed["benches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bench| bench["name"] == "com-aprovacao")
        .and_then(|bench| bench["path"].as_str())
        .unwrap()
        .to_owned();
    let marker = PathBuf::from(&path).join("forja-setup-marker.txt");
    wait(&mut runtime, |_| marker.exists());
    assert!(marker.exists(), "approved setup ran in the bench");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn archive_on_ready_stores_bench_automatically() {
    let root = temp_dir("archive-ready");
    std::fs::create_dir_all(&root).unwrap();
    let project_path = repo(&root);
    let project = forja_contracts::ProjectRef {
        id: "p1".to_owned(),
        name: "Projeto".to_owned(),
        path: project_path.to_string_lossy().into_owned(),
    };
    let mut runtime = runtime(&root, Some(project));
    runtime
        .set_setting(
            "bench.stage.archive_on_ready",
            forja_kernel::SettingValue::Bool(true),
        )
        .unwrap();
    runtime
        .call_method(
            "bench.create",
            Map::from_iter([
                ("project_id".to_owned(), json!("p1")),
                ("name".to_owned(), json!("auto-ready")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |runtime| {
        let listed = runtime.call_method("bench.list", Map::new()).unwrap();
        listed["benches"].as_array().unwrap().len() >= 2
    });
    let listed = runtime.call_method("bench.list", Map::new()).unwrap();
    let bench_id = listed["benches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|bench| bench["name"] == "auto-ready")
        .and_then(|bench| bench["bench_id"].as_str())
        .unwrap()
        .to_owned();
    runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id.clone())),
                ("stage".to_owned(), json!("forging")),
            ]),
        )
        .unwrap();
    runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id.clone())),
                ("stage".to_owned(), json!("review")),
            ]),
        )
        .unwrap();
    runtime
        .call_method(
            "bench.stage.set",
            Map::from_iter([
                ("bench_id".to_owned(), json!(bench_id.clone())),
                ("stage".to_owned(), json!("ready")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |runtime| {
        let listed = runtime
            .call_method(
                "bench.list_cards",
                Map::from_iter([("include_archived".to_owned(), json!(true))]),
            )
            .unwrap();
        listed["benches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["bench"]["bench_id"] == bench_id && b["archived"] == true)
    });
    let _ = std::fs::remove_dir_all(root);
}

struct StubTerminal {
    closes: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
}

impl Module for StubTerminal {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("terminal", "test.terminal.name", "0.1.0")
    }
    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            "test-terminal-name = Terminal\ntest-terminal-close = Close terminal\n",
            "test-terminal-name = Terminal\ntest-terminal-close = Fechar terminal\n",
        );
        let closes = std::sync::Arc::clone(&self.closes);
        r.command("terminal.close", "test.terminal.close", move |_cx, args| {
            if let Some(path) = args.str("bench_path") {
                closes.lock().unwrap().push(path.to_owned());
            }
            Ok(json!({ "closed": true }))
        });
    }
}

#[test]
fn sleep_respects_stop_helpers_setting() {
    let root = temp_dir("sleep-stop-helpers");
    let project_path = repo(&root);
    let project = forja_contracts::ProjectRef {
        id: "proj-stop".to_owned(),
        name: "projeto".to_owned(),
        path: project_path.display().to_string(),
    };
    let closes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut runtime = Runtime::new(
        vec![
            Box::new(BenchModule {
                state: Shared::new(),
            }),
            Box::new(TestProjects { project }),
            Box::new(StubTerminal {
                closes: std::sync::Arc::clone(&closes),
            }),
        ],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("modules register cleanly");

    runtime
        .set_setting(
            "bench.close.stop_helpers",
            forja_kernel::SettingValue::Bool(false),
        )
        .unwrap();
    let _ = runtime
        .call_method(
            "bench.create",
            Map::from_iter([
                ("project_id".to_owned(), json!("proj-stop")),
                ("name".to_owned(), json!("no-stop")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |runtime| {
        let listed = runtime.call_method("bench.list", Map::new()).unwrap();
        listed["benches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["name"] == "no-stop")
    });
    let listed = runtime.call_method("bench.list", Map::new()).unwrap();
    let bench_id = listed["benches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "no-stop")
        .unwrap()["bench_id"]
        .as_str()
        .unwrap()
        .to_owned();
    runtime
        .call_method(
            "bench.sleep",
            Map::from_iter([("bench_id".to_owned(), json!(bench_id))]),
        )
        .unwrap();
    assert!(closes.lock().unwrap().is_empty());

    runtime
        .call_method(
            "bench.wake",
            Map::from_iter([("bench_id".to_owned(), json!(bench_id))]),
        )
        .unwrap();
    runtime
        .set_setting(
            "bench.close.stop_helpers",
            forja_kernel::SettingValue::Bool(true),
        )
        .unwrap();
    runtime
        .call_method(
            "bench.sleep",
            Map::from_iter([("bench_id".to_owned(), json!(bench_id))]),
        )
        .unwrap();
    assert_eq!(closes.lock().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(root);
}
