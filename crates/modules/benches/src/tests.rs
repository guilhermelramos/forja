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
        // Runners have no global identity; every commit in a temporary repo
        // must carry its own author and committer.
        .env("GIT_AUTHOR_NAME", "Forja Test")
        .env("GIT_AUTHOR_EMAIL", "forja@example.invalid")
        .env("GIT_COMMITTER_NAME", "Forja Test")
        .env("GIT_COMMITTER_EMAIL", "forja@example.invalid")
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

#[test]
fn unborn_repo_is_detected_before_opening_a_bench() {
    let root = temp_dir("unborn");
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("vazio");
    std::fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-b", "main"]);
    git(&path, &["config", "user.name", "Forja Test"]);
    git(&path, &["config", "user.email", "forja@example.invalid"]);
    assert!(!crate::git::has_commits(&path));
    assert!(!crate::git::has_remote(&path));
    std::fs::write(path.join("a.txt"), "a\n").unwrap();
    git(&path, &["add", "a.txt"]);
    git(&path, &["commit", "-m", "primeiro"]);
    assert!(crate::git::has_commits(&path));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn failed_creation_keeps_the_form_and_shows_the_error() {
    use crate::state::{CreateForm, Dialog};
    let shared = Shared::new();
    shared.set_dialog(Dialog::Create(CreateForm {
        name: "teste-agentes".to_owned(),
        creating: true,
        ..CreateForm::default()
    }));
    shared.finish_create(Some("git fetch failed".to_owned()));
    let Dialog::Create(form) = shared.dialog() else {
        panic!("the create dialog must stay open after a failure");
    };
    assert_eq!(form.name, "teste-agentes");
    assert!(!form.creating);
    assert_eq!(form.error.as_deref(), Some("git fetch failed"));
    shared.finish_create(None);
    assert!(matches!(shared.dialog(), Dialog::None));
}

#[test]
fn bench_errors_are_humanized_without_raw_terms() {
    let root = temp_dir("humanize-errors");
    let project_path = repo(&root);
    let project = forja_contracts::ProjectRef {
        id: "proj-hum".to_owned(),
        name: "projeto".to_owned(),
        path: project_path.display().to_string(),
    };
    let mut runtime = Runtime::new(
        vec![
            Box::new(BenchModule {
                state: Shared::new(),
            }),
            Box::new(TestProjects { project }),
        ],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("modules register cleanly");

    {
        let cx = runtime.ctx("benches");
        let base_missing = crate::create::humanize_error(&cx, "base branch `master` does not exist");
        assert!(!base_missing.summary.contains("`master`"));
        assert!(!base_missing.summary.contains("does not exist"));
        assert_eq!(
            base_missing.detail.as_deref(),
            Some("base branch `master` does not exist")
        );

        let dirty = crate::create::humanize_error(&cx, "bench has uncommitted changes; confirm force");
        assert!(dirty.summary.contains("mudanças não salvas"));
        assert!(dirty.detail.is_none());

        let git_fail = crate::create::humanize_error(&cx, "git worktree add failed: fatal: path already exists");
        assert!(!git_fail.summary.to_lowercase().contains("failed"));
        assert!(!git_fail.summary.to_lowercase().contains("fatal:"));
    }

    runtime.set_locale(forja_kernel::Locale::En);
    let cx = runtime.ctx("benches");
    let git_fail_en = crate::create::humanize_error(&cx, "git worktree add failed: Permission denied (os error 5)");
    assert!(!git_fail_en.summary.to_lowercase().contains("failed"));
    assert!(!git_fail_en.summary.to_lowercase().contains("os error"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_activation_switches_current_bench_and_default_create_project() {
    struct MultiProjects {
        projects: Vec<forja_contracts::ProjectRef>,
        active: std::sync::Arc<std::sync::Mutex<String>>,
    }

    impl Module for MultiProjects {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("project", "test.project.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n(
                "test-project-name = Projects\n",
                "test-project-name = Projetos\n",
            );
            r.contract_event::<forja_contracts::ProjectActivated>(
                forja_contracts::names::PROJECT_ACTIVATED,
            );
            let list = self.projects.clone();
            r.command("project.list", "test.project.name", move |_cx, _| {
                Ok(json!({ "projects": list.clone() }))
            });
            let projects = self.projects.clone();
            let active = self.active.clone();
            r.command("project.current", "test.project.name", move |_cx, _| {
                let id = active.lock().unwrap().clone();
                let found = projects.iter().find(|p| p.id == id).cloned();
                Ok(serde_json::to_value(found)?)
            });
        }
    }

    let root = temp_dir("switch-project");
    let path_a = root.join("alfa");
    let path_b = root.join("beta");
    std::fs::create_dir_all(&path_a).unwrap();
    std::fs::create_dir_all(&path_b).unwrap();
    let proj_a = forja_contracts::ProjectRef {
        id: "p-alfa".to_owned(),
        name: "alfa".to_owned(),
        path: path_a.display().to_string(),
    };
    let proj_b = forja_contracts::ProjectRef {
        id: "p-beta".to_owned(),
        name: "beta".to_owned(),
        path: path_b.display().to_string(),
    };
    let active = std::sync::Arc::new(std::sync::Mutex::new("p-alfa".to_owned()));
    let state = Shared::new();
    let mut runtime = Runtime::new(
        vec![
            Box::new(BenchModule {
                state: state.clone(),
            }),
            Box::new(MultiProjects {
                projects: vec![proj_a.clone(), proj_b.clone()],
                active: active.clone(),
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

    let initial = runtime.call_method("bench.current", Map::new()).unwrap();
    assert_eq!(initial["project_id"], "p-alfa");

    *active.lock().unwrap() = "p-beta".to_owned();
    {
        let mut cx = runtime.ctx("project");
        cx.emit(&forja_contracts::ProjectActivated(proj_b.clone()));
    }
    runtime.poll_tasks();

    let switched = runtime.call_method("bench.current", Map::new()).unwrap();
    assert_eq!(switched["project_id"], "p-beta");

    let _ = runtime.run_command("bench.create_dialog", forja_kernel::Args::new());
    match state.dialog() {
        crate::state::Dialog::Create(form) => assert_eq!(form.project_id, "p-beta"),
        other => panic!("expected create dialog for p-beta, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(root);
}
