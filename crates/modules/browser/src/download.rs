//! Download file names stay inside the configured folder.

use std::path::Path;

/// Picks one plain file name: the server suggestion when safe, otherwise the
/// last URL segment, otherwise `download`.
pub fn file_name(url: &str, suggested: &Path) -> String {
    let suggested = suggested
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .and_then(|name| safe_name(&name));
    let from_url = url.rsplit('/').next().and_then(safe_name);
    suggested
        .or(from_url)
        .unwrap_or_else(|| "download".to_owned())
}

/// `Some` only for a single Windows-safe file name. Folder names, drives, ADS,
/// device names and parent references are rejected.
pub fn safe_name(name: &str) -> Option<String> {
    let name = name.split(['?', '#']).next().unwrap_or("").trim();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    if name.chars().any(|character| {
        character < ' '
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    }) {
        return None;
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return None;
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        return None;
    }
    Some(name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_and_device_names_are_refused() {
        assert_eq!(
            file_name("https://x.test/a\\..\\..\\Startup\\evil.exe", Path::new("")),
            "download"
        );
        assert_eq!(file_name("https://x.test/..", Path::new("")), "download");
        assert_eq!(file_name("https://x.test/NUL", Path::new("")), "download");
        assert_eq!(
            file_name("https://x.test/com1.log", Path::new("")),
            "download"
        );
        assert_eq!(
            file_name("https://x.test/", Path::new("x.exe:stream")),
            "download"
        );
    }

    #[test]
    fn plain_names_survive_and_drop_the_query() {
        assert_eq!(
            file_name("https://x.test/files/report.pdf?sig=1", Path::new("")),
            "report.pdf"
        );
        assert_eq!(
            file_name("https://x.test/a.txt", Path::new("suggested.txt")),
            "suggested.txt"
        );
        assert_eq!(
            file_name("https://x.test/a.txt", Path::new("CON.txt")),
            "a.txt"
        );
    }
}
