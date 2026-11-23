//! Lançamento do ajudante e eventos de hook (fora de `commands.rs`).

use crate::commands::{current_bench, ensure, ensure_current, ensure_probed, inject_account_env};
use crate::context;
use crate::hooks;
use crate::launch;
use crate::launch::LaunchRequest;
use crate::launch_context;
use crate::profile;
use crate::ready::{self, ReadyMatcher};
use crate::resume;
use crate::status::{self, Run};
use crate::Shared;
use forja_contracts::{HelperStatus, HelperStatusChanged, RequestRef};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::path::PathBuf;

pub(crate) fn launch_request(args: &Args) -> LaunchRequest {
    LaunchRequest {
        profile_id: args.str("profile_id").map(str::to_owned),
        bench_path: args.str("bench_path").map(str::to_owned),
        prompt: args.str("prompt").map(str::to_owned),
        permission: args.str("permission").map(str::to_owned),
        model: args.str("model").map(str::to_owned),
        effort: args.str("effort").map(str::to_owned),
        from_request: args
            .get("from_request")
            .cloned()
            .and_then(|value| serde_json::from_value::<RequestRef>(value).ok()),
        initial_context: args.str("initial_context").map(str::to_owned),
    }
}

pub(crate) fn launch(cx: &mut Ctx<'_>, core: &Shared, request: LaunchRequest) -> Result<Value, String> {
    let bench = request
        .bench_path
        .clone()
        .map(PathBuf::from)
        .or_else(|| current_bench(cx))
        .unwrap_or_else(|| PathBuf::from("."));
    ensure(cx, core, Some(bench.clone()))?;
    ensure_probed(core);
    let default_profile = cx
        .setting("helper.default.profile")
        .and_then(|value| value.as_str())
        .unwrap_or("auto")
        .to_owned();
    let default_permission = cx
        .setting("helper.default.permissions")
        .and_then(|value| value.as_str())
        .unwrap_or("ask")
        .to_owned();
    let selected = core.with_state(|state| {
        let profile = match &request.profile_id {
            Some(id) => state
                .profiles
                .iter()
                .find(|profile| &profile.id == id)
                .cloned(),
            None => profile::pick_default(
                &state.profiles,
                |id| state.is_installed(id),
                &default_profile,
            )
            .cloned(),
        };
        let profile = profile.ok_or_else(|| cx.t("helper.view.installed-empty"))?;
        let overrides = state.overrides(&profile.id);
        let detected = state.executable_for(&profile.id).map(std::path::Path::to_path_buf);
        Ok::<_, String>((profile, overrides, detected))
    })?;
    let (profile, overrides, detected) = selected?;
    if overrides.disabled {
        return Err(format!("helper profile `{}` is disabled", profile.id));
    }
    if !profile.trusted {
        return Err(profile
            .risk_reason
            .clone()
            .unwrap_or_else(|| format!("helper profile `{}` is not trusted", profile.id)));
    }
    let include_rules = cx
        .setting("helper.launch.include_rules")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let context_mode = request
        .initial_context
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            cx.setting("helper.launch.initial_context")
                .and_then(|value| value.as_str())
                .unwrap_or("request")
                .to_owned()
        });
    let bench_ctx = context::fetch_bench_context(cx, &bench);
    let request_ref = request
        .from_request
        .clone()
        .or_else(|| bench_ctx.as_ref().and_then(|ctx| ctx.request_ref.clone()));
    let candidates = if include_rules {
        profile.rule_files()
    } else {
        Vec::new()
    };
    let cached_rules = context::resolve_rules_for_launch(cx, core, &bench, &profile);
    let effective_prompt = context::compose_initial_prompt(
        &context_mode,
        bench_ctx.as_ref(),
        request_ref.as_ref(),
        cached_rules.as_deref(),
        request.prompt.as_deref(),
    );
    let mut launch_req = request.clone();
    // Nada de contexto entra no argv: shims .cmd/.bat interpretam `&` e `>`.
    launch_req.prompt = None;

    let chat_mode = cx
        .setting("helper.launch.mode")
        .and_then(|value| value.as_str())
        .unwrap_or("tui")
        == "chat";
    if chat_mode && cx.registry().command("chat.open").is_some() {
        let mut open = Args::new();
        open.insert("profile_id", json!(profile.id.clone()));
        open.insert("bench_path", json!(bench.display().to_string()));
        open.insert(
            "permission",
            json!(request
                .permission
                .clone()
                .unwrap_or_else(|| default_permission.clone())),
        );
        if let Some(model) = &request.model {
            open.insert("model", json!(model));
        }
        if let Some(effort) = &request.effort {
            open.insert("effort", json!(effort));
        }
        let opened = cx
            .run_command("chat.open", open)
            .map_err(|error| error.to_string())?;
        let session_id = opened
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned);
        resume::record_bench_session(
            cx,
            core,
            &bench.display().to_string(),
            &profile.id,
            session_id.clone(),
        );
        launch_context::deliver_context(
            cx,
            core,
            launch_context::ContextDelivery {
                bench: bench.clone(),
                candidates,
                context_mode,
                request_ref,
                bench_ctx,
                explicit_prompt: request.prompt.clone(),
                chat_opened: true,
                chat_session: session_id,
                terminal_id: None,
            },
        );
        return Ok(json!({
            "terminal_id": Value::Null,
            "profile_id": profile.id,
            "session_id": opened.get("session_id").cloned().unwrap_or(Value::Null),
            "command_line": "",
            "mode": "chat"
        }));
    }
    if cx.registry().command("terminal.open").is_none() {
        return Err("terminal module is not available; helper.launch needs terminal.open".to_owned());
    }
    let mut plan = launch::build(
        &profile,
        &launch_req,
        &default_permission,
        bench.clone(),
        &overrides,
    )?;
    if detected.is_none() {
        let body = cx.t_args("helper.launch.not-installed", &[("name", &profile.name)]);
        let mut notify = Args::new();
        notify.insert("title", json!(cx.t("helper.notify.error-title")));
        notify.insert("body", json!(body.clone()));
        notify.insert("level", json!("warn"));
        let _ = cx.run_command("notify.show", notify);
        return Err(body);
    }
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
    args.insert("cwd", json!(bench.display().to_string()));
    args.insert("command", json!(plan.program.clone()));
    args.insert("args", json!(plan.args_text()));
    args.insert("title", json!(profile.name));
    args.insert("env", json!(plan.environment));
    let opened = cx.run_command("terminal.open", args).map_err(|_| {
        "terminal module is not available; helper.launch needs terminal.open".to_owned()
    })?;
    let terminal_id = opened
        .get("terminal_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "terminal.open returned no terminal_id".to_owned())?
        .to_owned();
    let run = Run {
        terminal_id: terminal_id.clone(),
        profile_id: profile.id.clone(),
        bench_path: bench.display().to_string(),
        started_at: chrono::Utc::now().to_rfc3339(),
        hook_token,
        status: if effective_prompt.is_some() {
            HelperStatus::Working
        } else {
            HelperStatus::Idle
        },
        last_event: Some("launch".to_owned()),
    };
    core.with_state(|state| {
        state.runs.insert(terminal_id.clone(), run.clone());
        state
            .last_benches
            .insert(profile.id.clone(), bench.display().to_string());
        state.refresh_awake();
    })?;
    resume::record_bench_session(cx, core, &bench.display().to_string(), &profile.id, None);
    let _ = cx.emit(&HelperStatusChanged {
        bench_path: run.bench_path.clone(),
        terminal_id: Some(terminal_id.clone()),
        profile_id: run.profile_id.clone(),
        status: run.status,
    });
    let ready_timeout = ready::timeout_from_settings(cx);
    let matcher = ReadyMatcher::from_patterns(profile.ready_markers());
    ready::start_watch(
        cx,
        core,
        terminal_id.clone(),
        bench.display().to_string(),
        profile.name.clone(),
        matcher,
        ready_timeout,
        None,
    );
    launch_context::deliver_context(
        cx,
        core,
        launch_context::ContextDelivery {
            bench: bench.clone(),
            candidates,
            context_mode,
            request_ref,
            bench_ctx,
            explicit_prompt: request.prompt.clone(),
            chat_opened: false,
            chat_session: None,
            terminal_id: Some(terminal_id.clone()),
        },
    );
    Ok(json!({
        "terminal_id": terminal_id,
        "profile_id": profile.id,
        "command_line": plan.command_line(),
        "mode": "tui"
    }))
}

