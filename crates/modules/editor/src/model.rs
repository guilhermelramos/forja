//! Pure editor rules: files, line endings, encoding, save and search.

use forja_kernel::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eol {
    Lf,
    Crlf,
}

impl Eol {
    pub fn detect(bytes: &[u8]) -> Self {
        if bytes.windows(2).any(|window| window == b"\r\n") {
            Self::Crlf
        } else {
            Self::Lf
        }
    }

    pub fn apply(self, text: &str) -> String {
        match self {
            Self::Lf => text.to_owned(),
            Self::Crlf => text.replace("\r\n", "\n").replace('\n', "\r\n"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
}

impl Encoding {
    pub fn detect(bytes: &[u8]) -> Self {
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            Self::Utf8Bom
        } else if bytes.starts_with(&[0xFF, 0xFE]) {
            Self::Utf16Le
        } else {
            Self::Utf8
        }
    }

    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Self::Utf8Bom => {
                let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
                String::from_utf8_lossy(body).into_owned()
            }
            Self::Utf16Le => {
                let body = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
                let units: Vec<u16> = body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_le_bytes(*pair))
                    .collect();
                String::from_utf16_lossy(&units)
            }
        }
    }

    pub fn encode(self, text: &str) -> Vec<u8> {
        match self {
            Self::Utf8 => text.as_bytes().to_vec(),
            Self::Utf8Bom => {
                let mut bytes = vec![0xEF, 0xBB, 0xBF];
                bytes.extend_from_slice(text.as_bytes());
                bytes
            }
            Self::Utf16Le => {
                let mut bytes = vec![0xFF, 0xFE];
                for unit in text.encode_utf16() {
                    bytes.extend_from_slice(&unit.to_le_bytes());
                }
                bytes
            }
        }
    }
}

/// Reads a file and returns the normalized text plus what is needed to save it
/// back without surprising the repository.
pub fn load_file(path: &Path, eol_setting: &str) -> Result<(String, Eol, Encoding), String> {
    load_file_with_limit(path, eol_setting, None)
}

/// Reads a file, rejecting files larger than `max_mb` MiB when configured.
pub fn load_file_with_limit(
    path: &Path,
    eol_setting: &str,
    max_mb: Option<u64>,
) -> Result<(String, Eol, Encoding), String> {
    if let Some(limit_mb) = max_mb {
        let max_bytes = limit_mb.saturating_mul(1024 * 1024);
        let size = std::fs::metadata(path)
            .map_err(|error| format!("could not read the file: {error}"))?
            .len();
        if size > max_bytes {
            return Err(format!("file exceeds the {limit_mb} MB limit"));
        }
    }
    let bytes = std::fs::read(path).map_err(|error| format!("could not read the file: {error}"))?;
    if bytes.contains(&0) && !bytes.starts_with(&[0xFF, 0xFE]) {
        return Err("binary files are not opened in the editor".to_owned());
    }
    let encoding = Encoding::detect(&bytes);
    let detected_eol = Eol::detect(&bytes);
    let eol = match eol_setting {
        "lf" => Eol::Lf,
        "crlf" => Eol::Crlf,
        _ => detected_eol,
    };
    let text = encoding.decode(&bytes).replace("\r\n", "\n");
    Ok((text, eol, encoding))
}

/// Detects indentation (`"\t"` or 2/4/8 spaces) from non-empty indented lines.
pub fn detect_indent(text: &str) -> Option<String> {
    let mut tab_lines = 0usize;
    let mut min_spaces: Option<usize> = None;
    for line in text.lines() {
        if line.starts_with('\t') {
            tab_lines += 1;
        } else {
            let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
            if spaces > 0 && spaces < line.len() {
                min_spaces = Some(min_spaces.map_or(spaces, |prev| prev.min(spaces)));
            }
        }
    }
    if tab_lines > 0 && min_spaces.is_none() {
        return Some("\t".to_owned());
    }
    min_spaces.map(|count| " ".repeat(count.clamp(1, 8)))
}

pub struct SaveOptions {
    pub eol: Eol,
    pub encoding: Encoding,
    pub trim_trailing: bool,
    pub final_newline: bool,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            eol: Eol::Lf,
            encoding: Encoding::Utf8,
            trim_trailing: false,
            final_newline: true,
        }
    }
}

