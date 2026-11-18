//! Behavior of the Files panel's commands.
//!
//! Each function checks its input on the interface thread, then runs any disk
//! work through [`start_op`] in a kernel task. Results come back to the interface
//! thread, where they update the history, the cache, and the status line.

use crate::ops::{self, Action};
use crate::platform;
use crate::state::{lock, Dialog, Shared};
use crate::tree::{self, TreeOptions};
use crate::watch::{self, Batch};
use forja_contracts::{BenchRef, FileOpenRequested};
use forja_kernel::{Args, Ctx, KernelError, Result as KResult};
use serde_json::{json, Value};
use std::any::TypeId;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How often the panel asks which bench is active.
const BENCH_CHECK_EVERY: Duration = Duration::from_millis(750);

/// Reads the two tree settings from the catalog values.
pub fn tree_options(cx: &Ctx<'_>) -> TreeOptions {
    TreeOptions {
        show_ignored: cx
            .setting("explorer.tree.show_ignored")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        follow_symlinks: cx
            .setting("explorer.tree.follow_symlinks")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
    }
}

/// Runs `work` in a kernel task unless another disk operation is still running.
/// `done` gets the result on the interface thread. Returns false when busy.
pub fn start_op<T, W, D>(cx: &Ctx<'_>, shared: &Shared, work: W, done: D) -> bool
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: for<'c, 'd> FnOnce(&'d mut Ctx<'c>, T) + Send + 'static,
{
    {
        let mut state = lock(shared);
        if state.busy {
            state.notice = Some(cx.t("explorer.notice.busy"));
            return false;
        }
        state.busy = true;
        state.notice = None;
    }
    let flag = Arc::clone(shared);
    cx.spawn(work, move |cx, value| {
        lock(&flag).busy = false;
        done(cx, value);
    });
    true
}

/// Folders whose listing can change after `action`.
fn affected_dirs(action: &Action) -> Vec<PathBuf> {
    match action {
        Action::Created { path, .. } => parent_of(path).into_iter().collect(),
        Action::Renamed { from, to } => parent_of(from).into_iter().chain(parent_of(to)).collect(),
        Action::Duplicated { copy, .. } => parent_of(copy).into_iter().collect(),
    }
}

fn parent_of(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// The name a status line should show for an action.
fn describe(action: &Action) -> String {
    match action {
        Action::Created { path, .. } => name_of(path),
        Action::Renamed { to, .. } => name_of(to),
        Action::Duplicated { copy, .. } => name_of(copy),
    }
}

pub(crate) fn humanize_io_error(cx: &Ctx<'_>, error: &io::Error) -> String {
    let detail = error.to_string();
    match error.kind() {
        io::ErrorKind::NotFound => cx.t_args("explorer.error.not.found", &[("detail", &detail)]),
        io::ErrorKind::PermissionDenied => {
            cx.t_args("explorer.error.permission", &[("detail", &detail)])
        }
        io::ErrorKind::AlreadyExists => cx.t_args("explorer.error.exists", &[("detail", &detail)]),
        _ => cx.t_args("explorer.notice.error", &[("message", &detail)]),
    }
}

/// Applies the result of a disk action: records it, refreshes the folders it
/// touched, and writes the status line from `key`.
fn finish_action(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    result: io::Result<Action>,
    dirs: Vec<PathBuf>,
    key: &'static str,
) {
    match result {
        Ok(action) => {
            let text = cx.t_args(key, &[("name", &describe(&action))]);
            let mut state = lock(shared);
            for dir in &dirs {
                state.tree.invalidate(dir);
            }
            state.history.record(action);
            state.notice = Some(text);
        }
        Err(error) => {
            let text = humanize_io_error(cx, &error);
            let mut state = lock(shared);
            for dir in &dirs {
                state.tree.invalidate(dir);
            }
            state.notice = Some(text);
        }
    }
}

fn checked_name(cx: &Ctx<'_>, name: &str) -> KResult<String> {
    match ops::validate_name(name) {
        Ok(()) => Ok(name.trim().to_owned()),
        Err(ops::NameError::Empty) => Err(KernelError::message(cx.t("explorer.notice.name_empty"))),
        Err(ops::NameError::Invalid) => {
            Err(KernelError::message(cx.t("explorer.notice.name_invalid")))
        }
    }
}

/// Creates a file or folder. Without a name, opens the name dialog.
pub fn new_entry(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    dir: PathBuf,
    name: Option<String>,
    is_dir: bool,
    template: Option<String>,
) -> KResult<Value> {
    let Some(name) = name else {
        lock(shared).dialog = Some(Dialog::NewEntry {
            dir,
            is_dir,
            name: String::new(),
            template,
        });
        return Ok(json!({ "dialog": "name" }));
    };
    let name = checked_name(cx, &name)?;
    let target = ops::join(&dir, &name);
    let body = match (&template, is_dir) {
        (Some(template), false) => {
            let mut args = Args::new();
            args.insert("id", json!(template));
            cx.run_command("content.templates.render", args)
                .ok()
                .and_then(|value| {
                    value
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                })
        }
        _ => None,
    };
    let dirs = vec![dir];
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || {
            let action = ops::create(&target, is_dir)?;
            if let Some(body) = body {
                std::fs::write(&target, body)?;
            }
            Ok(action)
        },
        move |cx, result: io::Result<Action>| {
            finish_action(cx, &done_shared, result, dirs, "explorer.notice.created")
        },
    );
    Ok(json!({ "started": started }))
}

/// Renames `path`. Without a name, opens the rename dialog.
pub fn rename(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    path: PathBuf,
    name: Option<String>,
) -> KResult<Value> {
    let Some(name) = name else {
        lock(shared).dialog = Some(Dialog::Rename {
            name: name_of(&path),
            path,
        });
        return Ok(json!({ "dialog": "name" }));
    };
    let name = checked_name(cx, &name)?;
    let dir = parent_of(&path).ok_or_else(|| KernelError::message("the root cannot be renamed"))?;
    let to = ops::join(&dir, &name);
    if to == path {
        return Ok(json!({ "started": false }));
    }
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || ops::rename(&path, &to),
        move |cx, result: io::Result<Action>| {
            finish_action(
                cx,
                &done_shared,
                result,
                vec![dir],
                "explorer.notice.renamed",
            )
        },
    );
    Ok(json!({ "started": started }))
}

