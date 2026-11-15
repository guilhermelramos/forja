//! File index of a bench for the finder.
//!
//! The list comes from `git ls-files` (tracked plus untracked files that git does
//! not ignore). Outside a repository it falls back to walking the folder with the
//! `ignore` crate. Either way the list is capped, and building it runs in a
//! kernel task, never on the interface thread.

use crate::fuzzy::Candidate;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Files kept per index. Beyond this the finder says the list is partial.
pub const FILE_CAP: usize = 50_000;

/// How the index was built, for the status line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Git,
    Walk,
}

/// What the index respects. Mirrors the `find.search.*` settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub respect_gitignore: bool,
    pub include_hidden: bool,
    pub cap: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            include_hidden: false,
            cap: FILE_CAP,
        }
    }
}

#[derive(Debug)]
pub struct Index {
    pub root: PathBuf,
    pub files: Vec<Candidate>,
    pub truncated: bool,
    pub source: Source,
}

/// Builds the index of `root`. Blocking: call it from a task.
pub fn build(root: &Path, options: Options) -> Index {
    let (paths, source) = match git_files(root, options) {
        Some(paths) => (paths, Source::Git),
        None => (walk_files(root, options), Source::Walk),
    };
    let mut truncated = false;
    let mut files = Vec::new();
    for rel in paths {
        if !visible(&rel, options) {
            continue;
        }
        if files.len() >= options.cap {
            truncated = true;
            break;
        }
        files.push(Candidate::new(rel));
    }
    Index {
        root: root.to_path_buf(),
        files,
        truncated,
        source,
    }
}

/// Hides `.git` always, and dot-entries unless hidden files are included.
fn visible(rel: &str, options: Options) -> bool {
    rel.split('/').all(|part| {
        if part == ".git" {
            return false;
        }
        options.include_hidden || !part.starts_with('.')
    })
}

/// Lists files with `git ls-files`. `None` when `root` is not a repository or git is missing.
fn git_files(root: &Path, options: Options) -> Option<Vec<String>> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--cached", "--others"]);
    if options.respect_gitignore {
        command.arg("--exclude-standard");
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: never flash a console window.
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
            .map(|part| String::from_utf8_lossy(part).into_owned())
            .collect(),
    )
}

/// Walks the folder with the `ignore` crate when git cannot answer.
fn walk_files(root: &Path, options: Options) -> Vec<String> {
    let walker = ignore::WalkBuilder::new(root)
        .hidden(!options.include_hidden)
        .git_ignore(options.respect_gitignore)
        .git_global(options.respect_gitignore)
        .git_exclude(options.respect_gitignore)
        .require_git(false)
        .build();
    let mut out = Vec::new();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        out.push(rel.to_string_lossy().replace('\\', "/"));
        if out.len() > options.cap.saturating_mul(2) {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-find-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(index: &Index) -> Vec<String> {
        let mut out: Vec<String> = index.files.iter().map(|f| f.rel.clone()).collect();
        out.sort();
        out
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn walk_respects_gitignore_and_hidden_files_in_a_plain_folder() {
        let root = temp_dir("walk");
        write(&root.join(".gitignore"), "target/\n*.log\n");
        write(&root.join("src").join("main.rs"), "");
        write(&root.join("target").join("debug.bin"), "");
        write(&root.join("run.log"), "");
        write(&root.join(".env"), "");
        write(&root.join(".github").join("ci.yml"), "");

        let index = build(&root, Options::default());
        assert_eq!(index.source, Source::Walk, "no git repository here");
        assert_eq!(
            names(&index),
            vec!["src/main.rs"],
            "dotfiles are hidden by default"
        );

        let with_hidden = build(
            &root,
            Options {
                respect_gitignore: false,
                include_hidden: true,
                cap: FILE_CAP,
            },
        );
        let all = names(&with_hidden);
        assert!(all.contains(&"target/debug.bin".to_owned()));
        assert!(all.contains(&".github/ci.yml".to_owned()));
        assert!(all.contains(&"run.log".to_owned()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_cap_marks_the_index_as_truncated() {
        let root = temp_dir("cap");
        for n in 0..5 {
            write(&root.join(format!("f{n}.txt")), "");
        }
        let index = build(
            &root,
            Options {
                cap: 3,
                ..Options::default()
            },
        );
        assert_eq!(index.files.len(), 3);
        assert!(index.truncated);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn git_repository_lists_tracked_and_untracked_but_not_ignored() {
        let root = temp_dir("git");
        let init = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["init", "-q"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .status();
        if !init.is_ok_and(|status| status.success()) {
            // Git is not installed here; the walk tests cover the fallback.
            let _ = std::fs::remove_dir_all(root);
            return;
        }
        for args in [
            ["config", "user.name", "Forja Test"],
            ["config", "user.email", "forja@example.invalid"],
        ] {
            let _ = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .stdin(Stdio::null())
                .status();
        }
        write(&root.join(".gitignore"), "secret.env\n");
        write(&root.join("tracked.rs"), "");
        write(&root.join("new.rs"), "");
        write(&root.join("secret.env"), "");
        let _ = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["add", "tracked.rs", ".gitignore"])
            .stdin(Stdio::null())
            .status();

        let index = build(&root, Options::default());
        assert_eq!(index.source, Source::Git);
        assert_eq!(names(&index), vec!["new.rs", "tracked.rs"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
