//! `extension.toml`, capacidades e hash da pasta inteira.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Texto localizado no formato `{ ["pt-BR"] = "...", en = "..." }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localized {
    #[serde(rename = "pt-BR")]
    pub pt_br: String,
    pub en: String,
}

impl Localized {
    pub fn resolve(&self, locale: &str) -> &str {
        if locale.to_ascii_lowercase().starts_with("pt") && !self.pt_br.is_empty() {
            &self.pt_br
        } else if !self.en.is_empty() {
            &self.en
        } else if !self.pt_br.is_empty() {
            &self.pt_br
        } else {
            ""
        }
    }

    fn from_toml(value: Option<&toml::Value>) -> Self {
        match value {
            Some(toml::Value::String(text)) => Self {
                pt_br: text.clone(),
                en: text.clone(),
            },
            Some(toml::Value::Table(table)) => Self {
                pt_br: table
                    .get("pt-BR")
                    .or_else(|| table.get("pt_BR"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                en: table
                    .get("en")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            },
            _ => Self::default(),
        }
    }
}

/// Camada de onde a extensão veio.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    User,
    Project,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// Capacidades declaradas no manifesto.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    #[serde(default)]
    pub commands: bool,
    #[serde(rename = "ui.panel", default)]
    pub ui_panel: bool,
    #[serde(default)]
    pub settings: bool,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(rename = "terminal.send", default)]
    pub terminal_send: bool,
    #[serde(rename = "git.read", default)]
    pub git_read: bool,
    #[serde(rename = "fs.read", default)]
    pub fs_read: Vec<String>,
    #[serde(rename = "fs.write", default)]
    pub fs_write: Vec<String>,
    #[serde(default)]
    pub http: Vec<String>,
    #[serde(rename = "ui.open_url", default)]
    pub ui_open_url: Vec<String>,
}

impl Capabilities {
    /// Lista legível para a tela de aprovação.
    pub fn entries(&self) -> Vec<(String, String)> {
        let mut rows = Vec::new();
        if self.commands {
            rows.push(("commands".to_owned(), String::new()));
        }
        if self.ui_panel {
            rows.push(("ui.panel".to_owned(), String::new()));
        }
        if self.settings {
            rows.push(("settings".to_owned(), String::new()));
        }
        if !self.events.is_empty() {
            rows.push(("events".to_owned(), self.events.join(", ")));
        }
        if self.terminal_send {
            rows.push(("terminal.send".to_owned(), String::new()));
        }
        if self.git_read {
            rows.push(("git.read".to_owned(), String::new()));
        }
        if !self.fs_read.is_empty() {
            rows.push(("fs.read".to_owned(), self.fs_read.join(", ")));
        }
        if !self.fs_write.is_empty() {
            rows.push(("fs.write".to_owned(), self.fs_write.join(", ")));
        }
        if !self.http.is_empty() {
            rows.push(("http".to_owned(), self.http.join(", ")));
        }
        if !self.ui_open_url.is_empty() {
            rows.push(("ui.open_url".to_owned(), self.ui_open_url.join(", ")));
        }
        rows
    }

}

/// Manifesto validado.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: Localized,
    pub version: String,
    pub author: String,
    pub description: Localized,
    pub api: String,
    pub entry: String,
    pub capabilities: Capabilities,
    pub layer: Layer,
    pub dir: String,
    pub hash: String,
    pub fingerprint: String,
}

impl Manifest {
    /// Lê e valida o manifesto da pasta.
    pub fn read(dir: &Path, layer: Layer) -> Result<Self, String> {
        let manifest_path = dir.join("extension.toml");
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
        let value: toml::Value = toml::from_str(&text).map_err(|error| error.to_string())?;
        let table = value
            .as_table()
            .ok_or_else(|| "manifest is not a table".to_owned())?;
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| valid_id(value))
            .ok_or_else(|| "`id` is missing or invalid ([a-z][a-z0-9-]{1,39})".to_owned())?;
        let name = Localized::from_toml(table.get("name"));
        if name.pt_br.is_empty() && name.en.is_empty() {
            return Err("`name` is missing".to_owned());
        }
        let version = table
            .get("version")
            .and_then(toml::Value::as_str)
            .filter(|value| valid_version(value))
            .ok_or_else(|| "`version` must be MAJOR.MINOR.PATCH".to_owned())?;
        let author = table
            .get("author")
            .and_then(toml::Value::as_str)
            .unwrap_or("unknown")
            .to_owned();
        let api = table
            .get("api")
            .and_then(toml::Value::as_str)
            .unwrap_or("1")
            .to_owned();
        if api != "1" {
            return Err(format!("API version `{api}` is newer than Forja's `1`"));
        }
        let entry = table
            .get("entry")
            .and_then(toml::Value::as_str)
            .unwrap_or("init.lua")
            .to_owned();
        if entry.contains("..") || entry.starts_with('/') || entry.starts_with('\\') {
            return Err("`entry` must be a relative file name".to_owned());
        }
        let capabilities = parse_capabilities(table.get("capabilities"))?;
        let (fingerprint, hash) = fingerprint_folder(dir)?;
        Ok(Self {
            id: id.to_owned(),
            name,
            version: version.to_owned(),
            author,
            description: Localized::from_toml(table.get("description")),
            api,
            entry,
            capabilities,
            layer,
            dir: dir.display().to_string(),
            hash,
            fingerprint,
        })
    }
}

