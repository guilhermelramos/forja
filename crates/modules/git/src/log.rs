//! History graph and blame parsing.

use serde::Serialize;
use std::path::Path;

use crate::engine::git;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LogCommit {
    pub hash: String,
    pub short: String,
    pub parents: Vec<String>,
    pub author: String,
    pub email: String,
    /// Unix seconds.
    pub time: i64,
    pub subject: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LogLine {
    /// ASCII graph prefix produced by `git log --graph`.
    pub graph: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<LogCommit>,
}

pub fn log(path: &Path, limit: u32) -> Result<Vec<LogLine>, String> {
    let limit = limit.clamp(1, 2000).to_string();
    let text = git(
        path,
        &[
            "log",
            "--graph",
            "--date-order",
            &format!("--max-count={limit}"),
            "--pretty=format:%H%x1f%h%x1f%P%x1f%an%x1f%ae%x1f%at%x1f%s",
        ],
    )?;
    Ok(parse_log(&text))
}

/// Commit metadata and patch for the details pane, cut to a readable size.
pub fn show(path: &Path, hash: &str) -> Result<String, String> {
    if hash.is_empty() || !hash.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err("invalid commit hash".to_owned());
    }
    let text = git(
        path,
        &[
            "show",
            "--no-color",
            "--stat",
            "--patch",
            "--format=fuller",
            hash,
        ],
    )?;
    Ok(text.chars().take(40_000).collect())
}

pub fn parse_log(text: &str) -> Vec<LogLine> {
    let mut lines = Vec::new();
    for line in text.lines() {
        let Some(separator) = line.find('\u{1f}') else {
            if !line.trim().is_empty() {
                lines.push(LogLine {
                    graph: line.to_owned(),
                    commit: None,
                });
            }
            continue;
        };
        let prefix = &line[..separator];
        let Some(hash_start) = last_hash_start(prefix) else {
            lines.push(LogLine {
                graph: line.to_owned(),
                commit: None,
            });
            continue;
        };
        let graph = line[..hash_start].to_owned();
        let fields: Vec<&str> = line[hash_start..].split('\u{1f}').collect();
        if fields.len() < 7 {
            lines.push(LogLine {
                graph: line.to_owned(),
                commit: None,
            });
            continue;
        }
        let parents = fields[2].split_whitespace().map(str::to_owned).collect();
        lines.push(LogLine {
            graph,
            commit: Some(LogCommit {
                hash: fields[0].to_owned(),
                short: fields[1].to_owned(),
                parents,
                author: fields[3].to_owned(),
                email: fields[4].to_owned(),
                time: fields[5].parse().unwrap_or(0),
                subject: fields[6].to_owned(),
            }),
        });
    }
    lines
}

fn last_hash_start(prefix: &str) -> Option<usize> {
    let bytes = prefix.as_bytes();
    let mut found = None;
    let mut index = 0;
    while index + 40 <= bytes.len() {
        if bytes[index..index + 40]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
        {
            found = Some(index);
            index += 40;
        } else {
            index += 1;
        }
    }
    found
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct BlameLine {
    pub line: u32,
    pub hash: String,
    pub short: String,
    pub author: String,
    /// Unix seconds.
    pub time: i64,
    pub content: String,
}

pub fn blame(path: &Path, file: &str) -> Result<Vec<BlameLine>, String> {
    let text = git(path, &["blame", "--line-porcelain", "--", file])?;
    Ok(parse_blame(&text, 4000))
}

pub fn parse_blame(text: &str, limit: usize) -> Vec<BlameLine> {
    let mut lines = Vec::new();
    let mut hash = String::new();
    let mut line_number = 0u32;
    let mut author = String::new();
    let mut time = 0i64;
    for line in text.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            if lines.len() >= limit {
                break;
            }
            lines.push(BlameLine {
                line: line_number,
                short: if hash.chars().all(|character| character == '0') {
                    "local".to_owned()
                } else {
                    hash.chars().take(8).collect()
                },
                hash: hash.clone(),
                author: author.clone(),
                time,
                content: content.to_owned(),
            });
            continue;
        }
        if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_owned();
            continue;
        }
        if let Some(rest) = line.strip_prefix("author-time ") {
            time = rest.parse().unwrap_or(0);
            continue;
        }
        if line.len() >= 40 && line[..40].chars().all(|c| c.is_ascii_hexdigit()) {
            let mut parts = line[41..].split_whitespace();
            let _original = parts.next();
            line_number = parts
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            hash = line[..40].to_owned();
            author.clear();
            time = 0;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_separate_graph_from_commit_fields() {
        let text = "* abcdef0123456789abcdef0123456789abcdef01\u{1f}abcdef0\u{1f}parent\u{1f}Ana\u{1f}a@b\u{1f}1700000000\u{1f}assunto\n|\\\n| * 1234567890abcdef1234567890abcdef12345678\u{1f}1234567\u{1f}\u{1f}Bo\u{1f}b@c\u{1f}1700000001\u{1f}outro\n";
        let lines = parse_log(text);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].graph, "* ");
        assert_eq!(lines[0].commit.as_ref().unwrap().short, "abcdef0");
        assert_eq!(lines[1].graph, "|\\");
        assert!(lines[1].commit.is_none());
        assert_eq!(lines[2].commit.as_ref().unwrap().author, "Bo");
    }

    #[test]
    fn blame_blocks_become_lines() {
        let text = "abcdef0123456789abcdef0123456789abcdef01 1 1 1\nauthor Ana\nauthor-mail <a@b>\nauthor-time 1700000000\nsummary x\nfilename a.txt\n\tlinha um\nabcdef0123456789abcdef0123456789abcdef01 2 2\nauthor Ana\nauthor-time 1700000000\n\tlinha dois\n";
        let lines = parse_blame(text, 10);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line, 1);
        assert_eq!(lines[1].content, "linha dois");
        assert_eq!(lines[0].author, "Ana");
    }
}
