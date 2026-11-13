//! Replace in files, with a preview and a guarded apply.
//!
//! [`plan`] reads the files from disk and computes the new text, keeping a SHA-256
//! fingerprint of each original. [`apply`] reads every file again and refuses to
//! write anything if one of them changed since the preview. Each write is atomic
//! (temporary file, then rename). If a write fails midway, the files already written
//! are restored from the text read at preview time.
//!
//! Both functions do disk work. The panel calls them only from kernel tasks.

use crate::engine::{self, looks_binary, relative, MAX_FILE_BYTES};
use crate::query::{build_regex, overrides, SearchOptions};
use forja_kernel::store::write_atomic_bytes;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Preview lines kept per file. The full change is still applied.
pub const PREVIEW_LINES_PER_FILE: usize = 40;
/// Above this many files the preview is refused: narrow the search with globs first.
pub const MAX_PLAN_FILES: usize = 500;
/// Above this many bytes of original text the preview is refused.
pub const MAX_PLAN_BYTES: usize = 64 * 1024 * 1024;

/// One changed line in the preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub line: u32,
    pub before: String,
    pub after: String,
}

/// One file the replacement would change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub rel: String,
    pub count: usize,
    pub fingerprint: [u8; 32],
    pub original: String,
    pub updated: String,
    /// Changed lines, capped at [`PREVIEW_LINES_PER_FILE`]. Empty when the change spans lines.
    pub changes: Vec<Change>,
    pub multiline: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReplacePlan {
    pub files: Vec<PlannedFile>,
    pub matches: usize,
}

/// Why an apply did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyError {
    /// These files changed on disk after the preview. Nothing was written.
    Changed(Vec<String>),
    /// A write failed. `written` files were written, `restored` were put back.
    Io {
        message: String,
        written: usize,
        restored: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyReport {
    pub files: usize,
    pub matches: usize,
}

/// SHA-256 of the file's bytes. Used to detect changes between preview and apply.
pub fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Computes the replacement for every matching file under `root`.
pub fn plan(
    root: &Path,
    options: &SearchOptions,
    replacement: &str,
    include_hidden: bool,
    respect_gitignore: bool,
) -> Result<ReplacePlan, String> {
    if options.is_empty() {
        return Ok(ReplacePlan::default());
    }
    let re = build_regex(options)?;
    let filter = overrides(root, &options.include, &options.exclude)?;
    let walker = ignore::WalkBuilder::new(root)
        .hidden(!include_hidden)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .require_git(false)
        .overrides(filter)
        .build();

    let mut out = ReplacePlan::default();
    let mut total_bytes = 0usize;
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        if path.components().any(|part| part.as_os_str() == ".git") {
            continue;
        }
        if entry
            .metadata()
            .is_ok_and(|meta| meta.len() > MAX_FILE_BYTES)
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if looks_binary(&bytes) {
            continue;
        }
        let Ok(original) = String::from_utf8(bytes.clone()) else {
            continue;
        };
        let count = re.find_iter(&original).count();
        if count == 0 {
            continue;
        }
        total_bytes = total_bytes.saturating_add(original.len());
        if out.files.len() >= MAX_PLAN_FILES || total_bytes > MAX_PLAN_BYTES {
            return Err(
                "too many files for one replacement: narrow the search with include globs".into(),
            );
        }
        // Literal replacements must not expand `$1`, so the replacer is wrapped.
        let updated = if options.regex {
            re.replace_all(&original, replacement).into_owned()
        } else {
            re.replace_all(&original, regex::NoExpand(replacement))
                .into_owned()
        };
        let (changes, multiline) = line_changes(&original, &updated);
        out.matches += count;
        out.files.push(PlannedFile {
            path: path.to_path_buf(),
            rel: relative(root, path),
            count,
            fingerprint: fingerprint(&bytes),
            original,
            updated,
            changes,
            multiline,
        });
    }
    Ok(out)
}

