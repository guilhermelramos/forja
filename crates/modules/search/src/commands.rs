//! Behavior of the search panel: starting and stopping searches, streaming results,
//! the replace preview and apply, and opening a hit.
//!
//! The walk runs on a worker thread (see `engine`). Its batches reach the panel
//! through a chain of kernel tasks, one batch at a time. The preview and the apply
//! run as kernel tasks too, so the interface thread never reads or writes files.

use crate::engine::{self, Msg, Plan};
use crate::platform;
use crate::replace::{self, ApplyError, ReplacePlan};
use crate::state::{lock, Form, Shared};
use forja_contracts::{BenchRef, FileOpenRequested};
use forja_kernel::{Args, Ctx, KernelError, Result as KResult, SettingValue};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the panel asks which bench is active.
const BENCH_CHECK_EVERY: Duration = Duration::from_millis(750);
/// Messages applied per UI-thread callback, so a burst does not block a frame.
const MSG_PER_CALLBACK: usize = 64;

/// Reads the walk settings from the catalog values (owned by the `find` module).
fn walk_settings(cx: &Ctx<'_>) -> (usize, bool, bool) {
    let context = cx
        .setting("find.grep.context")
        .and_then(|value| value.as_i64())
        .unwrap_or(2)
        .clamp(0, 10);
    let include_hidden = cx
        .setting("find.search.include_hidden")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let respect_gitignore = cx
        .setting("find.search.respect_gitignore")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    (
        usize::try_from(context).unwrap_or(2),
        include_hidden,
        respect_gitignore,
    )
}

/// Follows the active bench (throttled). A new root clears the results.
pub fn sync_root(cx: &mut Ctx<'_>, shared: &Shared) {
    let read = {
        let mut state = lock(shared);
        let due = state
            .bench_checked
            .is_none_or(|at| at.elapsed() >= BENCH_CHECK_EVERY);
        let wanted = state.bench_checked.is_none() || state.root.is_none();
        if due && wanted {
            state.bench_checked = Some(Instant::now());
            true
        } else {
            false
        }
    };
    if read {
        let bench = cx
            .run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| serde_json::from_value::<BenchRef>(value).ok());
        apply_root(shared, bench.map(|bench| PathBuf::from(bench.path)));
    }
}

/// Handles `bench.activated`: the panel now searches that bench.
pub fn on_bench_activated(shared: &Shared, bench: &BenchRef) {
    apply_root(shared, Some(PathBuf::from(&bench.path)));
}

/// Targets a specific root folder directly.
pub fn set_root(shared: &Shared, root: PathBuf) {
    apply_root(shared, Some(root));
}

/// Returns the current search state and matching files as JSON.
pub fn results_snapshot(shared: &Shared) -> Value {
    let state = lock(shared);
    let files: Vec<Value> = state
        .files
        .iter()
        .map(|file| {
            let hits: Vec<Value> = file
                .hits
                .iter()
                .map(|hit| json!({ "line": hit.line }))
                .collect();
            json!({
                "rel": file.rel,
                "path": file.path.display().to_string(),
                "hits": hits,
            })
        })
        .collect();
    json!({
        "running": state.running,
        "searched": state.searched,
        "matches": state.matches,
        "scanned": state.scanned,
        "truncated": state.truncated,
        "error": state.error,
        "notice": state.notice,
        "files": files,
    })
}

/// Sets the root. A different root stops a running walk and clears the results and the preview.
fn apply_root(shared: &Shared, root: Option<PathBuf>) {
    let mut state = lock(shared);
    if state.root == root {
        return;
    }
    state.root = root;
    if let Some(cancel) = state.cancel.take() {
        cancel.store(true, Ordering::Relaxed);
    }
    state.generation += 1;
    state.running = false;
    state.files.clear();
    state.matches = 0;
    state.plan = None;
    state.plan_form = None;
    state.searched = false;
}

/// True when some module subscribes to `file.open_requested` (an editor).
/// Declaring the event is not enough: every publisher declares it.
fn has_file_listener(cx: &Ctx<'_>) -> bool {
    let type_id = std::any::TypeId::of::<FileOpenRequested>();
    cx.registry()
        .subscriptions()
        .iter()
        .any(|subscription| subscription.type_id == type_id)
}

/// Replaces the form. Any change makes an older preview stale.
pub fn set_form(shared: &Shared, form: Form) {
    let mut state = lock(shared);
    if state.form != form {
        state.form = form;
        state.confirm = false;
    }
}

