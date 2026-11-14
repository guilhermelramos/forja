//! Image before/after for binary diffs.
//!
//! `git.diff` reports images as binary, so the tab reads the worktree file and
//! asks git for the committed blob (`:file` when staged, `HEAD:file` otherwise).
//! Both reads run in a task; only texture upload happens on the UI thread.

use forja_ui_kit::egui::ColorImage;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct Pair {
    pub old: Option<ColorImage>,
    pub new: Option<ColorImage>,
    pub old_bytes: usize,
    pub new_bytes: usize,
}

pub fn is_image(path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico"
    )
}

pub fn load(bench: &str, file: &str, staged: bool) -> Pair {
    let current = std::fs::read(Path::new(bench).join(file)).ok();
    let spec = if staged {
        format!(":{file}")
    } else {
        format!("HEAD:{file}")
    };
    let old = Command::new("git")
        .arg("-C")
        .arg(bench)
        .args(["show", &spec])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout);
    Pair {
        old: old.as_deref().and_then(decode),
        new: current.as_deref().and_then(decode),
        old_bytes: old.map(|bytes| bytes.len()).unwrap_or(0),
        new_bytes: current.map(|bytes| bytes.len()).unwrap_or(0),
    }
}

pub fn load_comparison(bench: &str, file: &str, base: &str) -> Pair {
    let base_ref = format!("{base}^{{commit}}");
    let old_ref = git_output(
        bench,
        &["rev-parse", "--verify", "--end-of-options", &base_ref],
    )
    .ok()
    .and_then(|hash| String::from_utf8(hash).ok())
    .map(|hash| format!("{}:{file}", hash.trim()));
    let new_ref = git_output(bench, &["rev-parse", "--verify", "HEAD^{commit}"])
        .ok()
        .and_then(|hash| String::from_utf8(hash).ok())
        .map(|hash| format!("{}:{file}", hash.trim()));
    let old = old_ref
        .as_deref()
        .and_then(|reference| git_output(bench, &["show", reference]).ok());
    let new = new_ref
        .as_deref()
        .and_then(|reference| git_output(bench, &["show", reference]).ok());
    Pair {
        old: old.as_deref().and_then(decode),
        new: new.as_deref().and_then(decode),
        old_bytes: old.as_ref().map(Vec::len).unwrap_or(0),
        new_bytes: new.as_ref().map(Vec::len).unwrap_or(0),
    }
}

fn git_output(bench: &str, args: &[&str]) -> Result<Vec<u8>, ()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(bench)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| ())?;
    output.status.success().then_some(output.stdout).ok_or(())
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [decoded.width() as usize, decoded.height() as usize],
        decoded.as_raw(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_extensions_are_detected() {
        assert!(is_image("logo.PNG"));
        assert!(is_image("a.gif"));
        assert!(!is_image("a.rs"));
    }
}
