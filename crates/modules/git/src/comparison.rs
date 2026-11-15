//! Commit and file summary for a branch comparison.

use crate::engine::git;
use serde::Serialize;
use std::path::Path;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ComparisonCommit {
    pub hash: String,
    pub short: String,
    pub title: String,
    pub author: String,
    /// Unix seconds.
    pub time: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ComparisonFile {
    pub path: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ComparisonReport {
    pub branch: String,
    pub head: String,
    pub base: Option<String>,
    pub base_commit: Option<String>,
    pub remote: bool,
    pub branches: Vec<String>,
    pub ahead: u32,
    pub commits: Vec<ComparisonCommit>,
    pub files: Vec<ComparisonFile>,
    pub additions: u32,
    pub deletions: u32,
}

pub fn compare(path: &Path, requested_base: Option<&str>) -> Result<ComparisonReport, String> {
    let head = git(path, &["rev-parse", "--verify", "HEAD^{commit}"])?
        .trim()
        .to_owned();
    let branch = git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|_| "HEAD".to_owned());
    let remote = git(path, &["remote", "get-url", "origin"]).is_ok();
    let branches = list_branches(path)?;
    let base = match requested_base.filter(|base| !base.trim().is_empty()) {
        Some(base) => Some(base.trim().to_owned()),
        None => default_base(path, &branch),
    };
    let Some(base) = base else {
        return Ok(ComparisonReport {
            branch,
            head,
            base: None,
            base_commit: None,
            remote,
            branches,
            ahead: 0,
            commits: Vec::new(),
            files: Vec::new(),
            additions: 0,
            deletions: 0,
        });
    };
    let base_commit = resolve_commit(path, &base)?;
    let ahead = git(
        path,
        &["rev-list", "--count", &format!("{base_commit}..HEAD")],
    )?
    .trim()
    .parse::<u32>()
    .map_err(|error| format!("could not read commits ahead of base: {error}"))?;
    let commits = list_commits(path, &base_commit)?;
    let files = list_files(path, &base_commit)?;
    let additions = files.iter().filter_map(|file| file.additions).sum();
    let deletions = files.iter().filter_map(|file| file.deletions).sum();
    Ok(ComparisonReport {
        branch,
        head,
        base: Some(base),
        base_commit: Some(base_commit),
        remote,
        branches,
        ahead,
        commits,
        files,
        additions,
        deletions,
    })
}

pub fn is_current(path: &Path, requested_base: Option<&str>, previous: &ComparisonReport) -> bool {
    let Ok(head) = git(path, &["rev-parse", "--verify", "HEAD^{commit}"]) else {
        return false;
    };
    let branch = git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|value| value.trim().to_owned())
        .unwrap_or_else(|_| "HEAD".to_owned());
    let remote = git(path, &["remote", "get-url", "origin"]).is_ok();
    let base = match requested_base.filter(|base| !base.trim().is_empty()) {
        Some(base) => Some(base.trim().to_owned()),
        None => default_base(path, &branch),
    };
    let base_commit = base
        .as_deref()
        .and_then(|base| resolve_commit(path, base).ok());
    head.trim() == previous.head
        && branch == previous.branch
        && remote == previous.remote
        && list_branches(path).is_ok_and(|branches| branches == previous.branches)
        && base == previous.base
        && base_commit == previous.base_commit
}

pub fn diff_file(
    path: &Path,
    file: &str,
    comparison: &str,
) -> Result<crate::diff::FileDiff, String> {
    let (base, head) = comparison
        .split_once("...")
        .ok_or_else(|| "comparison must use the base...HEAD form".to_owned())?;
    if head != "HEAD" || base.trim().is_empty() {
        return Err("comparison must use the base...HEAD form".to_owned());
    }
    let base_commit = resolve_commit(path, base.trim())?;
    crate::diff::diff_comparison(path, file, &base_commit)
}

fn resolve_commit(path: &Path, base: &str) -> Result<String, String> {
    let expression = format!("{base}^{{commit}}");
    git(
        path,
        &["rev-parse", "--verify", "--end-of-options", &expression],
    )
    .map(|value| value.trim().to_owned())
}

fn default_base(path: &Path, branch: &str) -> Option<String> {
    if let Ok(value) = git(
        path,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    ) {
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_owned());
        }
    }
    if resolve_commit(path, "origin/main").is_ok() {
        return Some("origin/main".to_owned());
    }
    if branch != "main" && resolve_commit(path, "main").is_ok() {
        return Some("main".to_owned());
    }
    None
}