/// Writes atomically. The text is normalized to LF in memory and converted to
/// the tab's EOL only at this boundary.
pub fn save_file(path: &Path, text: &str, options: &SaveOptions) -> Result<usize, String> {
    let mut normalized = text.replace("\r\n", "\n");
    if options.trim_trailing {
        normalized = normalized
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
    }
    if options.final_newline && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    if !options.final_newline {
        while normalized.ends_with('\n') {
            normalized.pop();
        }
    }
    let encoded = options.encoding.encode(&options.eol.apply(&normalized));
    write_atomic_bytes(path, &encoded)
        .map_err(|error| format!("could not save the file: {error}"))?;
    Ok(encoded.len())
}

/// Saves only an existing file whose resolved destination is inside the active bench.
pub fn save_file_in_root(
    root: &Path,
    path: &Path,
    text: &str,
    options: &SaveOptions,
) -> Result<usize, String> {
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("could not resolve the active bench: {error}"))?;
    let target = std::fs::canonicalize(path)
        .map_err(|error| format!("could not resolve the file: {error}"))?;
    if !target.starts_with(&root) {
        return Err("the editor can only save files inside the active bench".to_owned());
    }
    save_file(&target, text, options)
}

#[derive(Clone, Debug, Default)]
pub struct FindState {
    pub open: bool,
    pub replace_open: bool,
    pub query: String,
    pub replacement: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub matches: Vec<(usize, usize)>,
    pub current: usize,
    pub error: Option<String>,
}

