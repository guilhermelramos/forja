//! `cargo xtask installer`: builds `dist/Forja-Setup-<versão>.exe`.
//!
//! Runs `package` first (unless `--skip-build`), then compiles
//! `installer/forja.iss` with Inno Setup 6 (`ISCC.exe`). Only `dist/` is written.

use crate::package;
use crate::util::repo_root;
use std::path::PathBuf;

pub fn run(skip_build: bool) -> Result<(), String> {
    package::run(skip_build)?;
    let root = repo_root();
    let version = package::workspace_version(&root)?;
    let iscc = find_iscc().ok_or_else(|| {
        "ISCC.exe (Inno Setup 6) not found; install it with \
         `winget install JRSoftware.InnoSetup`"
            .to_owned()
    })?;
    let status = std::process::Command::new(&iscc)
        .arg("/Q")
        .arg(format!("/DAppVersion={version}"))
        .arg(root.join("installer").join("forja.iss"))
        .status()
        .map_err(|error| format!("{}: {error}", iscc.display()))?;
    if !status.success() {
        return Err(format!("ISCC.exe failed with {status}"));
    }
    let setup = root
        .join("dist")
        .join(format!("Forja-Setup-{version}.exe"));
    let size = std::fs::metadata(&setup)
        .map_err(|error| format!("{}: {error}", setup.display()))?
        .len();
    println!("Installer: {}", setup.display());
    println!("Size:      {:.2} MB", size as f64 / 1_048_576.0);
    Ok(())
}

/// Looks for `ISCC.exe` on PATH and in the default per-user and machine installs.
fn find_iscc() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join("ISCC.exe"))
                .collect()
        })
        .unwrap_or_default();
    for var in ["LOCALAPPDATA", "ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(base) = std::env::var_os(var) {
            let base = PathBuf::from(base);
            let base = if var == "LOCALAPPDATA" {
                base.join("Programs")
            } else {
                base
            };
            candidates.push(base.join("Inno Setup 6").join("ISCC.exe"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}
