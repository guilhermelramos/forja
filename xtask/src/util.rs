//! Repository helpers shared by the xtask commands.

use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the repository")
        .to_path_buf()
}

pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn write(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    std::fs::write(path, contents).map_err(|error| format!("{}: {error}", path.display()))
}

/// `hello` -> `Hello`, `hello_world` -> `HelloWorld`, `hello-world` -> `HelloWorld`.
pub fn pascal_case(name: &str) -> String {
    name.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Rejects names that are not lowercase snake_case identifiers.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name is empty".to_owned());
    }
    if !name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
    {
        return Err("name must start with a lowercase letter".to_owned());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("use only lowercase letters, digits and underscore".to_owned());
    }
    if name.ends_with('_') || name.contains("__") {
        return Err("name must not end with `_` or contain `__`".to_owned());
    }
    const RESERVED: &[&str] = &["std", "core", "test", "self", "super", "crate", "forja"];
    if RESERVED.contains(&name) {
        return Err(format!("`{name}` is reserved"));
    }
    Ok(())
}

/// Extension ids allow hyphens, unlike Rust module names.
pub fn validate_extension_name(name: &str) -> Result<(), String> {
    if !(2..=40).contains(&name.len()) {
        return Err("extension name must have 2 to 40 characters".to_owned());
    }
    if !name
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
    {
        return Err("extension name must start with a lowercase letter".to_owned());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("use only lowercase letters, digits and hyphen".to_owned());
    }
    if name.ends_with('-') || name.contains("--") {
        return Err("extension name must not end with `-` or contain `--`".to_owned());
    }
    Ok(())
}

/// Matches a git-style glob: `*` stays inside one path segment, `**` crosses
/// separators. The provenance allowlist relies on `design/specs/*.md` not
/// covering `design/specs/reports/`.
pub fn wildcard_match(pattern: &str, path: &str) -> bool {
    fn go(pattern: &[u8], path: &[u8]) -> bool {
        if pattern.is_empty() {
            return path.is_empty();
        }
        if pattern.starts_with(b"**") {
            return go(&pattern[2..], path)
                || (!path.is_empty() && go(pattern, &path[1..]));
        }
        if pattern[0] == b'*' {
            return go(&pattern[1..], path)
                || (!path.is_empty() && path[0] != b'/' && go(pattern, &path[1..]));
        }
        !path.is_empty() && pattern[0] == path[0] && go(&pattern[1..], &path[1..])
    }
    go(pattern.as_bytes(), path.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_validated() {
        assert!(validate_name("hello").is_ok());
        assert!(validate_name("hello_world2").is_ok());
        assert!(validate_name("Hello").is_err());
        assert!(validate_name("hello-world").is_err());
        assert!(validate_name("hello__world").is_err());
        assert!(validate_name("forja").is_err());
    }

    #[test]
    fn pascal_case_handles_segments() {
        assert_eq!(pascal_case("hello"), "Hello");
        assert_eq!(pascal_case("hello_world"), "HelloWorld");
        assert_eq!(pascal_case("hello-world"), "HelloWorld");
    }

    #[test]
    fn extension_names_allow_hyphens() {
        assert!(validate_extension_name("todo-counter").is_ok());
        assert!(validate_extension_name("hello_world").is_err());
        assert!(validate_extension_name("Todo").is_err());
        assert!(validate_extension_name("a--b").is_err());
    }

    #[test]
    fn wildcard_matching_covers_allowlist_patterns() {
        assert!(wildcard_match("AGENTS.md", "AGENTS.md"));
        assert!(wildcard_match("design/specs/*.md", "design/specs/b-kernel.md"));
        assert!(!wildcard_match("design/specs/*.md", "design/specs/reports/b.md"));
        assert!(wildcard_match("design/**/*.md", "design/specs/reports/b.md"));
        assert!(!wildcard_match("*.toml", "xtask/provenance-allow.toml"));
    }
}
