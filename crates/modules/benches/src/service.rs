//! Commands, worktree lifecycle and stage transitions for benches.

use crate::config;
pub(crate) use crate::create::create;
use crate::git;
use crate::model::{self, now_millis, BenchRecord, ScriptApproval};
use crate::stages::{self, StageError};
use crate::state::{ApproveForm, CreateForm, Dialog, Shared, StoreForm};
use crate::tabs;
pub(crate) use crate::store::store;
use forja_contracts::{
    BenchActivated, BenchCard, BenchRef, BenchSleepChanged, ProjectRef, StageChanged,
};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

pub fn setting_str(cx: &Ctx<'_>, key: &str, fallback: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

pub fn setting_bool(cx: &Ctx<'_>, key: &str, fallback: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(fallback)
}

pub fn setting_i64(cx: &Ctx<'_>, key: &str, fallback: i64) -> i64 {
    cx.setting(key)
        .and_then(SettingValue::as_i64)
        .unwrap_or(fallback)
}

pub fn setting_list(cx: &Ctx<'_>, key: &str, fallback: &[&str]) -> Vec<String> {
    cx.setting(key)
        .and_then(SettingValue::as_list)
        .map(|list| list.to_vec())
        .unwrap_or_else(|| fallback.iter().map(|value| (*value).to_owned()).collect())
}

pub fn projects(cx: &mut Ctx<'_>) -> Vec<ProjectRef> {
    let Ok(value) = cx.run_command("project.list", Args::new()) else {
        return Vec::new();
    };
    serde_json::from_value(value.get("projects").cloned().unwrap_or(Value::Null))
        .unwrap_or_default()
}

pub fn project_by_id(cx: &mut Ctx<'_>, project_id: &str) -> Option<ProjectRef> {
    projects(cx)
        .into_iter()
        .find(|project| project.id == project_id)
}

pub fn project_path(cx: &mut Ctx<'_>, project_id: &str) -> Option<String> {
    project_by_id(cx, project_id).map(|project| project.path)
}

pub fn active_project_id(cx: &mut Ctx<'_>) -> Option<String> {
    let value = cx.run_command("project.current", Args::new()).ok()?;
    value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Adds the main-checkout record for every registered project once. This is
/// metadata only; the branch is filled by `bench.refresh`.
pub fn ensure_main(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), String> {
    shared.ensure_loaded(cx);
    let projects = projects(cx);
    if projects.is_empty() {
        return Ok(());
    }
    let default_stage = setting_str(cx, "bench.stage.creation_default", stages::DRAFT);
    let existing = shared.file();
    let need_current = existing
        .current
        .as_deref()
        .is_none_or(|id| existing.find(id).is_none())
        && (!existing.benches.is_empty() || !projects.is_empty());
    let missing = projects.iter().any(|project| {
        existing.main_for(&project.id).is_none()
            || existing
                .main_for(&project.id)
                .map(|bench| bench.path != project.path || bench.name != project.name)
                .unwrap_or(false)
    });
    if !missing && !need_current {
        return Ok(());
    }
    let preferred_project = if need_current {
        active_project_id(cx)
    } else {
        None
    };
    shared.mutate(cx, |file| {
        for project in &projects {
            match file.main_for(&project.id).cloned() {
                Some(main) => {
                    if let Some(bench) = file.find_mut(&main.bench_id) {
                        bench.path = project.path.clone();
                        bench.name = project.name.clone();
                    }
                }
                None => file.benches.push(BenchRecord {
                    bench_id: model::main_bench_id(&project.id),
                    project_id: project.id.clone(),
                    name: project.name.clone(),
                    path: project.path.clone(),
                    branch: None,
                    is_main: true,
                    stage: default_stage.clone(),
                    archived: false,
                    pinned: false,
                    unread: false,
                    sleeping: false,
                    created_at: now_millis(),
                    activated_at: 0,
                    base_branch: None,
                    parent_bench_id: None,
                    created_head: None,
                    request_command: None,
                    open_tabs: Vec::new(),
                    request_key: None,
                }),
            }
        }
        if file
            .current
            .as_deref()
            .is_none_or(|id| file.find(id).is_none())
        {
            file.current = preferred_project
                .as_deref()
                .and_then(|project_id| file.main_for(project_id))
                .or_else(|| file.benches.iter().find(|bench| bench.is_main))
                .map(|bench| bench.bench_id.clone());
        }
    })
}

pub fn refs(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project_id: Option<&str>,
    include_archived: bool,
) -> Result<Vec<BenchRef>, String> {
    ensure_main(cx, shared)?;
    let file = shared.file();
    Ok(model::ordered(&file.benches, include_archived)
        .into_iter()
        .filter(|bench| project_id.is_none_or(|id| bench.project_id == id))
        .map(BenchRecord::to_ref)
        .collect())
}

pub fn cards(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project_id: Option<&str>,
    include_archived: bool,
) -> Result<Vec<BenchCard>, String> {
    ensure_main(cx, shared)?;
    let file = shared.file();
    Ok(model::ordered(&file.benches, include_archived)
        .into_iter()
        .filter(|bench| project_id.is_none_or(|id| bench.project_id == id))
        .map(BenchRecord::to_card)
        .collect())
}

pub fn current(shared: &Shared) -> Option<BenchRef> {
    shared.current().map(|bench| bench.to_ref())
}

/// Saves the terminal tabs open in the bench being left, so returning to it can
/// restore them (`bench.layout.restore`).
fn remember_tabs_when_leaving(cx: &mut Ctx<'_>, shared: &Shared) {
    let Some(previous) = shared.current() else {
        return;
    };
    let open = tabs::open_tab_ids(cx, &previous);
    let _ = shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(&previous.bench_id) {
            bench.open_tabs = open;
        }
    });
}

