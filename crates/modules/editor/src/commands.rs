//! File opening, saving, search, line edits and preview helpers.
//!
//! Everything that touches the disk, a process or another module's command runs
//! inside `cx.spawn`; only the tab bookkeeping happens on the UI thread.

use crate::model::{self, Encoding, Eol, FindState, SaveOptions};
use crate::state::Shared;
use forja_contracts::ViewerDocument;
use forja_kernel::{Args, Ctx, Result, SettingValue};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Images, notebooks, tables and PDFs are read-only; when the viewers module
/// is present it owns the tab. A line request means the caller wants text.
fn delegate_to_viewer(cx: &mut Ctx<'_>, path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(
        extension.as_str(),
        "csv" | "tsv" | "ipynb" | "pdf" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico"
    ) {
        return false;
    }
    let mut args = Args::new();
    args.insert("path", json!(path));
    match cx.run_command("viewers.open", args) {
        Ok(value) => value.get("opened").and_then(serde_json::Value::as_bool) == Some(true),
        Err(_) => false,
    }
}

pub fn active_path(cx: &Ctx<'_>, shared: &Shared, args: &Args) -> Result<String> {
    if let Some(path) = args.str("path") {
        return Ok(path.to_owned());
    }
    shared
        .active_tab()
        .map(|tab| tab.path)
        .ok_or_else(|| forja_kernel::KernelError::Message(cx.t("editor.error.no-tab")))
}

