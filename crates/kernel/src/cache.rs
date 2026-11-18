//! Age-based cleanup of module cache directories (`app.storage.cleanup_days`).
//!
//! Modules declare cache-only subdirectories under `data_dir` via
//! [`crate::Registrar::cache_dir`]. Once per startup, [`clean_stale_caches`]
//! removes files older than `cleanup_days` inside those declared folders on a
//! background task. User data directories are never touched; `0` disables
//! cleanup.

use crate::registry::CacheDirSpec;
use std::fs;
use std::path::{Component, Path};
use std::time::{Duration, SystemTime};

/// Summary of a cache cleanup pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheCleanupReport {
    pub dirs_scanned: usize,
    pub files_removed: usize,
    pub bytes_reclaimed: u64,
}

/// Validates that a module-declared cache path stays inside `data_dir`.
pub fn validate_cache_relative_path(relative: &str) -> Result<(), String> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return Err("cache directory path cannot be empty".to_owned());
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(format!("cache directory `{relative}` must be relative to data_dir"));
    }
    let mut has_normal = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => has_normal = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "cache directory `{relative}` cannot escape data_dir"
                ));
            }
        }
    }
    if !has_normal {
        return Err(format!("cache directory `{relative}` has no directory segment"));
    }
    Ok(())
}

/// Removes files older than `max_age_days` inside declared module cache folders.
///
/// `max_age_days == 0` is a no-op ("never clean").
pub fn clean_stale_caches(
    data_dir: &Path,
    cache_dirs: &[CacheDirSpec],
    max_age_days: u64,
    now: SystemTime,
) -> CacheCleanupReport {
    if max_age_days == 0 || cache_dirs.is_empty() {
        return CacheCleanupReport::default();
    }
    let max_age = Duration::from_secs(max_age_days.saturating_mul(86_400));
    let mut report = CacheCleanupReport::default();
    for spec in cache_dirs {
        if validate_cache_relative_path(&spec.relative_path).is_err() {
            continue;
        }
        let dir = data_dir.join(&spec.relative_path);
        if !dir.is_dir() {
            continue;
        }
        report.dirs_scanned += 1;
        prune_dir(&dir, max_age, now, &mut report, false);
    }
    report
}

fn prune_dir(
    dir: &Path,
    max_age: Duration,
    now: SystemTime,
    report: &mut CacheCleanupReport,
    remove_if_empty: bool,
) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let mut all_removed = true;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            all_removed = false;
            continue;
        };
        if file_type.is_symlink() {
            all_removed = false;
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            if !prune_dir(&path, max_age, now, report, true) {
                all_removed = false;
            }
            continue;
        }
        if !file_type.is_file() {
            all_removed = false;
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            all_removed = false;
            continue;
        };
        let timestamp = metadata.modified().or_else(|_| metadata.created());
        let Ok(modified) = timestamp else {
            all_removed = false;
            continue;
        };
        let expired = now
            .duration_since(modified)
            .is_ok_and(|age| age >= max_age);
        if expired {
            let len = metadata.len();
            if fs::remove_file(&path).is_ok() {
                report.files_removed += 1;
                report.bytes_reclaimed += len;
            } else {
                all_removed = false;
            }
        } else {
            all_removed = false;
        }
    }
    if remove_if_empty && all_removed {
        let _ = fs::remove_dir(dir);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-cache-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn rejects_escaping_or_absolute_cache_paths() {
        assert!(validate_cache_relative_path("cache/demo").is_ok());
        assert!(validate_cache_relative_path("../outside").is_err());
        assert!(validate_cache_relative_path("").is_err());
        assert!(validate_cache_relative_path(".").is_err());
        #[cfg(windows)]
        assert!(validate_cache_relative_path(r"C:\Temp\cache").is_err());
    }

    #[test]
    fn cleans_only_expired_files_in_declared_cache_dirs_and_respects_zero() {
        let dir = temp_dir("prune");
        let cache = dir.join("cache").join("demo");
        let nested = cache.join("sub");
        let user_data = dir.join("modules");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&user_data).unwrap();

        let old_file = nested.join("old.bin");
        let fresh_file = cache.join("fresh.bin");
        let state_file = user_data.join("demo.json");
        fs::write(&old_file, b"123456").unwrap();
        fs::write(&fresh_file, b"abc").unwrap();
        fs::write(&state_file, b"{}").unwrap();

        let now = SystemTime::now();
        let old_mtime = now - Duration::from_secs(40 * 86_400);
        let file = fs::OpenOptions::new().write(true).open(&old_file).unwrap();
        file.set_modified(old_mtime).unwrap();
        drop(file);

        let specs = vec![CacheDirSpec {
            module: "demo".to_owned(),
            relative_path: "cache/demo".to_owned(),
        }];

        // 0 means never clean.
        let zero = clean_stale_caches(&dir, &specs, 0, now);
        assert_eq!(zero.files_removed, 0);
        assert!(old_file.exists());

        // 30 days removes old_file and its empty subdir, keeping fresh_file and user_data.
        let report = clean_stale_caches(&dir, &specs, 30, now);
        assert_eq!(report.dirs_scanned, 1);
        assert_eq!(report.files_removed, 1);
        assert_eq!(report.bytes_reclaimed, 6);
        assert!(!old_file.exists());
        assert!(!nested.exists());
        assert!(fresh_file.exists());
        assert!(state_file.exists());

        let _ = fs::remove_dir_all(dir);
    }
}