/// Copies `path` next to itself under a free name.
pub fn duplicate(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf) -> KResult<Value> {
    let suffix = cx.t("explorer.copy_suffix");
    let dirs: Vec<PathBuf> = parent_of(&path).into_iter().collect();
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || ops::duplicate(&path, &suffix),
        move |cx, result: io::Result<Action>| {
            finish_action(cx, &done_shared, result, dirs, "explorer.notice.duplicated")
        },
    );
    Ok(json!({ "started": started }))
}

/// Re-applies or inverts the newest history step on disk, and moves it to the other stack.
fn replay(cx: &mut Ctx<'_>, shared: &Shared, forward: bool) -> KResult<Value> {
    let taken = {
        let mut state = lock(shared);
        if forward {
            state.history.take_redo()
        } else {
            state.history.take_undo()
        }
    };
    let Some(action) = taken else {
        let key = if forward {
            "explorer.notice.nothing_to_redo"
        } else {
            "explorer.notice.nothing_to_undo"
        };
        lock(shared).notice = Some(cx.t(key));
        return Ok(json!({ "done": false }));
    };
    let dirs = affected_dirs(&action);
    let work_action = action.clone();
    let back = action.clone();
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || {
            if forward {
                ops::redo(&work_action)
            } else {
                ops::undo(&work_action)
            }
        },
        move |cx, result: io::Result<()>| {
            let text = match &result {
                Ok(()) => {
                    let key = if forward {
                        "explorer.notice.redone"
                    } else {
                        "explorer.notice.undone"
                    };
                    cx.t_args(key, &[("name", &describe(&back))])
                }
                Err(error) => humanize_io_error(cx, error),
            };
            let mut state = lock(&done_shared);
            for dir in &dirs {
                state.tree.invalidate(dir);
            }
            // Success moves the step to the other stack. Failure puts it back where it came from.
            match (result.is_ok(), forward) {
                (true, true) => state.history.push_undo(back),
                (true, false) => state.history.push_redo(back),
                (false, true) => state.history.push_redo(back),
                (false, false) => state.history.push_undo(back),
            }
            state.notice = Some(text);
        },
    );
    if !started {
        // start_op refused: nothing ran, so the step goes back where it was taken from.
        let mut state = lock(shared);
        if forward {
            state.history.push_redo(action);
        } else {
            state.history.push_undo(action);
        }
    }
    Ok(json!({ "started": started }))
}

