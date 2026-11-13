//! The search walk: finds matching lines under a root and streams them to the panel.
//!
//! The walk runs on its own thread (it is long-running and produces many small
//! batches). The panel receives the batches on the interface thread through
//! kernel tasks. Each file is read whole, skipped when it is binary, too large,
//! or not UTF-8. A cancel flag stops the walk between files.

use crate::query::{build_regex, overrides, SearchOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Files larger than this are not searched.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
/// Matching lines reported before the search stops and says it was truncated.
pub const MAX_MATCHES: usize = 2_000;
/// A line longer than this is shown cut.
pub const MAX_LINE_CHARS: usize = 300;

/// Files per batch sent to the panel (or this much time since the last batch).
const BATCH_FILES: usize = 25;
const BATCH_TIME: Duration = Duration::from_millis(80);

/// One line of output: a matching line or a context line around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextLine {
    pub number: u32,
    pub text: String,
    pub is_match: bool,
}

/// One matching line in a file, with its context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub line: u32,
    pub lines: Vec<ContextLine>,
}

/// All hits in one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHits {
    pub rel: String,
    pub path: PathBuf,
    pub hits: Vec<Hit>,
}

/// What the walk sends to the panel.
#[derive(Debug)]
pub enum Msg {
    Files(Vec<FileHits>),
    Finished {
        scanned: usize,
        matches: usize,
        truncated: bool,
        error: Option<String>,
    },
}

/// Which engine lists the candidate files. `Auto` uses ripgrep when it is on the PATH.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Auto,
    Builtin,
}

/// Everything the walk needs, copied so the thread owns it.
#[derive(Clone, Debug)]
pub struct Plan {
    pub root: PathBuf,
    pub options: SearchOptions,
    pub context: usize,
    pub include_hidden: bool,
    pub respect_gitignore: bool,
    /// `find.grep.engine`, resolved on the search thread.
    pub engine: Engine,
}

/// Turns a line into displayable text: trailing spaces removed, length capped.
pub fn display_line(line: &str) -> String {
    let trimmed = line.trim_end();
    if trimmed.chars().count() > MAX_LINE_CHARS {
        let mut cut: String = trimmed.chars().take(MAX_LINE_CHARS).collect();
        cut.push('…');
        cut
    } else {
        trimmed.to_owned()
    }
}

/// True when the file's first bytes contain a NUL, which marks binary data.
pub fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8 * 1024).any(|byte| *byte == 0)
}

/// Matches one text against `re` and builds the hits with `context` lines around each.
pub fn hits_in_text(re: &regex::Regex, text: &str, context: usize) -> (Vec<Hit>, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let matching: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| re.is_match(line))
        .map(|(index, _)| index)
        .collect();
    let mut hits = Vec::with_capacity(matching.len());
    for &index in &matching {
        let from = index.saturating_sub(context);
        let to = (index + context + 1).min(lines.len());
        let lines_out = (from..to)
            .map(|number| ContextLine {
                number: u32::try_from(number + 1).unwrap_or(u32::MAX),
                text: display_line(lines[number]),
                is_match: matching.binary_search(&number).is_ok(),
            })
            .collect();
        hits.push(Hit {
            line: u32::try_from(index + 1).unwrap_or(u32::MAX),
            lines: lines_out,
        });
    }
    (hits, matching.len())
}

/// Walks the root and sends batches over `sender` until done, cancelled, or truncated.
/// The candidate files come from ripgrep when the engine is `Auto` and `rg` is found;
/// otherwise from the built-in walker. Both feed the same match code.
pub fn run(plan: &Plan, sender: &Sender<Msg>, cancel: &Arc<AtomicBool>) {
    let finish = |scanned, matches, truncated, error: Option<String>| {
        let _ = sender.send(Msg::Finished {
            scanned,
            matches,
            truncated,
            error,
        });
    };
    if plan.options.is_empty() {
        finish(0, 0, false, None);
        return;
    }
    let re = match build_regex(&plan.options) {
        Ok(re) => re,
        Err(error) => return finish(0, 0, false, Some(error)),
    };
    let candidates: Box<dyn Iterator<Item = PathBuf>> = if plan.engine == Engine::Auto
        && crate::rg::available()
    {
        match crate::rg::candidate_files(plan) {
            Ok(files) => Box::new(files.into_iter()),
            Err(error) => return finish(0, 0, false, Some(error)),
        }
    } else {
        let filter = match overrides(&plan.root, &plan.options.include, &plan.options.exclude) {
            Ok(filter) => filter,
            Err(error) => return finish(0, 0, false, Some(error)),
        };
        let walker = ignore::WalkBuilder::new(&plan.root)
            .hidden(!plan.include_hidden)
            .git_ignore(plan.respect_gitignore)
            .git_global(plan.respect_gitignore)
            .git_exclude(plan.respect_gitignore)
            .require_git(false)
            .overrides(filter)
            .build();
        Box::new(
            walker
                .flatten()
                .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
                .map(|entry| entry.into_path()),
        )
    };

    let mut scanned = 0usize;
    let mut matches = 0usize;
    let mut truncated = false;
    let mut batch: Vec<FileHits> = Vec::new();
    let mut last_send = Instant::now();
    for path in candidates {
        if cancel.load(Ordering::Relaxed) {
            return finish(scanned, matches, true, None);
        }
        if path.components().any(|part| part.as_os_str() == ".git") {
            continue;
        }
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        if looks_binary(&bytes) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        scanned += 1;
        let (hits, count) = hits_in_text(&re, &text, plan.context);
        if !hits.is_empty() {
            let rel = relative(&plan.root, &path);
            batch.push(FileHits {
                rel,
                path: path.clone(),
                hits,
            });
        }
        matches += count;
        if matches >= MAX_MATCHES {
            truncated = true;
        }
        if batch.len() >= BATCH_FILES || (!batch.is_empty() && last_send.elapsed() >= BATCH_TIME) {
            if sender.send(Msg::Files(std::mem::take(&mut batch))).is_err() {
                return;
            }
            last_send = Instant::now();
        }
        if truncated {
            break;
        }
    }
    if !batch.is_empty() {
        let _ = sender.send(Msg::Files(batch));
    }
    finish(scanned, matches, truncated, None);
}

