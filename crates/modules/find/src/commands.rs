//! Behavior of the finder: bench sync, index and history tasks, ranking, and opening a result.
//!
//! Disk work (indexing, reading and writing the history, opening files) runs in
//! kernel tasks. Ranking also runs in a task, so typing never waits for a scan.

use crate::fuzzy;
use crate::history::History;
use crate::index::{self, Index, Options};
use crate::platform;
use crate::state::{lock, Shared, Which};
use forja_contracts::{BenchRef, FileOpenRequested};
use forja_kernel::{Args, Ctx, KernelError, Result as KResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How often the finder asks which bench is active.
const BENCH_CHECK_EVERY: Duration = Duration::from_millis(750);
/// A listing older than this is rebuilt the next time the picker opens.
const INDEX_MAX_AGE: Duration = Duration::from_secs(60);

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Reads the index settings from the catalog values.
pub fn index_options(cx: &Ctx<'_>) -> Options {
    Options {
        respect_gitignore: cx
            .setting("find.search.respect_gitignore")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        include_hidden: cx
            .setting("find.search.include_hidden")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        cap: index::FILE_CAP,
    }
}

/// `(result limit, recent count)` from the catalog values.
pub fn limits(cx: &Ctx<'_>) -> (usize, usize) {
    let limit = cx
        .setting("find.results.limit")
        .and_then(|value| value.as_i64())
        .unwrap_or(200);
    let recent = cx
        .setting("find.recent.count")
        .and_then(|value| value.as_i64())
        .unwrap_or(20);
    (
        usize::try_from(limit.clamp(50, 1_000)).unwrap_or(200),
        usize::try_from(recent.clamp(5, 50)).unwrap_or(20),
    )
}

/// Reads the active bench once at start, and again while none is known (one may appear
/// later). Later changes arrive as `bench.activated`, handled by [`on_bench_activated`].
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

/// Handles `bench.activated`: the finder now searches that bench.
pub fn on_bench_activated(shared: &Shared, bench: &BenchRef) {
    apply_root(shared, Some(PathBuf::from(&bench.path)));
}

/// Sets the root. A different root drops the old listing, so the next picker opening rebuilds it.
fn apply_root(shared: &Shared, root: Option<PathBuf>) {
    let mut state = lock(shared);
    if state.root != root {
        state.root = root;
        state.index = None;
        state.indexed_at = None;
    }
}

/// True when some module subscribes to `file.open_requested` (an editor).
/// Declaring the event is not enough: every publisher declares it.
pub fn has_file_listener(cx: &Ctx<'_>) -> bool {
    let type_id = std::any::TypeId::of::<FileOpenRequested>();
    cx.registry()
        .subscriptions()
        .iter()
        .any(|subscription| subscription.type_id == type_id)
}

/// Starts an index build when there is none, it is for another root, or it is old.
pub fn ensure_index(cx: &Ctx<'_>, shared: &Shared) {
    let options = index_options(cx);
    let root = {
        let mut state = lock(shared);
        let Some(root) = state.root.clone() else {
            return;
        };
        let current = state.index.as_ref().is_some_and(|index| index.root == root);
        let fresh = state
            .indexed_at
            .is_some_and(|at| at.elapsed() < INDEX_MAX_AGE);
        if state.indexing || (current && fresh) {
            return;
        }
        state.indexing = true;
        root
    };
    let work_root = root.clone();
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || index::build(&work_root, options),
        move |cx, built: Index| {
            let refresh = {
                let mut state = lock(&done_shared);
                state.indexing = false;
                if state.root.as_ref() == Some(&root) {
                    state.index = Some(Arc::new(built));
                    state.indexed_at = Some(Instant::now());
                    true
                } else {
                    false
                }
            };
            if refresh {
                let query = lock(&done_shared).picker.query.clone();
                let anvil_query = lock(&done_shared).anvil.query.clone();
                // The picker always re-ranks: an empty query shows recent files, which need the index.
                request_rank(cx, &done_shared, Which::Picker, query);
                if !anvil_query.is_empty() {
                    request_rank(cx, &done_shared, Which::Anvil, anvil_query);
                }
            }
        },
    );
}

/// Loads the saved history once. The read runs in a task.
pub fn ensure_history(cx: &Ctx<'_>, shared: &Shared) {
    {
        let mut state = lock(shared);
        if state.history_loaded {
            return;
        }
        state.history_loaded = true;
    }
    let path = cx.store().path();
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || {
            std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<History>(&text).ok())
        },
        move |_cx, loaded: Option<History>| {
            let Some(loaded) = loaded else {
                return;
            };
            let mut state = lock(&done_shared);
            // Opens that happened before the load finished are merged in.
            let meanwhile = std::mem::replace(&mut state.history, loaded);
            for (root, entries) in meanwhile.roots {
                for (rel, usage) in entries {
                    let target = state
                        .history
                        .roots
                        .entry(root.clone())
                        .or_default()
                        .entry(rel)
                        .or_default();
                    target.count = target.count.saturating_add(usage.count);
                    target.last = target.last.max(usage.last);
                }
            }
        },
    );
}

