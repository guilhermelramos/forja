//! Lightweight symbol rules used by the editor shortcuts.
//!
//! The Forja editor has no language server; these helpers give the catalog
//! actions (`goto.definition`, `peek.definition`, `rename`, `completion`,
//! `parameter_hint`) a real, predictable behavior inside the open buffer. They
//! never read other files and never touch the disk.

/// Word characters for symbol purposes.
fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Word under `char_index`, with its character range (not bytes).
///
/// When the caret sits on a separator the word to its left is returned, which
/// matches the usual editor behavior for `F2`/`F12`.
pub fn word_at(text: &str, char_index: usize) -> Option<(String, std::ops::Range<usize>)> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let index = char_index.min(chars.len());
    let mut probe = index;
    if probe >= chars.len() || !is_word(chars[probe]) {
        while probe > 0 && !is_word(chars[probe - 1]) {
            probe -= 1;
        }
    }
    let mut start = probe;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = probe;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    if start == end {
        None
    } else {
        Some((chars[start..end].iter().collect(), start..end))
    }
}

/// Call word immediately before `(` at the caret, e.g. `soma` in `soma(1,`.
pub fn call_word_at(text: &str, char_index: usize) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut probe = char_index.min(chars.len());
    while probe > 0 && chars[probe - 1].is_whitespace() {
        probe -= 1;
    }
    if probe == 0 || chars[probe - 1] != '(' {
        return None;
    }
    let mut end = probe - 1;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    if start == end {
        None
    } else {
        Some(chars[start..end].iter().collect())
    }
}

const DEFINITION_PREFIXES: &[&str] = &[
    "fn ",
    "def ",
    "function ",
    "class ",
    "struct ",
    "enum ",
    "trait ",
    "interface ",
    "type ",
    "const ",
    "static ",
    "let ",
    "var ",
    "module ",
    "impl ",
];

fn defined_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    for prefix in DEFINITION_PREFIXES {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let name = rest
                .trim_start()
                .split(|character: char| !is_word(character))
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    // `nome = ...` and `nome := ...` count as a definition too.
    let name: String = trimmed.chars().take_while(|character| is_word(*character)).collect();
    if !name.is_empty() {
        let rest = trimmed[name.len()..].trim_start();
        if rest.starts_with('=') && !rest.starts_with("==") {
            return Some(&trimmed[..name.len()]);
        }
    }
    None
}

/// 1-based line of the first definition of `word` in the buffer.
pub fn definition_line(text: &str, word: &str) -> Option<usize> {
    text.lines()
        .position(|line| defined_name(line) == Some(word))
        .map(|index| index + 1)
}

/// 1-based line and source line of a function-like signature for `word`.
pub fn signature_line(text: &str, word: &str) -> Option<(usize, String)> {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let starts_like_fn = ["fn ", "def ", "function "]
            .iter()
            .any(|prefix| trimmed.starts_with(prefix));
        if !starts_like_fn {
            continue;
        }
        if let Some(defined) = defined_name(line) {
            if defined == word {
                return Some((index + 1, trimmed.to_owned()));
            }
        }
    }
    None
}

/// Replaces whole-word occurrences of `word` in the buffer.
pub fn rename(
    text: &str,
    word: &str,
    replacement: &str,
    case_sensitive: bool,
) -> (String, usize) {
    if word.is_empty() {
        return (text.to_owned(), 0);
    }
    let pattern = format!(r"\b{}\b", regex::escape(word));
    let Ok(regex) = regex::RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
    else {
        return (text.to_owned(), 0);
    };
    let mut count = 0;
    let replaced = regex
        .replace_all(text, |_: &regex::Captures<'_>| {
            count += 1;
            replacement.to_owned()
        })
        .into_owned();
    (replaced, count)
}

/// Buffer identifiers that start with `prefix`, shortest first.
pub fn completions(text: &str, prefix: &str, limit: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for character in text.chars().chain(std::iter::once(' ')) {
        if is_word(character) {
            current.push(character);
        } else if !current.is_empty() {
            if current.starts_with(prefix) && current != prefix {
                words.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    words.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.cmp(b)));
    words.dedup();
    words.truncate(limit.max(1));
    words
}

/// Partial word immediately before the caret, for completion.
pub fn prefix_at(text: &str, char_index: usize) -> (String, std::ops::Range<usize>) {
    let chars: Vec<char> = text.chars().collect();
    let end = char_index.min(chars.len());
    let mut start = end;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    (chars[start..end].iter().collect(), start..end)
}

/// Removes trailing spaces and tabs from every line.
pub fn trim_trailing(text: &str) -> String {
    text.split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Keeps exactly one final newline (or none for an empty buffer).
pub fn ensure_final_newline(text: &str) -> String {
    format!("{}\n", text.trim_end_matches('\n'))
}

/// Converts leading tabs to the configured amount of spaces.
pub fn tabs_to_spaces(text: &str, size: usize) -> String {
    let size = size.clamp(1, 8);
    text.split('\n')
        .map(|line| {
            let tabs = line.chars().take_while(|character| *character == '\t').count();
            if tabs == 0 {
                line.to_owned()
            } else {
                format!("{}{}", " ".repeat(tabs * size), &line[tabs..])
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_at_finds_the_symbol_under_or_before_the_caret() {
        let text = "let valor_total = soma(1, 2);";
        assert_eq!(word_at(text, 5).unwrap().0, "valor_total");
        assert_eq!(word_at(text, 0).unwrap().0, "let");
        assert_eq!(word_at("a b", 1).unwrap().0, "a");
        assert_eq!(word_at("", 0), None);
    }

    #[test]
    fn definitions_cover_common_declaration_shapes() {
        let text = "fn soma(a: i32) -> i32 {\n  a\n}\nlet total = soma(1, 2);\n";
        assert_eq!(definition_line(text, "soma"), Some(1));
        assert_eq!(definition_line(text, "total"), Some(4));
        assert_eq!(definition_line(text, "nada"), None);
        assert_eq!(
            signature_line(text, "soma").unwrap().1,
            "fn soma(a: i32) -> i32 {"
        );
    }

    #[test]
    fn rename_replaces_whole_words_only() {
        let text = "soma somatorio soma\n";
        let (renamed, count) = rename(text, "soma", "total", true);
        assert_eq!(renamed, "total somatorio total\n");
        assert_eq!(count, 2);
    }

    #[test]
    fn completions_are_shortest_first_and_unique() {
        let text = "valor valor_total value valor";
        let found = completions(text, "val", 10);
        assert_eq!(found, vec!["valor", "value", "valor_total"]);
        let (prefix, range) = prefix_at("let tot", 7);
        assert_eq!(prefix, "tot");
        assert_eq!(range, 4..7);
        assert_eq!(prefix_at("let ", 4).0, "");
    }

    #[test]
    fn fixes_keep_the_buffer_predictable() {
        assert_eq!(trim_trailing("a  \nb\t\n"), "a\nb\n");
        assert_eq!(ensure_final_newline("a\n\n"), "a\n");
        assert_eq!(tabs_to_spaces("\t\ta", 2), "    a");
    }

    #[test]
    fn call_word_reads_the_function_before_the_parenthesis() {
        assert_eq!(call_word_at("x = soma(1,", 9).unwrap(), "soma");
        assert_eq!(call_word_at("x = soma (", 10).unwrap(), "soma");
        assert_eq!(call_word_at("x = soma", 8), None);
    }
}