/// Starts a search with the current form, cancelling any search still running.
pub fn start_search(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    let (context, include_hidden, respect_gitignore) = walk_settings(cx);
    let (root, options, generation, cancel) = {
        let mut state = lock(shared);
        let Some(root) = state.root.clone() else {
            state.notice = Some(cx.t("search.notice.no_bench"));
            return Ok(json!({ "started": false }));
        };
        if let Some(previous) = state.cancel.take() {
            previous.store(true, Ordering::Relaxed);
        }
        state.generation += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        state.cancel = Some(Arc::clone(&cancel));
        state.running = true;
        state.searched = true;
        state.files.clear();
        state.matches = 0;
        state.scanned = 0;
        state.truncated = false;
        state.error = None;
        state.notice = None;
        (root, state.form.options(), state.generation, cancel)
    };
    if options.is_empty() {
        let mut state = lock(shared);
        state.running = false;
        state.notice = Some(cx.t("search.notice.empty"));
        return Ok(json!({ "started": false }));
    }
    let engine = match cx.setting("find.grep.engine").and_then(SettingValue::as_str) {
        Some("builtin") => crate::engine::Engine::Builtin,
        _ => crate::engine::Engine::Auto,
    };
    let plan = Plan {
        root,
        options,
        context,
        include_hidden,
        respect_gitignore,
        engine,
    };
    let (sender, receiver) = channel::<Msg>();
    let spawned = std::thread::Builder::new()
        .name("forja-search".to_owned())
        .spawn(move || engine::run(&plan, &sender, &cancel));
    if spawned.is_err() {
        let mut state = lock(shared);
        state.running = false;
        state.error = Some(cx.t("search.notice.thread"));
        return Ok(json!({ "started": false }));
    }
    pump(cx, receiver, Arc::clone(shared), generation);
    Ok(json!({ "started": true }))
}

/// Stops the running walk. Results so far stay on screen.
pub fn stop_search(shared: &Shared) {
    let mut state = lock(shared);
    if let Some(cancel) = &state.cancel {
        cancel.store(true, Ordering::Relaxed);
    }
    state.running = false;
}

/// Waits for the next batch of messages from the walk, then applies it, then waits again.
fn pump(cx: &Ctx<'_>, receiver: Receiver<Msg>, shared: Shared, generation: u64) {
    cx.spawn(
        move || {
            let mut batch = Vec::new();
            let alive = match receiver.recv() {
                Ok(first) => {
                    batch.push(first);
                    while batch.len() < MSG_PER_CALLBACK {
                        match receiver.try_recv() {
                            Ok(next) => batch.push(next),
                            Err(_) => break,
                        }
                    }
                    true
                }
                Err(_) => false,
            };
            (receiver, batch, alive)
        },
        move |cx, (receiver, batch, alive): (Receiver<Msg>, Vec<Msg>, bool)| {
            let mut finished = false;
            let current = {
                let mut state = lock(&shared);
                if state.generation != generation {
                    false
                } else {
                    for message in batch {
                        match message {
                            Msg::Files(files) => state.files.extend(files),
                            Msg::Finished {
                                scanned,
                                matches,
                                truncated,
                                error,
                            } => {
                                finished = true;
                                state.running = false;
                                state.scanned = scanned;
                                state.matches = matches;
                                state.truncated = truncated;
                                state.error = error;
                            }
                        }
                    }
                    true
                }
            };
            if alive && current && !finished {
                pump(cx, receiver, shared, generation);
            }
        },
    );
}

/// Computes the replacement preview from disk, in a task.
pub fn preview(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    let (_, include_hidden, respect_gitignore) = walk_settings(cx);
    let (root, form) = {
        let mut state = lock(shared);
        let Some(root) = state.root.clone() else {
            state.notice = Some(cx.t("search.notice.no_bench"));
            return Ok(json!({ "started": false }));
        };
        if state.form.pattern.is_empty() {
            state.notice = Some(cx.t("search.notice.empty"));
            return Ok(json!({ "started": false }));
        }
        if state.planning || state.applying {
            return Ok(json!({ "started": false }));
        }
        state.planning = true;
        state.confirm = false;
        state.plan = None;
        state.plan_error = None;
        (root, state.form.clone())
    };
    let options = form.options();
    let replacement = form.replacement.clone();
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || {
            replace::plan(
                &root,
                &options,
                &replacement,
                include_hidden,
                respect_gitignore,
            )
        },
        move |_cx, result: Result<ReplacePlan, String>| {
            let mut state = lock(&done_shared);
            state.planning = false;
            match result {
                Ok(plan) => {
                    state.plan = Some(plan);
                    state.plan_form = Some(form);
                }
                Err(error) => {
                    state.plan = None;
                    state.plan_form = None;
                    state.plan_error = Some(error);
                }
            }
        },
    );
    Ok(json!({ "started": true }))
}