fn parse_capabilities(value: Option<&toml::Value>) -> Result<Capabilities, String> {
    let Some(value) = value else {
        return Ok(Capabilities::default());
    };
    let table = value
        .as_table()
        .ok_or_else(|| "`capabilities` is not a table".to_owned())?;
    let list = |key: &str| -> Vec<String> {
        table
            .get(key)
            .and_then(toml::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(Capabilities {
        commands: table.get("commands").and_then(toml::Value::as_bool).unwrap_or(false),
        ui_panel: table
            .get("ui.panel")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        settings: table.get("settings").and_then(toml::Value::as_bool).unwrap_or(false),
        events: list("events"),
        terminal_send: table
            .get("terminal.send")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        git_read: table
            .get("git.read")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        fs_read: list("fs.read"),
        fs_write: list("fs.write"),
        http: list("http"),
        ui_open_url: list("ui.open_url"),
    })
}

fn valid_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        && (2..=40).contains(&value.len())
}

fn valid_version(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 3 && parts.iter().all(|part| part.parse::<u32>().is_ok())
}

/// Lê todos os arquivos da pasta em ordem de caminho relativo.
pub fn folder_files(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(files)
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, files)?;
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        files.push((relative, text));
    }
    Ok(())
}

/// Texto canônico cujo SHA-256 identifica a pasta inteira.
pub fn fingerprint_folder(dir: &Path) -> Result<(String, String), String> {
    let files = folder_files(dir)?;
    let mut canonical = String::new();
    for (path, text) in files {
        canonical.push_str(&path);
        canonical.push('\0');
        canonical.push_str(&text.len().to_string());
        canonical.push('\0');
        canonical.push_str(&text);
        canonical.push('\0');
    }
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hash = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(hash, "{byte:02x}");
    }
    Ok((canonical, hash))
}

/// Extensões descobertas em uma raiz, em ordem de nome.
pub fn discover(root: &Path, layer: Layer) -> Vec<Result<Manifest, (PathBuf, String)>> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| Manifest::read(&dir, layer).map_err(|error| (dir, error)))
        .collect()
}

/// Leitura dos arquivos de i18n da extensão.
pub fn read_i18n(dir: &Path, locale: &str) -> BTreeMap<String, String> {
    let mut texts = BTreeMap::new();
    let mut files = vec![dir.join("i18n").join("en.toml")];
    if !locale.eq_ignore_ascii_case("en") {
        files.push(dir.join("i18n").join(format!("{locale}.toml")));
    }
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&text) else {
            continue;
        };
        if let Some(table) = value.as_table() {
            for (key, value) in table {
                if let Some(value) = value.as_str() {
                    texts.insert(key.clone(), value.to_owned());
                }
            }
        }
    }
    texts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-ext-manifest-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_manifest(dir: &Path, id: &str, extra: &str) {
        std::fs::write(
            dir.join("extension.toml"),
            format!(
                "id = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\nauthor = \"Forja\"\napi = \"1\"\n{extra}"
            ),
        )
        .unwrap();
        std::fs::write(dir.join("init.lua"), "-- init\n").unwrap();
    }

    #[test]
    fn manifest_requires_id_version_and_api() {
        let dir = temp_dir("manifest");
        write_manifest(&dir, "good-ext", "");
        assert_eq!(Manifest::read(&dir, Layer::User).unwrap().id, "good-ext");
        std::fs::write(dir.join("extension.toml"), "id = \"Bad\"\n").unwrap();
        assert!(Manifest::read(&dir, Layer::User).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn folder_hash_changes_with_any_byte() {
        let dir = temp_dir("hash");
        write_manifest(&dir, "hash-ext", "");
        let first = Manifest::read(&dir, Layer::User).unwrap().hash;
        std::fs::write(dir.join("init.lua"), "-- changed\n").unwrap();
        let second = Manifest::read(&dir, Layer::User).unwrap().hash;
        assert_ne!(first, second);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn capabilities_read_dotted_keys_and_lists() {
        let dir = temp_dir("caps");
        write_manifest(
            &dir,
            "caps-ext",
            "[capabilities]\ncommands = true\n\"ui.panel\" = true\n\"fs.read\" = [\"**/*.rs\"]\nhttp = [\"api.github.com\"]\n",
        );
        let manifest = Manifest::read(&dir, Layer::User).unwrap();
        assert!(manifest.capabilities.commands);
        assert!(manifest.capabilities.ui_panel);
        assert_eq!(manifest.capabilities.fs_read, vec!["**/*.rs"]);
        assert_eq!(manifest.capabilities.http, vec!["api.github.com"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