pub(crate) fn hook_event(cx: &mut Ctx<'_>, core: &Shared, payload: Value) {
    if !cx
        .setting("helper.hooks.enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
    {
        return;
    }
    let _ = ensure_current(cx, core);
    let Some(profile_id) = payload.get("forja_profile_id").and_then(Value::as_str) else {
        return;
    };
    let Some(token) = payload.get("forja_token").and_then(Value::as_str) else {
        return;
    };
    let hook_payload = payload.get("payload").unwrap_or(&payload);
    let Some(cwd) = ["cwd", "workspace", "workspace_path", "project_path"]
        .iter()
        .find_map(|key| hook_payload.get(key).and_then(Value::as_str))
    else {
        return;
    };
    let Some(status) = hooks::map_event(&payload) else {
        return;
    };
    let event = payload
        .get("forja_event")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let target = core
        .with_state(|state| {
            status::match_hook_run(&state.runs, profile_id, token, cwd)
                .map(|run| run.terminal_id.clone())
        })
        .ok()
        .flatten();
    if let Some(terminal_id) = target {
        if ReadyMatcher::accepts_hook(event.as_deref()) {
            let _ = core.with_state(|state| {
                let entry = state.launch.ready_states.entry(terminal_id.clone()).or_default();
                entry.ready = true;
                entry.timed_out = false;
                entry.identity = state.runs.get(&terminal_id).map(|run| run.started_at.clone());
            });
            ready::flush_deferred_sends(cx, core, &terminal_id);
        }
        status::update_run_by_id(cx, core, &terminal_id, status, event);
    }
}

pub(crate) fn hook_status_json(status: &hooks::HookStatus) -> Value {
    json!({
        "state": status.state.as_str(),
        "config_path": status.config_path.as_ref().map(|path| path.display().to_string()),
        "detail": status.detail
    })
}

