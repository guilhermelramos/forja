use crate::engine;
use crate::state::Shared;
use forja_contracts::GitStatusChanged;
use forja_kernel::forja_protocol::RpcError;
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{Map, Value};
pub(crate) fn refresh_now(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    path: String,
) -> forja_kernel::Result<Value> {
    let report = engine::status(std::path::Path::new(&path)).map_err(kernel_error)?;
    shared.set_report(report.clone());
    shared.invalidate_log();
    cx.emit(&GitStatusChanged {
        bench_path: path,
        summary: report.summary.clone(),
    });
    Ok(serde_json::to_value(report)?)
}

pub(crate) fn refresh_async(cx: &mut Ctx<'_>, shared: &Shared) {
    let Some(path) = shared.path() else {
        return;
    };
    if !shared.begin_refresh(0) {
        return;
    }
    let state = shared.clone();
    let target = path.clone();
    cx.spawn(
        move || engine::status(std::path::Path::new(&target)),
        move |cx, result| match result {
            Ok(report) => {
                let summary = report.summary.clone();
                state.set_report(report);
                cx.emit(&GitStatusChanged {
                    bench_path: path,
                    summary,
                });
            }
            Err(error) => {
                state.finish_refresh();
                state.set_error(Some(error));
            }
        },
    );
}

pub(crate) fn unit(result: Result<(), String>) -> Result<String, String> {
    result.map(|_| String::new())
}

pub(crate) fn spawn_op<F>(cx: &mut Ctx<'_>, shared: &Shared, work: F)
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
{
    spawn_op_with_success(cx, shared, work, || {});
}

pub(crate) fn spawn_op_with_success<F, S>(cx: &mut Ctx<'_>, shared: &Shared, work: F, on_success: S)
where
    F: FnOnce() -> Result<String, String> + Send + 'static,
    S: FnOnce() + Send + 'static,
{
    let state = shared.clone();
    state.set_busy(true);
    state.set_error(None);
    state.set_notice(None);
    cx.spawn(work, move |cx, result| {
        state.set_busy(false);
        match result {
            Ok(text) => {
                on_success();
                let text = text.trim().to_owned();
                state.set_notice(Some(if text.is_empty() {
                    cx.t("git.notice.ok")
                } else {
                    text
                }));
            }
            Err(error) => state.set_error(Some(error)),
        }
        state.invalidate_log();
        refresh_async(cx, &state);
    });
}
pub(crate) fn path_arg(args: &Args) -> forja_kernel::Result<&str> {
    args.required_str("path")
}

pub(crate) fn files_arg(args: &Args) -> forja_kernel::Result<(String, Vec<String>)> {
    let path = args.required_str("path")?.to_owned();
    let files: Vec<String> = args
        .get("files")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if files.is_empty() {
        return Err(kernel_error("no file selected".to_owned()));
    }
    Ok((path, files))
}

pub(crate) fn files_from(params: &Map<String, Value>) -> Result<Vec<String>, RpcError> {
    let files = params
        .get("files")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if files.is_empty() {
        return Err(RpcError::invalid_params("`files` cannot be empty"));
    }
    Ok(files)
}

pub(crate) fn required<'a>(
    params: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::invalid_params(format!("missing `{name}`")))
}

pub(crate) fn setting_str(cx: &Ctx<'_>, key: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub(crate) fn truthy(cx: &Ctx<'_>, key: &str) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(false)
}

pub(crate) fn setting_list(cx: &Ctx<'_>, key: &str, fallback: &[&str]) -> Vec<String> {
    cx.setting(key)
        .and_then(SettingValue::as_list)
        .map(|list| list.to_vec())
        .unwrap_or_else(|| fallback.iter().map(|value| (*value).to_owned()).collect())
}

pub(crate) fn kernel_error(message: String) -> forja_kernel::KernelError {
    forja_kernel::KernelError::Message(message)
}
