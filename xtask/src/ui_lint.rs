//! Ratcheting audit of raw widgets in product modules (not tests or comments).
use std::path::Path;

const NAMES: [&str; 4] = ["button", "label", "color", "space"];

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    button: usize,
    label: usize,
    color: usize,
    space: usize,
}
impl Counts {
    fn values(&self) -> [usize; 4] {
        [self.button, self.label, self.color, self.space]
    }
    fn increment(&mut self, index: usize) {
        match index {
            0 => self.button += 1,
            1 => self.label += 1,
            2 => self.color += 1,
            _ => self.space += 1,
        }
    }
    fn add(&mut self, other: &Self) {
        for (i, value) in other.values().into_iter().enumerate() {
            for _ in 0..value {
                self.increment(i);
            }
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Ceiling {
    ceiling: Counts,
    allowed: Counts,
}

/// Token and original line, so exemptions never leak into another statement.
struct Token {
    text: String,
    line: usize,
}

fn lex(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let (mut i, mut line) = (0, 0);
    let mut tokens = Vec::new();
    while i < bytes.len() {
        let start = i;
        let token_line = line;
        if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    if bytes[i] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if bytes[i] == b'\''
            && (bytes.get(i + 2) == Some(&b'\'') || bytes.get(i + 1) == Some(&b'\\'))
        {
            i += 1;
            if bytes.get(i) == Some(&b'\\') {
                i += 2;
            } else {
                i += 1;
            }
            while i < bytes.len() && bytes[i] != b'\'' {
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            tokens.push(Token {
                text: "<char>".into(),
                line: token_line,
            });
        } else if bytes[i] == b'"'
            || (bytes[i] == b'r' && bytes.get(i + 1).is_some_and(|b| *b == b'#' || *b == b'"'))
        {
            let raw = bytes[i] == b'r';
            let mut hashes = 0;
            if raw {
                i += 1;
                while bytes.get(i) == Some(&b'#') {
                    hashes += 1;
                    i += 1;
                }
            }
            if bytes.get(i) != Some(&b'"') {
                continue;
            }
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\n' {
                    line += 1;
                }
                if !raw && bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                    continue;
                }
                if bytes[i] == b'"'
                    && bytes
                        .get(i + 1..i + 1 + hashes)
                        .is_some_and(|s| s.iter().all(|b| *b == b'#'))
                {
                    i += 1 + hashes;
                    break;
                }
                i += 1;
            }
            tokens.push(Token {
                text: "<string>".into(),
                line: token_line,
            });
        } else if bytes[i].is_ascii_whitespace() {
            if bytes[i] == b'\n' {
                line += 1;
            }
            i += 1;
        } else {
            i += 1;
            if bytes[start].is_ascii_alphanumeric() || bytes[start] == b'_' {
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric()
                        || bytes[i] == b'_'
                        || (bytes[start].is_ascii_digit() && bytes[i] == b'.'))
                {
                    i += 1;
                }
            }
            tokens.push(Token {
                text: String::from_utf8_lossy(&bytes[start..i]).into_owned(),
                line: token_line,
            });
        }
    }
    tokens
}

