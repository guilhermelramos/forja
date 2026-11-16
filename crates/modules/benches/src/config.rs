//! `forja.toml`: setup, teardown, initial tabs and the request command.
//!
//! ```toml
//! schema_version = 1
//! setup = "npm ci"
//! teardown = "npm run clean"
//! tabs = ["terminal", "helper"]
//!
//! [request]
//! command = "forja pedido abrir {id}"
//! ```
//!
//! Unknown keys are errors so a typo does not silently disable setup. A missing
//! file is valid and uses the catalog defaults.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForjaConfig {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub setup: String,
    #[serde(default)]
    pub teardown: String,
    #[serde(default)]
    pub tabs: Vec<String>,
    #[serde(default)]
    pub request: RequestConfig,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestConfig {
    /// Template run when the bench was created from a request. `{id}` is
    /// replaced by the request id. Other modules own the actual counter.
    #[serde(default)]
    pub command: String,
}

#[derive(Clone, Debug, Default)]
pub struct LoadedConfig {
    pub config: ForjaConfig,
    pub warnings: Vec<String>,
    /// False when `forja.toml` was not present.
    pub present: bool,
    pub setup_hash: Option<String>,
    pub teardown_hash: Option<String>,
}

pub fn load(project_path: &str) -> LoadedConfig {
    let path = std::path::Path::new(project_path).join("forja.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadedConfig::default()
        }
        Err(error) => {
            return LoadedConfig {
                warnings: vec![format!("{}: {error}", path.display())],
                ..LoadedConfig::default()
            }
        }
    };
    let mut loaded = LoadedConfig {
        present: true,
        ..LoadedConfig::default()
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    match toml::from_str::<ForjaConfig>(text) {
        Ok(config) => {
            loaded.setup_hash = hash_script(&config.setup);
            loaded.teardown_hash = hash_script(&config.teardown);
            loaded.config = sanitized(config, &mut loaded.warnings);
        }
        Err(error) => {
            loaded.present = false;
            loaded.warnings.push(format!("{}: {error}", path.display()));
        }
    }
    loaded
}

fn sanitized(mut config: ForjaConfig, warnings: &mut Vec<String>) -> ForjaConfig {
    config.setup = config.setup.trim().to_owned();
    config.teardown = config.teardown.trim().to_owned();
    config.request.command = config.request.command.trim().to_owned();
    let mut seen = std::collections::BTreeSet::new();
    config.tabs = config
        .tabs
        .into_iter()
        .map(|tab| tab.trim().to_lowercase())
        .filter(|tab| {
            if tab.is_empty() {
                return false;
            }
            if tab.len() > 40
                || !tab.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
            {
                warnings.push(format!("forja.toml: tab `{tab}` ignored"));
                return false;
            }
            if !seen.insert(tab.clone()) {
                warnings.push(format!("forja.toml: repeated tab `{tab}` ignored"));
                return false;
            }
            true
        })
        .collect();
    config
}

pub fn hash_script(script: &str) -> Option<String> {
    let script = script.trim();
    if script.is_empty() {
        return None;
    }
    let digest = Sha256::digest(script.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    Some(hex)
}

pub fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

pub fn tabs(config: &ForjaConfig, fallback: &[String]) -> Vec<String> {
    if config.tabs.is_empty() {
        fallback.to_vec()
    } else {
        config
            .tabs
            .iter()
            .filter(|tab| tool_tab_view_id(tab).is_none())
            .cloned()
            .collect()
    }
}

/// Maps a `forja.toml` tab entry that names a tool tab onto its [`Slot::BenchTab`] view ID.
pub fn tool_tab_view_id(tab: &str) -> Option<&'static str> {
    match tab.trim().to_ascii_lowercase().as_str() {
        "terminal.tab" => Some("terminal.tab"),
        "chat" | "chat.tab" => Some("chat.tab"),
        "diff" | "diff.tab" => Some("diff.tab"),
        "editor" | "editor.tab" => Some("editor.tab"),
        "browser" | "browser.tab" => Some("browser.tab"),
        "viewer" | "viewers" | "viewers.tab" => Some("viewers.tab"),
        "bench" | "benches" | "bench.detail" => Some("bench.detail"),
        "review" | "review.tab" => Some("review.tab"),
        "history" | "git.history" => Some("git.history"),
        "extensions" | "extension" | "extension.panels.bench" => Some("extension.panels.bench"),
        _ => None,
    }
}

