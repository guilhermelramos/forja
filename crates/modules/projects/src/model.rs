//! Project records and their JSON file.
//!
//! One project is a folder the Forja knows about. The record never owns files:
//! removing a project only removes the entry, never the folder.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const DEFAULT_ICON: &str = "\u{25a3}";
pub const DEFAULT_COLOR: &str = "#6FA8DC";
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub created_at: u64,
}

fn default_icon() -> String {
    DEFAULT_ICON.to_owned()
}

fn default_color() -> String {
    DEFAULT_COLOR.to_owned()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectsFile {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub projects: Vec<Project>,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

impl ProjectsFile {
    pub fn sorted(mut self) -> Self {
        self.projects.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.path.cmp(&b.path))
        });
        self
    }
}

/// What an add or create form supplies.
#[derive(Clone, Debug, Default)]
pub struct NewProject {
    pub path: String,
    pub name: String,
    pub icon: String,
    pub color: String,
}

/// Stable id derived from the normalized absolute path.
pub fn project_id(path: &Path) -> String {
    let normalized = normalize(path);
    let digest = Sha256::digest(normalized.as_bytes());
    let mut hex = String::with_capacity(20);
    for byte in digest.iter().take(10) {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("p-{hex}")
}

pub fn normalize(path: &Path) -> String {
    let mut text = path.to_string_lossy().replace('\\', "/");
    while text.ends_with('/') && text.len() > 1 {
        text.pop();
    }
    text.to_lowercase()
}

pub fn normalize_color(value: &str) -> Option<String> {
    let value = value.trim();
    let valid = value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|c| c.is_ascii_hexdigit());
    if valid {
        Some(value.to_ascii_uppercase())
    } else {
        None
    }
}

pub fn normalize_icon(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        DEFAULT_ICON.to_owned()
    } else {
        value.chars().take(4).collect()
    }
}

/// Resolves a user path to an existing folder. Returns a friendly message.
pub fn existing_folder(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path.trim());
    if path.trim().is_empty() {
        return Err("path is empty".to_owned());
    }
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("folder does not exist: {error}"))?;
    if !canonical.is_dir() {
        return Err("path is not a folder".to_owned());
    }
    Ok(forja_kernel::paths::clean_path(canonical))
}

pub fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("projeto")
        .to_owned()
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Builds a record from a form. Duplicate paths are rejected by the caller.
pub fn build_project(form: &NewProject, canonical: &Path, order: usize) -> Result<Project, String> {
    let name = if form.name.trim().is_empty() {
        display_name(canonical)
    } else {
        form.name.trim().to_owned()
    };
    let color = match form.color.trim() {
        "" => DEFAULT_COLOR.to_owned(),
        value => normalize_color(value).ok_or_else(|| "color must be #RRGGBB".to_owned())?,
    };
    let mut id = project_id(canonical);
    if order > 0 {
        id.push_str(&format!("-{order}"));
    }
    Ok(Project {
        id,
        name,
        path: canonical.to_string_lossy().into_owned(),
        icon: normalize_icon(&form.icon),
        color,
        created_at: now_millis(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_normalized_and_validated() {
        assert_eq!(normalize_color("#f2762e"), Some("#F2762E".to_owned()));
        assert_eq!(normalize_color("orange"), None);
        assert_eq!(normalize_color("#F2762EFF"), None);
    }

    #[test]
    fn ids_are_stable_and_case_insensitive_on_windows_paths() {
        let a = project_id(Path::new(r"C:\Work\Forja"));
        let b = project_id(Path::new("c:/work/forja/"));
        assert_eq!(a, b);
        assert!(a.starts_with("p-"));
    }

    #[test]
    fn missing_folder_has_a_clear_error() {
        let error = existing_folder("C:/forja/definitely-missing-12345").unwrap_err();
        assert!(error.contains("does not exist"));
    }

    #[test]
    fn default_name_comes_from_the_last_segment() {
        assert_eq!(display_name(Path::new("C:/work/tools")), "tools");
    }
}
