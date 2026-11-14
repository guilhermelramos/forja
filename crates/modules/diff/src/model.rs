//! Diff data and the pure rules for rows, whitespace filtering and review notes.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GitFile {
    pub path: String,
    #[serde(default)]
    pub index: char,
    #[serde(default)]
    pub worktree: char,
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub unstaged: bool,
    #[serde(default)]
    pub untracked: bool,
    #[serde(default)]
    pub conflict: bool,
    #[serde(default)]
    pub comparison: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Changes {
    #[serde(default)]
    pub files: Vec<GitFile>,
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default)]
    pub detached: bool,
    #[serde(default)]
    pub comparison: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DiffLine {
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Hunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FileDiff {
    pub path: String,
    #[serde(default)]
    pub staged: bool,
    #[serde(default)]
    pub binary: bool,
    #[serde(default)]
    pub hunks: Vec<Hunk>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub number: u32,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Header,
    Context,
    Change,
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: RowKind,
    pub old: Option<Cell>,
    pub new: Option<Cell>,
    pub header: String,
    pub skipped: u32,
}

impl Row {
    pub fn note_line(&self) -> Option<u32> {
        self.new
            .as_ref()
            .map(|cell| cell.number)
            .or_else(|| self.old.as_ref().map(|cell| cell.number))
    }
}

/// Builds the display rows for a unified diff. Changed blocks are paired
/// deletion/new so the side-by-side view can align them; with `collapse`,
/// leading and trailing context is trimmed to `context` lines and skipped
/// stretches between hunks become explicit gap rows.
pub fn rows(diff: &FileDiff, context: usize, ignore_whitespace: bool, collapse: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut previous_end: Option<u32> = None;
    for hunk in &diff.hunks {
        if hunk.lines.is_empty() {
            continue;
        }
        if ignore_whitespace && only_whitespace(hunk) {
            continue;
        }
        if collapse {
            if let Some(end) = previous_end {
                let gap = hunk.old_start.saturating_sub(end + 1);
                if gap > 0 {
                    rows.push(Row {
                        kind: RowKind::Gap,
                        old: None,
                        new: None,
                        header: String::new(),
                        skipped: gap,
                    });
                }
            }
        }
        rows.push(Row {
            kind: RowKind::Header,
            old: None,
            new: None,
            header: format!(
                "@@ -{},{} +{},{} @@ {}",
                hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines, hunk.header
            )
            .trim_end()
            .to_owned(),
            skipped: 0,
        });
        let leading = hunk
            .lines
            .iter()
            .take_while(|line| line.kind == "context")
            .count();
        let trailing = hunk
            .lines
            .iter()
            .rev()
            .take_while(|line| line.kind == "context")
            .count();
        let skip_start = if collapse {
            leading.saturating_sub(context)
        } else {
            0
        };
        let skip_end = if collapse {
            trailing.saturating_sub(context)
        } else {
            0
        };
        let end = hunk.lines.len().saturating_sub(skip_end);
        let mut old_number = hunk.old_start + skip_start as u32;
        let mut new_number = hunk.new_start + skip_start as u32;
        let mut deleted: Vec<Cell> = Vec::new();
        let mut added: Vec<Cell> = Vec::new();
        for line in &hunk.lines[skip_start..end] {
            let body = line.text.get(1..).unwrap_or(&line.text);
            match line.kind.as_str() {
                "del" => {
                    deleted.push(Cell {
                        number: old_number,
                        text: body.to_owned(),
                    });
                    old_number += 1;
                }
                "add" => {
                    added.push(Cell {
                        number: new_number,
                        text: body.to_owned(),
                    });
                    new_number += 1;
                }
                "context" => {
                    flush_changes(&mut rows, &mut deleted, &mut added);
                    rows.push(Row {
                        kind: RowKind::Context,
                        old: Some(Cell {
                            number: old_number,
                            text: body.to_owned(),
                        }),
                        new: Some(Cell {
                            number: new_number,
                            text: body.to_owned(),
                        }),
                        header: String::new(),
                        skipped: 0,
                    });
                    old_number += 1;
                    new_number += 1;
                }
                _ => {}
            }
        }
        flush_changes(&mut rows, &mut deleted, &mut added);
        previous_end = Some(hunk.old_start + hunk.old_lines.saturating_sub(1));
    }
    rows
}

fn flush_changes(rows: &mut Vec<Row>, deleted: &mut Vec<Cell>, added: &mut Vec<Cell>) {
    let count = deleted.len().max(added.len());
    for index in 0..count {
        rows.push(Row {
            kind: RowKind::Change,
            old: deleted.get(index).cloned(),
            new: added.get(index).cloned(),
            header: String::new(),
            skipped: 0,
        });
    }
    deleted.clear();
    added.clear();
}

fn only_whitespace(hunk: &Hunk) -> bool {
    let deleted: Vec<&str> = hunk
        .lines
        .iter()
        .filter(|line| line.kind == "del")
        .map(|line| line.text.get(1..).unwrap_or(&line.text).trim())
        .collect();
    let added: Vec<&str> = hunk
        .lines
        .iter()
        .filter(|line| line.kind == "add")
        .map(|line| line.text.get(1..).unwrap_or(&line.text).trim())
        .collect();
    !deleted.is_empty() && deleted == added
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub path: String,
    pub line: u32,
    pub text: String,
}

impl Note {
    pub fn new(path: &str, line: u32, text: &str) -> Self {
        Self {
            path: path.to_owned(),
            line,
            text: text.trim().to_owned(),
        }
    }
}

/// Formats notes grouped by file, in file/line order, so the text can be sent
/// to a helper terminal or copied. `heading` comes from i18n.
pub fn format_notes(notes: &[Note], heading: &str) -> String {
    let mut ordered: Vec<&Note> = notes
        .iter()
        .filter(|note| !note.text.trim().is_empty())
        .collect();
    ordered.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.line.cmp(&right.line))
    });
    let mut output = String::new();
    let mut current = "";
    for note in ordered {
        if current.is_empty() {
            output.push_str(heading);
            output.push('\n');
        }
        if note.path != current {
            if !current.is_empty() {
                output.push('\n');
            }
            output.push_str("## ");
            output.push_str(&note.path);
            output.push('\n');
            current = &note.path;
        }
        output.push_str(&format!("- {}: {}\n", note.line, note.text.trim()));
    }
    output.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff() -> FileDiff {
        FileDiff {
            path: "src/main.rs".into(),
            staged: false,
            binary: false,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 6,
                new_start: 1,
                new_lines: 7,
                header: "fn main".into(),
                lines: vec![
                    line("context", " a"),
                    line("del", "-b"),
                    line("add", "+c"),
                    line("add", "+d"),
                    line("context", " e"),
                    line("context", " f"),
                ],
            }],
        }
    }

    fn line(kind: &str, text: &str) -> DiffLine {
        DiffLine {
            kind: kind.into(),
            text: text.into(),
        }
    }

    #[test]
    fn changed_lines_are_paired_and_numbered() {
        let rows = rows(&diff(), 3, false, false);
        assert_eq!(rows[0].kind, RowKind::Header);
        let changes: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == RowKind::Change)
            .collect();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].old.as_ref().unwrap().text, "b");
        assert_eq!(changes[0].new.as_ref().unwrap().text, "c");
        assert!(changes[1].old.is_none());
        assert_eq!(changes[1].new.as_ref().unwrap().number, 3);
        let contexts = rows
            .iter()
            .filter(|row| row.kind == RowKind::Context)
            .count();
        assert_eq!(contexts, 3);
    }

    #[test]
    fn collapse_trims_context_and_marks_the_gap_between_hunks() {
        let mut diff = diff();
        diff.hunks.push(Hunk {
            old_start: 20,
            old_lines: 2,
            new_start: 21,
            new_lines: 2,
            header: String::new(),
            lines: vec![line("context", " x"), line("add", "+y")],
        });
        let rows = rows(&diff, 0, false, true);
        assert!(rows.iter().any(|row| row.kind == RowKind::Gap));
        let gap = rows
            .iter()
            .find(|row| row.kind == RowKind::Gap)
            .expect("gap row");
        assert_eq!(gap.skipped, 13);
        assert_eq!(
            rows.iter()
                .filter(|row| row.kind == RowKind::Context)
                .count(),
            0
        );
    }

    #[test]
    fn whitespace_only_hunks_can_be_ignored() {
        let diff = FileDiff {
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
                header: String::new(),
                lines: vec![line("del", "-  a"), line("add", "+\ta")],
            }],
            ..FileDiff::default()
        };
        assert!(only_whitespace(&diff.hunks[0]));
        assert!(rows(&diff, 3, true, false).is_empty());
        assert!(!rows(&diff, 3, false, false).is_empty());
    }

    #[test]
    fn notes_are_grouped_sorted_and_trimmed() {
        let notes = vec![
            Note::new("b.rs", 4, " segundo "),
            Note::new("a.rs", 9, "nono"),
            Note::new("a.rs", 2, "primeiro"),
            Note::new("a.rs", 1, "   "),
        ];
        let text = format_notes(&notes, "## Notas");
        assert_eq!(
            text,
            "## Notas\n## a.rs\n- 2: primeiro\n- 9: nono\n\n## b.rs\n- 4: segundo"
        );
    }
}
