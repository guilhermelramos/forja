//! Métodos RPC `trust.*`.

use crate::state::parse_kind;
use crate::Shared;
use forja_contracts::TrustKind;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::Registrar;
use serde_json::{json, Map, Value};
use std::sync::Arc;

pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
    let check_core = Arc::clone(core);
    r.rpc(
        Method::new("trust.check", "trust.method.check.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("kind", ParamKind::String))
            .param(ParamSchema::required("source_path", ParamKind::String))
            .param(ParamSchema::required("content_hash", ParamKind::String))
            .result(
                ResultSchema::new("trust.method.check.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "approved": { "type": "boolean" },
                        "decided_at": { "type": ["string", "null"] }
                    },
                    "required": ["approved"]
                })),
            ),
        move |cx, params| {
            let (kind, source) = parse_kind_source(&params)?;
            let hash = string_param(&params, "content_hash")?;
            let (approved, decided_at) = check_core
                .with_state(cx, |state| state.check(kind, source, hash))
                .map_err(RpcError::app)?;
            Ok(json!({ "approved": approved, "decided_at": decided_at }))
        },
    );

    let request_core = Arc::clone(core);
    r.rpc(
        Method::new("trust.request", "trust.method.request.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("kind", ParamKind::String))
            .param(ParamSchema::required("source_path", ParamKind::String))
            .param(ParamSchema::required("content", ParamKind::String))
            .param(ParamSchema::optional("summary", ParamKind::String))
            .result(ResultSchema::new("trust.method.request.result").schema(json!({
                "type": "object",
                "properties": {
                    "request_id": { "type": ["string", "null"] },
                    "approved": { "type": "boolean" }
                },
                "required": ["approved"]
            }))),
        move |cx, params| {
            let (kind, source) = parse_kind_source(&params)?;
            let content = string_param(&params, "content")?.to_owned();
            let summary = params
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let (request_id, approved) = request_core
                .with_state(cx, |state| {
                    state.request(kind, source.to_owned(), &content, summary)
                })
                .map_err(RpcError::app)?;
            Ok(json!({ "request_id": request_id, "approved": approved }))
        },
    );

    r.rpc(
        Method::new("trust.revoke", "trust.method.revoke.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("kind", ParamKind::String))
            .param(ParamSchema::required("source_path", ParamKind::String))
            .param(ParamSchema::optional("content_hash", ParamKind::String))
            .result(ResultSchema::new("trust.method.revoke.result").schema(json!({
                "type": "object",
                "properties": { "revoked": { "type": "boolean" } },
                "required": ["revoked"]
            }))),
        move |cx, params| {
            let (kind, source) = parse_kind_source(&params)?;
            let mut args = forja_kernel::Args::new();
            args.insert("kind", json!(kind_str(kind)));
            args.insert("source_path", json!(source));
            if let Some(hash) = params.get("content_hash").and_then(Value::as_str) {
                args.insert("content_hash", json!(hash));
            }
            cx.run_command("trust.revoke", args)
                .map_err(|error| RpcError::app(error.to_string()))
        },
    );

    let list_core = Arc::clone(core);
    r.rpc(
        Method::new("trust.list", "trust.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("trust.method.list.result").schema(json!({
                "type": "object",
                "properties": { "entries": { "type": "array" } },
                "required": ["entries"]
            }))),
        move |cx, _| {
            let entries = list_core
                .with_state(cx, |state| state.list())
                .map_err(RpcError::app)?;
            Ok(json!({ "entries": entries }))
        },
    );
}

fn parse_kind_source(
    params: &Map<String, Value>,
) -> Result<(TrustKind, &str), RpcError> {
    let kind = params
        .get("kind")
        .and_then(Value::as_str)
        .and_then(parse_kind)
        .ok_or_else(|| RpcError::app("missing or unknown kind"))?;
    let source = params
        .get("source_path")
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::app("missing source_path"))?;
    Ok((kind, source))
}

fn kind_str(kind: TrustKind) -> &'static str {
    match kind {
        TrustKind::Setup => "setup",
        TrustKind::Teardown => "teardown",
        TrustKind::Recipe => "recipe",
        TrustKind::Command => "command",
        TrustKind::Extension => "extension",
        TrustKind::HelperProfile => "helper_profile",
    }
}

fn string_param<'a>(params: &'a Map<String, Value>, name: &str) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::app(format!("missing {name}")))
}