pub fn undo(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    replay(cx, shared, false)
}

pub fn redo(cx: &mut Ctx<'_>, shared: &Shared) -> KResult<Value> {
    replay(cx, shared, true)
}

/// Sends `path` to the Recycle Bin. Not recorded in the undo history.
pub fn trash(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf) -> KResult<Value> {
    let dir = parent_of(&path);
    let name = name_of(&path);
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || platform::trash(&path).map(|()| path),
        move |cx, result: io::Result<PathBuf>| match result {
            Ok(path) => {
                let text = cx.t_args("explorer.notice.trashed", &[("name", &name)]);
                let mut state = lock(&done_shared);
                if let Some(dir) = &dir {
                    state.tree.invalidate(dir);
                }
                state.expanded.remove(&path);
                if state.selected.as_deref() == Some(path.as_path()) {
                    state.selected = None;
                }
                state.notice = Some(text);
            }
            Err(error) => {
                let text = humanize_io_error(cx, &error);
                lock(&done_shared).notice = Some(text);
            }
        },
    );
    Ok(json!({ "started": started }))
}

/// Opens Windows Explorer with `path` selected.
pub fn reveal(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf) -> KResult<Value> {
    let done_shared = Arc::clone(shared);
    let started = start_op(
        cx,
        shared,
        move || platform::reveal(&path),
        move |cx, result: io::Result<()>| {
            if let Err(error) = result {
                let text = cx.t_args("explorer.notice.error", &[("message", &error.to_string())]);
                lock(&done_shared).notice = Some(text);
            }
        },
    );
    Ok(json!({ "started": started }))
}

/// Puts the absolute or root-relative path on the clipboard (the view does the copy).
pub fn copy_path(cx: &Ctx<'_>, shared: &Shared, path: &Path, relative: bool) -> KResult<Value> {
    let text = {
        let state = lock(shared);
        if relative {
            let root = state.tree.root();
            match root.and_then(|root| path.strip_prefix(root).ok()) {
                Some(rel) => rel.to_string_lossy().replace('\\', "/"),
                None => forja_kernel::paths::clean_path(path).display().to_string(),
            }
        } else {
            forja_kernel::paths::clean_path(path).display().to_string()
        }
    };
    let mut state = lock(shared);
    state.pending_copy = Some(text.clone());
    state.notice = Some(cx.t("explorer.notice.copied"));
    Ok(json!({ "text": text }))
}

/// Opens the terminal module in `dir`. Without the terminal module, says so.
pub fn open_terminal(cx: &mut Ctx<'_>, shared: &Shared, dir: &Path) -> KResult<Value> {
    let mut args = Args::new();
    args.insert("cwd", json!(dir.to_string_lossy()));
    match cx.run_command("terminal.open", args) {
        Ok(value) => Ok(value),
        Err(KernelError::UnknownCommand(_)) => {
            lock(shared).notice = Some(cx.t("explorer.notice.no_terminal"));
            Ok(json!({ "opened": false }))
        }
        Err(error) => Err(error),
    }
}

/// Forgets every cached listing so the panel reads the disk again.
pub fn refresh(shared: &Shared) -> KResult<Value> {
    let mut state = lock(shared);
    state.tree.invalidate_all();
    state.ignores.clear();
    Ok(json!({}))
}

/// True when some module subscribes to `file.open_requested` (an editor).
/// Declaring the event is not enough: every publisher declares it.
pub fn has_file_listener(cx: &Ctx<'_>) -> bool {
    let type_id = TypeId::of::<FileOpenRequested>();
    cx.registry()
        .subscriptions()
        .iter()
        .any(|subscription| subscription.type_id == type_id)
}