/// Opens the picker: loads history, refreshes the index if needed, and ranks the current query.
pub fn open_picker(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    {
        let mut state = lock(shared);
        state.open = true;
        state.focus_query = true;
        state.notice = None;
    }
    ensure_history(cx, shared);
    sync_root(cx, shared);
    ensure_index(cx, shared);
    let query = lock(shared).picker.query.clone();
    request_rank(cx, shared, Which::Picker, query);
    Ok(json!({ "open": true }))
}

pub fn close_picker(shared: &Shared) {
    lock(shared).open = false;
}

/// Ranks `query` against the index in a task. Only the newest request is kept.
pub fn request_rank(cx: &Ctx<'_>, shared: &Shared, which: Which, query: String) {
    let (limit, recent) = limits(cx);
    let (index, history, seq) = {
        let mut state = lock(shared);
        let index = state.index.clone();
        let slot = which.slot(&mut state);
        slot.query = query.clone();
        slot.seq += 1;
        let seq = slot.seq;
        slot.pending = index.is_some();
        if index.is_none() {
            slot.items.clear();
        }
        (index, state.history.clone(), seq)
    };
    let Some(index) = index else {
        return;
    };
    let root_key = index.root.display().to_string();
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || {
            fuzzy::rank(
                &index.files,
                &query,
                &history,
                &root_key,
                now_secs(),
                limit,
                recent,
            )
        },
        move |_cx, items: Vec<String>| {
            let mut state = lock(&done_shared);
            let slot = which.slot(&mut state);
            if slot.seq == seq {
                slot.items = items;
                slot.pending = false;
                slot.selected = 0;
            }
        },
    );
}

/// Opens `rel` (relative to the root) at an optional line, and records the open.
pub fn open_result(cx: &mut Ctx<'_>, shared: &Shared, rel: &str, line: Option<u32>) {
    let root = lock(shared).root.clone();
    let Some(root) = root else {
        lock(shared).notice = Some(cx.t("find.notice.no_bench"));
        return;
    };
    record_open(cx, shared, &root, rel);
    let path = root.join(rel);
    close_picker(shared);
    open_path(cx, shared, path, line);
}

/// Asks the editor to open `path`: emits `file.open_requested` when a module listens to it.
/// Otherwise runs the `file.open` command, and without that the system's default app.
pub fn open_path(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf, line: Option<u32>) {
    let event = FileOpenRequested {
        path: path.to_string_lossy().into_owned(),
        line,
        column: None,
    };
    if has_file_listener(cx) {
        cx.emit(&event);
        return;
    }
    let mut args = Args::new();
    args.insert("path", json!(event.path));
    if let Some(line) = line {
        args.insert("line", json!(line));
    }
    match cx.run_command("file.open", args) {
        Ok(_) => {}
        Err(KernelError::UnknownCommand(_)) => {
            let done_shared = Arc::clone(shared);
            cx.spawn(
                move || platform::open_default(&path),
                move |cx, result: std::io::Result<()>| {
                    if let Err(error) = result {
                        let text =
                            cx.t_args("find.notice.error", &[("message", &error.to_string())]);
                        lock(&done_shared).notice = Some(text);
                    }
                },
            );
        }
        Err(error) => lock(shared).notice = Some(error.to_string()),
    }
}

/// Adds one open of `rel` under `root` to the history and saves it in a task.
pub fn record_open(cx: &Ctx<'_>, shared: &Shared, root: &Path, rel: &str) {
    let key = root.display().to_string();
    let snapshot = {
        let mut state = lock(shared);
        state.history.record(&key, rel, now_secs());
        state.history.clone()
    };
    let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) else {
        return;
    };
    let path = cx.store().path();
    let done_shared = Arc::clone(shared);
    cx.spawn(
        move || forja_kernel::store::write_atomic_bytes(&path, &bytes).map_err(|e| e.to_string()),
        move |cx, result: Result<(), String>| {
            if let Err(error) = result {
                let text = cx.t_args("find.notice.save_failed", &[("message", &error)]);
                lock(&done_shared).notice = Some(text);
            }
        },
    );
}

/// The `find.record_open` command: `path` is absolute and must be under the active root.
pub fn record_open_command(cx: &Ctx<'_>, shared: &Shared, path: &str) -> KResult<Value> {
    let root = lock(shared).root.clone();
    let Some(root) = root else {
        return Err(KernelError::message(cx.t("find.notice.no_bench")));
    };
    let path = PathBuf::from(path);
    let Ok(rel) = path.strip_prefix(&root) else {
        return Ok(json!({ "recorded": false }));
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    record_open(cx, shared, &root, &rel);
    Ok(json!({ "recorded": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_selects_the_right_slot() {
        let mut state = crate::state::State::default();
        Which::Anvil.slot(&mut state).query = "a".into();
        Which::Picker.slot(&mut state).query = "p".into();
        assert_eq!(state.anvil.query, "a");
        assert_eq!(state.picker.query, "p");
    }
}
