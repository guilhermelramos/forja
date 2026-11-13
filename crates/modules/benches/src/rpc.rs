use crate::service;
use crate::state::Shared;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::Registrar;
use serde_json::{json, Map, Value};
pub(crate) fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let s = shared.clone();
    r.rpc(
        Method::new("bench.current", "bench.method.current.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("bench.method.current.result")),
        move |cx, _| {
            s.ensure_loaded(cx);
            service::ensure_main(cx, &s).map_err(RpcError::app)?;
            Ok(serde_json::to_value(service::current(&s)).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.list", "bench.method.list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("project_id", ParamKind::String).alias("project-id"))
            .param(ParamSchema::optional(
                "include_archived",
                ParamKind::Boolean,
            ))
            .result(ResultSchema::new("bench.method.list.result")),
        move |cx, params| {
            let benches = service::refs(
                cx,
                &s,
                params.get("project_id").and_then(Value::as_str),
                params
                    .get("include_archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .map_err(RpcError::app)?;
            Ok(json!({ "benches": benches }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.list_cards", "bench.method.list_cards.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("project_id", ParamKind::String).alias("project-id"))
            .param(ParamSchema::optional(
                "include_archived",
                ParamKind::Boolean,
            ))
            .result(ResultSchema::new("bench.method.list_cards.result")),
        move |cx, params| {
            let benches = service::cards(
                cx,
                &s,
                params.get("project_id").and_then(Value::as_str),
                params
                    .get("include_archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            )
            .map_err(RpcError::app)?;
            Ok(json!({ "benches": benches }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.activate", "bench.method.activate.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bancada")
                    .alias("bench-id"),
            )
            .result(ResultSchema::new("bench.method.activate.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let bench = service::activate(cx, &s, id).map_err(RpcError::app)?;
            Ok(serde_json::to_value(bench).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.create", "bench.method.create.description")
            .effect(MethodEffect::Write)
            .network(true)
            .param(
                ParamSchema::required("project_id", ParamKind::String)
                    .alias("projeto")
                    .alias("project-id"),
            )
            .param(ParamSchema::required("name", ParamKind::String).alias("nome"))
            .param(ParamSchema::optional("base", ParamKind::String))
            .param(
                ParamSchema::optional("from_request", ParamKind::String)
                    .alias("pedido")
                    .alias("from-request"),
            )
            .param(ParamSchema::optional("fetch", ParamKind::Boolean))
            .result(ResultSchema::new("bench.method.create.result")),
        move |cx, params| {
            let project_id = required(&params, "project_id")?;
            let name = required(&params, "name")?;
            let bench = crate::create::create(
                cx,
                &s,
                project_id,
                name,
                params.get("base").and_then(Value::as_str),
                params.get("from_request").and_then(Value::as_str),
                params.get("fetch").and_then(Value::as_bool),
            )
            .map_err(RpcError::app)?;
            Ok(serde_json::to_value(bench).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.store", "bench.method.store.description")
            .effect(MethodEffect::Destructive)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bancada")
                    .alias("bench-id"),
            )
            .param(
                ParamSchema::optional("delete_branch", ParamKind::Boolean)
                    .alias("apagar_branch")
                    .alias("delete-branch"),
            )
            .param(ParamSchema::optional("force", ParamKind::Boolean).alias("forcar"))
            .result(ResultSchema::new("bench.method.store.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            crate::store::store(
                cx,
                &s,
                id,
                params.get("delete_branch").and_then(Value::as_bool),
                params.get("force").and_then(Value::as_bool),
            )
            .map_err(RpcError::app)?;
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.stage.set", "bench.method.stage_set.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bancada")
                    .alias("bench-id"),
            )
            .param(ParamSchema::required("stage", ParamKind::String).alias("para"))
            .param(ParamSchema::optional("reason", ParamKind::String).alias("motivo"))
            .param(
                ParamSchema::optional("confirm_backward", ParamKind::Boolean)
                    .alias("confirm-backward"),
            )
            .result(ResultSchema::new("bench.method.stage_set.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let stage = required(&params, "stage")?;
            let card = service::set_stage(
                cx,
                &s,
                id,
                stage,
                params.get("reason").and_then(Value::as_str),
                params
                    .get("confirm_backward")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .map_err(RpcError::app)?;
            Ok(serde_json::to_value(card).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.stage.list", "bench.method.stage_list.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("project_id", ParamKind::String).alias("project-id"))
            .result(ResultSchema::new("bench.method.stage_list.result")),
        move |cx, params| {
            service::stage_list(cx, &s, params.get("project_id").and_then(Value::as_str))
                .map_err(RpcError::app)
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.sleep", "bench.method.sleep.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bench-id"),
            )
            .result(ResultSchema::new("bench.method.sleep.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let card = service::sleep(cx, &s, id).map_err(RpcError::app)?;
            Ok(serde_json::to_value(card).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.wake", "bench.method.wake.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bench-id"),
            )
            .result(ResultSchema::new("bench.method.wake.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let card = service::wake(cx, &s, id).map_err(RpcError::app)?;
            Ok(serde_json::to_value(card).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.pin", "bench.method.pin.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bench-id"),
            )
            .param(ParamSchema::optional("pinned", ParamKind::Boolean))
            .result(ResultSchema::new("bench.method.pin.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let card = service::pin(cx, &s, id, params.get("pinned").and_then(Value::as_bool))
                .map_err(RpcError::app)?;
            Ok(serde_json::to_value(card).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.read", "bench.method.read.description")
            .effect(MethodEffect::Write)
            .param(
                ParamSchema::required("bench_id", ParamKind::String)
                    .alias("bench")
                    .alias("bench-id"),
            )
            .param(ParamSchema::optional("unread", ParamKind::Boolean))
            .result(ResultSchema::new("bench.method.read.result")),
        move |cx, params| {
            let id = required(&params, "bench_id")?;
            let card = service::read(cx, &s, id, params.get("unread").and_then(Value::as_bool))
                .map_err(RpcError::app)?;
            Ok(serde_json::to_value(card).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("bench.refresh", "bench.method.refresh.description")
            .effect(MethodEffect::Read)
            .result(ResultSchema::new("bench.method.refresh.result")),
        move |cx, _| {
            crate::refresh::refresh(cx, &s).map_err(RpcError::app)?;
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new(
            "bench.script.approve",
            "bench.method.script_approve.description",
        )
        .effect(MethodEffect::Write)
        .param(
            ParamSchema::required("project_id", ParamKind::String)
                .alias("projeto")
                .alias("project-id"),
        )
        .param(ParamSchema::optional("kind", ParamKind::String))
        .result(ResultSchema::new("bench.method.script_approve.result")),
        move |cx, params| {
            let project_id = required(&params, "project_id")?;
            service::approve(
                cx,
                &s,
                project_id,
                params.get("kind").and_then(Value::as_str),
            )
            .map_err(RpcError::app)
        },
    );
}

fn required<'a>(params: &'a Map<String, Value>, name: &str) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params(format!("missing `{name}`")))
}
