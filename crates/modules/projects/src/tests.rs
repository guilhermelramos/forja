use super::*;
use forja_kernel::{contract, Runtime, RuntimeOptions};
use std::path::PathBuf;

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "forja-projects-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn runtime(dir: &std::path::Path) -> Runtime {
    Runtime::new(
        vec![Box::new(ProjectsModule {
            state: Shared::new(),
        })],
        RuntimeOptions {
            data_dir: Some(dir.to_path_buf()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("module registers cleanly")
}

#[test]
fn contract_and_project_lifecycle() {
    let root = temp_dir("lifecycle");
    std::fs::create_dir_all(&root).unwrap();
    let project_dir = root.join("oficina");
    std::fs::create_dir_all(&project_dir).unwrap();
    let mut runtime = runtime(&root);
    contract::assert_ok(runtime.registry(), runtime.i18n());

    let params = Map::from_iter([
        ("path".to_owned(), json!(project_dir)),
        ("name".to_owned(), json!("Oficina")),
        ("icon".to_owned(), json!("F")),
        ("color".to_owned(), json!("#f2762e")),
    ]);
    let added = runtime.call_method("project.add", params).unwrap();
    assert_eq!(added["name"], "Oficina");
    assert_eq!(added["color"], Value::Null, "ProjectRef has no color");

    let listed = runtime.call_method("project.list", Map::new()).unwrap();
    assert_eq!(listed["projects"].as_array().unwrap().len(), 1);
    assert_eq!(listed["projects"][0]["name"], "Oficina");

    let duplicate = runtime.call_method(
        "project.add",
        Map::from_iter([("path".to_owned(), json!(project_dir))]),
    );
    assert!(duplicate.is_err());

    let removed = runtime
        .call_method(
            "project.remove",
            Map::from_iter([("project_id".to_owned(), json!(added["id"]))]),
        )
        .unwrap();
    assert_eq!(removed["removed"], true);
    assert!(
        project_dir.exists(),
        "removing a project never deletes files"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn added_project_persists_in_the_module_file() {
    let root = temp_dir("persist");
    std::fs::create_dir_all(root.join("loja")).unwrap();
    let path = root.join("loja").to_string_lossy().into_owned();
    {
        let mut runtime = runtime(&root);
        runtime
            .run_command(
                "project.add",
                Args::from_map(Map::from_iter([("path".to_owned(), json!(path))])),
            )
            .unwrap();
    }
    let mut runtime = runtime(&root);
    let listed = runtime.call_method("project.list", Map::new()).unwrap();
    assert_eq!(listed["projects"][0]["name"], "loja");
    assert!(root.join("modules/project.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn clone_copies_a_local_repository() {
    let root = temp_dir("clone");
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("origem");
    std::fs::create_dir_all(&source).unwrap();
    let run = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(&["init", "-b", "main"]);
    std::fs::write(source.join("a.txt"), "a").unwrap();
    run(&["add", "a.txt"]);
    run(&[
        "-c",
        "user.name=Teste",
        "-c",
        "user.email=t@example.invalid",
        "commit",
        "-m",
        "a",
    ]);
    let destination = root.join("clonado");
    state::clone_repository(&source.to_string_lossy(), &destination.to_string_lossy()).unwrap();
    assert!(destination.join(".git").exists());
    assert!(destination.join("a.txt").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn create_initializes_a_git_repository() {
    let root = temp_dir("create");
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("novo");
    let mut runtime = runtime(&root);
    runtime
        .run_command(
            "project.create",
            Args::from_map(Map::from_iter([("path".to_owned(), json!(target))])),
        )
        .unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    runtime.set_wake(std::sync::Arc::new(move || { let _ = sender.send(()); }));
    loop {
        runtime.poll_tasks();
        let listed = runtime.call_method("project.list", Map::new()).unwrap();
        if listed["projects"].as_array().unwrap().len() == 1 { break; }
        receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("project creation did not signal completion");
    }
    assert!(target.join(".git").exists(), "git init ran");
    let listed = runtime.call_method("project.list", Map::new()).unwrap();
    assert_eq!(listed["projects"].as_array().unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raw_project_errors_are_humanized_in_pt_br() {
    let root = temp_dir("humanize-err");
    std::fs::create_dir_all(&root).unwrap();
    let mut runtime = runtime(&root);
    let cx = runtime.ctx("project");
    assert_eq!(view::humanize_error(&cx, "path is empty"), "Informe o caminho da pasta.");
    let missing = view::humanize_error(
        &cx,
        "folder does not exist: The system cannot find the path specified. (os error 3)",
    );
    assert!(missing.starts_with("Pasta inexistente"), "{missing}");
    assert!(missing.contains("os error 3"), "{missing}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_project_switches_persists_and_falls_back_on_remove() {
    let root = temp_dir("active");
    let alfa = root.join("alfa");
    let beta = root.join("beta");
    std::fs::create_dir_all(&alfa).unwrap();
    std::fs::create_dir_all(&beta).unwrap();

    let (alfa_id, beta_id) = {
        let mut runtime = runtime(&root);
        let switch_cmd = runtime
            .registry()
            .command("project.switch")
            .expect("project.switch command registered");
        assert_eq!(switch_cmd.fields.len(), 1);
        assert_eq!(switch_cmd.fields[0].arg, "project_id");

        let alfa_ref = runtime
            .call_method("project.add", Map::from_iter([("path".to_owned(), json!(alfa))]))
            .unwrap();
        let beta_ref = runtime
            .call_method("project.add", Map::from_iter([("path".to_owned(), json!(beta))]))
            .unwrap();
        let current = runtime.call_method("project.current", Map::new()).unwrap();
        assert_eq!(current["id"], beta_ref["id"]);

        let switched = runtime
            .call_method(
                "project.switch",
                Map::from_iter([("project_id".to_owned(), alfa_ref["id"].clone())]),
            )
            .unwrap();
        assert_eq!(switched["id"], alfa_ref["id"]);
        let current = runtime.call_method("project.current", Map::new()).unwrap();
        assert_eq!(current["id"], alfa_ref["id"]);

        (
            alfa_ref["id"].as_str().unwrap().to_owned(),
            beta_ref["id"].as_str().unwrap().to_owned(),
        )
    };

    // Reopen runtime on the same data_dir and verify active project persisted.
    let mut reopened = runtime(&root);
    let current = reopened.call_method("project.current", Map::new()).unwrap();
    assert_eq!(current["id"], alfa_id);

    reopened
        .call_method(
            "project.remove",
            Map::from_iter([("project_id".to_owned(), json!(alfa_id))]),
        )
        .unwrap();
    let fallback = reopened.call_method("project.current", Map::new()).unwrap();
    assert_eq!(fallback["id"], beta_id);
    let _ = std::fs::remove_dir_all(root);
}
