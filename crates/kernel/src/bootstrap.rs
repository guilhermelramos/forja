//! Startup directory bootstrap (`bootstrap.toml`) and storage path helpers.
//!
//! Resolves `data_dir` and `user_dir` (`content_dir`) before settings or modules
//! load:
//! 1. `FORJA_DATA_DIR` / `FORJA_CONTENT_DIR` environment variables always win
//!    (tests and isolated runs depend on them).
//! 2. Otherwise `%APPDATA%\Forja\bootstrap.toml` (the fixed default location)
//!    may redirect either folder, provided the target is absolute and writable.
//! 3. Invalid or unwritable folders fall back to the built-in defaults and
//!    record a warning for the log.

use crate::error::{KernelError, Result};
use crate::paths::{base_default_data_dir, base_default_user_dir, clean_path, home_dir};
use crate::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Fixed file name in the default data directory (`%APPDATA%\Forja\bootstrap.toml`).
pub const BOOTSTRAP_FILE: &str = "bootstrap.toml";

static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Persisted overrides read before `settings.toml` is opened.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_dir: Option<String>,
}

/// Result of resolving `data_dir` and `user_dir` at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBootstrap {
    pub data_dir: PathBuf,
    pub user_dir: PathBuf,
    pub warnings: Vec<String>,
}

/// Summary of copying existing data/content into a newly chosen folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyReport {
    pub files_copied: usize,
    pub dirs_created: usize,
    pub bytes_copied: u64,
}

/// Resolves startup directories using `FORJA_DATA_DIR`, `FORJA_CONTENT_DIR`,
/// and `%APPDATA%\Forja\bootstrap.toml`.
pub fn resolve() -> ResolvedBootstrap {
    resolve_with(
        &bootstrap_file_path(),
        env_path("FORJA_DATA_DIR"),
        env_path("FORJA_CONTENT_DIR"),
        base_default_data_dir(),
        base_default_user_dir(),
    )
}

/// Fixed path of `bootstrap.toml` (`%APPDATA%\Forja\bootstrap.toml`).
pub fn bootstrap_file_path() -> PathBuf {
    base_default_data_dir().join(BOOTSTRAP_FILE)
}

/// Testable core of bootstrap directory resolution.
pub fn resolve_with(
    bootstrap_path: &Path,
    env_data_dir: Option<PathBuf>,
    env_user_dir: Option<PathBuf>,
    base_data_dir: PathBuf,
    base_user_dir: PathBuf,
) -> ResolvedBootstrap {
    let mut warnings = Vec::new();
    let config = if env_data_dir.is_none() || env_user_dir.is_none() {
        match load_config_optional(bootstrap_path) {
            Ok(config) => config,
            Err(warning) => {
                warnings.push(warning);
                BootstrapConfig::default()
            }
        }
    } else {
        BootstrapConfig::default()
    };

    let data_dir = match env_data_dir {
        Some(path) => path,
        None => resolve_override(
            "app.storage.data_dir",
            config.data_dir.as_deref(),
            &base_data_dir,
            &mut warnings,
        ),
    };
    let user_dir = match env_user_dir {
        Some(path) => path,
        None => resolve_override(
            "app.storage.content_dir",
            config.content_dir.as_deref(),
            &base_user_dir,
            &mut warnings,
        ),
    };

    ResolvedBootstrap {
        data_dir,
        user_dir,
        warnings,
    }
}

fn resolve_override(
    key: &str,
    configured: Option<&str>,
    default_dir: &Path,
    warnings: &mut Vec<String>,
) -> PathBuf {
    let Some(raw) = configured.map(str::trim).filter(|s| !s.is_empty()) else {
        return default_dir.to_path_buf();
    };
    let Some(expanded) = expand_path(raw) else {
        warnings.push(format!(
            "bootstrap.toml: `{key}` is empty; falling back to {}",
            default_dir.display()
        ));
        return default_dir.to_path_buf();
    };
    match validate_writable_dir(&expanded) {
        Ok(()) => expanded,
        Err(error) => {
            warnings.push(format!(
                "bootstrap.toml: `{key}` (`{raw}`) is invalid or not writable ({error}); falling back to {}",
                default_dir.display()
            ));
            default_dir.to_path_buf()
        }
    }
}

/// Expands `~` and `%ENV_VAR%` segments in a storage path.
pub fn expand_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_env = expand_percent_vars(trimmed);
    let normalized = with_env.replace('\\', "/");
    if normalized == "~" {
        return home_dir();
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        let home = home_dir()?;
        return Some(if rest.is_empty() {
            home
        } else {
            home.join(rest)
        });
    }
    Some(PathBuf::from(with_env))
}