fn count_source(source: &str) -> Result<(Counts, Counts), String> {
    let tokens = lex(source);
    let lines: Vec<_> = source.lines().collect();
    let (mut actual, mut allowed) = (Counts::default(), Counts::default());
    let mut i = 0;
    while i < tokens.len() {
        let matches = |pattern: &[&str]| {
            tokens
                .get(i..i + pattern.len())
                .is_some_and(|slice| slice.iter().zip(pattern).all(|(a, b)| a.text == *b))
        };
        if matches(&["#", "[", "cfg", "(", "test", ")", "]"]) || matches(&["#", "[", "test", "]"]) {
            while i < tokens.len() && tokens[i].text != "{" && tokens[i].text != ";" {
                i += 1;
            }
            if tokens.get(i).is_some_and(|t| t.text == "{") {
                let mut depth = 1;
                i += 1;
                while i < tokens.len() && depth > 0 {
                    if tokens[i].text == "{" {
                        depth += 1;
                    }
                    if tokens[i].text == "}" {
                        depth -= 1;
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
            continue;
        }
        let kind = if matches(&["ui", ".", "button", "("]) {
            Some(0)
        } else if matches(&["ui", ".", "label", "("]) {
            Some(1)
        } else if matches(&["Color32", ":", ":"])
            && tokens
                .get(i + 3)
                .is_some_and(|t| t.text.starts_with("from_rgb"))
            && tokens.get(i + 4).is_some_and(|t| t.text == "(")
        {
            Some(2)
        } else if matches(&["add_space", "("])
            && tokens
                .get(i + 2)
                .is_some_and(|t| t.text.starts_with(|c: char| c.is_ascii_digit()))
            && tokens.get(i + 3).is_some_and(|t| t.text == ")")
        {
            Some(3)
        } else {
            None
        };
        if let Some(kind) = kind {
            let line = tokens[i].line;
            let annotation = [Some(line), line.checked_sub(1)]
                .into_iter()
                .flatten()
                .find_map(|n| lines.get(n).and_then(|s| s.split_once("// ui-lint: allow")));
            if let Some((_, reason)) = annotation {
                if reason.trim().is_empty() {
                    return Err(format!(
                        "line {}: ui-lint exemption requires a reason",
                        line + 1
                    ));
                }
                allowed.increment(kind);
            } else {
                actual.increment(kind);
            }
        }
        i += 1;
    }
    Ok((actual, allowed))
}

fn count_tree(root: &Path) -> Result<(Counts, Counts), String> {
    let (mut actual, mut allowed) = (Counts::default(), Counts::default());
    let mut entries: Vec<_> = std::fs::read_dir(root)
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if ["tests", "benches", "fixtures", "target"].contains(&name.as_str())
            || name.ends_with("_tests.rs")
            || name == "tests.rs"
        {
            continue;
        }
        let counts = if path.is_dir() {
            count_tree(&path)?
        } else if path.extension().is_some_and(|e| e == "rs") {
            count_source(&crate::util::read(&path)?)
                .map_err(|e| format!("{}: {e}", path.display()))?
        } else {
            continue;
        };
        actual.add(&counts.0);
        allowed.add(&counts.1);
    }
    Ok((actual, allowed))
}

fn check(actual: &Counts, ceiling: &Counts) -> Result<(), String> {
    let over: Vec<_> = actual
        .values()
        .into_iter()
        .zip(ceiling.values())
        .enumerate()
        .filter(|(_, (n, max))| n > max)
        .map(|(i, (n, max))| format!("{}: {n} > {max}", NAMES[i]))
        .collect();
    if over.is_empty() {
        Ok(())
    } else {
        Err(over.join(", "))
    }
}

pub fn run() -> Result<(), String> {
    let root = crate::util::repo_root();
    let modules = root.join("crates/modules");
    let (actual, allowed) = count_tree(&modules)?;
    println!("ui-lint: button label color space (exemptions separately)");
    for entry in std::fs::read_dir(&modules).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            let (counts, exempt) = count_tree(&path)?;
            if counts
                .values()
                .iter()
                .chain(exempt.values().iter())
                .any(|n| *n > 0)
            {
                println!(
                    "{}: {:?}; allowed {:?}",
                    path.file_name().unwrap().to_string_lossy(),
                    counts.values(),
                    exempt.values()
                );
            }
        }
    }
    println!(
        "TOTAL: {:?}; allowed {:?}",
        actual.values(),
        allowed.values()
    );
    let ceiling: Ceiling = toml::from_str(&crate::util::read(&root.join("design/ui-lint.toml"))?)
        .map_err(|e| e.to_string())?;
    check(&actual, &ceiling.ceiling)?;
    check(&allowed, &ceiling.allowed).map_err(|e| format!("exemptions increased: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_fixture_ignoring_comments_strings_and_tests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ui-lint");
        let (actual, allowed) = count_tree(&root).unwrap();
        assert_eq!(
            actual,
            Counts {
                button: 1,
                label: 2,
                color: 1,
                space: 2
            }
        );
        assert_eq!(
            allowed,
            Counts {
                color: 1,
                ..Counts::default()
            }
        );
    }
    #[test]
    fn ceiling_fails_on_each_increase() {
        for i in 0..4 {
            let mut actual = Counts::default();
            actual.increment(i);
            assert!(check(&actual, &Counts::default()).is_err());
        }
        assert!(check(&Counts::default(), &Counts::default()).is_ok());
    }
    #[test]
    fn exemptions_require_reasons_and_raw_strings_are_not_code() {
        assert!(count_source("ui.label(x); // ui-lint: allow").is_err());
        assert_eq!(
            count_source("let a = r###\"ui.label(x);\"###; /* nested /* ui.button(x) */ */")
                .unwrap()
                .0,
            Counts::default()
        );
    }
}
