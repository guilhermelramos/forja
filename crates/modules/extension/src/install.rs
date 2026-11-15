//! Safe folder installation for user extensions.

use crate::manifest::{Layer, Manifest};
use crate::state::{ExtensionCore, InstallNotice};
use forja_kernel::Ctx;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(crate) enum InstallError {
    NotDirectory,
    MissingManifest,
    InvalidManifest(String),
    MissingEntry(String),
    AlreadyInstalled(String),
    SymbolicLink(String),
    Io(String),
}

/// Copies one validated extension into the user's content directory.
pub(crate) fn install_folder(
    source: &Path,
    extension_root: &Path,
) -> Result<PathBuf, InstallError> {
    if !source.is_dir() {
        return Err(InstallError::NotDirectory);
    }
    ensure_safe_tree(source)?;
    if !source.join("extension.toml").is_file() {
        return Err(InstallError::MissingManifest);
    }
    let manifest = Manifest::read(source, Layer::User).map_err(InstallError::InvalidManifest)?;
    if !source.join(&manifest.entry).is_file() {
        return Err(InstallError::MissingEntry(manifest.entry));
    }

    let target = extension_root.join(&manifest.id);
    match std::fs::symlink_metadata(extension_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(InstallError::SymbolicLink(
                extension_root.display().to_string(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    match std::fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(InstallError::SymbolicLink(target.display().to_string()));
        }
        Ok(_) => return Err(InstallError::AlreadyInstalled(manifest.id)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(error)),
    }
    std::fs::create_dir_all(extension_root).map_err(|error| InstallError::Io(error.to_string()))?;
    let stage = staging_path(extension_root, &manifest.id);
    std::fs::create_dir(&stage).map_err(|error| InstallError::Io(error.to_string()))?;
    if let Err(error) = copy_tree(source, &stage) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&stage, &target) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(InstallError::Io(error.to_string()));
    }
    Ok(target)
}

/// Copies a picked extension in a task, then asks the existing watcher to register it.
pub(crate) fn install_from_folder(core: &Arc<ExtensionCore>, cx: &mut Ctx<'_>, source: PathBuf) {
    core.with(|state| state.install_notice = None);
    let extension_root = cx.user_dir().join("extensions");
    let core = Arc::clone(core);
    cx.spawn(
        move || install_folder(&source, &extension_root),
        move |cx, result| match result {
            Ok(path) => {
                let path = path.display().to_string();
                let summary = cx.t_args("extension.page.installed", &[("path", path.as_str())]);
                core.with(|state| {
                    state.install_notice = Some(InstallNotice {
                        summary,
                        detail: None,
                        is_error: false,
                    });
                    state.dirty_at = Some(std::time::Instant::now());
                });
            }
            Err(error) => {
                let notice = install_error_notice(cx, error);
                core.with(|state| state.install_notice = Some(notice));
            }
        },
    );
}

fn install_error_notice(cx: &Ctx<'_>, error: InstallError) -> InstallNotice {
    let fallback = |raw: &str| {
        let human = forja_kernel::human_error::humanize_ctx(cx, raw);
        let detail = human
            .detail
            .map(|detail| format!("{}\n{detail}", human.summary))
            .or(Some(human.summary));
        InstallNotice {
            summary: cx.t("extension.install.error.failed"),
            detail,
            is_error: true,
        }
    };
    let summary = |key: &str| InstallNotice {
        summary: cx.t(key),
        detail: None,
        is_error: true,
    };
    match error {
        InstallError::NotDirectory => summary("extension.install.error.not-directory"),
        InstallError::MissingManifest => summary("extension.install.error.missing-manifest"),
        InstallError::InvalidManifest(raw) => fallback(&raw),
        InstallError::MissingEntry(entry) => InstallNotice {
            summary: cx.t_args(
                "extension.install.error.missing-entry",
                &[("entry", &entry)],
            ),
            detail: None,
            is_error: true,
        },
        InstallError::AlreadyInstalled(id) => InstallNotice {
            summary: cx.t_args("extension.install.error.exists", &[("id", &id)]),
            detail: None,
            is_error: true,
        },
        InstallError::SymbolicLink(path) => InstallNotice {
            summary: cx.t("extension.install.error.symbolic-link"),
            detail: Some(path),
            is_error: true,
        },
        InstallError::Io(raw) => fallback(&raw),
    }
}

fn ensure_safe_tree(root: &Path) -> Result<(), InstallError> {
    let metadata = std::fs::symlink_metadata(root).map_err(io_error)?;
    if metadata.file_type().is_symlink() {
        return Err(InstallError::SymbolicLink(root.display().to_string()));
    }
    for entry in std::fs::read_dir(root).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_symlink() {
            return Err(InstallError::SymbolicLink(path.display().to_string()));
        }
        if kind.is_dir() {
            ensure_safe_tree(&path)?;
        }
    }
    Ok(())
}

fn staging_path(root: &Path, id: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    root.join(format!(
        ".{id}.forja-installing-{}-{nanos}",
        std::process::id()
    ))
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), InstallError> {
    for entry in std::fs::read_dir(source).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_symlink() {
            return Err(InstallError::SymbolicLink(
                source_path.display().to_string(),
            ));
        }
        if kind.is_dir() {
            std::fs::create_dir(&target_path).map_err(io_error)?;
            copy_tree(&source_path, &target_path)?;
        } else if kind.is_file() {
            std::fs::copy(&source_path, &target_path).map_err(io_error)?;
        } else {
            return Err(InstallError::Io(format!(
                "unsupported extension entry: {}",
                source_path.display()
            )));
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> InstallError {
    InstallError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "forja-extension-install-{label}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn installs_a_valid_folder_and_refuses_overwriting_the_same_id() {
        let temp = TempDir::new("valid");
        let source = temp.0.join("source");
        let extension_root = temp.0.join("user").join("extensions");
        std::fs::create_dir_all(source.join("assets")).unwrap();
        std::fs::write(
            source.join("extension.toml"),
            "id = \"sample-addon\"\nname = \"Sample\"\nversion = \"1.0.0\"\n",
        )
        .unwrap();
        std::fs::write(source.join("init.lua"), "-- extension\n").unwrap();
        std::fs::write(source.join("assets/data.txt"), "ready\n").unwrap();

        let installed = install_folder(&source, &extension_root).expect("extension installs");
        assert_eq!(installed, extension_root.join("sample-addon"));
        assert_eq!(
            std::fs::read_to_string(installed.join("assets/data.txt")).unwrap(),
            "ready\n"
        );
        assert!(matches!(
            install_folder(&source, &extension_root),
            Err(InstallError::AlreadyInstalled(id)) if id == "sample-addon"
        ));
    }

    #[test]
    fn refuses_a_folder_without_an_extension_manifest() {
        let temp = TempDir::new("missing");
        assert!(matches!(
            install_folder(&temp.0, &temp.0.join("user/extensions")),
            Err(InstallError::MissingManifest)
        ));
    }
}