pub fn activate(cx: &mut Ctx<'_>, shared: &Shared, bench_id: &str) -> Result<BenchRef, String> {
    activate_inner(cx, shared, bench_id, true)
}

pub fn activate_project(cx: &mut Ctx<'_>, shared: &Shared, project_id: &str) -> Result<(), String> {
    shared.ensure_loaded(cx);
    ensure_main(cx, shared)?;
    if shared
        .current()
        .is_some_and(|bench| bench.project_id == project_id)
    {
        return Ok(());
    }
    let bench_id = {
        let file = shared.file();
        model::ordered(&file.benches, false)
            .into_iter()
            .find(|bench| bench.project_id == project_id)
            .map(|bench| bench.bench_id.clone())
            .or_else(|| {
                file.main_for(project_id)
                    .map(|bench| bench.bench_id.clone())
            })
    };
    if let Some(bench_id) = bench_id {
        // Keep the current rail view (like Board) open when switching project.
        activate_inner(cx, shared, &bench_id, false)?;
    }
    Ok(())
}

pub fn on_project_removed(cx: &mut Ctx<'_>, shared: &Shared, project_id: &str) {
    shared.ensure_loaded(cx);
    if shared
        .current()
        .is_some_and(|bench| bench.project_id == project_id)
    {
        let _ = shared.mutate(cx, |file| {
            file.current = None;
        });
    }
}

fn activate_inner(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench_id: &str,
    focus_shell: bool,
) -> Result<BenchRef, String> {
    shared.ensure_loaded(cx);
    ensure_main(cx, shared)?;
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    if shared.current_id().as_deref() != Some(bench_id) {
        remember_tabs_when_leaving(cx, shared);
    }
    if setting_bool(cx, "bench.layout.restore", true) {
        let open = tabs::open_tab_ids(cx, &bench);
        let missing = tabs::missing_tabs(&bench.open_tabs, &open);
        tabs::open_tabs(cx, &bench, &missing);
    }
    let now = now_millis();
    shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(bench_id) {
            bench.unread = false;
            bench.sleeping = false;
            bench.activated_at = now;
        }
        file.current = Some(bench_id.to_owned());
    })?;
    let bench = shared.find(bench_id).unwrap_or(bench);
    cx.emit(&BenchActivated(bench.to_ref()));
    // The shell leaves the rail page and shows the active bench's tabs. The
    // command is optional so the module still works without the app shell.
    if let Some(project) = project_by_id(cx, &bench.project_id) {
        let mut args = Map::new();
        args.insert("project".to_owned(), json!(project.name));
        args.insert("bench".to_owned(), json!(bench.name));
        if !focus_shell {
            args.insert("keep_page".to_owned(), json!(true));
        }
        let _ = cx.run_command("shell.bench.focus", Args::from_map(args));
    }
    Ok(bench.to_ref())
}

pub fn stage_list(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project_id: Option<&str>,
) -> Result<Value, String> {
    ensure_main(cx, shared)?;
    let project_id = project_id
        .map(str::to_owned)
        .or_else(|| shared.current().map(|bench| bench.project_id));
    let path = project_id.as_deref().and_then(|id| project_path(cx, id));
    let (stages, warnings) = stages::load(path.as_deref());
    let stages: Vec<Value> = stages
        .iter()
        .map(|stage| {
            json!({
                "id": stage.id,
                "label": stage.label(cx),
                "order": stage.order,
                "parent": stage.parent,
                "locked": stage.locked,
                "dot": stage.dot,
            })
        })
        .collect();
    Ok(json!({ "stages": stages, "warnings": warnings }))
}

