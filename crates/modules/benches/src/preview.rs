//! Preview data for creating a bench from a request.

use crate::config;
use crate::git;
use crate::model::{self, BenchFile, ScriptApproval};
use forja_contracts::ProjectRef;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub(crate) struct PreviewJob {
    pub project: ProjectRef,
    pub project_ids: Vec<String>,
    pub request_key: String,
    pub request_name: String,
    pub name_template: String,
    pub branch_prefix: String,
    pub bench_root: PathBuf,
    pub store_path: PathBuf,
    pub base_mode: String,
    pub setup_policy: String,
    pub fetch_before: bool,
    pub helper: bool,
}

pub(crate) fn build(job: PreviewJob) -> Result<Value, String> {
    let file = load_preview_file(&job.store_path)?;
    let missing_main = job
        .project_ids
        .iter()
        .filter(|project_id| file.main_for(project_id).is_none())
        .count() as u32;
    let number = file.benches.len() as u32 + missing_main + 1;
    let name = model::bench_name(
        &job.name_template,
        Some(&job.request_key),
        &job.request_name,
        number,
    );
    let branch_wanted = format!("{}{name}", job.branch_prefix);
    let path = job.bench_root.join(&name);
    let path_registered = file
        .benches
        .iter()
        .any(|bench| !bench.archived && git::same_path(Path::new(&bench.path), &path));
    let approval = file
        .approval(&job.project.id)
        .cloned()
        .unwrap_or_else(|| ScriptApproval {
            project_id: job.project.id.clone(),
            ..ScriptApproval::default()
        });
    let project_path = Path::new(&job.project.path);
    git::ensure_repo(project_path)?;
    if !git::has_commits(project_path) {
        return Err("no-commits".to_owned());
    }
    let base = git::resolve_base(project_path, &job.base_mode)?;
    let branch = git::unique_branch(project_path, &branch_wanted);
    let loaded = config::load(&job.project.path);
    let setup_state = if loaded.config.setup.is_empty() {
        "none"
    } else if job.setup_policy == "never" {
        "disabled"
    } else if loaded
        .setup_hash
        .as_deref()
        .is_some_and(|hash| approval.setup_hash.as_deref() == Some(hash))
    {
        "approved"
    } else {
        "needs_approval"
    };
    Ok(json!({
        "name": name,
        "branch": branch,
        "base": base.name,
        "path": path.display().to_string(),
        "path_exists": path.exists(),
        "path_registered": path_registered,
        "setup": loaded.config.setup,
        "setup_state": setup_state,
        "setup_warnings": loaded.warnings,
        "fetch_before": job.fetch_before && git::has_remote(project_path),
        "helper": job.helper,
    }))
}

fn load_preview_file(path: &Path) -> Result<BenchFile, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(serde_json::from_str(&contents).unwrap_or_default()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BenchFile::default()),
        Err(error) => Err(format!("could not read bench metadata: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::Store;
    use std::process::{Command, Stdio};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "forja-bench-preview-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git(path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .stdin(Stdio::null())
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?} succeeds");
    }

    #[test]
    fn request_preview_matches_branch_base_path_and_setup_trust() {
        let temp = TempDir::new("approved");
        let project_path = temp.0.join("project");
        std::fs::create_dir_all(&project_path).unwrap();
        git(&project_path, &["init", "--quiet", "--initial-branch=main"]);
        git(
            &project_path,
            &["config", "user.email", "test@example.invalid"],
        );
        git(&project_path, &["config", "user.name", "Forja Test"]);
        std::fs::write(project_path.join("README.md"), "project\n").unwrap();
        git(&project_path, &["add", "README.md"]);
        git(&project_path, &["commit", "--quiet", "-m", "seed"]);
        std::fs::write(project_path.join("forja.toml"), "setup = \"cargo check\"\n").unwrap();

        let setup_hash = config::load(&project_path.to_string_lossy())
            .setup_hash
            .expect("setup hash");
        let bench_root = temp.0.join("data").join("bancadas").join("p1");
        let store = Store::new(temp.0.join("data"));
        let store_path = store.path_for("bench");
        store
            .save(
                "bench",
                &BenchFile {
                    approvals: vec![ScriptApproval {
                        project_id: "p1".into(),
                        setup_hash: Some(setup_hash),
                        teardown_hash: None,
                    }],
                    ..BenchFile::default()
                },
            )
            .unwrap();
        let value = build(PreviewJob {
            project: ProjectRef {
                id: "p1".into(),
                name: "project".into(),
                path: project_path.to_string_lossy().into_owned(),
            },
            project_ids: vec!["p1".into()],
            request_key: "LOCAL-1".into(),
            request_name: "pedido-local-1-review".into(),
            name_template: "{pedido}-{slug}".into(),
            branch_prefix: "forja/".into(),
            bench_root: bench_root.clone(),
            store_path,
            base_mode: "auto".into(),
            setup_policy: "ask".into(),
            fetch_before: false,
            helper: true,
        })
        .expect("preview builds");

        assert_eq!(value["branch"], "forja/pedido-local-1-review");
        assert_eq!(value["base"], "main");
        assert_eq!(
            value["path"],
            bench_root
                .join("pedido-local-1-review")
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(value["setup"], "cargo check");
        assert_eq!(value["setup_state"], "approved");
        assert_eq!(value["helper"], true);
        assert_eq!(value["path_exists"], false);
    }
}
