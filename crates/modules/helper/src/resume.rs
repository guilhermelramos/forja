//! Política de retomada de sessões ao reabrir uma bancada (`helper.sessions.resume_policy`).

use crate::commands::{ensure, inject_account_env, save_file};
use crate::context;
use crate::hooks;
use crate::launch::{self, ProfileOverride};
use crate::profile::{Layer, Profile};
use crate::status::Run;
use crate::store::{BenchSession, HelperFile};
use crate::Shared;
use forja_contracts::{BenchRef, HelperStatus, HelperStatusChanged};
use forja_kernel::logging::{self, LogLevel};
use forja_kernel::store::LoadResult;
use forja_kernel::{Args, Ctx, KernelError, Registrar, Store};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Política configurada em `helper.sessions.resume_policy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumePolicy {
    Ask,
    Auto,
    Never,
}

impl ResumePolicy {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value.map(str::trim).unwrap_or("ask") {
            "auto" => Self::Auto,
            "never" => Self::Never,
            _ => Self::Ask,
        }
    }
}

/// Convite pendente para o usuário confirmar retomada quando `resume_policy = "ask"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingResume {
    pub bench_id: String,
    pub bench_path: String,
    pub profile_id: String,
    pub profile_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub supported: bool,
}

/// Registra comandos de retomada para uso pela futura UI ou pelo CLI/testes.
pub(crate) fn register(r: &mut Registrar<'_>, core: &Shared) {
    let resume_core = Arc::clone(core);
    r.command("helper.resume", "helper.command.resume", move |cx, args| {
        resume_command(cx, &resume_core, &args).map_err(KernelError::Message)
    });

    let dismiss_core = Arc::clone(core);
    r.command(
        "helper.resume.dismiss",
        "helper.command.resume-dismiss",
        move |cx, args| {
            let bench = target_bench_path(cx, args.str("bench_path"));
            let key = context::path_key(Path::new(&bench));
            let removed = dismiss_core
                .with_state(|state| state.launch.pending_resumes.remove(&key).is_some())
                .map_err(KernelError::Message)?;
            Ok(json!({ "dismissed": removed }))
        },
    );

    let restore_core = Arc::clone(core);
    r.command(
        "helper.resume.evaluate",
        "helper.command.resume-evaluate",
        move |cx, args| {
            let path = target_bench_path(cx, args.str("bench_path"));
            let bench = BenchRef {
                project_id: args.str("project_id").unwrap_or("").to_owned(),
                bench_id: args.str("bench_id").unwrap_or("").to_owned(),
                name: args.str("name").unwrap_or("").to_owned(),
                path: path.clone(),
                branch: args.str("branch").map(str::to_owned),
            };
            let key = context::path_key(std::path::Path::new(&path));
            let first = restore_core
                .with_state(|state| state.launch.restored_evaluated.insert(key))
                .unwrap_or(false);
            if first {
                on_bench_activated(cx, &restore_core, &bench);
            }
            Ok(json!({ "evaluated": first }))
        },
    );

    let pending_core = Arc::clone(core);
    r.command(
        "helper.resume.pending",
        "helper.command.resume-pending",
        move |cx, args| {
            let bench = target_bench_path(cx, args.str("bench_path"));
            let key = context::path_key(Path::new(&bench));
            let (pending, notice) = pending_core
                .with_state(|state| {
                    (
                        state.launch.pending_resumes.get(&key).cloned(),
                        state.launch.resume_notices.get(&key).cloned(),
                    )
                })
                .map_err(KernelError::Message)?;
            Ok(json!({ "pending": pending, "notice": notice }))
        },
    );
}

fn target_bench_path(cx: &mut Ctx<'_>, explicit: Option<&str>) -> String {
    if let Some(path) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
        return path.to_owned();
    }
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| value.get("path").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| ".".to_owned())
}