fn list_branches(path: &Path) -> Result<Vec<String>, String> {
    let value = git(
        path,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    Ok(value
        .lines()
        .map(str::trim)
        .filter(|branch| !branch.is_empty() && *branch != "origin/HEAD")
        .map(str::to_owned)
        .collect())
}

fn list_commits(path: &Path, base_commit: &str) -> Result<Vec<ComparisonCommit>, String> {
    let range = format!("{base_commit}..HEAD");
    let value = git(
        path,
        &[
            "log",
            "--reverse",
            "--format=%H%x1f%h%x1f%an%x1f%at%x1f%s",
            &range,
        ],
    )?;
    value
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let fields: Vec<&str> = line.split('\u{1f}').collect();
            if fields.len() != 5 {
                return Err("could not parse a commit in the comparison".to_owned());
            }
            Ok(ComparisonCommit {
                hash: fields[0].to_owned(),
                short: fields[1].to_owned(),
                author: fields[2].to_owned(),
                time: fields[3]
                    .parse()
                    .map_err(|error| format!("could not parse commit date: {error}"))?,
                title: fields[4].to_owned(),
            })
        })
        .collect()
}

fn list_files(path: &Path, base_commit: &str) -> Result<Vec<ComparisonFile>, String> {
    let range = format!("{base_commit}...HEAD");
    let value = git(path, &["diff", "--numstat", "-z", &range, "--"])?;
    let mut records = value.split('\0');
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, '\t');
        let additions = fields.next().unwrap_or_default();
        let deletions = fields.next().unwrap_or_default();
        let mut file_path = fields.next().unwrap_or_default();
        if file_path.is_empty() {
            let _old_path = records.next();
            file_path = records.next().unwrap_or_default();
        }
        if file_path.is_empty() {
            continue;
        }
        files.push(ComparisonFile {
            path: file_path.to_owned(),
            additions: additions.parse().ok(),
            deletions: deletions.parse().ok(),
        });
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRepo(std::path::PathBuf);

    impl TestRepo {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "forja-git-compare-{}-{}-{}",
                std::process::id(),
                {
                    // Parallel tests can read the same clock value.
                    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                },
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn run(&self, args: &[&str]) {
            let output = Command::new("git")
                .args(args)
                .current_dir(&self.0)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn commit(&self, message: &str) {
            self.run(&["add", "-A"]);
            self.run(&["commit", "-m", message]);
        }
    }

    impl Drop for TestRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn initialized() -> TestRepo {
        let repo = TestRepo::new();
        repo.run(&["init", "-q", "-b", "main"]);
        repo.run(&["config", "user.name", "Forja Test"]);
        repo.run(&["config", "user.email", "forja-test@example.invalid"]);
        std::fs::write(repo.0.join("file.txt"), "base\n").unwrap();
        repo.commit("Base commit");
        repo
    }

    #[test]
    fn compares_commits_and_file_totals_ahead_of_base() {
        let repo = initialized();
        repo.run(&["switch", "-q", "-c", "feature/report"]);
        std::fs::write(repo.0.join("file.txt"), "base\nadded\n").unwrap();
        repo.commit("Add a line");

        let report = compare(&repo.0, Some("main")).unwrap();
        assert_eq!(report.branch, "feature/report");
        assert_eq!(report.base.as_deref(), Some("main"));
        assert_eq!(report.ahead, 1);
        assert_eq!(report.commits[0].title, "Add a line");
        assert_eq!(report.commits[0].author, "Forja Test");
        assert_eq!(report.files.len(), 1);
        assert_eq!(report.files[0].path, "file.txt");
        assert_eq!(report.additions, 1);
        assert_eq!(report.deletions, 0);
        assert!(!report.remote);
        let diff = diff_file(&repo.0, "file.txt", "main...HEAD").unwrap();
        assert!(
            diff.hunks
                .iter()
                .flat_map(|hunk| &hunk.lines)
                .any(|line| { line.kind == "add" && line.text.contains("added") })
        );
    }

    #[test]
    fn comparison_without_a_base_offers_local_branches() {
        let repo = initialized();
        let report = compare(&repo.0, None).unwrap();
        assert_eq!(report.base, None);
        assert!(report.branches.contains(&"main".to_owned()));
        assert_eq!(report.ahead, 0);
    }

    #[test]
    fn comparison_at_base_has_nothing_to_send() {
        let repo = initialized();
        let report = compare(&repo.0, Some("main")).unwrap();
        assert_eq!(report.ahead, 0);
        assert!(report.commits.is_empty());
        assert!(report.files.is_empty());
    }
}