/// Writes the previewed replacement. Only runs after the user confirmed once.
/// The files are checked again on disk before anything is written.
pub fn apply(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    let plan = {
        let mut state = lock(shared);
        let form_matches = state.plan_form.as_ref() == Some(&state.form);
        let Some(plan) = state.plan.clone().filter(|_| form_matches) else {
            state.notice = Some(cx.t("search.notice.no_preview"));
            return Ok(json!({ "applied": false }));
        };
        if !state.confirm {
            state.confirm = true;
            return Ok(json!({ "applied": false, "confirm": true }));
        }
        if state.applying {
            return Ok(json!({ "applied": false }));
        }
        state.applying = true;
        state.confirm = false;
        plan
    };
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || replace::apply(&plan),
        move |cx, result: Result<replace::ApplyReport, ApplyError>| {
            let text = match &result {
                Ok(report) => cx.t_args(
                    "search.notice.replaced",
                    &[
                        ("files", &report.files.to_string()),
                        ("matches", &report.matches.to_string()),
                    ],
                ),
                Err(ApplyError::Changed(files)) => {
                    cx.t_args("search.notice.changed", &[("files", &files.join(", "))])
                }
                Err(ApplyError::Io {
                    message,
                    written,
                    restored,
                }) => {
                    let clean = forja_kernel::human_error::humanize_ctx(cx, message).summary;
                    cx.t_args(
                        "search.notice.io",
                        &[
                            ("message", clean.as_str()),
                            ("written", &written.to_string()),
                            ("restored", &restored.to_string()),
                        ],
                    )
                }
            };
            {
                let mut state = lock(&done_shared);
                state.applying = false;
                state.plan = None;
                state.plan_form = None;
                state.notice = Some(text);
            }
            if result.is_ok() {
                // The files changed, so the results on screen are out of date.
                let _ = start_search(cx, &done_shared);
            }
        },
    );
    Ok(json!({ "applied": true }))
}

/// Asks the editor to open `path` at `line`: emits `file.open_requested` when a module listens
/// to it. Otherwise runs the `file.open` command, and without that the system's default app.
pub fn open_hit(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf, line: u32) {
    let event = FileOpenRequested {
        path: path.to_string_lossy().into_owned(),
        line: Some(line),
        column: None,
    };
    if has_file_listener(cx) {
        cx.emit(&event);
        return;
    }
    let mut args = Args::new();
    args.insert("path", json!(event.path));
    args.insert("line", json!(line));
    match cx.run_command("file.open", args) {
        Ok(_) => {}
        Err(KernelError::UnknownCommand(_)) => {
            let done_shared = Arc::clone(shared);
            cx.spawn(
                move || platform::open_default(&path),
                move |cx, result: std::io::Result<()>| {
                    if let Err(error) = result {
                        let clean =
                            forja_kernel::human_error::humanize_ctx(cx, &error.to_string()).summary;
                        let text = cx.t_args("search.notice.error", &[("message", &clean)]);
                        lock(&done_shared).notice = Some(text);
                    }
                },
            );
        }
        Err(error) => {
            let clean = forja_kernel::human_error::humanize_ctx(cx, &error.to_string()).summary;
            lock(shared).notice = Some(clean);
        }
    }
}

/// The `search.focus` command: asks the panel to focus its query box.
pub fn focus(shared: &Shared) -> KResult<Value> {
    lock(shared).focus_query = true;
    Ok(json!({ "focused": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_changes_clear_the_confirmation() {
        let shared: Shared = Arc::new(std::sync::Mutex::new(crate::state::State::default()));
        lock(&shared).confirm = true;
        set_form(
            &shared,
            Form {
                pattern: "x".into(),
                ..Form::default()
            },
        );
        assert!(!lock(&shared).confirm);
        // Setting the same form again keeps the state as it is.
        lock(&shared).confirm = true;
        set_form(
            &shared,
            Form {
                pattern: "x".into(),
                ..Form::default()
            },
        );
        assert!(lock(&shared).confirm);
    }
}
