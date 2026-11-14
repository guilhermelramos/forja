//! Commands for the diff tab: change list, file diff and review notes.

use crate::model::{self, Changes, FileDiff};
use crate::state::Shared;
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{Value, json};

pub fn bench_path(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<String> {
    if let Some(path) = args.str("path") {
        shared.set_bench(Some(path.to_owned()));
        return Ok(path.to_owned());
    }
    if let Some(path) = shared.bench() {
        return Ok(path);
    }
    let value = cx
        .run_command("bench.current", Args::new())
        .map_err(kernel_error)?;
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| KernelError::Message(cx.t("diff.error.no-bench")))?
        .to_owned();
    shared.set_bench(Some(path.clone()));
    Ok(path)
}

pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let path = bench_path(cx, shared, args)?;
    let comparison = shared.comparison();
    let changes = if let Some(range) = &comparison {
        let base = comparison_base(range)
            .ok_or_else(|| KernelError::Message(cx.t("diff.error.comparison")))?;
        let mut result_args = Args::new();
        result_args.insert("path", json!(path));
        let result = cx
            .run_command("git.compare.result", result_args)
            .map_err(kernel_error)?;
        if let Some(error) = result.get("error").and_then(Value::as_str) {
            shared.set_status(Some(error.to_owned()));
            return Ok(json!({ "pending": false, "files": 0 }));
        }
        let report = result
            .get("comparison")
            .filter(|value| !value.is_null())
            .cloned()
            .map(serde_json::from_value::<ComparisonChanges>)
            .transpose()
            .map_err(app_error)?;
        let report = match report.filter(|report| report.base.as_deref() == Some(base)) {
            Some(report) => report,
            None => {
                let loading = result
                    .get("loading")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if !loading {
                    let mut compare_args = Args::new();
                    compare_args.insert("path", json!(path));
                    compare_args.insert("base", json!(base));
                    cx.run_command("git.compare", compare_args)
                        .map_err(kernel_error)?;
                }
                shared.set_stale(true);
                return Ok(json!({ "pending": true }));
            }
        };
        Changes {
            files: report
                .files
                .into_iter()
                .map(|file| model::GitFile {
                    path: file.path,
                    comparison: true,
                    ..model::GitFile::default()
                })
                .collect(),
            upstream: None,
            detached: false,
            comparison: Some(range.clone()),
        }
    } else {
        let mut changes_args = Args::new();
        changes_args.insert("path", json!(path));
        let value = cx
            .run_command("git.changes", changes_args)
            .map_err(kernel_error)?;
        serde_json::from_value(value).map_err(app_error)?
    };
    shared.set_changes(changes.clone());
    let selected = shared.with(|state| state.selected.clone()).flatten();
    let staged = shared.with(|state| state.staged).unwrap_or(false);
    let target = selected
        .filter(|selected| changes.files.iter().any(|file| &file.path == selected))
        .or_else(|| changes.files.first().map(|file| file.path.clone()));
    if let Some(file) = target {
        select(cx, shared, &path, &file, staged, comparison.as_deref())?;
    }
    shared.set_stale(false);
    Ok(json!({
        "files": changes.files.len(),
        "detached": changes.detached,
        "upstream": changes.upstream,
        "comparison": changes.comparison
    }))
}

pub fn select(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    path: &str,
    file: &str,
    staged: bool,
    comparison: Option<&str>,
) -> Result<Value> {
    let mut args = Args::new();
    args.insert("path", json!(path));
    args.insert("file", json!(file));
    args.insert("staged", json!(staged));
    if let Some(comparison) = comparison {
        args.insert("comparison", json!(comparison));
    }
    let value = cx.run_command("git.diff", args).map_err(kernel_error)?;
    let diff: FileDiff = serde_json::from_value(value).map_err(app_error)?;
    shared.set_diff(diff.clone());
    shared.set_status(None);
    Ok(json!({
        "file": diff.path,
        "binary": diff.binary,
        "hunks": diff.hunks.len()
    }))
}

pub fn open(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let path = bench_path(cx, shared, args)?;
    let file = args.str("file").map(str::to_owned);
    let staged = args.bool("staged").unwrap_or(false);
    if let Some(comparison) = args.str("comparison") {
        if comparison_base(comparison).is_none() {
            return Err(KernelError::Message(cx.t("diff.error.comparison")));
        }
        if shared.comparison().as_deref() != Some(comparison) {
            shared.set_comparison(Some(comparison.to_owned()));
        }
    } else if file.is_none() {
        shared.set_comparison(None);
    }
    let comparison = shared.comparison();
    if let Some(file) = file {
        select(cx, shared, &path, &file, staged, comparison.as_deref())?;
        return Ok(json!({ "open": true }));
    }
    refresh(cx, shared, args)
}

pub fn comparison_base(range: &str) -> Option<&str> {
    let (base, head) = range.split_once("...")?;
    (head == "HEAD" && !base.trim().is_empty()).then_some(base.trim())
}

#[derive(serde::Deserialize)]
struct ComparisonChanges {
    base: Option<String>,
    #[serde(default)]
    files: Vec<ComparisonFile>,
}

#[derive(serde::Deserialize)]
struct ComparisonFile {
    path: String,
}

pub fn send_notes(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let notes = shared.notes();
    if notes.is_empty() {
        shared.set_notice(Some(cx.t("diff.notes.empty")));
        return Ok(json!({ "sent": false }));
    }
    let bench = shared.bench().unwrap_or_default();
    let terminal = match args.str("terminal_id") {
        Some(id) => id.to_owned(),
        None => match pick_terminal(cx, &bench) {
            Some(id) => id,
            None => {
                shared.set_notice(Some(cx.t("diff.notes.no-terminal")));
                return Ok(json!({ "sent": false }));
            }
        },
    };
    let send_mode = cx
        .setting("diff.notes.send_mode")
        .and_then(forja_kernel::SettingValue::as_str)
        .unwrap_or("append");
    let intro = match send_mode {
        "replace" => cx.t("diff.notes.intro.replace"),
        "ask" => cx.t("diff.notes.intro.ask"),
        _ => cx.t("diff.notes.intro.append"),
    };
    let heading = cx.t("diff.notes.heading");
    let body = model::format_notes(&notes, &heading);
    let text = if body.is_empty() {
        intro
    } else {
        format!("{intro}\n\n{body}")
    };
    let mut send = Args::new();
    send.insert("terminal_id", json!(terminal));
    send.insert("text", json!(text));
    send.insert("enter", json!(true));
    cx.run_command("terminal.send", send)
        .map_err(kernel_error)?;
    shared.set_notice(Some(cx.t("diff.notes.sent")));
    Ok(json!({ "sent": true, "terminal_id": terminal }))
}

fn pick_terminal(cx: &mut Ctx<'_>, bench: &str) -> Option<String> {
    let mut args = Args::new();
    args.insert("bench_path", json!(bench));
    if let Ok(value) = cx.run_command("helper.list", args.clone()) {
        if let Some(id) = find_terminal_id(&value) {
            return Some(id);
        }
    }
    let value = cx.run_command("terminal.list", args).ok()?;
    find_terminal_id(&value)
}

fn find_terminal_id(value: &Value) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(id)) = map.get("terminal_id") {
                return Some(id.clone());
            }
            map.values().find_map(find_terminal_id)
        }
        Value::Array(items) => items.iter().find_map(find_terminal_id),
        _ => None,
    }
}

pub fn kernel_error(error: KernelError) -> KernelError {
    error
}

fn app_error(error: serde_json::Error) -> KernelError {
    KernelError::Message(error.to_string())
}
