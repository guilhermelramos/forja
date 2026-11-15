//! Narrow git wrapper for worktrees and branches.
//!
//! Every command runs with `GIT_TERMINAL_PROMPT=0` and a null stdin, so a
//! missing credential fails instead of freezing a worker thread.

use std::path::{Path, PathBuf};
use std::process::Stdio;

pub fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = forja_kernel::process::command("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not run git: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let first = stderr.lines().next().unwrap_or("").trim();
        return Err(format!(
            "git {} failed{}",
            args.join(" "),
            if first.is_empty() {
                String::new()
            } else {
                format!(": {first}")
            }
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn ensure_repo(path: &Path) -> Result<(), String> {
    git(path, &["rev-parse", "--git-dir"]).map(|_| ())
}

/// False for a repository created by `git init` before its first commit.
pub fn has_commits(path: &Path) -> bool {
    git(path, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

pub fn has_remote(path: &Path) -> bool {
    git(path, &["remote"])
        .map(|remotes| !remotes.trim().is_empty())
        .unwrap_or(false)
}

pub fn current_branch(path: &Path) -> Result<String, String> {
    let branch = git(path, &["branch", "--show-current"])?;
    let branch = branch.trim().to_owned();
    if branch.is_empty() {
        Err("detached HEAD has no branch".to_owned())
    } else {
        Ok(branch)
    }
}

pub fn head(path: &Path) -> Result<String, String> {
    Ok(git(path, &["rev-parse", "HEAD"])?.trim().to_owned())
}

pub fn local_branch_exists(project: &Path, branch: &str) -> bool {
    git(
        project,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )
    .is_ok()
}

pub fn remote_branch_exists(project: &Path, branch: &str) -> bool {
    git(
        project,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/remotes/origin/{branch}"),
        ],
    )
    .is_ok()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub name: String,
    pub git_ref: String,
}

/// Resolves `bench.worktree.base_branch` without touching the network.
pub fn resolve_base(project: &Path, mode: &str) -> Result<Base, String> {
    match mode {
        "current" => {
            let name = current_branch(project)?;
            Ok(Base {
                git_ref: name.clone(),
                name,
            })
        }
        "main" | "master" => named_base(project, mode),
        _ => {
            if let Ok(text) = git(project, &["rev-parse", "--abbrev-ref", "origin/HEAD"]) {
                let reference = text.trim();
                if let Some(name) = reference.strip_prefix("origin/") {
                    if remote_branch_exists(project, name) {
                        return Ok(Base {
                            name: name.to_owned(),
                            git_ref: reference.to_owned(),
                        });
                    }
                }
            }
            let name = current_branch(project)?;
            Ok(Base {
                git_ref: name.clone(),
                name,
            })
        }
    }
}

fn named_base(project: &Path, name: &str) -> Result<Base, String> {
    if remote_branch_exists(project, name) {
        return Ok(Base {
            name: name.to_owned(),
            git_ref: format!("origin/{name}"),
        });
    }
    if local_branch_exists(project, name) {
        return Ok(Base {
            name: name.to_owned(),
            git_ref: name.to_owned(),
        });
    }
    Err(format!("base branch `{name}` does not exist"))
}

pub fn fetch(project: &Path) -> Result<(), String> {
    git(project, &["fetch", "--prune", "origin"]).map(|_| ())
}

pub fn unique_branch(project: &Path, wanted: &str) -> String {
    if !local_branch_exists(project, wanted) {
        return wanted.to_owned();
    }
    for suffix in 2..=99 {
        let candidate = format!("{wanted}-{suffix}");
        if !local_branch_exists(project, &candidate) {
            return candidate;
        }
    }
    format!("{wanted}-{}", crate::model::now_millis())
}

pub fn add_worktree(project: &Path, path: &Path, branch: &str, base: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create the bench folder: {error}"))?;
    }
    if path.exists() {
        return Err(format!(
            "the bench folder already exists: {}",
            path.display()
        ));
    }
    let path_text = path.to_string_lossy().into_owned();
    git(
        project,
        &["worktree", "add", "-b", branch, "--", &path_text, base],
    )
    .map(|_| ())
}

pub fn remove_worktree(project: &Path, path: &Path, force: bool) -> Result<(), String> {
    let path_text = path.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&path_text);
    git(project, &args).map(|_| ())
}

pub fn delete_branch(project: &Path, branch: &str) -> Result<(), String> {
    git(project, &["branch", "-D", branch]).map(|_| ())
}

pub fn is_dirty(path: &Path) -> Result<bool, String> {
    let status = git(path, &["status", "--porcelain"])?;
    Ok(!status.trim().is_empty())
}

/// True when HEAD moved from the recorded creation commit.
pub fn has_new_commits(path: &Path, created_head: Option<&str>) -> bool {
    let Some(created_head) = created_head else {
        return false;
    };
    head(path)
        .map(|current| current != created_head)
        .unwrap_or(false)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub detached: bool,
    pub bare: bool,
}

impl WorktreeEntry {
    pub fn is_main(&self, project: &Path) -> bool {
        same_path(&self.path, project)
    }
}

pub fn list_worktrees(project: &Path) -> Result<Vec<WorktreeEntry>, String> {
    let output = git(project, &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_list(&output))
}

pub fn parse_worktree_list(text: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut current: Option<WorktreeEntry> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            if let Some(entry) = current.take() {
                entries.push(entry);
            }
            continue;
        }
        let Some((key, value)) = line.split_once(' ') else {
            continue;
        };
        match key {
            "worktree" => {
                if let Some(entry) = current.take() {
                    entries.push(entry);
                }
                current = Some(WorktreeEntry {
                    path: PathBuf::from(value),
                    branch: None,
                    head: None,
                    detached: false,
                    bare: false,
                });
            }
            "HEAD" => {
                if let Some(entry) = &mut current {
                    entry.head = Some(value.to_owned());
                }
            }
            "branch" => {
                if let Some(entry) = &mut current {
                    entry.branch = Some(value.trim_start_matches("refs/heads/").to_owned());
                }
            }
            "detached" => {
                if let Some(entry) = &mut current {
                    entry.detached = true;
                }
            }
            "bare" => {
                if let Some(entry) = &mut current {
                    entry.bare = true;
                }
            }
            _ => {}
        }
    }
    if let Some(entry) = current {
        entries.push(entry);
    }
    entries
}

pub fn same_path(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    };
    normalize(left) == normalize(right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_list_is_parsed() {
        let text = "worktree C:/src/app\nHEAD abc\nbranch refs/heads/main\n\nworktree C:/bancadas/x\nHEAD def\nbranch refs/heads/forja/x\n";
        let entries = parse_worktree_list(text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].path, PathBuf::from("C:/bancadas/x"));
    }
}