pub fn set_stage(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench_id: &str,
    to: &str,
    reason: Option<&str>,
    confirm_backward: bool,
) -> Result<BenchCard, String> {
    ensure_main(cx, shared)?;
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    let project_path = project_path(cx, &bench.project_id);
    let (stages, warnings) = stages::load(project_path.as_deref());
    if !warnings.is_empty() {
        shared.set_notice(Some(warnings.join("; ")));
    }
    let has_new_commits = bench.stage == stages::FORGING
        && to == stages::DRAFT
        && git::has_new_commits(Path::new(&bench.path), bench.created_head.as_deref());
    stages::check(&stages, &bench.stage, to, confirm_backward, has_new_commits)
        .map_err(|error| stage_error(cx, error))?;
    if bench.stage == to {
        return Ok(bench.to_card());
    }
    let from = bench.stage.clone();
    let reason = reason
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| cx.t("bench.stage.reason.manual"));
    shared.mutate(cx, |file| {
        let is_current = file.current.as_deref() == Some(bench_id);
        if let Some(bench) = file.find_mut(bench_id) {
            bench.stage = to.to_owned();
            if !is_current {
                bench.unread = true;
            }
        }
    })?;
    let bench = shared.find(bench_id).unwrap_or(bench);
    cx.emit(&StageChanged {
        bench_id: bench_id.to_owned(),
        from,
        to: to.to_owned(),
        actor: "user".to_owned(),
        origin: "manual".to_owned(),
        reason,
        at: now_millis(),
        request: if setting_bool(cx, "bench.stage.sync_request", true) {
            bench.request_key.clone()
        } else {
            None
        },
    });
    if to == stages::READY
        && !bench.is_main
        && !bench.archived
        && setting_bool(cx, "bench.stage.archive_on_ready", false)
    {
        if let Err(error) = crate::store::store(cx, shared, bench_id, None, None) {
            shared.set_notice(Some(error));
        }
    }
    Ok(bench.to_card())
}

fn stage_error(cx: &Ctx<'_>, error: StageError) -> String {
    cx.t(error.key())
}

pub fn pin(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench_id: &str,
    pinned: Option<bool>,
) -> Result<BenchCard, String> {
    shared.ensure_loaded(cx);
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    let value = pinned.unwrap_or(!bench.pinned);
    shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(bench_id) {
            bench.pinned = value;
        }
    })?;
    Ok(shared.find(bench_id).unwrap_or(bench).to_card())
}

pub fn read(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench_id: &str,
    unread: Option<bool>,
) -> Result<BenchCard, String> {
    shared.ensure_loaded(cx);
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    let value = unread.unwrap_or(false);
    shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(bench_id) {
            bench.unread = value;
        }
    })?;
    Ok(shared.find(bench_id).unwrap_or(bench).to_card())
}

pub fn sleep(cx: &mut Ctx<'_>, shared: &Shared, bench_id: &str) -> Result<BenchCard, String> {
    shared.ensure_loaded(cx);
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    if bench.sleeping {
        return Ok(bench.to_card());
    }
    shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(bench_id) {
            bench.sleeping = true;
        }
    })?;
    if setting_bool(cx, "bench.close.stop_helpers", true) {
        close_terminals(cx, &bench.path);
    }
    let bench = shared.find(bench_id).unwrap_or(bench);
    cx.emit(&BenchSleepChanged {
        bench_id: bench_id.to_owned(),
        path: bench.path.clone(),
        sleeping: true,
    });
    Ok(bench.to_card())
}

pub fn wake(cx: &mut Ctx<'_>, shared: &Shared, bench_id: &str) -> Result<BenchCard, String> {
    shared.ensure_loaded(cx);
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(bench_id) {
            bench.sleeping = false;
        }
    })?;
    let bench = shared.find(bench_id).unwrap_or(bench);
    cx.emit(&BenchSleepChanged {
        bench_id: bench_id.to_owned(),
        path: bench.path.clone(),
        sleeping: false,
    });
    activate(cx, shared, bench_id)?;
    Ok(bench.to_card())
}

pub(crate) fn close_terminals(cx: &mut Ctx<'_>, bench_path: &str) {
    let mut args = Map::new();
    args.insert("bench_path".to_owned(), json!(bench_path));
    let _ = cx.run_command("terminal.close", Args::from_map(args));
}

