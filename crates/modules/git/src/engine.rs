//! Git engine: status, staging, commits, synchronization and conflicts.
//!
//! Every command uses `GIT_TERMINAL_PROMPT=0` and a null stdin. Reads may run
//! on the caller thread; writes are dispatched by the module through
//! `Ctx::spawn` so the interface never waits on a hook or a remote.

use forja_contracts::GitSummary;
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;

/// Git writes `.git/index.lock` for many read commands, so concurrent status
/// and staging collide. One process-wide lock keeps every Git call serialized.
static GIT_LOCK: Mutex<()> = Mutex::new(());

pub fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let _guard = GIT_LOCK.lock().unwrap_or_else(|error| error.into_inner());
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct GitFile {
    pub path: String,
    pub index: char,
    pub worktree: char,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflict: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct StatusReport {
    pub summary: GitSummary,
    pub files: Vec<GitFile>,
    pub upstream: Option<String>,
    pub detached: bool,
}

pub fn status(path: &Path) -> Result<StatusReport, String> {
    let text = git(
        path,
        &[
            "-c",
            "core.quotepath=false",
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ],
    )?;
    Ok(parse_status(&text))
}

pub fn parse_status(text: &str) -> StatusReport {
    let mut report = StatusReport::default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            let branch = rest.trim();
            if branch == "(detached)" {
                report.detached = true;
                report.summary.branch = None;
            } else {
                report.summary.branch = Some(branch.to_owned());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            report.upstream = Some(rest.trim().to_owned());
            continue;
        }
        if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            if let Some(ahead) = parts.next().and_then(|value| value.strip_prefix('+')) {
                report.summary.ahead = ahead.parse().unwrap_or(0);
            }
            if let Some(behind) = parts.next().and_then(|value| value.strip_prefix('-')) {
                report.summary.behind = behind.parse().unwrap_or(0);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("1 ") {
            if let Some(file) = parse_ordinary(rest) {
                push_file(&mut report, file);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("2 ") {
            if let Some(file) = parse_renamed(rest) {
                push_file(&mut report, file);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("u ") {
            if let Some(file) = parse_unmerged(rest) {
                push_file(&mut report, file);
            }
            continue;
        }
        if let Some(path) = line.strip_prefix("? ") {
            push_file(
                &mut report,
                GitFile {
                    path: path.to_owned(),
                    index: '?',
                    worktree: '?',
                    staged: false,
                    unstaged: true,
                    untracked: true,
                    conflict: false,
                },
            );
        }
    }
    report
}

fn parse_ordinary(rest: &str) -> Option<GitFile> {
    let fields: Vec<&str> = rest.splitn(8, ' ').collect();
    let xy = fields.first()?;
    let path = fields.last().copied().unwrap_or("").to_owned();
    Some(file_from_xy(path, xy, false))
}

fn parse_renamed(rest: &str) -> Option<GitFile> {
    let fields: Vec<&str> = rest.splitn(9, ' ').collect();
    let xy = fields.first()?;
    let path = fields
        .last()
        .copied()
        .unwrap_or("")
        .split('\t')
        .next()
        .unwrap_or("")
        .to_owned();
    Some(file_from_xy(path, xy, false))
}

fn parse_unmerged(rest: &str) -> Option<GitFile> {
    let fields: Vec<&str> = rest.splitn(10, ' ').collect();
    let xy = fields.first()?;
    let path = fields.last().copied().unwrap_or("").to_owned();
    let mut file = file_from_xy(path, xy, true);
    file.conflict = true;
    Some(file)
}

fn file_from_xy(path: String, xy: &str, conflict: bool) -> GitFile {
    let mut chars = xy.chars();
    let index = chars.next().unwrap_or('.');
    let worktree = chars.next().unwrap_or('.');
    GitFile {
        path,
        index,
        worktree,
        staged: index != '.' && index != '?' && !conflict,
        unstaged: worktree != '.' && worktree != '?' && !conflict,
        untracked: false,
        conflict,
    }
}

fn push_file(report: &mut StatusReport, file: GitFile) {
    if file.staged {
        report.summary.staged += 1;
    }
    if file.unstaged && !file.untracked {
        report.summary.unstaged += 1;
    }
    if file.untracked {
        report.summary.untracked += 1;
    }
    if file.conflict {
        report.summary.conflicts += 1;
    }
    report.files.push(file);
}

pub fn stage(path: &Path, files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        return Err("no file selected".to_owned());
    }
    let mut args = vec!["add", "--"];
    args.extend(files.iter().map(String::as_str));
    git(path, &args).map(|_| ())
}

pub fn stage_all(path: &Path) -> Result<(), String> {
    git(path, &["add", "--all"]).map(|_| ())
}

pub fn unstage(path: &Path, files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        return Err("no file selected".to_owned());
    }
    let mut args = vec!["restore", "--staged", "--"];
    args.extend(files.iter().map(String::as_str));
    git(path, &args).map(|_| ())
}

pub fn unstage_all(path: &Path) -> Result<(), String> {
    git(path, &["restore", "--staged", "--", "."]).map(|_| ())
}

pub fn discard(path: &Path, files: &[String]) -> Result<(), String> {
    if files.is_empty() {
        return Err("no file selected".to_owned());
    }
    let report = status(path)?;
    let mut tracked = Vec::new();
    for file in files {
        if let Some(entry) = report.files.iter().find(|entry| &entry.path == file) {
            if entry.untracked || entry.index == 'A' {
                let absolute = path.join(file);
                if absolute.is_file() {
                    std::fs::remove_file(&absolute).map_err(|error| {
                        format!("could not remove {}: {error}", absolute.display())
                    })?;
                }
                continue;
            }
        }
        tracked.push(file.as_str());
    }
    if !tracked.is_empty() {
        let mut args = vec!["restore", "--staged", "--worktree", "--"];
        args.extend(tracked);
        if git(path, &args).is_err() {
            let mut fallback = vec!["checkout", "--"];
            fallback.extend(files.iter().map(String::as_str));
            git(path, &fallback)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct CommitOptions<'a> {
    pub message: &'a str,
    pub all: bool,
    pub sign: bool,
    pub gpg_key: &'a str,
    pub sign_off: bool,
    pub name: &'a str,
    pub email: &'a str,
}

pub fn commit(path: &Path, options: &CommitOptions<'_>) -> Result<String, String> {
    let message = options.message.trim();
    if message.is_empty() {
        return Err("commit message is empty".to_owned());
    }
    if options.all {
        stage_all(path)?;
    }
    let mut args: Vec<String> = Vec::new();
    if !options.name.trim().is_empty() {
        args.push("-c".to_owned());
        args.push(format!("user.name={}", options.name.trim()));
    }
    if !options.email.trim().is_empty() {
        args.push("-c".to_owned());
        args.push(format!("user.email={}", options.email.trim()));
    }
    args.push("commit".to_owned());
    args.push("-m".to_owned());
    args.push(message.to_owned());
    if options.sign_off {
        args.push("--signoff".to_owned());
    }
    if options.sign {
        if options.gpg_key.trim().is_empty() {
            args.push("-S".to_owned());
        } else {
            args.push(format!("--gpg-sign={}", options.gpg_key.trim()));
        }
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git(path, &refs)?;
    Ok(output.lines().next().unwrap_or("").to_owned())
}

pub fn is_protected(branch: &str, protected: &[String]) -> bool {
    protected
        .iter()
        .any(|entry| entry.trim().eq_ignore_ascii_case(branch.trim()))
}

pub fn protected_branch(path: &Path, protected: &[String]) -> Result<Option<String>, String> {
    let report = status(path)?;
    let Some(branch) = report.summary.branch.as_deref() else {
        return Ok(None);
    };
    if is_protected(branch, protected) {
        Ok(Some(branch.to_owned()))
    } else {
        Ok(None)
    }
}

pub fn push(
    path: &Path,
    force_with_lease: bool,
    publish: bool,
    set_upstream: bool,
) -> Result<String, String> {
    let report = status(path)?;
    let Some(branch) = report.summary.branch.as_deref() else {
        return Err("detached HEAD has no branch to push".to_owned());
    };
    let mut args: Vec<String> = vec!["push".to_owned()];
    if force_with_lease {
        args.push("--force-with-lease".to_owned());
    }
    if publish || (report.upstream.is_none() && set_upstream) {
        args.push("-u".to_owned());
        args.push("origin".to_owned());
        args.push(branch.to_owned());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git(path, &refs)
}

pub fn pull(path: &Path) -> Result<String, String> {
    if !has_remote(path) {
        return Err("no-remote".to_owned());
    }
    git(path, &["pull", "--ff-only"])
}

pub fn fetch(path: &Path) -> Result<String, String> {
    git(path, &["fetch", "--prune"])
}

pub fn sync(path: &Path, set_upstream: bool) -> Result<String, String> {
    fetch(path)?;
    pull(path)?;
    push(path, false, false, set_upstream)
}

pub fn publish(path: &Path) -> Result<String, String> {
    if !has_remote(path) {
        return Err("no-remote".to_owned());
    }
    push(path, false, true, true)
}

fn has_remote(path: &Path) -> bool {
    git(path, &["remote"])
        .map(|remotes| !remotes.trim().is_empty())
        .unwrap_or(false)
}

pub fn rebase(path: &Path, base: Option<&str>) -> Result<String, String> {
    let mut args = vec!["rebase"];
    if let Some(base) = base.filter(|value| !value.trim().is_empty()) {
        args.push(base);
    }
    git(path, &args)
}

pub fn abort(path: &Path) -> Result<String, String> {
    if git(path, &["rebase", "--abort"]).is_ok() {
        return Ok("rebase aborted".to_owned());
    }
    if git(path, &["merge", "--abort"]).is_ok() {
        return Ok("merge aborted".to_owned());
    }
    Err("nothing-to-abort".to_owned())
}

pub fn conflicts(path: &Path) -> Result<Vec<String>, String> {
    let text = git(path, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_v2_status_is_counted() {
        let text = "# branch.head forja/x\n# branch.upstream origin/forja/x\n# branch.ab +2 -1\n1 M. N... 100644 100644 100644 aaa bbb src/main.rs\n1 .M N... 100644 100644 100644 aaa bbb README.md\n? novo.txt\nu UU N... 100644 100644 100644 100644 aaa bbb ccc conflito.rs\n";
        let report = parse_status(text);
        assert_eq!(report.summary.branch.as_deref(), Some("forja/x"));
        assert_eq!(report.summary.ahead, 2);
        assert_eq!(report.summary.behind, 1);
        assert_eq!(report.summary.staged, 1);
        assert_eq!(report.summary.unstaged, 1);
        assert_eq!(report.summary.untracked, 1);
        assert_eq!(report.summary.conflicts, 1);
        assert_eq!(report.files.len(), 4);
    }

    #[test]
    fn protected_branches_are_case_insensitive() {
        assert!(is_protected("MAIN", &["main".to_owned()]));
        assert!(!is_protected("forja/x", &["main".to_owned()]));
    }
}