/// Asks the editor to open `path`: emits `file.open_requested` when a module listens
/// to it. Otherwise runs the `file.open` command, and without that the system's default app.
pub fn open_file(cx: &mut Ctx<'_>, shared: &Shared, path: PathBuf, line: Option<u32>) {
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
            let started = start_op(
                cx,
                shared,
                move || platform::open_default(&path),
                move |cx, result: io::Result<()>| {
                    if let Err(error) = result {
                        let text = humanize_io_error(cx, &error);
                        lock(&done_shared).notice = Some(text);
                    }
                },
            );
            let _ = started;
        }
        Err(error) => {
            let text = cx.t_args("explorer.notice.error", &[("message", &error.to_string())]);
            lock(shared).notice = Some(text);
        }
    }
}

/// Makes sure `dir` has a listing. Cached or already loading folders are skipped.
pub fn request_children(cx: &Ctx<'_>, shared: &Shared, dir: &Path) {
    let (root, options, ignores, generation) = {
        let mut state = lock(shared);
        let Some(root) = state.tree.root().map(Path::to_path_buf) else {
            return;
        };
        if !state.tree.begin_load(dir) {
            return;
        }
        (
            root,
            state.tree.options(),
            state.ignores.clone(),
            state.tree.generation(),
        )
    };
    let dir_work = dir.to_path_buf();
    let dir_done = dir.to_path_buf();
    let shared = Arc::clone(shared);
    cx.spawn(
        move || tree::read_dir(&dir_work, &root, options, &ignores),
        move |_cx, entries| {
            lock(&shared)
                .tree
                .finish_load(&dir_done, generation, entries);
        },
    );
}

/// Keeps the panel in step with the settings and the active bench. `bench.current`
/// is read once at start, and again while no bench is known (one may appear later).
/// Later changes arrive as `bench.activated`, handled by [`on_bench_activated`].
pub fn sync(cx: &mut Ctx<'_>, shared: &Shared) {
    let options = tree_options(cx);
    lock(shared).tree.set_options(options);
    let read = {
        let mut state = lock(shared);
        let due = state
            .bench_checked
            .is_none_or(|at| at.elapsed() >= BENCH_CHECK_EVERY);
        let wanted = state.bench_checked.is_none() || state.tree.root().is_none();
        if due && wanted {
            state.bench_checked = Some(Instant::now());
            true
        } else {
            false
        }
    };
    if read {
        let bench = read_current(cx);
        apply_bench(cx, shared, bench.as_ref());
    }
}

/// Reads the bench the benches module reports as active, if any.
fn read_current(cx: &mut Ctx<'_>) -> Option<BenchRef> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| serde_json::from_value::<BenchRef>(value).ok())
}

/// Handles `bench.activated`: the panel shows that bench's files.
pub fn on_bench_activated(cx: &Ctx<'_>, shared: &Shared, bench: &BenchRef) {
    apply_bench(cx, shared, Some(bench));
}

/// Handles `project.activated` when `benches` is absent or has not activated yet.
pub fn on_project_activated(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project: &forja_contracts::ProjectRef,
) {
    if let Some(bench) = read_current(cx).filter(|bench| bench.project_id == project.id) {
        apply_bench(cx, shared, Some(&bench));
        return;
    }
    let root = PathBuf::from(&project.path);
    let current = lock(shared).tree.root().map(Path::to_path_buf);
    if current.as_ref() != Some(&root) {
        switch_root(cx, shared, Some(root), Some(project.name.clone()));
    }
}

/// Handles `project.removed`: clears the tree if no active bench/project remains.
pub fn on_project_removed(cx: &mut Ctx<'_>, shared: &Shared) {
    if read_current(cx).is_none() {
        apply_bench(cx, shared, None);
    }
}

/// Makes `bench` the panel's root, or no root. Switching again to the same bench does nothing.
fn apply_bench(cx: &Ctx<'_>, shared: &Shared, bench: Option<&BenchRef>) {
    let root = bench.map(|bench| PathBuf::from(&bench.path));
    let current = lock(shared).tree.root().map(Path::to_path_buf);
    if root != current {
        switch_root(cx, shared, root, bench.map(|bench| bench.name.clone()));
    }
}

