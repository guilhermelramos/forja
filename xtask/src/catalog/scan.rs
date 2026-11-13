//! Scans `crates/` Rust sources for real runtime reads of registered settings.
//!
//! Test items (`#[cfg(test)]`), pure `Registrar` registration functions, and
//! registration calls (`SettingSpec::new(...)`, `.setting(...)`) are stripped so
//! only real setting lookups and effects remain.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Returns a map from setting key to sorted workspace-relative `.rs` files
/// that read or apply the setting outside registration and test code.
pub fn find_used_settings(
    root: &Path,
    keys: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut files = Vec::new();
    collect_rs_files(&root.join("crates"), &mut files)?;
    files.sort();

    let mut usage: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in files {
        if is_test_path(&path) {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let found = scan_source(&source, keys);
        if found.is_empty() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for key in found {
            usage.entry(key).or_default().push(rel.clone());
        }
    }
    Ok(usage)
}

/// Scans one Rust source file and returns any registered setting keys referenced
/// outside tests, comments, pure `Registrar` functions, and spec calls.
pub fn scan_source(source: &str, keys: &BTreeSet<String>) -> BTreeSet<String> {
    let without_tests_and_reg = strip_tests_and_pure_registrars(source);
    let uncommented = strip_comments(&without_tests_and_reg);
    let cleaned = strip_registration_calls(&uncommented);
    extract_matching_strings(&cleaned, keys)
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name != "target" && name != "tests" {
                collect_rs_files(&path, out)?;
            }
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn is_test_path(path: &Path) -> bool {
    let is_tests_file = path.file_name().and_then(|n| n.to_str()) == Some("tests.rs");
    let in_test_dir = path.components().any(|part| part.as_os_str() == "tests");
    is_tests_file || in_test_dir
}

fn strip_tests_and_pure_registrars(source: &str) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut kept = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            i = skip_annotated_item(&lines, i + 1);
            continue;
        }
        if looks_like_fn_start(trimmed) {
            if let Some(next_index) = try_skip_pure_registrar_fn(&lines, i) {
                i = next_index;
                continue;
            }
        }
        kept.push(lines[i]);
        i += 1;
    }
    kept.join("\n")
}

fn skip_annotated_item(lines: &[&str], mut index: usize) -> usize {
    let mut depth = 0i32;
    let mut saw_brace = false;
    while index < lines.len() {
        let line = lines[index];
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                saw_brace = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        let semi_end = !saw_brace && line.trim_end().ends_with(';');
        index += 1;
        if semi_end || (saw_brace && depth <= 0) {
            break;
        }
    }
    index
}

fn looks_like_fn_start(trimmed: &str) -> bool {
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    tokens.iter().any(|tok| *tok == "fn" || tok.starts_with("fn("))
}

fn try_skip_pure_registrar_fn(lines: &[&str], start: usize) -> Option<usize> {
    let mut header = String::new();
    let mut j = start;
    while j < lines.len() {
        header.push_str(lines[j]);
        header.push(' ');
        if lines[j].contains('{') || lines[j].contains(';') {
            break;
        }
        j += 1;
    }
    if !header.contains("Registrar") || !header.contains('{') {
        return None;
    }

    let mut depth = 0i32;
    let mut saw_brace = false;
    let mut end = start;
    let mut body = String::new();
    while end < lines.len() {
        let line = lines[end];
        body.push_str(line);
        body.push('\n');
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                saw_brace = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        end += 1;
        if saw_brace && depth <= 0 {
            break;
        }
    }

    if body.contains("cx.") || body.contains("ctx.") || body.contains(".command(") {
        None
    } else {
        Some(end)
    }
}

fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        if in_string {
            let ch = source[i..].chars().next().expect("valid char");
            out.push(ch);
            i += ch.len_utf8();
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            i += 2;
            while i + 1 < bytes.len() && !bytes[i..].starts_with(b"*/") {
                if bytes[i] == b'\n' {
                    out.push('\n');
                }
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }

        let ch = source[i..].chars().next().expect("valid char");
        out.push(ch);
        i += ch.len_utf8();
        if ch == '"' {
            in_string = true;
        }
    }
    out
}

fn strip_registration_calls(source: &str) -> String {
    let mut text = source.to_owned();
    for prefix in [
        "SettingSpec::new(",
        "r.setting(",
        "registrar.setting(",
        "text_setting(",
    ] {
        while let Some(start) = text.find(prefix) {
            let open_paren = start + prefix.len() - 1;
            let end = find_matching_paren(&text, open_paren);
            text.replace_range(start..end, " ");
        }
    }
    text
}

fn find_matching_paren(text: &str, open_paren: usize) -> usize {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[open_paren..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return open_paren + offset + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

fn extract_matching_strings(source: &str, keys: &BTreeSet<String>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut chars = source.char_indices();
    while let Some((idx, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let content_start = idx + 1;
        let mut escaped = false;
        let mut content_end = source.len();
        for (end_idx, inner) in chars.by_ref() {
            if escaped {
                escaped = false;
            } else if inner == '\\' {
                escaped = true;
            } else if inner == '"' {
                content_end = end_idx;
                break;
            }
        }
        let candidate = &source[content_start..content_end];
        if keys.contains(candidate) {
            found.insert(candidate.to_owned());
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_registration_and_tests_but_detects_command_reads() {
        let src = r#"
#[cfg(test)]
mod tests {
    fn f() { let _ = "alpha.one"; }
}

fn register_settings(r: &mut Registrar<'_>) {
    let mut add = |k: &str| { let _ = k; };
    add("alpha.one");
    add("alpha.two");
}

fn register(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new("alpha.three", SettingKind::Bool, SettingValue::Bool(false), "", ""));
    r.command("alpha.run", "title", |cx, _| {
        let _ = cx.setting("alpha.two");
        // "alpha.one" in comment
        Ok(json!({}))
    });
}
"#;
        let keys: BTreeSet<String> = ["alpha.one", "alpha.two", "alpha.three"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        let found = scan_source(src, &keys);
        assert!(found.contains("alpha.two"));
        assert!(!found.contains("alpha.one"));
        assert!(!found.contains("alpha.three"));
    }
}
