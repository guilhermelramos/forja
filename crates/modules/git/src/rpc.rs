use crate::comparison;
use crate::diff;
use crate::engine;
use crate::helpers::{files_from, required, setting_list, setting_str, spawn_op, truthy, unit};
use crate::log;
use crate::state::Shared;
use forja_contracts::GitStatusChanged;
use forja_kernel::Registrar;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use serde_json::{Value, json};
pub(crate) fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let s = shared.clone();
    r.rpc(
        Method::new("git.status", "git.method.status.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String).alias("caminho"))
            .result(ResultSchema::new("git.method.status.result")),
        move |cx, params| {
            let path = required(&params, "path")?;
            let report = engine::status(std::path::Path::new(path)).map_err(RpcError::app)?;
            if s.path().as_deref() == Some(path) {
                s.set_report(report.clone());
            }
            cx.emit(&GitStatusChanged {
                bench_path: path.to_owned(),
                summary: report.summary.clone(),
            });
            Ok(serde_json::to_value(report.summary).unwrap_or(Value::Null))
        },
    );

    r.rpc(
        Method::new("git.changes", "git.method.changes.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.changes.result")),
        move |_cx, params| {
            let path = required(&params, "path")?;
            let report = engine::status(std::path::Path::new(path)).map_err(RpcError::app)?;
            Ok(serde_json::to_value(report).unwrap_or(Value::Null))
        },
    );

    r.rpc(
        Method::new("git.diff", "git.method.diff.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("file", ParamKind::String).alias("arquivo"))
            .param(ParamSchema::optional("staged", ParamKind::Boolean))
            .param(ParamSchema::optional("comparison", ParamKind::String))
            .result(ResultSchema::new("git.method.diff.result")),
        move |_cx, params| {
            let path = required(&params, "path")?;
            let file = required(&params, "file")?;
            let staged = params
                .get("staged")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let diff = match params.get("comparison").and_then(Value::as_str) {
                Some(range) => comparison::diff_file(std::path::Path::new(path), file, range),
                None => diff::diff(std::path::Path::new(path), file, staged),
            }
            .map_err(RpcError::app)?;
            Ok(serde_json::to_value(diff).unwrap_or(Value::Null))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.stage", "git.method.stage.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("files", ParamKind::Json).alias("arquivos"))
            .result(ResultSchema::new("git.method.stage.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let files = files_from(&params)?;
            spawn_op(cx, &s, move || {
                unit(engine::stage(std::path::Path::new(&path), &files))
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.unstage", "git.method.unstage.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("files", ParamKind::Json))
            .result(ResultSchema::new("git.method.unstage.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let files = files_from(&params)?;
            spawn_op(cx, &s, move || {
                unit(engine::unstage(std::path::Path::new(&path), &files))
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.discard", "git.method.discard.description")
            .effect(MethodEffect::Destructive)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("files", ParamKind::Json))
            .param(ParamSchema::required("confirm", ParamKind::Boolean))
            .result(ResultSchema::new("git.method.discard.result")),
        move |cx, params| {
            if !params
                .get("confirm")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                return Err(RpcError::invalid_params("discard needs `confirm: true`"));
            }
            let path = required(&params, "path")?.to_owned();
            let files = files_from(&params)?;
            spawn_op(cx, &s, move || {
                unit(engine::discard(std::path::Path::new(&path), &files))
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.commit", "git.method.commit.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("message", ParamKind::String).alias("mensagem"))
            .param(ParamSchema::optional("all", ParamKind::Boolean).alias("tudo"))
            .result(ResultSchema::new("git.method.commit.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let message = required(&params, "message")?.to_owned();
            let all = params.get("all").and_then(Value::as_bool).unwrap_or(false);
            let sign = truthy(cx, "git.commit.sign");
            let sign_off = truthy(cx, "git.commit.sign_off");
            let gpg_key = setting_str(cx, "git.commit.gpg_key");
            let name = setting_str(cx, "git.identity.name");
            let email = setting_str(cx, "git.identity.email");
            let protected = setting_list(cx, "git.protected.branches", &["main", "master"]);
            spawn_op(cx, &s, move || {
                let path = std::path::Path::new(&path);
                if let Some(branch) = engine::protected_branch(path, &protected)? {
                    return Err(format!("protected:{branch}"));
                }
                engine::commit(
                    path,
                    &engine::CommitOptions {
                        message: &message,
                        all,
                        sign,
                        gpg_key: &gpg_key,
                        sign_off,
                        name: &name,
                        email: &email,
                    },
                )
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.push", "git.method.push.description")
            .effect(MethodEffect::Write)
            .network(true)
            .idempotent(false)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(
                ParamSchema::optional("force_with_lease", ParamKind::Boolean)
                    .alias("force-with-lease"),
            )
            .param(ParamSchema::optional("publish", ParamKind::Boolean))
            .result(ResultSchema::new("git.method.push.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let force = params
                .get("force_with_lease")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let publish = params
                .get("publish")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let set_upstream = truthy(cx, "git.push.auto_set_upstream");
            spawn_op(cx, &s, move || {
                engine::push(std::path::Path::new(&path), force, publish, set_upstream)
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.pull", "git.method.pull.description")
            .effect(MethodEffect::Write)
            .network(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.pull.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            spawn_op(cx, &s, move || engine::pull(std::path::Path::new(&path)));
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.fetch", "git.method.fetch.description")
            .effect(MethodEffect::Write)
            .network(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.fetch.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            spawn_op(cx, &s, move || engine::fetch(std::path::Path::new(&path)));
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.sync", "git.method.sync.description")
            .effect(MethodEffect::Write)
            .network(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.sync.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let set_upstream = truthy(cx, "git.push.auto_set_upstream");
            spawn_op(cx, &s, move || {
                engine::sync(std::path::Path::new(&path), set_upstream)
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.publish", "git.method.publish.description")
            .effect(MethodEffect::Write)
            .network(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.publish.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            spawn_op(cx, &s, move || engine::publish(std::path::Path::new(&path)));
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.rebase", "git.method.rebase.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::optional("base", ParamKind::String))
            .result(ResultSchema::new("git.method.rebase.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            let base = params
                .get("base")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let protected = setting_list(cx, "git.protected.branches", &["main", "master"]);
            spawn_op(cx, &s, move || {
                let path = std::path::Path::new(&path);
                if let Some(branch) = engine::protected_branch(path, &protected)? {
                    return Err(format!("protected:{branch}"));
                }
                engine::rebase(path, base.as_deref())
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.rpc(
        Method::new("git.abort", "git.method.abort.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.abort.result")),
        move |cx, params| {
            let path = required(&params, "path")?.to_owned();
            spawn_op(cx, &s, move || engine::abort(std::path::Path::new(&path)));
            Ok(json!({ "pending": true }))
        },
    );

    r.rpc(
        Method::new("git.conflicts", "git.method.conflicts.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .result(ResultSchema::new("git.method.conflicts.result")),
        move |_cx, params| {
            let path = required(&params, "path")?;
            let files = engine::conflicts(std::path::Path::new(path)).map_err(RpcError::app)?;
            Ok(json!({ "files": files }))
        },
    );

    r.rpc(
        Method::new("git.log", "git.method.log.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::optional("limit", ParamKind::Integer))
            .result(ResultSchema::new("git.method.log.result")),
        move |_cx, params| {
            let path = required(&params, "path")?;
            let limit = params
                .get("limit")
                .and_then(Value::as_i64)
                .unwrap_or(200)
                .clamp(1, 2000) as u32;
            let lines = log::log(std::path::Path::new(path), limit).map_err(RpcError::app)?;
            Ok(json!({ "lines": lines }))
        },
    );

    r.rpc(
        Method::new("git.blame", "git.method.blame.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("path", ParamKind::String))
            .param(ParamSchema::required("file", ParamKind::String).alias("arquivo"))
            .result(ResultSchema::new("git.method.blame.result")),
        move |_cx, params| {
            let path = required(&params, "path")?;
            let file = required(&params, "file")?;
            let lines = log::blame(std::path::Path::new(path), file).map_err(RpcError::app)?;
            Ok(json!({ "lines": lines }))
        },
    );
}
