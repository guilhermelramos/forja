//! Métodos RPC `helper.*`.

use crate::Shared;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{Args, Registrar};
use serde_json::{json, Value};

pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
    let _ = core;
    r.rpc(
        Method::new("helper.list", "helper.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .result(ResultSchema::new("helper.method.list.result")),
        |cx, params| {
            let mut args = Args::new();
            if let Some(bench) = params.get("bench_path").and_then(Value::as_str) {
                args.insert("bench_path", json!(bench));
            }
            cx.run_command("helper.list", args).map_err(|error| RpcError::app(error.to_string()))
        },
    );

    r.rpc(
        Method::new("helper.detect", "helper.method.detect.description")
            .effect(MethodEffect::Read)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .result(ResultSchema::new("helper.method.detect.result")),
        |cx, params| {
            let mut args = Args::new();
            if let Some(bench) = params.get("bench_path").and_then(Value::as_str) {
                args.insert("bench_path", json!(bench));
            }
            cx.run_command("helper.detect", args).map_err(|error| RpcError::app(error.to_string()))
        },
    );

    r.rpc(
        Method::new("helper.launch", "helper.method.launch.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::optional("profile_id", ParamKind::String).alias("profile"),
            )
            .param(ParamSchema::optional("bench_path", ParamKind::String).alias("bench"))
            .param(ParamSchema::optional("prompt", ParamKind::String))
            .param(ParamSchema::optional("permission", ParamKind::String))
            .param(ParamSchema::optional("model", ParamKind::String))
            .param(ParamSchema::optional("effort", ParamKind::String))
            .param(ParamSchema::optional("from_request", ParamKind::Json))
            .param(ParamSchema::optional("initial_context", ParamKind::String))
            .result(ResultSchema::new("helper.method.launch.result").schema(json!({
                "type": "object",
                "properties": {
                    "terminal_id": { "type": ["string", "null"] },
                    "profile_id": { "type": "string" },
                    "session_id": { "type": ["string", "null"] },
                    "command_line": { "type": "string" },
                    "mode": { "type": "string" }
                },
                "required": ["profile_id", "mode"]
            }))),
        |cx, params| {
            let args = Args::from_map(params);
            cx.run_command("helper.launch", args)
                .map_err(|error| RpcError::app(error.to_string()))
        },
    );

    r.rpc(
        Method::new("helper.hook", "helper.method.hook.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("payload", ParamKind::Json))
            .result(ResultSchema::new("helper.method.hook.result")),
        |cx, params| {
            let args = Args::from_map(params);
            cx.run_command("helper.hook", args).map_err(|error| RpcError::app(error.to_string()))
        },
    );

    r.rpc(
        Method::new("helper.focus", "helper.method.focus.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("terminal_id", ParamKind::String))
            .result(ResultSchema::new("helper.method.focus.result")),
        |cx, params| {
            let terminal = params
                .get("terminal_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::app("missing terminal_id"))?;
            let mut args = Args::new();
            args.insert("terminal_id", json!(terminal));
            cx.run_command("helper.focus", args).map_err(|error| RpcError::app(error.to_string()))
        },
    );

    r.rpc(
        Method::new("helper.status", "helper.method.status.description")
            .effect(MethodEffect::Read)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .param(ParamSchema::optional("terminal_id", ParamKind::String))
            .result(ResultSchema::new("helper.method.status.result")),
        |cx, params| {
            let mut args = Args::new();
            if let Some(bench) = params.get("bench_path").and_then(Value::as_str) {
                args.insert("bench_path", json!(bench));
            }
            if let Some(terminal) = params.get("terminal_id").and_then(Value::as_str) {
                args.insert("terminal_id", json!(terminal));
            }
            cx.run_command("helper.status", args).map_err(|error| RpcError::app(error.to_string()))
        },
    );

    for method in [
        "helper.hooks.status",
        "helper.hooks.install",
        "helper.hooks.remove",
    ] {
        let method: &'static str = method;
        let effect = if method.ends_with("status") {
            MethodEffect::Read
        } else {
            MethodEffect::Write
        };
        let schema = Method::new(method, "helper.method.hooks.description")
            .effect(effect)
            .param(ParamSchema::required("profile_id", ParamKind::String))
            .param(ParamSchema::optional("refresh", ParamKind::Boolean))
            .result(ResultSchema::new("helper.method.hooks.result"));
        r.rpc(schema, move |cx, params| {
            let id = params
                .get("profile_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::app("missing profile_id"))?;
            let mut args = Args::new();
            args.insert("profile_id", json!(id));
            if let Some(refresh) = params.get("refresh").and_then(Value::as_bool) {
                args.insert("refresh", json!(refresh));
            }
            cx.run_command(method, args).map_err(|error| RpcError::app(error.to_string()))
        });
    }
}

