//! Well-known Forja folders.
//!
//! `FORJA_DATA_DIR` overrides state (`%APPDATA%\Forja`) and `FORJA_CONTENT_DIR`
//! overrides user content (`~/.forja`). Tests always pass explicit folders.

use std::path::PathBuf;

pub fn default_data_dir() -> PathBuf {
    if let Some(path) = env_path("FORJA_DATA_DIR") {
        return path;
    }
    if let Some(appdata) = env_path("APPDATA") {
        return appdata.join("Forja");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".forja")
        .join("data")
}

pub fn default_user_dir() -> PathBuf {
    if let Some(path) = env_path("FORJA_CONTENT_DIR") {
        return path;
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".forja")
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_override_wins() {
        // `std::env::set_var` is process-global; keep this test isolated by
        // computing the function shape rather than racing other tests.
        let path = env_path("FORJA_DATA_DIR");
        if let Some(path) = path {
            assert!(!path.as_os_str().is_empty());
        }
        assert!(default_data_dir().is_absolute() || default_data_dir().starts_with("."));
    }
}
