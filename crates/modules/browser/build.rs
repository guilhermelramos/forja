//! Copies WebView2Loader.dll next to the built executable.
//!
//! `webview2-com-sys` emits the loader into its own build directory; wry loads
//! it at runtime by name. Cargo does not copy it, so the browser crate does.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("..").join("..").join("..").join("target"));
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86"
    };
    let build = target.join(&profile).join("build");
    let Ok(entries) = std::fs::read_dir(&build) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("webview2-com-sys-") {
            continue;
        }
        let candidate = entry
            .path()
            .join("out")
            .join(arch)
            .join("WebView2Loader.dll");
        if candidate.is_file() {
            let _ = copy_if_newer(&candidate, &target.join(profile).join("WebView2Loader.dll"));
            break;
        }
    }
}

fn copy_if_newer(from: &Path, to: &Path) -> std::io::Result<()> {
    if let (Ok(source), Ok(destination)) = (from.metadata(), to.metadata()) {
        if destination.modified()? >= source.modified()? {
            return Ok(());
        }
    }
    std::fs::copy(from, to).map(|_| ())
}