fn expand_percent_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after_start[..end];
        if !name.is_empty() {
            if let Some(value) = std::env::var_os(name) {
                out.push_str(&value.to_string_lossy());
                rest = &after_start[end + 1..];
                continue;
            }
        }
        out.push('%');
        out.push_str(name);
        out.push('%');
        rest = &after_start[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Verifies that `path` is absolute, is a directory (creating it if needed),
/// and can be written to by the current user.
pub fn validate_writable_dir(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "folder path must be absolute",
        ));
    }
    if path.exists() && !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "target path exists and is not a directory",
        ));
    }
    fs::create_dir_all(path)?;
    let seq = PROBE_SEQ.fetch_add(1, Ordering::Relaxed);
    let probe = path.join(format!(".forja-write-probe-{}-{seq}", std::process::id()));
    let file = OpenOptions::new().write(true).create_new(true).open(&probe)?;
    drop(file);
    let _ = fs::remove_file(&probe);
    Ok(())
}

/// Reads `bootstrap.toml`, returning `Default` when missing.
pub fn load_config_optional(path: &Path) -> std::result::Result<BootstrapConfig, String> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str::<BootstrapConfig>(&text)
            .map_err(|error| format!("bootstrap.toml could not be parsed: {error}")),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BootstrapConfig::default()),
        Err(error) => Err(format!("bootstrap.toml could not be read: {error}")),
    }
}

/// Atomically writes `bootstrap.toml`.
pub fn save_config(path: &Path, config: &BootstrapConfig) -> Result<()> {
    let text = toml::to_string_pretty(config)?;
    write_atomic_bytes(path, text.as_bytes())?;
    Ok(())
}

/// Validates a storage setting (`app.storage.data_dir` or
/// `app.storage.content_dir`) and updates `bootstrap.toml` for the next startup.
pub fn update_storage_setting(
    bootstrap_path: &Path,
    setting_key: &str,
    raw_value: &str,
    default_dir: &Path,
) -> Result<PathBuf> {
    let trimmed = raw_value.trim();
    let (target, override_value) = if trimmed.is_empty() {
        validate_writable_dir(default_dir).map_err(|error| {
            KernelError::Settings(format!(
                "`{setting_key}` default folder `{}` is not writable: {error}",
                default_dir.display()
            ))
        })?;
        (default_dir.to_path_buf(), None)
    } else {
        let expanded = expand_path(trimmed).ok_or_else(|| {
            KernelError::Settings(format!("`{setting_key}` path could not be resolved"))
        })?;
        validate_writable_dir(&expanded).map_err(|error| {
            KernelError::Settings(format!(
                "`{setting_key}` (`{trimmed}`) is invalid or not writable: {error}"
            ))
        })?;
        let cleaned = clean_path(&expanded);
        let cleaned_default = clean_path(default_dir);
        let stored = if same_path(&cleaned, &cleaned_default) {
            None
        } else {
            Some(cleaned.display().to_string())
        };
        (cleaned, stored)
    };

    let mut config = load_config_optional(bootstrap_path).unwrap_or_default();
    match setting_key {
        "app.storage.data_dir" => config.data_dir = override_value,
        "app.storage.content_dir" => config.content_dir = override_value,
        other => {
            return Err(KernelError::Settings(format!(
                "unknown bootstrap storage setting `{other}`"
            )))
        }
    }
    save_config(bootstrap_path, &config)?;
    Ok(target)
}

/// Copies files and subdirectories from `from` into `to` without deleting `from`.
///
/// Skips `bootstrap.toml`, temporary atomic files (`*.tmp-*`), symlinks, and
/// `to` itself if `to` resides inside `from`.
pub fn copy_dir_contents(from: &Path, to: &Path) -> io::Result<CopyReport> {
    validate_writable_dir(to)?;
    if !from.exists() {
        return Ok(CopyReport::default());
    }
    let from_canon = fs::canonicalize(from).unwrap_or_else(|_| from.to_path_buf());
    let to_canon = fs::canonicalize(to).unwrap_or_else(|_| to.to_path_buf());
    if same_path(&from_canon, &to_canon) {
        return Ok(CopyReport::default());
    }
    let mut visited = BTreeSet::new();
    let mut report = CopyReport::default();
    copy_dir_recursive(&from_canon, &to_canon, &to_canon, &mut visited, &mut report)?;
    Ok(report)
}