/// `path` relative to `root`, with `/` separators.
pub fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn temp_repo(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forja-search-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(
            root.join("src").join("main.rs"),
            "fn main() {\n    let needle = 1;\n}\nlet other = 2;\n",
        )
        .unwrap();
        std::fs::write(root.join("target").join("out.rs"), "needle in build\n").unwrap();
        std::fs::write(root.join("bin.dat"), b"needle\0binary").unwrap();
        root
    }

    fn plan(root: &Path, pattern: &str) -> Plan {
        Plan {
            root: root.to_path_buf(),
            options: SearchOptions {
                pattern: pattern.into(),
                ..SearchOptions::default()
            },
            context: 1,
            include_hidden: false,
            respect_gitignore: true,
            engine: Engine::Builtin,
        }
    }

    fn collect(plan: &Plan) -> (Vec<FileHits>, usize, bool, Option<String>) {
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        run(plan, &sender, &cancel);
        drop(sender);
        let mut files = Vec::new();
        let mut result = (0, false, None);
        for message in receiver {
            match message {
                Msg::Files(batch) => files.extend(batch),
                Msg::Finished {
                    matches,
                    truncated,
                    error,
                    ..
                } => result = (matches, truncated, error),
            }
        }
        (files, result.0, result.1, result.2)
    }

    #[test]
    fn finds_lines_grouped_by_file_and_honors_gitignore_and_binary() {
        let root = temp_repo("walk");
        let (files, matches, truncated, error) = collect(&plan(&root, "needle"));
        assert!(error.is_none());
        assert!(!truncated);
        assert_eq!(files.len(), 1, "target/ is ignored and bin.dat is binary");
        assert_eq!(files[0].rel, "src/main.rs");
        assert_eq!(matches, 1);
        let hit = &files[0].hits[0];
        assert_eq!(hit.line, 2);
        assert!(hit.lines.iter().any(|l| l.number == 1 && !l.is_match));
        assert!(hit.lines.iter().any(|l| l.number == 2 && l.is_match));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_regex_reports_an_error_instead_of_searching() {
        let root = temp_repo("invalid");
        let mut p = plan(&root, "(open");
        p.options.regex = true;
        let (files, _, _, error) = collect(&p);
        assert!(files.is_empty());
        assert!(error.is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cancel_stops_the_walk() {
        let root = temp_repo("cancel");
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(true));
        run(&plan(&root, "needle"), &sender, &cancel);
        drop(sender);
        let last = receiver.iter().last();
        assert!(matches!(
            last,
            Some(Msg::Finished {
                truncated: true,
                ..
            })
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn context_lines_are_clipped_at_file_edges() {
        let re = build_regex(&SearchOptions {
            pattern: "x".into(),
            ..SearchOptions::default()
        })
        .unwrap();
        let (hits, count) = hits_in_text(&re, "x\nno\nno\nx", 2);
        assert_eq!(count, 2);
        assert_eq!(hits[0].lines.len(), 3, "lines 1..3 only");
        assert_eq!(hits[1].lines.first().map(|l| l.number), Some(2));
    }

    #[test]
    fn long_lines_are_cut_for_display() {
        let long = "a".repeat(MAX_LINE_CHARS + 50);
        let shown = display_line(&long);
        assert_eq!(shown.chars().count(), MAX_LINE_CHARS + 1);
        assert_eq!(display_line("ok   "), "ok");
    }
}

#[cfg(test)]
mod engine_choice_tests {
    use super::*;
    use std::sync::mpsc::channel;

    fn files(plan: &Plan) -> Vec<String> {
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        run(plan, &sender, &cancel);
        drop(sender);
        let mut names = Vec::new();
        for message in receiver {
            if let Msg::Files(batch) = message {
                names.extend(batch.into_iter().map(|file| file.rel));
            }
        }
        names.sort();
        names
    }

    #[test]
    fn ripgrep_and_the_builtin_walker_find_the_same_files() {
        let root = std::env::temp_dir().join(format!("forja-engines-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(root.join("src/lib.rs"), "let needle = 1;\n").unwrap();
        std::fs::write(root.join("target/out.rs"), "needle\n").unwrap();
        let options = SearchOptions {
            pattern: "needle".into(),
            ..SearchOptions::default()
        };
        let base = Plan {
            root: root.clone(),
            options,
            context: 0,
            include_hidden: false,
            respect_gitignore: true,
            engine: Engine::Builtin,
        };
        let builtin = files(&base);
        let auto = files(&Plan {
            engine: Engine::Auto,
            ..base.clone()
        });
        assert_eq!(builtin, vec!["src/lib.rs".to_owned()]);
        assert_eq!(auto, builtin);
        let _ = std::fs::remove_dir_all(&root);
    }
}
