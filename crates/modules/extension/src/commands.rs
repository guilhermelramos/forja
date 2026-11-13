//! Comandos e métodos RPC do módulo de extensões.

use crate::state::{self, ExtensionCore};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::{Args, Ctx, KernelError, Registrar};
use serde_json::{json, Value};
use std::sync::Arc;

/// Registra comandos e RPC.
pub fn register(r: &mut Registrar<'_>, core: &Arc<ExtensionCore>) {
    let list_core = Arc::clone(core);
    r.command("extension.list", "extension.command.list.title", move |cx, _| {
        list(&list_core, cx)
    });
    let run_core = Arc::clone(core);
    r.command("extension.run", "extension.command.run.title", move |cx, args| {
        let extension = args.required_str("extension")?.to_owned();
        let command = args.required_str("command")?.to_owned();
        let arguments = args.get("arguments").cloned().unwrap_or(Value::Null);
        state::run(&run_core, cx, &extension, &command, arguments)
            .map_err(KernelError::message)
    });
    let action_core = Arc::clone(core);
    r.command("extension.action", "extension.command.action.title", move |cx, args| {
        let extension = args.required_str("extension")?.to_owned();
        let action = args.required_str("action")?.to_owned();
        let event = args.get("event").cloned().unwrap_or(Value::Null);
        state::action(&action_core, cx, &extension, &action, event)
            .map_err(KernelError::message)
    });
    let approve_core = Arc::clone(core);
    r.command("extension.approve", "extension.command.approve.title", move |cx, args| {
        let id = args.required_str("id")?.to_owned();
        state::approve(&approve_core, cx, &id).map_err(KernelError::message)
    });
    let revoke_core = Arc::clone(core);
    r.command("extension.revoke", "extension.command.revoke.title", move |cx, args| {
        let id = args.required_str("id")?.to_owned();
        state::revoke(&revoke_core, cx, &id).map_err(KernelError::message)
    });
    let reload_core = Arc::clone(core);
    r.command("extension.reload", "extension.command.reload.title", move |cx, args| {
        let id = args.required_str("id")?.to_owned();
        state::reload(&reload_core, cx, &id).map_err(KernelError::message)
    });
    let rescan_core = Arc::clone(core);
    r.command("extension.rescan", "extension.command.rescan.title", move |cx, _| {
        let project = state_project(cx);
        state::scan(&rescan_core, cx, project);
        Ok(json!({ "rescanned": true }))
    });
    let set_core = Arc::clone(core);
    r.command("extension.set", "extension.command.set.title", move |_, args| {
        let id = args.required_str("id")?.to_owned();
        let key = args.required_str("key")?.to_owned();
        let value = args.get("value").cloned().unwrap_or(Value::Null);
        if state::set_setting(&set_core, &id, &key, value) {
            Ok(json!({ "set": true }))
        } else {
            Err(KernelError::message(format!("extension `{id}` is not active")))
        }
    });
    let logs_core = Arc::clone(core);
    r.command("extension.logs", "extension.command.logs.title", move |_, args| {
        let id = args.required_str("id")?.to_owned();
        let lines = args.i64("lines").unwrap_or(120).clamp(1, 1000) as usize;
        Ok(json!({ "id": id, "text": state::read_log(&logs_core, &id, lines) }))
    });

    register_rpc(r, core);
}

fn state_project(cx: &mut Ctx<'_>) -> Option<std::path::PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(Value::as_str)
                .map(std::path::PathBuf::from)
        })
}

fn register_rpc(r: &mut Registrar<'_>, core: &Arc<ExtensionCore>) {
    let list_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.list", "extension.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("extension.method.list.result")),
        move |cx, _| {
            list(&list_core, cx)
                .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
        },
    );
    let run_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.run", "extension.method.run.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("extension", ParamKind::String))
            .param(ParamSchema::required("command", ParamKind::String))
            .param(ParamSchema::optional("arguments", ParamKind::Json))
            .result(ResultSchema::new("extension.method.run.result")),
        move |cx, params| {
            let extension = params
                .get("extension")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("extension is required"))?
                .to_owned();
            let command = params
                .get("command")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("command is required"))?
                .to_owned();
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            state::run(&run_core, cx, &extension, &command, arguments)
                .map_err(forja_kernel::forja_protocol::RpcError::app)
        },
    );
    let approve_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.approve", "extension.method.approve.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("id", ParamKind::String))
            .result(ResultSchema::new("extension.method.approve.result")),
        move |cx, params| {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("id is required"))?;
            state::approve(&approve_core, cx, id)
                .map_err(forja_kernel::forja_protocol::RpcError::app)
        },
    );
    let revoke_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.revoke", "extension.method.revoke.description")
            .effect(MethodEffect::Destructive)
            .param(ParamSchema::required("id", ParamKind::String))
            .result(ResultSchema::new("extension.method.revoke.result")),
        move |cx, params| {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("id is required"))?;
            state::revoke(&revoke_core, cx, id)
                .map_err(forja_kernel::forja_protocol::RpcError::app)
        },
    );
    let reload_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.reload", "extension.method.reload.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("id", ParamKind::String))
            .result(ResultSchema::new("extension.method.reload.result")),
        move |cx, params| {
            let id = params
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::invalid_params("id is required"))?;
            state::reload(&reload_core, cx, id)
                .map_err(forja_kernel::forja_protocol::RpcError::app)
        },
    );
    let rescan_core = Arc::clone(core);
    r.rpc(
        Method::new("extension.rescan", "extension.method.rescan.description")
            .effect(MethodEffect::Write)
            .idempotent(true)
            .result(ResultSchema::new("extension.method.rescan.result")),
        move |cx, _| {
            let project = state_project(cx);
            state::scan(&rescan_core, cx, project);
            Ok(json!({ "rescanned": true }))
        },
    );
}

/// Lista as extensões com tudo o que a página precisa.
pub fn list(core: &ExtensionCore, cx: &mut Ctx<'_>) -> forja_kernel::Result<Value> {
    state::ensure(core, cx);
    let locale = cx.i18n().locale().id().to_owned();
    let entries = core.snapshot();
    let rows: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "id": entry.id,
                "name": entry.name,
                "title": entry.name.resolve(&locale),
                "description": entry.description,
                "version": entry.version,
                "author": entry.author,
                "layer": entry.layer,
                "dir": entry.dir,
                "hash": entry.hash,
                "state": entry.state.as_str(),
                "error": entry.error,
                "dev": entry.dev,
                "capabilities": entry.capabilities,
                "commands": entry.commands,
                "settings": entry.settings,
                "values": entry.values,
                "errors": entry.errors,
                "log": entry.log,
                "panels": entry.panels,
            })
        })
        .collect();
    let revision = core.with(|state| state.revision).unwrap_or(0);
    Ok(json!({ "revision": revision, "extensions": rows }))
}
