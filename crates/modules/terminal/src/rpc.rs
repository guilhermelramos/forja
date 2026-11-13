//! Métodos RPC/CLI do terminal.

use crate::requests::open_request_map;
use crate::settings::TermSettings;
use crate::Shared;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::Registrar;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;

pub(super) fn register_rpc(r: &mut Registrar<'_>, core: &Shared) {
    let open_core = Arc::clone(core);
    r.rpc(
        Method::new("terminal.open", "terminal.method.open.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("cwd", ParamKind::String).describe("Working folder of the new terminal."))
            .param(ParamSchema::optional("command", ParamKind::String).describe("Program that replaces the shell."))
            .param(ParamSchema::optional("args", ParamKind::String).describe("Arguments for the program."))
            .param(ParamSchema::optional("env", ParamKind::Json).describe("Extra environment variables."))
            .param(ParamSchema::optional("title", ParamKind::String).describe("Initial tab title."))
            .param(ParamSchema::optional("shell", ParamKind::String).describe("Shell choice: pwsh, cmd, git-bash, wsl or wsl:<distro>."))
            .result(
                ResultSchema::new("terminal.method.open.result").schema(json!({
                    "type": "object",
                    "properties": { "terminal_id": { "type": "string" } },
                    "required": ["terminal_id"]
                })),
            ),
        move |cx, params| {
            let settings = TermSettings::from_ctx(cx);
            let request = open_request_map(&params);
            open_core
                .with_hub(|hub| hub.open(&settings, request))
                .map_err(rpc_error)?
                .map_err(rpc_error)
                .map(|id| json!({ "terminal_id": id.to_string() }))
        },
    );

    let send_core = Arc::clone(core);
    r.rpc(
        Method::new("terminal.send", "terminal.method.send.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("terminal_id", ParamKind::String))
            .param(ParamSchema::required("text", ParamKind::String))
            .param(ParamSchema::optional("enter", ParamKind::Boolean))
            .result(ResultSchema::new("terminal.method.send.result").schema(json!({"type":"object"}))),
        move |_, params| {
            let id = params
                .get("terminal_id")
                .and_then(Value::as_str)
                .and_then(parse_id)
                .ok_or_else(|| rpc_error("missing terminal_id"))?;
            let text = params
                .get("text")
                .and_then(Value::as_str)
                .ok_or_else(|| rpc_error("missing text"))?;
            let enter = params.get("enter").and_then(Value::as_bool).unwrap_or(true);
            send_core
                .with_hub(|hub| {
                    hub.terminal_mut(id)
                        .ok_or_else(|| "terminal not found".to_owned())
                        .map(|terminal| {
                            terminal.send(text, enter);
                            json!({})
                        })
                })
                .map_err(rpc_error)?
                .map_err(rpc_error)
        },
    );

    let read_core = Arc::clone(core);
    r.rpc(
        Method::new("terminal.read", "terminal.method.read.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("terminal_id", ParamKind::String))
            .param(ParamSchema::optional("lines", ParamKind::Integer))
            .result(
                ResultSchema::new("terminal.method.read.result").schema(json!({
                    "type": "object",
                    "properties": { "text": { "type": "string" } },
                    "required": ["text"]
                })),
            ),
        move |cx, params| {
            let settings = TermSettings::from_ctx(cx);
            let id = params
                .get("terminal_id")
                .and_then(Value::as_str)
                .and_then(parse_id)
                .ok_or_else(|| rpc_error("missing terminal_id"))?;
            let lines = params
                .get("lines")
                .and_then(Value::as_i64)
                .unwrap_or(200)
                .clamp(1, 5_000) as usize;
            read_core
                .with_hub(|hub| {
                    let context = hub.ui_context.clone();
                    hub.poll(Instant::now(), &settings, context.as_ref());
                    hub.terminal(id)
                        .ok_or_else(|| "terminal not found".to_owned())
                        .map(|terminal| {
                            json!({
                                "text": terminal.surface.tail(lines),
                                "alive": terminal.alive(),
                                "error": terminal.error,
                                "dir": terminal.dir.display().to_string()
                            })
                        })
                })
                .map_err(rpc_error)?
                .map_err(rpc_error)
        },
    );

    let list_core = Arc::clone(core);
    r.rpc(
        Method::new("terminal.list", "terminal.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .result(
                ResultSchema::new("terminal.method.list.result").schema(json!({
                    "type": "object",
                    "properties": { "terminals": { "type": "array" } },
                    "required": ["terminals"]
                })),
            ),
        move |_, params| {
            let bench = params.get("bench_path").and_then(Value::as_str);
            list_core
                .with_hub(|hub| hub.list(bench))
                .map_err(rpc_error)
                .map(|terminals| json!({ "terminals": terminals }))
        },
    );
}


fn parse_id(value: &str) -> Option<u64> {
    value.parse().ok()
}

fn rpc_error(message: impl ToString) -> RpcError {
    RpcError::app(message.to_string())
}


