//! What a search looks for: the pattern and its options, and the file filters.
//!
//! The pattern is compiled once with the `regex` crate. Literal searches escape
//! the text first; "whole word" wraps the pattern in word boundaries. The
//! include and exclude globs use the `ignore` crate's override rules, which are
//! the same rules `.gitignore` uses.

use std::path::Path;

/// The options a user sets in the panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub pattern: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    /// Comma-separated globs; when present, only matching files are searched.
    pub include: String,
    /// Comma-separated globs; matching files are skipped.
    pub exclude: String,
}

impl SearchOptions {
    pub fn is_empty(&self) -> bool {
        self.pattern.is_empty()
    }
}

/// Compiles the pattern. Literal text is escaped, so only regex mode can fail.
pub fn build_regex(options: &SearchOptions) -> Result<regex::Regex, String> {
    let body = if options.regex {
        options.pattern.clone()
    } else {
        regex::escape(&options.pattern)
    };
    let body = if options.whole_word {
        format!(r"\b(?:{body})\b")
    } else {
        body
    };
    regex::RegexBuilder::new(&body)
        .case_insensitive(!options.case_sensitive)
        .build()
        .map_err(|error| error.to_string())
}

/// Splits `a, b` into globs, dropping empty items.
pub fn split_globs(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|glob| !glob.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Builds the include/exclude filter for a walk rooted at `root`.
pub fn overrides(
    root: &Path,
    include: &str,
    exclude: &str,
) -> Result<ignore::overrides::Override, String> {
    let mut builder = ignore::overrides::OverrideBuilder::new(root);
    for glob in split_globs(include) {
        builder.add(&glob).map_err(|error| error.to_string())?;
    }
    for glob in split_globs(exclude) {
        builder
            .add(&format!("!{glob}"))
            .map_err(|error| error.to_string())?;
    }
    builder.build().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(pattern: &str) -> SearchOptions {
        SearchOptions {
            pattern: pattern.to_owned(),
            ..SearchOptions::default()
        }
    }

    #[test]
    fn literal_search_escapes_and_ignores_case_by_default() {
        let re = build_regex(&options("a.b")).unwrap();
        assert!(re.is_match("A.B here"));
        assert!(!re.is_match("axb"), "the dot is literal in literal mode");
    }

    #[test]
    fn case_sensitive_and_whole_word_are_respected() {
        let mut opts = options("cat");
        opts.case_sensitive = true;
        opts.whole_word = true;
        let re = build_regex(&opts).unwrap();
        assert!(re.is_match("a cat sat"));
        assert!(!re.is_match("a Cat sat"));
        assert!(!re.is_match("concatenate"));
    }

    #[test]
    fn regex_mode_reports_invalid_patterns() {
        let mut opts = options("(unclosed");
        opts.regex = true;
        assert!(build_regex(&opts).is_err());
        opts.pattern = r"fn\s+\w+".into();
        assert!(build_regex(&opts).unwrap().is_match("fn main"));
    }

    #[test]
    fn globs_include_and_exclude() {
        let root = std::env::temp_dir();
        let filter = overrides(&root, "*.rs", "gen_*.rs").unwrap();
        assert!(filter.matched(root.join("a.rs"), false).is_whitelist());
        assert!(filter.matched(root.join("gen_b.rs"), false).is_ignore());
        assert!(filter.matched(root.join("c.md"), false).is_ignore());
    }

    #[test]
    fn globs_split_on_commas_and_skip_blanks() {
        assert_eq!(split_globs(" *.rs, ,src/** "), vec!["*.rs", "src/**"]);
        assert!(split_globs("").is_empty());
    }
}
