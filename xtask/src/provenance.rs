//! `cargo xtask provenance`: rejects identifiers and phrases from the
//! denylist in every tracked file.
//!
//! Candidate tokens: lowercase identifiers with dot/hyphen/camelCase and eight
//! or more alphanumeric characters, quoted string literals, isolated words,
//! whole text lines and sliding phrases of four or more words. Each candidate
//! is normalized, hashed with SHA-256 and checked against
//! `xtask/provenance-denylist.sha256`. Files matching `xtask/provenance-allow.toml`
//! are skipped with their reason recorded.

use crate::util::{read, repo_root, wildcard_match};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Deserialize)]
struct AllowFile {
    #[serde(default)]
    allow: Vec<Allow>,
}

#[derive(Deserialize)]
struct Allow {
    file: String,
    #[allow(dead_code)]
    reason: String,
}

pub fn run() -> Result<(), String> {
    let started = Instant::now();
    let root = repo_root();
    let allow = load_allowlist(&root)?;
    let denylist = load_denylist(&root)?;
    if denylist.is_empty() {
        return Err("xtask/provenance-denylist.sha256 has no hashes".to_owned());
    }
    let files = git_files(&root)?;
    let mut scanned = 0usize;
    let mut candidates = 0usize;
    let mut hits: Vec<Hit> = Vec::new();

    for file in &files {
        if allow.iter().any(|pattern| wildcard_match(pattern, file)) {
            continue;
        }
        let path = root.join(file);
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        scanned += 1;
        let mut tokens: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            collect_line(line, index + 1, &mut tokens);
        }
        candidates += tokens.len();
        for (token, (line, display)) in tokens {
            let hash = hash_token(&token);
            if denylist.contains(&hash) {
                hits.push(Hit {
                    file: file.clone(),
                    line,
                    token: display,
                });
            }
        }
    }

    hits.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    if hits.is_empty() {
        println!(
            "provenance: ok — {scanned} files, {candidates} candidates, {} hashes, {} ms",
            denylist.len(),
            started.elapsed().as_millis()
        );
        Ok(())
    } else {
        for hit in hits.iter().take(20) {
            eprintln!("{}:{}: banned token `{}`", hit.file, hit.line, hit.token);
        }
        if hits.len() > 20 {
            eprintln!("... and {} more", hits.len() - 20);
        }
        Err(format!(
            "provenance failed: {} denylist match(es) in {} files",
            hits.len(),
            scanned
        ))
    }
}

struct Hit {
    file: String,
    line: usize,
    token: String,
}

fn load_allowlist(root: &Path) -> Result<Vec<String>, String> {
    let text = read(&root.join("xtask/provenance-allow.toml"))?;
    let file: AllowFile = toml::from_str(&text).map_err(|error| error.to_string())?;
    Ok(file.allow.into_iter().map(|allow| allow.file).collect())
}

fn load_denylist(root: &Path) -> Result<HashSet<String>, String> {
    let text = read(&root.join("xtask/provenance-denylist.sha256"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter(|line| line.len() == 64 && line.chars().all(|c| c.is_ascii_hexdigit()))
        .map(|line| line.to_ascii_lowercase())
        .collect())
}

fn git_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("could not run git ls-files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| std::str::from_utf8(entry).ok())
        .map(|entry| entry.replace('\\', "/"))
        .collect())
}

fn collect_line(line: &str, line_number: usize, tokens: &mut BTreeMap<String, (usize, String)>) {
    let lower = line.to_lowercase();
    let trimmed = lower.trim();
    if trimmed.is_empty() {
        return;
    }
    add(trimmed, line_number, tokens);
    let collapsed = lower.split_whitespace().collect::<Vec<_>>().join(" ");
    add(&collapsed, line_number, tokens);

    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .collect();
    for word in &words {
        add(word, line_number, tokens);
    }
    for start in 0..words.len() {
        let max = (start + 12).min(words.len());
        for end in (start + 4)..=max {
            add(&words[start..end].join(" "), line_number, tokens);
        }
    }

    for chunk in lower.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | '"' | '\'' | '`'
            )
    }) {
        let chunk = chunk.trim_matches(|c: char| {
            !(c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == '/' || c == '\\')
        });
        if chunk.is_empty() {
            continue;
        }
        let alphanumeric = chunk.chars().filter(|c| c.is_alphanumeric()).count();
        let compound = chunk.contains(['.', '-', '_', '/', '\\']);
        let camel = chunk
            .chars()
            .zip(chunk.chars().skip(1))
            .any(|(a, b)| a.is_lowercase() && b.is_uppercase());
        if alphanumeric >= 8 && (compound || camel) {
            add(chunk, line_number, tokens);
        }
    }

    for literal in quoted_literals(&lower) {
        add(literal, line_number, tokens);
    }
}

fn quoted_literals(text: &str) -> Vec<&str> {
    let mut literals = Vec::new();
    for quote in ['"', '\''] {
        let mut rest = text;
        while let Some(start) = rest.find(quote) {
            let after = &rest[start + quote.len_utf8()..];
            let Some(end) = after.find(quote) else {
                break;
            };
            literals.push(&after[..end]);
            rest = &after[end + quote.len_utf8()..];
        }
    }
    literals
}

fn add(token: &str, line: usize, tokens: &mut BTreeMap<String, (usize, String)>) {
    let normalized = token
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned();
    if normalized.is_empty() {
        return;
    }
    tokens.entry(normalized.clone()).or_insert((line, normalized));
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_lowercase_sha256() {
        assert_eq!(
            hash_token("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn candidates_include_words_phrases_and_compound_ids() {
        let mut tokens = BTreeMap::new();
        collect_line(
            "bench.create starts alpha beta gamma delta epsilon",
            1,
            &mut tokens,
        );
        assert!(tokens.contains_key("bench.create"));
        assert!(tokens.contains_key("bench"));
        assert!(tokens.contains_key("alpha beta gamma delta"));
    }

    #[test]
    fn quoted_literals_are_extracted() {
        let literals = quoted_literals("let x = \"alpha beta\"; let y = 'gamma';");
        assert!(literals.contains(&"alpha beta"));
        assert!(literals.contains(&"gamma"));
    }
}