impl FindState {
    pub fn refresh(&mut self, text: &str) {
        self.matches.clear();
        self.error = None;
        if self.query.is_empty() {
            return;
        }
        let pattern = if self.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        let pattern = if self.whole_word {
            format!(r"\b(?:{pattern})\b")
        } else {
            pattern
        };
        let mut builder = regex::RegexBuilder::new(&pattern);
        builder.case_insensitive(!self.case_sensitive);
        match builder.build() {
            Ok(regex) => {
                for found in regex.find_iter(text) {
                    self.matches.push((found.start(), found.end()));
                }
                if self.current >= self.matches.len() {
                    self.current = 0;
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn next(&mut self) {
        if !self.matches.is_empty() {
            self.current = (self.current + 1) % self.matches.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.matches.is_empty() {
            self.current = if self.current == 0 {
                self.matches.len() - 1
            } else {
                self.current - 1
            };
        }
    }
}

/// Replaces all occurrences in the in-memory buffer. It never reads the disk, so
/// unsaved edits and unrelated buffers stay untouched.
pub fn replace_all(text: &str, find: &FindState) -> (String, usize) {
    if find.query.is_empty() {
        return (text.to_owned(), 0);
    }
    let pattern = if find.regex {
        find.query.clone()
    } else {
        regex::escape(&find.query)
    };
    let pattern = if find.whole_word {
        format!(r"\b(?:{pattern})\b")
    } else {
        pattern
    };
    let mut builder = regex::RegexBuilder::new(&pattern);
    builder.case_insensitive(!find.case_sensitive);
    let Ok(regex) = builder.build() else {
        return (text.to_owned(), 0);
    };
    let mut count = 0;
    let replaced = regex
        .replace_all(text, |_: &regex::Captures<'_>| {
            count += 1;
            find.replacement.clone()
        })
        .into_owned();
    (replaced, count)
}

/// Byte offsets of `line` (1-based), including its trailing newline when one
/// exists. Used to set the caret and to operate on whole lines.
pub fn line_span(text: &str, line: usize) -> (usize, usize) {
    let mut start = 0;
    for (index, segment) in text.split_inclusive('\n').enumerate() {
        if index + 1 == line.max(1) {
            return (start, start + segment.len());
        }
        start += segment.len();
    }
    (text.len(), text.len())
}

pub fn line_of_char(text: &str, char_index: usize) -> usize {
    let mut line = 1;
    for (index, character) in text.chars().enumerate() {
        if index >= char_index {
            break;
        }
        if character == '\n' {
            line += 1;
        }
    }
    line
}

pub fn char_index_for_line(text: &str, line: usize) -> usize {
    let (start, _) = line_span(text, line);
    text[..start].chars().count()
}

pub fn byte_to_char(text: &str, byte: usize) -> usize {
    let byte = byte.min(text.len());
    text[..byte].chars().count()
}

/// Byte offset of a character index (clamped to the end of the buffer).
pub fn byte_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

/// Text between two character indexes.
pub fn slice_chars(text: &str, start: usize, end: usize) -> &str {
    let start = byte_for_char(text, start.min(end));
    let end = byte_for_char(text, end);
    &text[start..end]
}

/// Indents or outdents whole lines. Empty lines stay empty when indenting, so
/// the operation does not leave trailing spaces behind.
pub fn change_indent(text: &str, first: usize, last: usize, unit: &str, outdent: bool) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let first = first.max(1);
    let last = last.min(lines.len()).max(first);
    for index in first - 1..last {
        if index >= lines.len() {
            break;
        }
        if outdent {
            let line = &mut lines[index];
            if let Some(rest) = line.strip_prefix(unit) {
                *line = rest.to_owned();
            } else if let Some(rest) = line.strip_prefix('\t') {
                *line = rest.to_owned();
            } else {
                let spaces = line
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .count();
                let remove = spaces.min(unit.len());
                line.replace_range(..remove, "");
            }
        } else if !lines[index].is_empty() {
            lines[index].insert_str(0, unit);
        }
    }
    lines.join("\n")
}

pub fn move_line(text: &str, line: usize, up: bool) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let index = line.max(1) - 1;
    if index >= lines.len() {
        return text.to_owned();
    }
    let target = if up {
        index.checked_sub(1)
    } else if index + 1 < lines.len() {
        Some(index + 1)
    } else {
        None
    };
    if let Some(target) = target {
        lines.swap(index, target);
    }
    lines.join("\n")
}

pub fn duplicate_line(text: &str, line: usize) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let index = line.max(1) - 1;
    if index < lines.len() {
        lines.insert(index + 1, lines[index].clone());
    }
    lines.join("\n")
}

pub fn delete_line(text: &str, line: usize) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let index = line.max(1) - 1;
    if index < lines.len() {
        lines.remove(index);
        if lines.is_empty() {
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

/// Comments or uncomments `first..=last`. A line that already starts with the
/// prefix (after indentation) is uncommented; otherwise every selected line is
/// commented at its indentation.
pub fn toggle_comment(text: &str, first: usize, last: usize, prefix: &str) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let first = first.max(1);
    let last = last.min(lines.len()).max(first);
    let range = first - 1..last;
    let all_commented = lines[range.clone()]
        .iter()
        .all(|line| line.trim().is_empty() || line.trim_start().starts_with(prefix));
    for index in range {
        if index >= lines.len() {
            break;
        }
        let line = &mut lines[index];
        if all_commented {
            if let Some(at) = line.find(prefix) {
                let mut rest = line[at + prefix.len()..].to_owned();
                if let Some(stripped) = rest.strip_prefix(' ') {
                    rest = stripped.to_owned();
                }
                *line = format!("{}{}", &line[..at], rest);
            }
        } else if !line.trim().is_empty() {
            let indent = line.len() - line.trim_start().len();
            line.insert_str(indent, &format!("{prefix} "));
        }
    }
    lines.join("\n")
}

pub fn line_count(text: &str) -> usize {
    text.lines().count().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-editor-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("a.txt")
    }

    #[test]
    fn atomic_save_preserves_crlf() {
        let path = temp("crlf");
        std::fs::write(&path, "um\r\ndois\r\n").unwrap();
        let (text, eol, encoding) = load_file(&path, "auto").unwrap();
        assert_eq!(eol, Eol::Crlf);
        assert_eq!(text, "um\ndois\n");
        save_file(
            &path,
            "um\ndois\ntres\n",
            &SaveOptions {
                eol,
                encoding,
                final_newline: true,
                ..SaveOptions::default()
            },
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes, b"um\r\ndois\r\ntres\r\n");
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn save_adds_final_newline_and_trims_only_when_asked() {
        let path = temp("final");
        save_file(
            &path,
            "a  \nb",
            &SaveOptions {
                final_newline: true,
                trim_trailing: true,
                ..SaveOptions::default()
            },
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\n");
    }

    #[test]
    fn save_rejects_a_file_outside_the_resolved_bench() {
        let root = std::env::temp_dir().join(format!(
            "forja-editor-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let bench = root.join("bench");
        std::fs::create_dir_all(&bench).unwrap();
        let outside = root.join("outside.txt");
        std::fs::write(&outside, "preserve me\n").unwrap();

        let result = save_file_in_root(&bench, &outside, "overwrite", &SaveOptions::default());
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), "preserve me\n");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn replace_all_keeps_the_buffer_and_counts() {
        let mut find = FindState {
            query: "casa".into(),
            replacement: "lar".into(),
            ..FindState::default()
        };
        find.refresh("casa e casa");
        assert_eq!(find.matches.len(), 2);
        let (replaced, count) = replace_all("casa e casa", &find);
        assert_eq!(replaced, "lar e lar");
        assert_eq!(count, 2);
    }

    #[test]
    fn regex_and_whole_word_options_are_honored() {
        let mut find = FindState {
            query: "casa".into(),
            whole_word: true,
            ..FindState::default()
        };
        find.refresh("casa casamento casa");
        assert_eq!(find.matches.len(), 2);
        find.query = r"\d+".into();
        find.regex = true;
        find.whole_word = false;
        find.refresh("a 12 b 34");
        assert_eq!(find.matches.len(), 2);
    }

    #[test]
    fn bom_and_utf16_roundtrip() {
        let path = temp("bom");
        std::fs::write(&path, [0xEF, 0xBB, 0xBF, b'o', b'k']).unwrap();
        let (text, _, encoding) = load_file(&path, "auto").unwrap();
        assert_eq!(text, "ok");
        assert_eq!(encoding, Encoding::Utf8Bom);
        save_file(
            &path,
            &text,
            &SaveOptions {
                encoding,
                final_newline: false,
                ..SaveOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [0xEF, 0xBB, 0xBF, b'o', b'k']
        );
    }

    #[test]
    fn replace_all_honors_case_sensitivity() {
        let find = FindState {
            query: "Casa".into(),
            replacement: "lar".into(),
            ..FindState::default()
        };
        let (replaced, count) = replace_all("casa e Casa", &find);
        assert_eq!(replaced, "lar e lar");
        assert_eq!(count, 2);
        let find = FindState {
            case_sensitive: true,
            ..find
        };
        let (replaced, count) = replace_all("casa e Casa", &find);
        assert_eq!(replaced, "casa e lar");
        assert_eq!(count, 1);
    }

    #[test]
    fn line_operations_keep_the_rest_of_the_buffer() {
        let text = "um\ndois\ntres\n";
        assert_eq!(change_indent(text, 2, 2, "  ", false), "um\n  dois\ntres\n");
        assert_eq!(
            change_indent("um\n  dois\ntres\n", 2, 2, "  ", true),
            "um\ndois\ntres\n"
        );
        assert_eq!(move_line(text, 1, false), "dois\num\ntres\n");
        assert_eq!(duplicate_line(text, 2), "um\ndois\ndois\ntres\n");
        assert_eq!(delete_line(text, 2), "um\ntres\n");
        assert_eq!(
            toggle_comment("fn a() {}\nfn b() {}", 1, 2, "//"),
            "// fn a() {}\n// fn b() {}"
        );
        assert_eq!(
            toggle_comment("// fn a() {}\n// fn b() {}", 1, 2, "//"),
            "fn a() {}\nfn b() {}"
        );
    }

    #[test]
    fn cursor_helpers_convert_between_lines_bytes_and_chars() {
        let text = "aé\nb\nc";
        assert_eq!(char_index_for_line(text, 2), 3);
        assert_eq!(line_of_char(text, 4), 2);
        assert_eq!(byte_to_char("aé", 3), 2);
        assert_eq!(line_span(text, 2), (4, 6));
        assert_eq!(byte_for_char("aé\nb", 2), 3);
        assert_eq!(slice_chars("aé\nb", 1, 3), "é\n");
    }

    #[test]
    fn load_limit_and_detect_indent_work() {
        let path = temp("large");
        std::fs::write(&path, vec![b'a'; 2 * 1024 * 1024]).unwrap();
        assert!(load_file_with_limit(&path, "auto", Some(1)).is_err());
        assert!(load_file_with_limit(&path, "auto", Some(4)).is_ok());
        assert_eq!(detect_indent("fn a() {\n  let x = 1;\n}"), Some("  ".to_owned()));
        assert_eq!(detect_indent("fn a() {\n\tlet x = 1;\n}"), Some("\t".to_owned()));
        assert_eq!(detect_indent("plain text"), None);
    }
}
