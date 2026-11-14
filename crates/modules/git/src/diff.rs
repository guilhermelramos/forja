//! Unified diff parsing into JSON hunks for the `diff` module of wave C2.

use serde::Serialize;
use std::path::Path;

use crate::engine::git;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct DiffLine {
    /// `context`, `add` or `del`.
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub staged: bool,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
}

pub fn diff(path: &Path, file: &str, staged: bool) -> Result<FileDiff, String> {
    let mut args = vec![
        "-c",
        "core.quotepath=false",
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--unified=3",
    ];
    if staged {
        args.push("--cached");
    }
    args.push("--");
    args.push(file);
    let text = git(path, &args)?;
    if text.trim().is_empty() {
        // An untracked file has no diff; expose its content as one added hunk.
        let absolute = path.join(file);
        if absolute.is_file() {
            let bytes = std::fs::read(&absolute)
                .map_err(|error| format!("could not read {}: {error}", absolute.display()))?;
            if bytes.contains(&0) {
                return Ok(FileDiff {
                    path: file.to_owned(),
                    staged,
                    binary: true,
                    hunks: Vec::new(),
                });
            }
            let text = String::from_utf8_lossy(&bytes);
            let lines: Vec<DiffLine> = text
                .lines()
                .map(|line| DiffLine {
                    kind: "add".to_owned(),
                    text: format!("+{line}"),
                })
                .collect();
            return Ok(FileDiff {
                path: file.to_owned(),
                staged,
                binary: false,
                hunks: vec![Hunk {
                    old_start: 0,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: lines.len() as u32,
                    header: "novo arquivo".to_owned(),
                    lines,
                }],
            });
        }
        return Ok(FileDiff {
            path: file.to_owned(),
            staged,
            binary: false,
            hunks: Vec::new(),
        });
    }
    if text.contains("Binary files") || text.contains("GIT binary patch") {
        return Ok(FileDiff {
            path: file.to_owned(),
            staged,
            binary: true,
            hunks: Vec::new(),
        });
    }
    Ok(FileDiff {
        path: file.to_owned(),
        staged,
        binary: false,
        hunks: parse_hunks(&text),
    })
}

/// Builds the same unified diff from a selected base through `HEAD`.
pub fn diff_comparison(path: &Path, file: &str, base_commit: &str) -> Result<FileDiff, String> {
    let range = format!("{base_commit}...HEAD");
    let text = git(
        path,
        &[
            "-c",
            "core.quotepath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--unified=3",
            &range,
            "--",
            file,
        ],
    )?;
    Ok(FileDiff {
        path: file.to_owned(),
        staged: false,
        binary: text.contains("Binary files") || text.contains("GIT binary patch"),
        hunks: parse_hunks(&text),
    })
}

pub fn parse_hunks(text: &str) -> Vec<Hunk> {
    let mut hunks = Vec::new();
    let mut current: Option<Hunk> = None;
    for line in text.lines() {
        if line.starts_with("@@") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            current = parse_header(line);
            continue;
        }
        let Some(hunk) = current.as_mut() else {
            continue;
        };
        if line.starts_with("diff --git") || line.starts_with("index ") {
            hunks.push(current.take().expect("hunk exists"));
            continue;
        }
        if line.starts_with('\\') {
            continue;
        }
        let kind = match line.chars().next() {
            Some('+') => "add",
            Some('-') => "del",
            Some(' ') => "context",
            _ => continue,
        };
        hunk.lines.push(DiffLine {
            kind: kind.to_owned(),
            text: line.to_owned(),
        });
    }
    if let Some(hunk) = current {
        hunks.push(hunk);
    }
    hunks
}

fn parse_header(line: &str) -> Option<Hunk> {
    let body = line.strip_prefix("@@")?;
    let end = body.find("@@")?;
    let ranges = &body[..end];
    let header = body[end + 2..].trim().to_owned();
    let mut parts = ranges.split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    let (old_start, old_lines) = parse_range(old);
    let (new_start, new_lines) = parse_range(new);
    Some(Hunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        header,
        lines: Vec::new(),
    })
}

fn parse_range(value: &str) -> (u32, u32) {
    match value.split_once(',') {
        Some((start, lines)) => (start.parse().unwrap_or(0), lines.parse().unwrap_or(0)),
        None => (value.parse().unwrap_or(0), 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_keep_context_added_and_removed_lines() {
        let text = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,3 @@ fn main\n a\n-b\n+c\n+d\n";
        let hunks = parse_hunks(text);
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 1);
        assert_eq!(hunks[0].old_lines, 2);
        assert_eq!(hunks[0].new_lines, 3);
        assert_eq!(hunks[0].header, "fn main");
        assert_eq!(hunks[0].lines.len(), 4);
        assert_eq!(hunks[0].lines[1].kind, "del");
        assert_eq!(hunks[0].lines[2].kind, "add");
    }

    #[test]
    fn ranges_without_count_default_to_one() {
        assert_eq!(parse_range("7"), (7, 1));
        assert_eq!(parse_range("7,0"), (7, 0));
    }
}