/// Pairs changed lines for the preview. Returns `multiline = true` when the line count changed.
fn line_changes(before: &str, after: &str) -> (Vec<Change>, bool) {
    let old: Vec<&str> = before.lines().collect();
    let new: Vec<&str> = after.lines().collect();
    if old.len() != new.len() {
        return (Vec::new(), true);
    }
    let changes = old
        .iter()
        .zip(new.iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .take(PREVIEW_LINES_PER_FILE)
        .map(|(index, (a, b))| Change {
            line: u32::try_from(index + 1).unwrap_or(u32::MAX),
            before: engine::display_line(a),
            after: engine::display_line(b),
        })
        .collect();
    (changes, false)
}

/// Writes the planned files, after checking every one of them against its fingerprint.
pub fn apply(plan: &ReplacePlan) -> Result<ApplyReport, ApplyError> {
    let changed: Vec<String> = plan
        .files
        .iter()
        .filter(|file| !unchanged(file))
        .map(|file| file.rel.clone())
        .collect();
    if !changed.is_empty() {
        return Err(ApplyError::Changed(changed));
    }
    let mut written = 0usize;
    for file in &plan.files {
        if !unchanged(file) {
            let restored = restore(&plan.files[..written]);
            return Err(ApplyError::Io {
                message: format!("{} changed while writing", file.rel),
                written,
                restored,
            });
        }
        if let Err(error) = write_atomic_bytes(&file.path, file.updated.as_bytes()) {
            let restored = restore(&plan.files[..written]);
            return Err(ApplyError::Io {
                message: error.to_string(),
                written,
                restored,
            });
        }
        written += 1;
    }
    Ok(ApplyReport {
        files: written,
        matches: plan.files.iter().map(|file| file.count).sum(),
    })
}

fn unchanged(file: &PlannedFile) -> bool {
    std::fs::read(&file.path).is_ok_and(|bytes| fingerprint(&bytes) == file.fingerprint)
}

/// Puts the preview-time text back into already written files. Returns how many were restored.
fn restore(files: &[PlannedFile]) -> usize {
    files
        .iter()
        .filter(|file| write_atomic_bytes(&file.path, file.original.as_bytes()).is_ok())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-search-replace-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn literal(pattern: &str) -> SearchOptions {
        SearchOptions {
            pattern: pattern.into(),
            ..SearchOptions::default()
        }
    }

    #[test]
    fn preview_then_apply_writes_and_keeps_crlf() {
        let dir = temp_dir("apply");
        let file = dir.join("notes.txt");
        std::fs::write(&file, "one\r\nold value\r\nthree\r\n").unwrap();
        let p = plan(&dir, &literal("old"), "new", true, false).unwrap();
        assert_eq!(p.files.len(), 1);
        assert_eq!(p.files[0].changes.len(), 1);
        assert_eq!(p.files[0].changes[0].line, 2);
        assert!(!p.files[0].multiline);
        let report = apply(&p).unwrap();
        assert_eq!(
            report,
            ApplyReport {
                files: 1,
                matches: 1
            }
        );
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "one\r\nnew value\r\nthree\r\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_refuses_when_a_file_changed_after_the_preview() {
        let dir = temp_dir("stale");
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, "needle a").unwrap();
        std::fs::write(&b, "needle b").unwrap();
        let p = plan(&dir, &literal("needle"), "pin", true, false).unwrap();
        assert_eq!(p.files.len(), 2);
        std::fs::write(&b, "needle changed by someone else").unwrap();
        match apply(&p) {
            Err(ApplyError::Changed(files)) => assert_eq!(files, vec!["b.txt".to_owned()]),
            other => panic!("expected a conflict, got {other:?}"),
        }
        assert_eq!(
            std::fs::read_to_string(&a).unwrap(),
            "needle a",
            "nothing was written"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn literal_replacement_does_not_expand_capture_syntax() {
        let dir = temp_dir("literal");
        let file = dir.join("x.txt");
        std::fs::write(&file, "price x").unwrap();
        let p = plan(&dir, &literal("x"), "$1 (x)", true, false).unwrap();
        apply(&p).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "price $1 (x)");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn regex_replacement_expands_groups() {
        let dir = temp_dir("regex");
        let file = dir.join("x.txt");
        std::fs::write(&file, "fn alpha()\nfn beta()\n").unwrap();
        let options = SearchOptions {
            pattern: r"fn (\w+)".into(),
            regex: true,
            ..SearchOptions::default()
        };
        let p = plan(&dir, &options, "pub fn $1", true, false).unwrap();
        apply(&p).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "pub fn alpha()\npub fn beta()\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn binary_and_unmatched_files_are_left_alone() {
        let dir = temp_dir("skip");
        std::fs::write(dir.join("bin.dat"), b"needle\0").unwrap();
        std::fs::write(dir.join("other.txt"), "nothing here").unwrap();
        let p = plan(&dir, &literal("needle"), "x", true, false).unwrap();
        assert!(p.files.is_empty());
        assert!(apply(&p).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_empty_pattern_plans_nothing() {
        let dir = temp_dir("empty");
        let p = plan(&dir, &literal(""), "x", true, false).unwrap();
        assert!(p.files.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn line_count_changes_are_reported_as_multiline() {
        let (changes, multiline) = line_changes("a\nb\n", "a\n\nb\n");
        assert!(changes.is_empty());
        assert!(multiline);
    }
}