fn copy_dir_recursive(
    from: &Path,
    to: &Path,
    skip_root: &Path,
    visited: &mut BTreeSet<PathBuf>,
    report: &mut CopyReport,
) -> io::Result<()> {
    if !visited.insert(from.to_path_buf()) {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == BOOTSTRAP_FILE || name_str.contains(".tmp-") {
            continue;
        }
        let src = entry.path();
        let src_canon = fs::canonicalize(&src).unwrap_or_else(|_| src.clone());
        if same_path(&src_canon, skip_root) {
            continue;
        }
        let dst = to.join(&name);
        if file_type.is_dir() {
            if !dst.exists() {
                fs::create_dir_all(&dst)?;
                report.dirs_created += 1;
            }
            copy_dir_recursive(&src_canon, &dst, skip_root, visited, report)?;
        } else if file_type.is_file() {
            let copied = fs::copy(&src, &dst)?;
            report.files_copied += 1;
            report.bytes_copied += copied;
        }
    }
    Ok(())
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |path: &Path| {
        let text = clean_path(path).display().to_string();
        let trimmed = text.trim_end_matches(['\\', '/']);
        if cfg!(windows) {
            trimmed.to_ascii_lowercase()
        } else {
            trimmed.to_owned()
        }
    };
    norm(a) == norm(b)
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-bootstrap-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn bootstrap_toml_overrides_defaults_and_env_keeps_precedence() {
        let dir = temp_dir("precedence");
        let bootstrap_path = dir.join("default-data").join(BOOTSTRAP_FILE);
        let custom_data = dir.join("custom-data");
        let custom_user = dir.join("custom-user");
        let default_data = dir.join("default-data");
        let default_user = dir.join("default-user");
        let env_data = dir.join("env-data");
        let env_user = dir.join("env-user");

        save_config(
            &bootstrap_path,
            &BootstrapConfig {
                data_dir: Some(custom_data.display().to_string()),
                content_dir: Some(custom_user.display().to_string()),
            },
        )
        .unwrap();

        // Without env overrides, bootstrap.toml takes effect.
        let resolved = resolve_with(
            &bootstrap_path,
            None,
            None,
            default_data.clone(),
            default_user.clone(),
        );
        assert!(resolved.warnings.is_empty(), "{:?}", resolved.warnings);
        assert_eq!(resolved.data_dir, custom_data);
        assert_eq!(resolved.user_dir, custom_user);

        // With env overrides, FORJA_DATA_DIR and FORJA_CONTENT_DIR win.
        let resolved = resolve_with(
            &bootstrap_path,
            Some(env_data.clone()),
            Some(env_user.clone()),
            default_data,
            default_user,
        );
        assert!(resolved.warnings.is_empty());
        assert_eq!(resolved.data_dir, env_data);
        assert_eq!(resolved.user_dir, env_user);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_or_unwritable_bootstrap_paths_fall_back_to_default_with_warnings() {
        let dir = temp_dir("invalid");
        let default_data = dir.join("default-data");
        let default_user = dir.join("default-user");
        let bootstrap_path = default_data.join(BOOTSTRAP_FILE);
        let file_not_dir = dir.join("not-a-directory");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&file_not_dir, "file").unwrap();

        save_config(
            &bootstrap_path,
            &BootstrapConfig {
                data_dir: Some("relative/data".to_owned()),
                content_dir: Some(file_not_dir.display().to_string()),
            },
        )
        .unwrap();

        let resolved = resolve_with(
            &bootstrap_path,
            None,
            None,
            default_data.clone(),
            default_user.clone(),
        );
        assert_eq!(resolved.data_dir, default_data);
        assert_eq!(resolved.user_dir, default_user);
        assert_eq!(resolved.warnings.len(), 2, "{:?}", resolved.warnings);

        // Corrupt bootstrap.toml also falls back safely with a warning.
        fs::write(&bootstrap_path, "data_dir = [unclosed").unwrap();
        let resolved = resolve_with(
            &bootstrap_path,
            None,
            None,
            default_data.clone(),
            default_user.clone(),
        );
        assert_eq!(resolved.data_dir, default_data);
        assert_eq!(resolved.user_dir, default_user);
        assert_eq!(resolved.warnings.len(), 1);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn copy_dir_contents_copies_without_moving_and_skips_bootstrap() {
        let dir = temp_dir("copy");
        let src = dir.join("src");
        let dst = dir.join("dst");
        fs::create_dir_all(src.join("modules")).unwrap();
        fs::write(src.join("settings.toml"), "a = 1\n").unwrap();
        fs::write(src.join("modules").join("notes.json"), "{}").unwrap();
        fs::write(src.join(BOOTSTRAP_FILE), "data_dir = \"x\"\n").unwrap();
        fs::write(src.join("settings.tmp-1-0"), "tmp").unwrap();

        let report = copy_dir_contents(&src, &dst).unwrap();
        assert_eq!(report.files_copied, 2);
        assert!(dst.join("settings.toml").is_file());
        assert!(dst.join("modules").join("notes.json").is_file());
        assert!(!dst.join(BOOTSTRAP_FILE).exists());
        assert!(!dst.join("settings.tmp-1-0").exists());
        // Source files remain untouched (copied, never moved).
        assert!(src.join("settings.toml").is_file());
        assert!(src.join("modules").join("notes.json").is_file());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expand_path_resolves_percent_env_vars_and_tilde() {
        let home = home_dir().expect("home_dir");
        assert_eq!(expand_path("~/.forja"), Some(home.join(".forja")));
        assert_eq!(expand_path("   "), None);
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let expected = PathBuf::from(appdata).join("Forja");
            assert_eq!(expand_path("%APPDATA%\\Forja"), Some(expected));
        }
    }
}