fn switch_root(cx: &Ctx<'_>, shared: &Shared, root: Option<PathBuf>, name: Option<String>) {
    let generation = {
        let mut state = lock(shared);
        state.tree.set_root(root.clone());
        state.bench_name = name;
        state.expanded.clear();
        state.selected = None;
        state.dialog = None;
        state.watcher = None;
        state.ignores.clear();
        state.watch_generation += 1;
        state.watch_generation
    };
    let Some(root) = root else {
        return;
    };
    let start_root = root.clone();
    let shared_start = Arc::clone(shared);
    cx.spawn(
        move || watch::start(&start_root),
        move |cx, result: Result<(notify::RecommendedWatcher, Receiver<PathBuf>), String>| {
            match result {
                Ok((watcher, receiver)) => {
                    let mut state = lock(&shared_start);
                    if state.watch_generation == generation {
                        state.watcher = Some(watcher);
                        drop(state);
                        pump(cx, receiver, shared_start, generation);
                    }
                }
                Err(error) => {
                    let text = cx.t_args("explorer.notice.error", &[("message", &error)]);
                    lock(&shared_start).notice = Some(text);
                }
            }
        },
    );
    request_children(cx, shared, &root);
}

/// Waits for the next change burst and applies it, then waits again.
fn pump(cx: &Ctx<'_>, receiver: Receiver<PathBuf>, shared: Shared, generation: u64) {
    cx.spawn_service(
        move || watch::wait_batch(receiver),
        move |cx, batch: Batch| {
            let current = {
                let mut state = lock(&shared);
                if state.watch_generation == generation {
                    apply_changes(&mut state, &batch.paths);
                    true
                } else {
                    false
                }
            };
            if batch.alive && current {
                pump(cx, batch.receiver, shared, generation);
            }
        },
    );
}

fn apply_changes(state: &mut crate::state::State, paths: &[PathBuf]) {
    for path in paths {
        if watch::is_git_internal(path) {
            continue;
        }
        if path.file_name().is_some_and(|name| name == ".gitignore") {
            state.ignores.clear();
        }
        state.tree.invalidate(path);
        if let Some(parent) = path.parent() {
            state.tree.invalidate(parent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_uses_the_entry_name() {
        let action = Action::Renamed {
            from: PathBuf::from("/a/old.txt"),
            to: PathBuf::from("/a/new.txt"),
        };
        assert_eq!(describe(&action), "new.txt");
        assert_eq!(
            affected_dirs(&action),
            vec![PathBuf::from("/a"), PathBuf::from("/a")]
        );
    }

    #[test]
    fn git_changes_never_invalidate_the_cache() {
        let root = PathBuf::from("/repo");
        let mut state = crate::state::State::default();
        state.tree.set_root(Some(root.clone()));
        let generation = state.tree.generation();
        state.tree.begin_load(&root);
        state.tree.finish_load(&root, generation, Vec::new());
        apply_changes(&mut state, &[root.join(".git").join("index")]);
        assert!(state.tree.is_cached(&root));
        let src = root.join("src");
        state.tree.begin_load(&src);
        state.tree.finish_load(&src, generation, Vec::new());
        apply_changes(&mut state, &[src.join("lib.rs")]);
        assert!(
            !state.tree.is_cached(&src),
            "the parent folder is refreshed"
        );
        assert!(
            state.tree.is_cached(&root),
            "unrelated folders keep their listing"
        );
    }

    #[test]
    fn io_errors_are_humanized_in_pt_br() {
        let dir = std::env::temp_dir().join(format!("forja-explorer-err-{}", std::process::id()));
        let mut runtime = forja_kernel::Runtime::new(
            vec![Box::new(crate::ExplorerModule::new())],
            forja_kernel::RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime");
        let cx = runtime.ctx("explorer");
        let not_found = humanize_io_error(&cx, &io::Error::new(io::ErrorKind::NotFound, "os error 2"));
        assert!(not_found.contains("não encontrado"), "{not_found}");
        assert!(not_found.contains("os error 2"), "{not_found}");
        let _ = std::fs::remove_dir_all(dir);
    }
}