/// Maps a `bench.layout.tool_tabs` entry (allowing `"terminal"`/`"helper"` short ids)
/// onto its [`Slot::BenchTab`] view ID.
pub fn setting_tool_tab_view_id(tab: &str) -> Option<&'static str> {
    match tab.trim().to_ascii_lowercase().as_str() {
        "terminal" => Some("terminal.tab"),
        "helper" => Some("chat.tab"),
        _ => tool_tab_view_id(tab),
    }
}

/// Resolves initial tool tab view IDs from `bench.layout.tool_tabs` extended by `forja.toml`.
pub fn tool_tabs(config: &ForjaConfig, configured: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |id: &str| {
        if !out.iter().any(|existing: &String| existing == id) {
            out.push(id.to_owned());
        }
    };
    for entry in configured {
        if let Some(view_id) = setting_tool_tab_view_id(entry) {
            push(view_id);
        }
    }
    for entry in &config.tabs {
        if let Some(view_id) = tool_tab_view_id(entry) {
            push(view_id);
        } else if entry == "terminal" {
            push("terminal.tab");
        }
    }
    out
}

pub fn request_command(config: &ForjaConfig, request: &str) -> Option<String> {
    if config.request.command.trim().is_empty() || request.trim().is_empty() {
        return None;
    }
    Some(config.request.command.replace("{id}", request))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_trimmed() {
        assert_eq!(hash_script("  npm ci  "), hash_script("npm ci"));
        assert_ne!(hash_script("npm ci"), hash_script("npm ci --force"));
        assert_eq!(hash_script("   "), None);
    }

    #[test]
    fn missing_file_is_valid() {
        let loaded = load("C:/forja/definitely-missing-12345");
        assert!(!loaded.present);
        assert!(loaded.warnings.is_empty());
        assert!(loaded.config.tabs.is_empty());
    }

    #[test]
    fn typo_is_reported_and_defaults_are_kept() {
        let root = std::env::temp_dir().join(format!(
            "forja-config-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("forja.toml"), "setpu = \"npm ci\"\n").unwrap();
        let loaded = load(&root.to_string_lossy());
        assert!(!loaded.warnings.is_empty());
        assert!(
            !loaded.present,
            "a malformed file is not treated as configuration"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tabs_dedupe_and_validate() {
        let root = std::env::temp_dir().join(format!(
            "forja-tabs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("forja.toml"),
            "tabs = [\"terminal\", \"Terminal\", \"bad tab\", \"helper\"]\n",
        )
        .unwrap();
        let loaded = load(&root.to_string_lossy());
        assert_eq!(loaded.config.tabs, vec!["terminal", "helper"]);
        assert_eq!(loaded.warnings.len(), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn tabs_partition_terminals_from_tool_tabs_and_extend_defaults() {
        let config = ForjaConfig {
            tabs: vec![
                "terminal".to_owned(),
                "helper".to_owned(),
                "browser".to_owned(),
                "review.tab".to_owned(),
            ],
            ..Default::default()
        };
        let fallback = vec!["terminal".to_owned(), "helper".to_owned()];
        assert_eq!(tabs(&config, &fallback), vec!["terminal", "helper"]);
        let defaults = vec![
            "terminal".to_owned(),
            "chat".to_owned(),
            "diff".to_owned(),
            "editor".to_owned(),
        ];
        assert_eq!(
            tool_tabs(&config, &defaults),
            vec![
                "terminal.tab",
                "chat.tab",
                "diff.tab",
                "editor.tab",
                "browser.tab",
                "review.tab",
            ]
        );
    }
}
