use super::*;
use forja_kernel::{contract, Runtime, RuntimeOptions};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::process::Command;

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "forja-git-{label}-{}-{}",
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

fn has_ref(path: &std::path::Path, reference: &str) -> bool {
    Command::new("git")
        .args(["rev-parse", "--verify", reference])
        .current_dir(path)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn repo(root: &std::path::Path) -> PathBuf {
    let path = root.join("repo");
    std::fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-b", "main"]);
    git(&path, &["config", "user.name", "Forja Test"]);
    git(&path, &["config", "user.email", "forja@example.invalid"]);
    std::fs::write(path.join("a.txt"), "um\ndois\n").unwrap();
    git(&path, &["add", "a.txt"]);
    git(&path, &["commit", "-m", "primeiro"]);
    git(&path, &["checkout", "-b", "forja/teste"]);
    path
}

fn runtime(dir: &std::path::Path) -> Runtime {
    Runtime::new(
        vec![Box::new(GitModule {
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
fn contract_passes() {
    let root = temp_dir("contract");
    let runtime = runtime(&root);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stage_commit_and_push_to_a_bare_remote() {
    let root = temp_dir("push");
    std::fs::create_dir_all(&root).unwrap();
    let remote = root.join("remote.git");
    std::fs::create_dir_all(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    let work = repo(&root);
    std::fs::write(work.join("a.txt"), "um\ndois\ntres\n").unwrap();
    let mut runtime = runtime(&root);
    let path = work.to_string_lossy().into_owned();

    runtime
        .call_method(
            "git.stage",
            Map::from_iter([
                ("path".to_owned(), json!(path)),
                ("files".to_owned(), json!(["a.txt"])),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |_| {
        let report = engine::status(&work).unwrap();
        report.summary.staged > 0 || !report.files.iter().any(|file| file.path == "a.txt")
    });

    runtime
        .call_method(
            "git.commit",
            Map::from_iter([
                ("path".to_owned(), json!(path)),
                ("message".to_owned(), json!("muda a.txt")),
            ]),
        )
        .unwrap();
    wait(&mut runtime, |_| {
        let report = engine::status(&work).unwrap();
        report.files.is_empty()
    });

    git(
        &work,
        &["remote", "add", "origin", &remote.to_string_lossy()],
    );
    runtime
        .call_method(
            "git.push",
            Map::from_iter([("path".to_owned(), json!(path))]),
        )
        .unwrap();
    let branch = "forja/teste";
    wait(&mut runtime, |_| has_ref(&remote, branch));
    let pushed = git(&remote, &["rev-parse", "--verify", branch]);
    assert_eq!(pushed.trim().len(), 40);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn log_and_blame_are_readable() {
    let root = temp_dir("log");
    std::fs::create_dir_all(&root).unwrap();
    let work = repo(&root);
    std::fs::write(work.join("a.txt"), "um\ndois alterado\n").unwrap();
    git(&work, &["add", "a.txt"]);
    git(&work, &["commit", "-m", "segundo commit"]);
    let lines = log::log(&work, 20).unwrap();
    assert!(lines.iter().any(|line| line.commit.is_some()));
    assert!(lines.iter().any(|line| line.graph.contains('*')));
    let blame = log::blame(&work, "a.txt").unwrap();
    assert_eq!(blame.len(), 2);
    assert!(blame.iter().any(|line| line.content == "dois alterado"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn commit_show_returns_metadata_and_patch() {
    let root = temp_dir("show");
    std::fs::create_dir_all(&root).unwrap();
    let work = repo(&root);
    let lines = log::log(&work, 20).unwrap();
    let hash = lines
        .iter()
        .filter_map(|line| line.commit.as_ref())
        .next()
        .unwrap()
        .hash
        .clone();
    let shown = log::show(&work, &hash).unwrap();
    assert!(shown.contains("primeiro"));
    assert!(shown.contains("a.txt"));
    assert!(shown.contains("@@"));
    assert!(log::show(&work, "--help").is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn publish_and_abort_without_remote_or_merge_have_clear_outcomes() {
    let root = temp_dir("outcomes");
    std::fs::create_dir_all(&root).unwrap();
    let work = repo(&root);
    assert_eq!(engine::publish(&work).unwrap_err(), "no-remote");
    assert_eq!(engine::pull(&work).unwrap_err(), "no-remote");
    assert_eq!(engine::abort(&work).unwrap_err(), "nothing-to-abort");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn status_refresh_keeps_operation_errors_visible() {
    let state = Shared::new();
    state.set_error(Some("pull failed".to_owned()));
    state.set_report(engine::StatusReport::default());
    assert_eq!(state.error().as_deref(), Some("pull failed"));
}

#[test]
fn mutations_invalidate_the_cached_history() {
    let state = Shared::new();
    state.set_commits("C:/bancada", Vec::new());
    assert!(state.log_for("C:/bancada"));
    state.invalidate_log();
    assert!(!state.log_for("C:/bancada"));
}

#[test]
fn generate_without_a_helper_explains_what_is_missing() {
    let root = temp_dir("generate");
    std::fs::create_dir_all(&root).unwrap();
    let work = repo(&root);
    let mut runtime = runtime(&root);
    let shared = Shared::new();
    let mut ctx = runtime.ctx("git");
    let error = crate::view_changes::generate_message(&mut ctx, &work.to_string_lossy(), &shared)
        .unwrap_err();
    assert!(error.contains("Nenhum ajudante instalado"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn editor_paths_are_made_relative_to_the_bench() {
    let bench = "C:/bancadas/projeto";
    assert_eq!(
        crate::view_history_panel::relative_to_bench(bench, "C:/bancadas/projeto/src/main.rs")
            .as_deref(),
        Some("src/main.rs")
    );
    assert_eq!(
        crate::view_history_panel::relative_to_bench(bench, "src/main.rs").as_deref(),
        Some("src/main.rs")
    );
    assert_eq!(
        crate::view_history_panel::relative_to_bench(bench, "C:/outro/main.rs"),
        None
    );
}

#[test]
fn diff_has_hunks() {
    let root = temp_dir("diff");
    std::fs::create_dir_all(&root).unwrap();
    let work = repo(&root);
    std::fs::write(work.join("a.txt"), "um\ndois alterado\n").unwrap();
    let parsed = diff::diff(&work, "a.txt", false).unwrap();
    assert!(!parsed.hunks.is_empty());
    assert!(parsed.hunks[0].lines.iter().any(|line| line.kind == "add"));
    let _ = std::fs::remove_dir_all(root);
}