pub fn open_path(cx: &mut Ctx<'_>, shared: &Shared, path: &str, line: Option<i64>, pinned: bool) {
    if line.is_none() && delegate_to_viewer(cx, path) {
        shared.set_status(None);
        return;
    }
    let path = path.to_owned();
    let eol_setting = setting_str(cx, "editor.files.eol", "auto");
    let wrap = setting_str(cx, "editor.wrap.mode", "off") != "off";
    let large_mb = cx
        .setting("editor.files.large_mb")
        .and_then(SettingValue::as_i64)
        .unwrap_or(10)
        .clamp(1, 100) as u64;
    let state = shared.clone();
    let target = path.clone();
    cx.spawn(
        move || model::load_file_with_limit(Path::new(&target), &eol_setting, Some(large_mb)),
        move |cx, result| match result {
            Ok((text, eol, encoding)) => {
                state.open(path.clone(), text, eol, encoding, pinned);
                state.set_wrap(&path, wrap);
                if let Some(line) = line {
                    state.with(|state| state.pending_line = Some(line.max(1) as usize));
                }
                let mut args = Args::new();
                args.insert("path", json!(path));
                let _ = cx.run_command("find.record_open", args);
                // Opening from the explorer, search or any other panel must
                // bring the editor tab to the front. The shell command is
                // optional for headless runs.
                let mut focus = Args::new();
                focus.insert("view_id", json!("editor.tab"));
                let _ = cx.run_command("shell.tab.open", focus);
                state.set_status(None);
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
}

pub fn current_bench_root(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        })
}

pub fn save(cx: &mut Ctx<'_>, shared: &Shared, path: &str, root: PathBuf) {
    let Some(tab) = shared.tab(path) else {
        return;
    };
    let trim = setting_bool(cx, "editor.save.trim_trailing", false);
    let final_newline = setting_bool(cx, "editor.save.final_newline", true);
    let format = setting_bool(cx, "editor.save.format", false);
    let eol = match setting_str(cx, "editor.files.eol", "auto").as_str() {
        "lf" => Eol::Lf,
        "crlf" => Eol::Crlf,
        _ => tab.eol,
    };
    let encoding = match setting_str(cx, "editor.files.encoding", "auto").as_str() {
        "utf8" => Encoding::Utf8,
        "utf8_bom" => Encoding::Utf8Bom,
        "utf16le" => Encoding::Utf16Le,
        _ => tab.encoding,
    };
    let path = path.to_owned();
    let content = tab.content.clone();
    let state = shared.clone();
    let target = path.clone();
    cx.spawn(
        move || {
            let content = if format && target.to_ascii_lowercase().ends_with(".rs") {
                format_rust(&content).unwrap_or(content)
            } else {
                content
            };
            let result = model::save_file_in_root(
                &root,
                Path::new(&target),
                &content,
                &SaveOptions {
                    eol,
                    encoding,
                    trim_trailing: trim,
                    final_newline,
                },
            );
            (result, content)
        },
        move |cx, (result, content)| match result {
            Ok(_) => {
                let mtime = std::fs::metadata(&path)
                    .and_then(|meta| meta.modified())
                    .ok();
                state.mark_saved(&path, content, mtime);
                state.set_status(Some(cx.t("editor.status.saved")));
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
}

pub fn check_external(cx: &mut Ctx<'_>, shared: &Shared) {
    let watched: Vec<(String, std::time::SystemTime)> = shared
        .tabs()
        .into_iter()
        .filter_map(|tab| tab.last_mtime.map(|mtime| (tab.path, mtime)))
        .collect();
    if watched.is_empty() {
        return;
    }
    let state = shared.clone();
    cx.spawn(
        move || {
            let mut changed = Vec::new();
            for (path, previous) in watched {
                if let Ok(current) = std::fs::metadata(&path).and_then(|meta| meta.modified()) {
                    if current > previous {
                        changed.push(path);
                    }
                }
            }
            changed
        },
        move |_cx, changed| {
            for path in changed {
                state.set_external(&path, true);
            }
        },
    );
}

pub fn reload(cx: &mut Ctx<'_>, shared: &Shared, path: &str) {
    let eol_setting = setting_str(cx, "editor.files.eol", "auto");
    let path = path.to_owned();
    let state = shared.clone();
    let target = path.clone();
    cx.spawn(
        move || model::load_file(Path::new(&target), &eol_setting),
        move |cx, result| match result {
            Ok((text, eol, encoding)) => {
                state.with(|state| {
                    if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                        tab.reload(text, eol, encoding);
                    }
                });
                state.set_status(Some(cx.t("editor.status.reloaded")));
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
}

pub fn compare(cx: &mut Ctx<'_>, shared: &Shared, path: &str) {
    let Some(tab) = shared.tab(path) else {
        return;
    };
    let eol_setting = setting_str(cx, "editor.files.eol", "auto");
    let path = path.to_owned();
    let state = shared.clone();
    let target = path.clone();
    cx.spawn(
        move || model::load_file(Path::new(&target), &eol_setting),
        move |_cx, result| match result {
            Ok((disk, _, _)) => {
                let _ = state.with(|state| state.compare = Some((disk, tab.content)));
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
}

pub fn preview(cx: &mut Ctx<'_>, shared: &Shared, path: &str) {
    let lower = path.to_ascii_lowercase();
    let markdown = lower.ends_with(".md") || lower.ends_with(".markdown");
    if markdown && !setting_bool(cx, "editor.markdown.preview", true) {
        shared.set_status(Some(cx.t("editor.status.preview-off")));
        return;
    }
    let mut args = Args::new();
    args.insert("path", json!(path));
    match cx.run_command("viewers.preview", args) {
        Ok(value) => match serde_json::from_value::<ViewerDocument>(value) {
            Ok(document) => {
                shared.set_preview(path, Some(document));
                shared.set_status(None);
            }
            Err(error) => shared.set_status(Some(error.to_string())),
        },
        Err(_) => shared.set_status(Some(cx.t("editor.status.viewer-missing"))),
    }
}

pub fn format_active(cx: &mut Ctx<'_>, shared: &Shared, path: &str) {
    let Some(tab) = shared.tab(path) else {
        return;
    };
    if !path.to_ascii_lowercase().ends_with(".rs") {
        shared.set_status(Some(cx.t("editor.status.format-unsupported")));
        return;
    }
    let state = shared.clone();
    let path = path.to_owned();
    let content = tab.content;
    cx.spawn(
        move || format_rust(&content),
        move |cx, result| match result {
            Some(formatted) => {
                state.set_content(&path, formatted);
                state.set_status(Some(cx.t("editor.status.formatted")));
            }
            None => state.set_status(Some(cx.t("editor.status.format-failed"))),
        },
    );
}

pub fn open_external(cx: &mut Ctx<'_>, path: &str) {
    let path = path.to_owned();
    cx.spawn(
        move || {
            #[cfg(windows)]
            {
                Command::new("cmd")
                    .args(["/C", "start", "", &path])
                    .spawn()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
            #[cfg(not(windows))]
            {
                Command::new("xdg-open")
                    .arg(&path)
                    .spawn()
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        },
        |_cx, _result| {},
    );
}

/// Runs a pure text transformation on the active tab and reports the result.
pub fn edit_active<F>(cx: &mut Ctx<'_>, shared: &Shared, path: &str, change: F, message_key: &str)
where
    F: FnOnce(&str, usize) -> Option<String>,
{
    let Some(tab) = shared.tab(path) else {
        return;
    };
    let line = shared.cursor_line();
    match change(&tab.content, line) {
        Some(next) => {
            shared.set_content(path, next);
            shared.set_status(Some(cx.t(message_key)));
        }
        None => shared.set_status(Some(cx.t("editor.status.line-out-of-range"))),
    }
}

pub fn find(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<serde_json::Value> {
    let query = args.str("query").map(str::to_owned);
    let replacement = args.str("replacement").map(str::to_owned);
    let replace = args.bool("replace").unwrap_or(false);
    let path = active_path(cx, shared, args)?;
    shared.with(|state| {
        state.find.open = true;
        state.find.replace_open |= replace || replacement.is_some();
        if let Some(query) = query {
            state.find.query = query;
        }
        if let Some(replacement) = replacement {
            state.find.replacement = replacement;
        }
        if let Some(tab) = state.tabs.iter().find(|tab| tab.path == path) {
            state.find.refresh(&tab.content);
        }
    });
    Ok(json!({ "open": true }))
}

pub fn replace_all(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<serde_json::Value> {
    let path = active_path(cx, shared, args)?;
    let Some(tab) = shared.tab(&path) else {
        return Ok(json!({ "replaced": 0 }));
    };
    let find = args.str("find").map(str::to_owned).or_else(|| {
        shared
            .with(|state| {
                if state.find.query.is_empty() {
                    None
                } else {
                    Some(state.find.query.clone())
                }
            })
            .flatten()
    });
    let replacement = args.str("replace").map(str::to_owned).or_else(|| {
        shared
            .with(|state| Some(state.find.replacement.clone()))
            .flatten()
    });
    let mut state = FindState {
        regex: setting_bool(cx, "editor.search.regex", false),
        case_sensitive: setting_bool(cx, "editor.search.case_sensitive", false),
        whole_word: setting_bool(cx, "editor.search.whole_word", false),
        ..FindState::default()
    };
    if let Some(query) = find {
        state.query = query;
    }
    if let Some(replacement) = replacement {
        state.replacement = replacement;
    }
    let (replaced, count) = model::replace_all(&tab.content, &state);
    if count > 0 {
        shared.set_content(&path, replaced);
    }
    shared.set_status(Some(
        cx.t_args("editor.status.replaced", &[("count", &count.to_string())]),
    ));
    Ok(json!({ "replaced": count }))
}

pub fn wrap_toggle(cx: &Ctx<'_>, shared: &Shared, path: &str) -> bool {
    let wrap = shared.tab(path).map(|tab| !tab.wrap).unwrap_or(true);
    shared.set_wrap(path, wrap);
    shared.set_status(Some(if wrap {
        cx.t("editor.status.wrap-on")
    } else {
        cx.t("editor.status.wrap-off")
    }));
    wrap
}

pub fn comment_prefix(path: &str) -> &'static str {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "py" | "rb" | "sh" | "ps1" | "toml" | "yaml" | "yml" | "r" | "pl" => "#",
        "lua" | "sql" | "hs" => "--",
        _ => "//",
    }
}

fn format_rust(content: &str) -> Option<String> {
    let mut child = Command::new("rustfmt")
        .args(["--emit", "stdout", "--edition", "2021"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    child.stdin.as_mut()?.write_all(content.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout).ok()
    } else {
        None
    }
}

fn setting_str(cx: &Ctx<'_>, key: &str, fallback: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

fn setting_bool(cx: &Ctx<'_>, key: &str, fallback: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(fallback)
}
