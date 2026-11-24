//! Comandos do módulo helper.

use crate::context;
use crate::detect;
use crate::hooks;
use crate::launch::{self, LaunchPlan, LaunchRequest};
use crate::profile::{self, Profile};
use crate::status;
use crate::store::HelperFile;
use crate::{HelperState, Shared};
use forja_contracts::HelperStatus;
use forja_kernel::{Args, Ctx, KernelError, Registrar};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
    let list_core = Arc::clone(core);
    r.command("helper.list", "helper.command.list", move |cx, args| {
        let bench = args
            .str("bench_path")
            .map(PathBuf::from)
            .or_else(|| current_bench(cx));
        ensure(cx, &list_core, bench.clone()).map_err(error)?;
        ensure_probed(&list_core);
        maybe_detect(cx, &list_core);
        let (detecting, profiles) = list_core
            .with_state(|state| (state.detecting, profile_infos(state)))
            .map_err(error)?;
        Ok(json!({ "detecting": detecting, "profiles": profiles }))
    });

    let detect_core = Arc::clone(core);
    r.command("helper.detect", "helper.command.detect", move |cx, args| {
        let bench = args
            .str("bench_path")
            .map(PathBuf::from)
            .or_else(|| current_bench(cx));
        ensure(cx, &detect_core, bench).map_err(error)?;
        start_detection(cx, &detect_core);
        Ok(json!({ "started": true }))
    });

    let launch_core = Arc::clone(core);
    r.command("helper.launch", "helper.command.launch", move |cx, args| {
        crate::launch_flow::launch(cx, &launch_core, crate::launch_flow::launch_request(&args)).map_err(KernelError::Message)
    });

    let launch_default_core = Arc::clone(core);
    r.command(
        "helper.launch.default",
        "helper.command.launch-default",
        move |cx, _args| {
            crate::launch_flow::launch(cx, &launch_default_core, LaunchRequest::default()).map_err(KernelError::Message)
        },
    );

    let hook_core = Arc::clone(core);
    r.command("helper.hook", "helper.command.hook", move |cx, args| {
        let payload = args.get("payload").cloned().unwrap_or(Value::Null);
        crate::launch_flow::hook_event(cx, &hook_core, payload);
        Ok(json!({}))
    });

    let focus_core = Arc::clone(core);
    r.command("helper.focus", "helper.command.focus", move |cx, args| {
        let explicit = args
            .str("terminal_id")
            .filter(|id| !id.trim().is_empty())
            .map(str::to_owned);
        let target = focus_core
            .with_state(|state| {
                if let Some(id) = explicit {
                    let bench = state.runs.get(&id).map(|run| run.bench_path.clone());
                    Some((id, bench))
                } else {
                    state
                        .runs
                        .values()
                        .filter(|run| !matches!(run.status, HelperStatus::Finished))
                        .max_by(|a, b| a.started_at.cmp(&b.started_at))
                        .map(|run| (run.terminal_id.clone(), Some(run.bench_path.clone())))
                }
            })
            .map_err(error)?;
        let Some((terminal_id, bench_path)) = target else {
            return Ok(json!({ "focused": false }));
        };
        if let Some(path) = bench_path.as_deref() {
            focus_bench_in_shell(cx, path);
        }
        if cx.registry().command("shell.tab.open").is_some() {
            let mut open = Args::new();
            open.insert("view_id", json!("terminal.tab"));
            let _ = cx.run_command("shell.tab.open", open);
        }
        let mut call = Args::new();
        call.insert("terminal_id", json!(terminal_id));
        let focused = cx.run_command("terminal.focus", call).is_ok();
        Ok(json!({ "focused": focused }))
    });

    let status_runs_core = Arc::clone(core);
    r.command("helper.status", "helper.command.status", move |_cx, args| {
        let bench_filter = args.str("bench_path").map(str::to_owned);
        let terminal_filter = args.str("terminal_id").map(str::to_owned);
        let runs = status_runs_core
            .with_state(|state| {
                state
                    .runs
                    .values()
                    .filter(|run| {
                        bench_filter
                            .as_ref()
                            .is_none_or(|path| run.bench_path == *path)
                            && terminal_filter
                                .as_ref()
                                .is_none_or(|id| run.terminal_id == *id)
                    })
                    .map(|run| {
                        let readiness = state
                            .launch.ready_states
                            .get(&run.terminal_id)
                            .cloned()
                            .unwrap_or_default();
                        json!({
                            "terminal_id": run.terminal_id,
                            "profile_id": run.profile_id,
                            "bench_path": run.bench_path,
                            "started_at": run.started_at,
                            "status": run.status,
                            "failed": status::is_failed(run),
                            "last_event": run.last_event,
                            "ready": readiness.ready,
                            "ready_timed_out": readiness.timed_out,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .map_err(error)?;
        Ok(json!({ "runs": runs }))
    });

    let consent_core = Arc::clone(core);
    r.command(
        "helper.hooks.consent",
        "helper.command.consent",
        move |cx, args| {
            let id = args.required_str("profile_id")?.to_owned();
            let granted = args.bool("granted").unwrap_or(false);
            let bench = current_bench(cx);
            ensure(cx, &consent_core, bench).map_err(error)?;
            let profile = consent_core
                .with_state(|state| {
                    state
                        .profiles
                        .iter()
                        .find(|profile| profile.id == id)
                        .cloned()
                })
                .map_err(error)?
                .ok_or_else(|| KernelError::Message(format!("unknown profile `{id}`")))?;
            let key = hooks::consent_key(&profile);
            consent_core
                .with_state(|state| {
                    state.file.hook_consent.insert(key, granted);
                })
                .map_err(error)?;
            crate::persistence::save_file(cx, &consent_core);
            Ok(json!({ "granted": granted }))
        },
    );

    let status_core = Arc::clone(core);
    r.command(
        "helper.hooks.status",
        "helper.command.hooks-status",
        move |cx, args| {
            let id = args.required_str("profile_id")?;
            let bench = current_bench(cx);
            ensure(cx, &status_core, bench).map_err(error)?;
            let profile = status_core
                .with_state(|state| state.profiles.iter().find(|profile| profile.id == id).cloned())
                .map_err(error)?
                .ok_or_else(|| KernelError::Message(format!("unknown profile `{id}`")))?;
            let key = hooks::consent_key(&profile);
            let cached = status_core.with_state(|state| state.hook_results.get(&key).cloned()).map_err(error)?;
            if args.bool("refresh").unwrap_or(false) {
                if !start_hook_task(cx, &status_core, profile, HookOperation::Status)? {
                    return Err(KernelError::Message("hook operation already in progress".into()));
                }
                return Ok(json!({ "pending": true }));
            }
            if let Some(result) = cached {
                return match result {
                    Ok(status) => Ok(crate::launch_flow::hook_status_json(&status)),
                    Err(message) => Err(KernelError::Message(message)),
                };
            }
            start_hook_task(cx, &status_core, profile, HookOperation::Status)?;
            Ok(json!({ "pending": true }))
        },
    );

    for (id, title, install) in [
        ("helper.hooks.install", "helper.command.hooks-install", true),
        ("helper.hooks.remove", "helper.command.hooks-remove", false),
    ] {
        let hook_core = Arc::clone(core);
        r.command(id, title, move |cx, args| {
            let id = args.required_str("profile_id")?.to_owned();
            let bench = current_bench(cx);
            ensure(cx, &hook_core, bench).map_err(error)?;
            let (profile, consent) = hook_core
                .with_state(|state| {
                    let profile = state
                        .profiles
                        .iter()
                        .find(|profile| profile.id == id)
                        .cloned();
                    let consent = profile
                        .as_ref()
                        .and_then(|profile| {
                            state.file.hook_consent.get(&hooks::consent_key(profile))
                        })
                        .copied()
                        .unwrap_or(false);
                    (profile, consent)
                })
                .map_err(error)?;
            let profile =
                profile.ok_or_else(|| KernelError::Message(format!("unknown profile `{id}`")))?;
            if !profile.trusted {
                return Err(KernelError::Message(
                    "project profile needs trust approval before changing hook configuration"
                        .into(),
                ));
            }
            let timeout = cx
                .setting("helper.hooks.timeout_s")
                .and_then(|value| value.as_i64())
                .unwrap_or(10)
                .clamp(1, 60) as u64;
            if !consent {
                return Err(KernelError::Message("hook consent is required".into()));
            }
            if !hooks::global_writes_allowed() {
                return Err(KernelError::Message("global config writes are disabled in isolated runs".into()));
            }
            let operation = if install { HookOperation::Install(timeout) } else { HookOperation::Remove };
            if !start_hook_task(cx, &hook_core, profile, operation)? {
                return Err(KernelError::Message("hook operation already in progress".into()));
            }
            Ok(json!({ "pending": true }))
        });
    }
}

enum HookOperation {
    Status,
    Install(u64),
    Remove,
}

fn start_hook_task(
    cx: &mut Ctx<'_>, core: &Shared, profile: Profile, operation: HookOperation,
) -> Result<bool, KernelError> {
    let key = hooks::consent_key(&profile);
    let started = core.with_state(|state| {
        if state.hook_busy { return false; }
        state.hook_busy = true;
        state.hook_results.remove(&key);
        true
    }).map_err(error)?;
    if !started { return Ok(false); }
    let owned = Arc::clone(core);
    cx.spawn(
        move || match operation {
            HookOperation::Status => Ok(hooks::status(&profile)),
            HookOperation::Install(timeout) => hooks::install(&profile, true, timeout),
            HookOperation::Remove => hooks::remove(&profile, true),
        },
        move |_cx, result| {
            let _ = owned.with_state(|state| {
                state.hook_results.insert(key, result);
                state.hook_busy = false;
            });
        },
    );
    Ok(true)
}

/// Carrega perfis e preferências para a bancada.
pub(crate) fn ensure(
    cx: &mut Ctx<'_>,
    core: &Shared,
    bench: Option<PathBuf>,
) -> Result<(), String> {
    {
        let state = core
            .state
            .lock()
            .map_err(|_| "helper state is poisoned".to_owned())?;
        if state.loaded_for == bench && !state.profiles.is_empty() {
            return Ok(());
        }
    }
    let need_file = core
        .with_state(|state| !state.launch.file_loaded)
        .unwrap_or(true);
    let file = if need_file {
        match cx.store().load::<HelperFile>() {
            forja_kernel::store::LoadResult::Loaded(file) => file,
            _ => HelperFile::default(),
        }
    } else {
        core.with_state(|state| state.file.clone())
            .unwrap_or_default()
    };
    let profiles = profile::load_all(bench.as_deref(), |profile| trust_allows(cx, profile));
    let mut state = core
        .state
        .lock()
        .map_err(|_| "helper state is poisoned".to_owned())?;
    state.profiles = profiles;
    state.file = file;
    state.launch.file_loaded = true;
    state.loaded_for = bench.clone();
    if state.runs.is_empty() {
        // `--demo` leaves two stored runs so the page has state to show; the
        // real file never carries them and no process is started.
        for run in crate::demo::load(&forja_kernel::Store::new(cx.data_dir())) {
            state.runs.insert(run.terminal_id.clone(), run);
        }
    }
    drop(state);
    if let Some(bench_dir) = bench {
        context::schedule_rules_preload(
            cx,
            core,
            bench_dir,
            vec!["AGENTS.md".to_owned(), "CLAUDE.md".to_owned()],
        );
    }
    Ok(())
}

/// Recarrega usando a bancada já conhecida.
pub(crate) fn ensure_current(cx: &mut Ctx<'_>, core: &Shared) -> Result<(), String> {
    let bench = core
        .state
        .lock()
        .ok()
        .and_then(|state| state.loaded_for.clone());
    ensure(cx, core, bench)
}

pub(crate) fn trust_allows(cx: &mut Ctx<'_>, profile: &Profile) -> bool {
    let mut args = Args::new();
    args.insert("kind", json!("helper_profile"));
    args.insert(
        "source_path",
        json!(profile.source_path.display().to_string()),
    );
    args.insert("content_hash", json!(profile.content_hash));
    cx.run_command("trust.check", args)
        .ok()
        .and_then(|value| value.get("approved").and_then(Value::as_bool))
        .unwrap_or(false)
}

pub(crate) fn current_bench(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| value.get("path").and_then(Value::as_str).map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
}

pub(crate) fn inject_account_env(cx: &mut Ctx<'_>, profile_id: &str, plan: &mut LaunchPlan) {
    let mut args = Args::new();
    args.insert("profile_id", json!(profile_id));
    let Ok(value) = cx.run_command("accounts.env", args) else {
        return;
    };
    plan.merge_env(&value);
}

fn profile_infos(state: &HelperState) -> Vec<Value> {
    let default_id = profile::pick_default(
        &state.profiles,
        |id| state.is_installed(id),
        "auto",
    )
    .map(|profile| profile.id.clone());
    state
        .profiles
        .iter()
        .map(|profile| {
            let detection = state.detection.get(&profile.id);
            let overrides = state.overrides(&profile.id);
            let executable = state
                .executable_for(&profile.id)
                .map(|path| path.display().to_string());
            json!({
                "id": profile.id,
                "name": profile.name,
                "maker": profile.maker,
                "site": profile.site,
                "status": profile.status,
                "installed": state.is_installed(&profile.id),
                "executable": executable,
                "version": detection.and_then(|d| d.version.clone()),
                "install_hint": detect::install_hint(&profile.id),
                "last_bench": state.last_benches.get(&profile.id),
                "enabled": !overrides.disabled,
                "default": default_id.as_deref() == Some(profile.id.as_str()),
                "permissions": profile.permissions.keys().collect::<Vec<_>>(),
                "prompt_mode": profile.prompt.mode,
                "hooks_supported": profile.hooks.supported,
                "trusted": profile.trusted,
                "risk_reason": profile.risk_reason,
                "extra_args": overrides.extra_args,
                "environment": overrides.environment,
                "completeness": profile.completeness(),
            })
        })
        .collect()
}

pub(crate) fn ensure_probed(core: &Shared) {
    let missing: Vec<(String, Vec<String>)> = match core.with_state(|state| {
        state
            .profiles
            .iter()
            .filter(|profile| {
                !state.detection.contains_key(&profile.id)
                    && !state.probed.contains_key(&profile.id)
            })
            .map(|profile| (profile.id.clone(), profile.detect.executables.clone()))
            .collect()
    }) {
        Ok(list) => list,
        Err(_) => return,
    };
    if missing.is_empty() {
        return;
    }
    let resolved: Vec<(String, Option<PathBuf>)> = missing
        .into_iter()
        .map(|(id, executables)| {
            let path = executables.iter().find_map(|name| detect::resolve(name));
            (id, path)
        })
        .collect();
    let _ = core.with_state(|state| {
        for (id, path) in resolved {
            state.probed.entry(id).or_insert(path);
        }
    });
}

pub(crate) fn maybe_detect(cx: &mut Ctx<'_>, core: &Shared) {
    let should = core
        .with_state(|state| state.detection.is_empty() && state.detect_runs == 0 && !state.detecting)
        .unwrap_or(false);
    if should {
        start_detection(cx, core);
    }
}

fn focus_bench_in_shell(cx: &mut Ctx<'_>, bench_path: &str) {
    if cx.registry().command("shell.bench.focus").is_none() {
        return;
    }
    let Ok(benches_val) = cx.run_command("bench.list", Args::new()) else {
        return;
    };
    let Ok(projects_val) = cx.run_command("project.list", Args::new()) else {
        return;
    };
    let target = std::path::Path::new(bench_path);
    let Some(benches) = benches_val.get("benches").and_then(Value::as_array) else {
        return;
    };
    let Some(projects) = projects_val.get("projects").and_then(Value::as_array) else {
        return;
    };
    let Some(bench) = benches.iter().find(|entry| {
        entry
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| std::path::Path::new(path) == target)
    }) else {
        return;
    };
    let Some(bench_name) = bench.get("name").and_then(Value::as_str) else {
        return;
    };
    let Some(project_id) = bench.get("project_id").and_then(Value::as_str) else {
        return;
    };
    let Some(project_name) = projects
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_str) == Some(project_id))
        .and_then(|entry| entry.get("name").and_then(Value::as_str))
    else {
        return;
    };
    let mut focus = Args::new();
    focus.insert("project", json!(project_name));
    focus.insert("bench", json!(bench_name));
    let _ = cx.run_command("shell.bench.focus", focus);
}

pub(crate) fn start_detection(cx: &mut Ctx<'_>, core: &Shared) {
    let profiles = match core.with_state(|state| {
        if state.detecting {
            return None;
        }
        state.detecting = true;
        Some(state.profiles.clone())
    }) {
        Ok(Some(profiles)) => profiles,
        _ => return,
    };
    let owned = Arc::clone(core);
    cx.spawn(
        move || detect::detect_all(&profiles),
        move |_cx, detections| {
            if let Ok(mut state) = owned.state.lock() {
                state.detection = detections;
                state.probed.clear();
                state.detecting = false;
                state.detect_runs += 1;
            }
        },
    );
}

impl crate::Core {
    pub(crate) fn with_state<T>(
        &self,
        run: impl FnOnce(&mut HelperState) -> T,
    ) -> Result<T, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "helper state is poisoned".to_owned())?;
        Ok(run(&mut state))
    }

    pub(crate) fn set_override(
        &self,
        id: &str,
        overrides: launch::ProfileOverride,
    ) -> Result<(), String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "helper state is poisoned".to_owned())?;
        state.file.profiles.insert(id.to_owned(), overrides);
        Ok(())
    }

}

fn error(message: impl ToString) -> KernelError {
    KernelError::Message(message.to_string())
}
