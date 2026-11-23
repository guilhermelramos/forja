//! Per-module JSON storage with atomic writes and corrupt-file quarantine.
//!
//! The layout is `%APPDATA%\Forja\modules\<id>.json`. Writes go to a sibling
//! temporary file, are synced, and replace the target in one rename. A file
//! that fails to parse is moved aside as `<id>.json.corrupt-<millis>` so the
//! module starts clean without destroying user data.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT_TMP_ID: AtomicU64 = AtomicU64::new(0);

/// Writes `bytes` to a temporary sibling, syncs it and renames it over `path`.
pub fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "missing parent directory")
    })?;
    fs::create_dir_all(parent)?;
    let seq = NEXT_TMP_ID.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let tmp = path.with_extension(format!("tmp-{}-{nanos}-{seq}", std::process::id()));
    let result = (|| {
        // create_new never follows an existing temporary file or link.
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        for attempt in 0..8 {
            match fs::rename(&tmp, path) {
                Ok(()) => return Ok(()),
                Err(error)
                    if attempt < 7 && error.kind() == std::io::ErrorKind::PermissionDenied =>
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    })();
    if result.is_err() {
        for attempt in 0..8 {
            if fs::remove_file(&tmp).is_ok() || !tmp.exists() {
                break;
            }
            if attempt < 7 {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
    result
}

/// Result of reading a module file. A corrupted file is already quarantined
/// by the time this value is returned.
#[derive(Debug, PartialEq)]
pub enum LoadResult<T> {
    Missing,
    Loaded(T),
    Quarantined(PathBuf),
}

impl<T> LoadResult<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Loaded(value) => Some(value),
            Self::Missing | Self::Quarantined(_) => None,
        }
    }

    pub fn is_quarantined(&self) -> bool {
        matches!(self, Self::Quarantined(_))
    }
}

/// Storage rooted at the Forja data directory (or an isolated test folder).
#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// `data_dir` is `%APPDATA%\Forja` in production.
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            root: data_dir.into().join("modules"),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_for(&self, module_id: &str) -> PathBuf {
        self.root.join(format!("{}.json", safe_id(module_id)))
    }

    pub fn load<T: DeserializeOwned>(&self, module_id: &str) -> LoadResult<T> {
        self.load_from(&self.path_for(module_id))
    }

    pub fn load_from<T: DeserializeOwned>(&self, path: &Path) -> LoadResult<T> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return LoadResult::Missing
            }
            Err(_) => {
                return LoadResult::Quarantined(self.quarantine(path));
            }
        };
        match serde_json::from_str(&text) {
            Ok(value) => LoadResult::Loaded(value),
            Err(_) => LoadResult::Quarantined(self.quarantine(path)),
        }
    }

    pub fn save<T: Serialize>(&self, module_id: &str, value: &T) -> std::io::Result<()> {
        self.save_to(&self.path_for(module_id), value)
    }

    pub fn save_to<T: Serialize>(&self, path: &Path, value: &T) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(value)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        write_atomic_bytes(path, &bytes)
    }

    /// Moves a broken file aside and returns its new path. If the rename fails
    /// the original path is returned; the caller still starts fresh.
    fn quarantine(&self, path: &Path) -> PathBuf {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("module.json");
        let target = path.with_file_name(format!("{file_name}.corrupt-{millis}"));
        if fs::rename(path, &target).is_ok() {
            target
        } else {
            path.to_path_buf()
        }
    }
}

fn safe_id(module_id: &str) -> String {
    module_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-store-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Notes {
        text: String,
    }

    #[test]
    fn saves_and_loads_json_atomically() {
        let dir = temp_dir("roundtrip");
        let store = Store::new(&dir);
        assert_eq!(store.load::<Notes>("hello"), LoadResult::Missing);
        store
            .save("hello", &Notes { text: "oi".into() })
            .unwrap();
        assert_eq!(
            store.load::<Notes>("hello"),
            LoadResult::Loaded(Notes { text: "oi".into() })
        );
        // No temporary sibling is left behind.
        let leftovers: Vec<_> = fs::read_dir(store.root())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_quarantined_not_deleted() {
        let dir = temp_dir("corrupt");
        let store = Store::new(&dir);
        fs::create_dir_all(store.root()).unwrap();
        let path = store.path_for("hello");
        fs::write(&path, "{ not json").unwrap();
        let result = store.load::<Notes>("hello");
        let LoadResult::Quarantined(quarantined) = result else {
            panic!("expected quarantine");
        };
        assert!(quarantined.exists());
        assert!(!path.exists());
        assert!(quarantined
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains(".corrupt-"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn ids_cannot_escape_the_modules_folder() {
        let store = Store::new("data");
        assert_eq!(
            store.path_for("../evil").file_name().unwrap(),
            "..-evil.json"
        );
    }
}