/// Persiste a última sessão associada à bancada no estado do módulo helper.
pub(crate) fn record_bench_session(
    cx: &mut Ctx<'_>,
    core: &Shared,
    bench_path: &str,
    profile_id: &str,
    session_id: Option<String>,
) {
    let key = context::path_key(Path::new(bench_path));
    let session = BenchSession {
        profile_id: profile_id.to_owned(),
        session_id: session_id.clone(),
        bench_path: bench_path.to_owned(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    let changed = core
        .with_state(|state| {
            state.launch.pending_resumes.remove(&key);
            // Um lançamento TUI ainda não sabe o id: não apague a sessão
            // efetiva que já conhecemos para esta bancada.
            if session_id.is_some() || !state.file.bench_sessions.contains_key(&key) {
                state.file.bench_sessions.insert(key, session);
                true
            } else {
                false
            }
        })
        .unwrap_or(false);
    if changed {
        save_file(cx, core);
    }
}

/// Prepara perfis, estado e sondagem em tarefa do kernel e só depois aplica a política.
pub(crate) fn on_bench_activated(cx: &mut Ctx<'_>, core: &Shared, bench: &BenchRef) {
    let store = Store::new(cx.data_dir());
    let owned = Arc::clone(core);
    let bench_dir = PathBuf::from(&bench.path);
    let work_dir = bench_dir.clone();
    let target = bench.clone();
    let initial = core
        .with_state(|state| (state.file.clone(), state.launch.file_loaded))
        .unwrap_or((HelperFile::default(), false));
    cx.spawn(
        move || {
            let file = if initial.1 {
                initial.0.clone()
            } else {
                match store.load::<HelperFile>("helper") {
                    LoadResult::Loaded(file) => file,
                    _ => HelperFile::default(),
                }
            };
            let profiles = crate::profile::load_all(Some(&work_dir), |_| false);
            (file, profiles)
        },
        move |cx, (file, mut profiles)| {
            for profile in &mut profiles {
                if profile.layer == Some(Layer::Project) && !crate::commands::trust_allows(cx, profile) {
                    profile.trusted = false;
                    profile.risk_reason = Some("project profile needs trust approval".to_owned());
                }
            }
            let applied = owned.with_state(|state| {
                state.file = file;
                state.profiles = profiles;
                state.loaded_for = Some(bench_dir);
                state.launch.file_loaded = true;
            });
            if applied.is_ok() {
                // A sondagem de executáveis também roda em tarefa do kernel.
                crate::commands::start_detection(cx, &owned);
                apply_policy(cx, &owned, &target);
            }
        },
    );
}

fn apply_policy(cx: &mut Ctx<'_>, core: &Shared, bench: &BenchRef) {
    let key = context::path_key(Path::new(&bench.path));
    let policy = ResumePolicy::from_setting(
        cx.setting("helper.sessions.resume_policy")
            .and_then(|value| value.as_str()),
    );
    if matches!(policy, ResumePolicy::Never) {
        let _ = core.with_state(|state| {
            state.launch.pending_resumes.remove(&key);
        });
        return;
    }
    let active_run = core
        .with_state(|state| {
            state.runs.values().any(|run| {
                crate::status::same_path(&run.bench_path, &bench.path)
                    && !matches!(run.status, HelperStatus::Finished)
            })
        })
        .unwrap_or(false);
    if active_run {
        return;
    }

    let Some(previous) = find_previous_session(cx, core, &bench.path) else {
        return;
    };
    let profile_info = core
        .with_state(|state| {
            let profile = state
                .profiles
                .iter()
                .find(|profile| profile.id == previous.profile_id)
                .cloned()?;
            let overrides = state.overrides(&profile.id);
            Some((profile, overrides))
        })
        .ok()
        .flatten();
    let Some((profile, overrides)) = profile_info else {
        return;
    };
    let supported = profile.supports_resume(previous.session_id.as_deref());

    match policy {
        ResumePolicy::Never => {}
        ResumePolicy::Ask => {
            let pending = PendingResume {
                bench_id: bench.bench_id.clone(),
                bench_path: bench.path.clone(),
                profile_id: profile.id.clone(),
                profile_name: profile.name.clone(),
                session_id: previous.session_id.clone(),
                supported,
            };
            let _ = core.with_state(|state| {
                state.launch.pending_resumes.insert(key, pending);
            });
            let title = cx.t("helper.resume.ask-title");
            let body = cx.t_args(
                "helper.resume.ask-body",
                &[("name", &profile.name), ("bench", &bench.name)],
            );
            notify_bench(cx, "info", &title, &body, &bench.path);
        }
        ResumePolicy::Auto => {
            if !supported {
                warn_unsupported_resume(cx, core, &profile, &bench.path);
                return;
            }
            // A sondagem é assíncrona; não deixe um cache ainda não preenchido
            // impedir a retomada da sessão exata desta bancada.
            if overrides.disabled || !profile.trusted {
                return;
            }
            let _ = execute_resume(
                cx,
                core,
                &profile,
                &bench.path,
                previous.session_id.as_deref(),
                &overrides,
            );
        }
    }
}

fn find_previous_session(cx: &mut Ctx<'_>, core: &Shared, bench_path: &str) -> Option<BenchSession> {
    let key = context::path_key(Path::new(bench_path));
    if let Some(session) = core
        .with_state(|state| state.file.bench_sessions.get(&key).cloned())
        .ok()
        .flatten()
        .filter(|session| session.session_id.is_some())
    {
        return Some(session);
    }
    cx.registry().command("archive.list")?;
    let listed = cx.run_command("archive.list", Args::new()).ok()?;
    let sessions = listed.get("sessions")?.as_array()?;
    let mut best: Option<(u64, BenchSession)> = None;
    for entry in sessions {
        let Some(project) = entry.get("project").and_then(Value::as_str) else {
            continue;
        };
        if !crate::status::same_path(project, bench_path) {
            continue;
        }
        let Some(profile_id) = entry.get("profile_id").and_then(Value::as_str) else {
            continue;
        };
        let session_id = entry
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let updated_ms = entry.get("updated_ms").and_then(Value::as_u64).unwrap_or(0);
        if best
            .as_ref()
            .is_none_or(|(best_ms, _)| updated_ms >= *best_ms)
        {
            best = Some((
                updated_ms,
                BenchSession {
                    profile_id: profile_id.to_owned(),
                    session_id,
                    bench_path: bench_path.to_owned(),
                    updated_at: String::new(),
                },
            ));
        }
    }
    best.map(|(_, session)| session)
}

fn warn_unsupported_resume(
    cx: &mut Ctx<'_>,
    core: &Shared,
    profile: &Profile,
    bench_path: &str,
) {
    let key = context::path_key(Path::new(bench_path));
    let title = cx.t("helper.resume.unsupported-title");
    let body = cx.t_args("helper.resume.unsupported-body", &[("name", &profile.name)]);
    let _ = core.with_state(|state| {
        state.launch.pending_resumes.remove(&key);
        state.launch.resume_notices.insert(key.clone(), body.clone());
        state.launch.bench_notices.insert(key, body.clone());
    });
    logging::log(
        LogLevel::Warn,
        format!(
            "forja: helper `{}` does not support resuming sessions; `helper.sessions.resume_policy = auto` fell back to `never` on `{bench_path}`",
            profile.id
        ),
    );
    notify_bench(cx, "warn", &title, &body, bench_path);
}

fn notify_bench(cx: &mut Ctx<'_>, level: &str, title: &str, body: &str, bench_path: &str) {
    let mut notify = Args::new();
    notify.insert("title", json!(title));
    notify.insert("body", json!(body));
    notify.insert("level", json!(level));
    notify.insert("bench_path", json!(bench_path));
    let _ = cx.run_command("notify.show", notify);
}

fn resume_command(cx: &mut Ctx<'_>, core: &Shared, args: &Args) -> Result<Value, String> {
    let bench_path = target_bench_path(cx, args.str("bench_path"));
    ensure(cx, core, Some(PathBuf::from(&bench_path)))?;

    let key = context::path_key(Path::new(&bench_path));
    let pending = core
        .with_state(|state| state.launch.pending_resumes.get(&key).cloned())
        .ok()
        .flatten();
    let previous = find_previous_session(cx, core, &bench_path);

    let profile_id = args
        .str("profile_id")
        .map(str::to_owned)
        .or_else(|| pending.as_ref().map(|item| item.profile_id.clone()))
        .or_else(|| previous.as_ref().map(|item| item.profile_id.clone()))
        .ok_or_else(|| cx.t("helper.resume.none-found"))?;
    let session_id = args
        .str("session_id")
        .map(str::to_owned)
        .or_else(|| pending.and_then(|item| item.session_id))
        .or_else(|| previous.and_then(|item| item.session_id));
    if session_id.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return Err(cx.t("helper.resume.none-found"));
    }

    let selected = core.with_state(|state| {
        let profile = state
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .cloned()
            .ok_or_else(|| format!("unknown helper profile `{profile_id}`"))?;
        let overrides = state.overrides(&profile.id);
        let installed = state.is_installed(&profile.id);
        Ok::<_, String>((profile, overrides, installed))
    })??;
    let (profile, overrides, installed) = selected;
    if !profile.supports_resume(session_id.as_deref()) {
        warn_unsupported_resume(cx, core, &profile, &bench_path);
        return Err(cx.t_args("helper.resume.unsupported-body", &[("name", &profile.name)]));
    }
    if !installed {
        return Err(cx.t_args("helper.launch.not-installed", &[("name", &profile.name)]));
    }
    execute_resume(
        cx,
        core,
        &profile,
        &bench_path,
        session_id.as_deref(),
        &overrides,
    )
}

fn execute_resume(
    cx: &mut Ctx<'_>,
    core: &Shared,
    profile: &Profile,
    bench_path: &str,
    session_id: Option<&str>,
    overrides: &ProfileOverride,
) -> Result<Value, String> {
    let Some(session_id) = session_id.map(str::trim).filter(|id| !id.is_empty()) else {
        return Err("no known session id for this bench; refusing to resume the global last session".to_owned());
    };
    if cx.registry().command("terminal.open").is_none() {
        return Err("terminal module is not available; helper.resume needs terminal.open".to_owned());
    }
    let mut plan = launch::build_resume(
        profile,
        Some(session_id),
        PathBuf::from(bench_path),
        overrides,
    )?;
    inject_account_env(cx, &profile.id, &mut plan);
    let hook_token = if profile.hooks.supported {
        let token = hooks::new_run_token()?;
        plan.environment
            .insert(hooks::HOOK_TOKEN_ENV.to_owned(), token.clone());
        token
    } else {
        String::new()
    };
    let mut args = Args::new();
    args.insert("cwd", json!(bench_path));
    args.insert("command", json!(plan.program.clone()));
    args.insert("args", json!(plan.args_text()));
    args.insert("title", json!(profile.name.clone()));
    args.insert("env", json!(plan.environment));
    let opened = cx.run_command("terminal.open", args).map_err(|_| {
        "terminal module is not available; helper.resume needs terminal.open".to_owned()
    })?;
    let terminal_id = opened
        .get("terminal_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "terminal.open returned no terminal_id".to_owned())?
        .to_owned();
    let run = Run {
        terminal_id: terminal_id.clone(),
        profile_id: profile.id.clone(),
        bench_path: bench_path.to_owned(),
        started_at: chrono::Utc::now().to_rfc3339(),
        hook_token,
        status: HelperStatus::Idle,
        last_event: Some("resume".to_owned()),
    };
    let key = context::path_key(Path::new(bench_path));
    core.with_state(|state| {
        state.launch.pending_resumes.remove(&key);
        state.launch.resume_notices.remove(&key);
        state.runs.insert(terminal_id.clone(), run.clone());
        state
            .last_benches
            .insert(profile.id.clone(), bench_path.to_owned());
        state.refresh_awake();
    })?;
    record_bench_session(
        cx,
        core,
        bench_path,
        &profile.id,
        Some(session_id.to_owned()),
    );
    let _ = cx.emit(&HelperStatusChanged {
        bench_path: run.bench_path.clone(),
        terminal_id: Some(terminal_id.clone()),
        profile_id: run.profile_id.clone(),
        status: run.status,
    });
    Ok(json!({
        "terminal_id": terminal_id,
        "profile_id": profile.id,
        "command_line": plan.command_line(),
        "resumed": true,
    }))
}

#[cfg(test)]
mod tests;