pub(crate) fn notify(cx: &mut Ctx<'_>, title: String, body: String, level: &str) {
    let mut args = Map::new();
    args.insert("title".to_owned(), json!(title));
    args.insert("body".to_owned(), json!(body));
    args.insert("level".to_owned(), json!(level));
    let _ = cx.run_command("notify.show", Args::from_map(args));
}

pub fn approve(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project_id: &str,
    kind: Option<&str>,
) -> Result<Value, String> {
    let project =
        project_by_id(cx, project_id).ok_or_else(|| "project is not registered".to_owned())?;
    let loaded = config::load(&project.path);
    let kind = kind.unwrap_or("both");
    let setup = if matches!(kind, "setup" | "both") {
        loaded.setup_hash.clone()
    } else {
        None
    };
    let teardown = if matches!(kind, "teardown" | "both") {
        loaded.teardown_hash.clone()
    } else {
        None
    };
    if setup.is_none() && teardown.is_none() {
        return Err("forja.toml has no script to approve".to_owned());
    }
    shared.mutate(cx, |file| {
        if let Some(approval) = file
            .approvals
            .iter_mut()
            .find(|approval| approval.project_id == project_id)
        {
            if setup.is_some() {
                approval.setup_hash = setup.clone();
            }
            if teardown.is_some() {
                approval.teardown_hash = teardown.clone();
            }
        } else {
            file.approvals.push(ScriptApproval {
                project_id: project_id.to_owned(),
                setup_hash: setup.clone(),
                teardown_hash: teardown.clone(),
            });
        }
    })?;
    Ok(json!({
        "approved": true,
        "setup_hash": setup,
        "teardown_hash": teardown
    }))
}

pub fn open_create_dialog(cx: &mut Ctx<'_>, shared: &Shared, project_id: Option<&str>) {
    let project_id = project_id
        .map(str::to_owned)
        .or_else(|| active_project_id(cx))
        .or_else(|| projects(cx).first().map(|project| project.id.clone()))
        .unwrap_or_default();
    shared.set_dialog(Dialog::Create(CreateForm {
        project_id,
        ..CreateForm::default()
    }));
}

pub fn open_store_dialog(shared: &Shared, bench: &BenchRecord) {
    shared.set_dialog(Dialog::Store(StoreForm {
        bench_id: bench.bench_id.clone(),
        name: bench.name.clone(),
        branch: bench.branch.clone(),
        delete_branch: false,
        force: false,
        error: None,
    }));
}

pub fn open_approve_dialog(cx: &mut Ctx<'_>, shared: &Shared, project_id: &str) {
    let Some(project) = project_by_id(cx, project_id) else {
        return;
    };
    let loaded = config::load(&project.path);
    shared.set_dialog(Dialog::Approve(ApproveForm {
        project_id: project.id.clone(),
        setup: loaded.config.setup,
        teardown: loaded.config.teardown,
        setup_hash: loaded.setup_hash,
        teardown_hash: loaded.teardown_hash,
        error: None,
    }));
}

pub(crate) fn bench_path(cx: &Ctx<'_>, project: &ProjectRef, bench_name: &str) -> PathBuf {
    let location = setting_str(cx, "bench.worktree.location", "data_dir");
    let root = match location.as_str() {
        "custom" => {
            let configured = setting_str(cx, "bench.worktree.path", "");
            let expanded = expand_path(&configured);
            if expanded.as_os_str().is_empty() {
                data_dir(cx).join("bancadas")
            } else {
                expanded
            }
        }
        "beside_project" => {
            let project_path = Path::new(&project.path);
            let parent = project_path.parent().unwrap_or(project_path);
            let name = project_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("projeto");
            parent.join(format!("{name}-bancadas"))
        }
        _ => data_dir(cx).join("bancadas"),
    };
    let project_key = model::slugify_path(&project.id);
    root.join(project_key).join(bench_name)
}

fn data_dir(cx: &Ctx<'_>) -> PathBuf {
    let store = cx.store().path();
    store
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn expand_path(value: &str) -> PathBuf {
    let mut text = value.trim().to_owned();
    for (key, env) in [
        ("%LOCALAPPDATA%", "LOCALAPPDATA"),
        ("%APPDATA%", "APPDATA"),
        ("%USERPROFILE%", "USERPROFILE"),
        ("~", "USERPROFILE"),
    ] {
        if let Some(folder) = std::env::var_os(env) {
            text = text.replace(key, &folder.to_string_lossy());
        }
    }
    PathBuf::from(text)
}
