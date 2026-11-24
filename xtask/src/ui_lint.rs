//! Ratcheting audit of raw widgets in product modules (not tests or comments).
use std::path::Path;

const NAMES: [&str; 8] = [
    "button",
    "label",
    "color",
    "space",
    "raw_text_edit",
    "raw_combo_box",
    "raw_checkbox",
    "raw_sense_click",
];

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Counts {
    button: usize,
    label: usize,
    color: usize,
    space: usize,
    raw_text_edit: usize,
    raw_combo_box: usize,
    raw_checkbox: usize,
    raw_sense_click: usize,
}
impl Counts {
    fn values(&self) -> [usize; 8] {
        [
            self.button,
            self.label,
            self.color,
            self.space,
            self.raw_text_edit,
            self.raw_combo_box,
            self.raw_checkbox,
            self.raw_sense_click,
        ]
    }
    fn increment(&mut self, index: usize) {
        match index {
            0 => self.button += 1,
            1 => self.label += 1,
            2 => self.color += 1,
            3 => self.space += 1,
            4 => self.raw_text_edit += 1,
            5 => self.raw_combo_box += 1,
            6 => self.raw_checkbox += 1,
            7 => self.raw_sense_click += 1,
            _ => unreachable!("unknown ui-lint counter"),
        }
    }
    fn add(&mut self, other: &Self) {
        for (i, value) in other.values().into_iter().enumerate() {
            for _ in 0..value {
                self.increment(i);
            }
        }
    }
    /// O shell entra só nos contadores de acessibilidade: os tetos do item 9
    /// ([9,159,5,50]) foram medidos em `crates/modules` e não podem subir.
    fn add_accessibility(&mut self, other: &Self) {
        for i in 4..NAMES.len() {
            for _ in 0..other.values()[i] {
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
            if let Some(reason) = source[start..i].strip_prefix("// ui-lint: allow") {
                tokens.push(Token {
                    text: format!("<allow>{reason}"),
                    line: token_line,
                });
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

/// Um `Sense::click` sem `widget_info` por perto vira controle clicável sem nome.
fn has_widget_info(tokens: &[Token], at: usize) -> bool {
    let start = at.saturating_sub(8);
    let end = (at + 40).min(tokens.len());
    tokens[start..end].iter().any(|t| t.text == "widget_info")
}

fn count_source(source: &str) -> Result<(Counts, Counts), String> {
    let tokens = lex(source);
    let annotations: std::collections::BTreeMap<_, _> = tokens
        .iter()
        .filter_map(|t| {
            t.text
                .strip_prefix("<allow>")
                .map(|reason| (t.line, reason.to_owned()))
        })
        .collect();
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|t| !t.text.starts_with("<allow>"))
        .collect();
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
        } else if matches(&["TextEdit", ":", ":"]) {
            Some(4)
        } else if matches(&["ComboBox", ":", ":"]) {
            Some(5)
        } else if matches(&["ui", ".", "checkbox", "("])
            || matches(&["Checkbox", ":", ":"])
        {
            Some(6)
        } else if (matches(&["Sense", ":", ":", "click"])
            || matches(&["Sense", ":", ":", "CLICK"])
            || matches(&["Sense", ":", ":", "click_and_drag"])
            || matches(&["Sense", ":", ":", "drag"]))
            && !has_widget_info(&tokens, i)
        {
            Some(7)
        } else {
            None
        };
        if let Some(kind) = kind {
            let line = tokens[i].line;
            let annotation = [Some(line), line.checked_sub(1)]
                .into_iter()
                .flatten()
                .find_map(|n| annotations.get(&n));
            if let Some(reason) = annotation {
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
    let shell = root.join("crates/shell/src");
    let (mut actual, mut allowed) = (Counts::default(), Counts::default());
    for root in [&modules, &shell] {
        let counts = count_tree(root)?;
        if root == &shell {
            actual.add_accessibility(&counts.0);
            allowed.add_accessibility(&counts.1);
        } else {
            actual.add(&counts.0);
            allowed.add(&counts.1);
        }
    }
    println!("ui-lint: {} (exemptions separately)", NAMES.join(" "));
    for root in [&modules, &shell] {
        for entry in std::fs::read_dir(root).map_err(|e| e.to_string())? {
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
    }
    let (shell_counts, shell_exempt) = count_tree(&shell)?;
    println!(
        "shell/src: {:?}; allowed {:?}",
        shell_counts.values(),
        shell_exempt.values()
    );
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
                space: 2,
                ..Counts::default()
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
    fn an_exemption_inside_a_string_is_not_a_comment() {
        let (actual, allowed) =
            count_source("ui.label(\"// ui-lint: allow not a comment\");").unwrap();
        assert_eq!(actual.label, 1);
        assert_eq!(allowed.label, 0);
    }

    #[test]
    fn ceiling_fails_on_each_increase() {
        for i in 0..NAMES.len() {
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

    #[test]
    fn raw_text_edit_counter_counts_only_direct_constructors() {
        assert_eq!(
            count_source("egui::TextEdit::singleline(&mut text);")
                .unwrap()
                .0
                .raw_text_edit,
            1
        );
        assert_eq!(
            count_source("text_field_labeled(ui, &mut text, \"Label\", \"\");")
                .unwrap()
                .0
                .raw_text_edit,
            0
        );
    }

    #[test]
    fn raw_combo_box_counter_counts_only_direct_constructors() {
        assert_eq!(
            count_source("egui::ComboBox::from_label(\"x\");")
                .unwrap()
                .0
                .raw_combo_box,
            1
        );
        assert_eq!(
            count_source("select_labeled(ui, &mut index, \"Label\", &[\"x\"]);")
                .unwrap()
                .0
                .raw_combo_box,
            0
        );
    }

    #[test]
    fn raw_checkbox_counter_counts_only_direct_checkboxes() {
        assert_eq!(
            count_source("ui.checkbox(&mut on, \"Label\");")
                .unwrap()
                .0
                .raw_checkbox,
            1
        );
        assert_eq!(
            count_source("toggle_labeled(ui, &mut on, \"Label\");")
                .unwrap()
                .0
                .raw_checkbox,
            0
        );
    }

    #[test]
    fn raw_sense_click_requires_a_name_or_an_allowed_reason() {
        let raw = "ui.allocate_exact_size(size, Sense::click());";
        assert_eq!(count_source(raw).unwrap().0.raw_sense_click, 1);
        assert_eq!(
            count_source(
                "let (rect, response) = ui.allocate_exact_size(size, Sense::click());\n\
                 response.widget_info(|| WidgetInfo::button(true));"
            )
            .unwrap()
            .0
            .raw_sense_click,
            0
        );
        let (actual, allowed) = count_source(&format!("{raw} // ui-lint: allow named elsewhere"))
            .unwrap();
        assert_eq!(actual.raw_sense_click, 0);
        assert_eq!(allowed.raw_sense_click, 1);
    }
}
