//! Despacho de tarefas para bancadas e ajudantes, limpeza e reenvios.

use crate::cleanup::{self, CleanupPolicy};
use crate::commands::notify;
use crate::model::{Assignment, AssignmentState, MusterRun, RunState, TaskState};
use crate::preamble::helper_preamble;
use crate::state::{self, with, Shared};
use forja_contracts::BenchStoreOutcome;
use forja_kernel::{Args, Ctx, KernelError, SettingValue};
use serde_json::{json, Value};
use std::sync::Arc;

pub(crate) fn check_parallel(cx: &Ctx<'_>, shared: &Shared) -> Result<(), KernelError> {
    let max_parallel = cx
        .setting("muster.dispatch.max_parallel")
        .and_then(SettingValue::as_i64)
        .unwrap_or(4)
        .clamp(1, 16) as usize;
    let active = with(shared, |state| {
        state
            .store
            .assignments
            .iter()
            .filter(|assignment| {
                matches!(
                    assignment.state,
                    AssignmentState::Starting
                        | AssignmentState::Running
                        | AssignmentState::NeedsYou
                )
            })
            .count()
            + state.store.pending_dispatches.len()
    })
    .unwrap_or(0);
    if active >= max_parallel {
        return Err(KernelError::Message(format!(
            "o limite de {max_parallel} ajudantes em paralelo já foi atingido"
        )));
    }
    Ok(())
}

/// Pede uma bancada nova ao `bench.create`. O despacho só continua quando
pub(crate) fn create_dispatch_bench(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    task_id: &str,
    run_id: &str,
    profile: Option<String>,
    permission: Option<String>,
) -> Result<Value, KernelError> {
    let current = cx
        .run_command("bench.current", Args::new())
        .map_err(|error| KernelError::Message(cx.t_args("muster.error.bench-service", &[("detail", &error.to_string())])))?;
    let project_id = current
        .get("project_id")
        .and_then(Value::as_str)
        .ok_or_else(|| KernelError::Message(cx.t("muster.error.bench-service-empty")))?
        .to_owned();
    let bench_name = format!("muster-{task_id}");
    let mut create = Args::new();
    create.insert("project_id", json!(project_id));
    create.insert("name", json!(bench_name));
    let created = cx
        .run_command("bench.create", create)
        .map_err(|error| KernelError::Message(cx.t_args("muster.error.bench-service", &[("detail", &error.to_string())])))?;
    let bench_id = created
        .get("bench_id")
        .and_then(Value::as_str)
        .ok_or_else(|| KernelError::Message(cx.t("muster.error.bench-service-empty")))?
        .to_owned();
    let name = created
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&bench_name)
        .to_owned();
    let pending = with(shared, |state| {
        state
            .store
            .queue_dispatch(task_id, run_id, &bench_id, &name, profile, permission)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?;
    state::persist(cx, shared);
    Ok(json!({
        "pending": true,
        "bench_id": bench_id,
        "dispatch": pending
    }))
}

pub(crate) fn launch_task(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    task_id: &str,
    run: &MusterRun,
    profile: Option<String>,
    permission: Option<String>,
    bench_path: &str,
) -> Result<Value, KernelError> {
    check_parallel(cx, shared)?;
    let task_for_preamble = with(shared, |state| state.store.task(task_id).cloned())
        .flatten()
        .ok_or_else(|| KernelError::Message("tarefa sumiu".to_owned()))?;
    let preamble = helper_preamble(run, &task_for_preamble, bench_path);
    let assignment = with(shared, |state| {
        state
            .store
            .start(task_id, profile.as_deref().unwrap_or(""), bench_path)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;

    open_helper(
        cx,
        shared,
        assignment,
        profile,
        permission,
        bench_path,
        preamble,
    )
}

pub(crate) fn open_helper(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    assignment: Assignment,
    profile: Option<String>,
    permission: Option<String>,
    bench_path: &str,
    preamble: String,
) -> Result<Value, KernelError> {
    let mut launch = Args::new();
    if let Some(profile) = &profile {
        launch.insert("profile_id", json!(profile));
    }
    launch.insert("bench_path", json!(bench_path));
    launch.insert("prompt", json!(preamble));
    if let Some(permission) = &permission {
        launch.insert("permission", json!(permission));
    }
    let opened = cx.run_command("helper.launch", launch);
    let opened = match opened {
        Ok(value) => value,
        Err(error) => {
            let reason = match error {
                KernelError::Command { message, .. } => message,
                other => other.to_string(),
            };
            with(shared, |state| {
                state.store.launch_failed(&assignment.id, &reason)
            });
            state::persist(cx, shared);
            return Err(KernelError::Message(format!(
                "não foi possível abrir o ajudante: {reason}"
            )));
        }
    };
    let profile_id = opened
        .get("profile_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| profile.as_deref().unwrap_or("auto"))
        .to_owned();
    let terminal_id = opened
        .get("terminal_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let session_id = opened
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mode = opened
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("tui");
    let launched = with(shared, |state| {
        state
            .store
            .launched(&assignment.id, &profile_id, terminal_id, session_id, mode)
            .map(|_| state.store.assignment(&assignment.id).cloned())
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    let assignment =
        launched.ok_or_else(|| KernelError::Message("atribuição sumiu".to_owned()))?;
    state::persist(cx, shared);
    Ok(json!({
        "assignment": assignment,
        "bench_path": bench_path,
        "preamble": preamble
    }))
}

pub(crate) fn retry_task(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    request: &crate::model::RetryRequest,
) -> Result<Value, KernelError> {
    let run = with(shared, |state| {
        state
            .store
            .runs
            .iter()
            .find(|run| run.id == request.run)
            .cloned()
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("mutirão não encontrado".to_owned()))?;
    let task = with(shared, |state| state.store.task(&request.task).cloned())
        .flatten()
        .ok_or_else(|| KernelError::Message("tarefa sumiu".to_owned()))?;
    let assignment = with(shared, |state| {
        state.store.retry(
            &request.task,
            &request.profile_id,
            &request.bench_path,
        )
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    let preamble = helper_preamble(&run, &task, &request.bench_path);
    open_helper(
        cx,
        shared,
        assignment,
        Some(request.profile_id.clone()),
        None,
        &request.bench_path,
        preamble,
    )
}

pub(crate) fn configure_dispatch_profile(cx: &Ctx<'_>, key: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "auto")
        .unwrap_or("auto")
        .to_owned()
}

pub(crate) fn finish_dispatch(cx: &mut Ctx<'_>, shared: &Shared, bench_id: &str, path: &str) {
    state::ensure_loaded(cx, shared);
    let Some(pending) = with(shared, |state| state.store.resolve_dispatch(bench_id)).flatten()
    else {
        return;
    };
    let Some(run) = with(shared, |state| {
        state
            .store
            .runs
            .iter()
            .find(|run| run.id == pending.run)
            .cloned()
    })
    .flatten()
    else {
        return;
    };
    with(shared, |state| {
        state.store.own_bench(&pending.run, &pending.task, bench_id)
    });
    let result = launch_task(
        cx,
        shared,
        &pending.task,
        &run,
        pending.profile_id,
        pending.permission,
        path,
    );
    if let Err(error) = result {
        with(shared, |state| {
            if let Some(task) = state.store.task(&pending.task) {
                let task_id = task.id.clone();
                if let Some(task) = state.store.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.state = TaskState::Failed;
                }
            }
        });
        let body = cx.t_args("muster.error.bench-service", &[("detail", &error.to_string())]);
        notify(cx, "muster.notify.alert-title", &body, None);
    }
    state::persist(cx, shared);
}

/// `bench.create_failed` fecha o despacho pelo mesmo `bench_id` e avisa.
pub(crate) fn fail_dispatch(cx: &mut Ctx<'_>, shared: &Shared, bench_id: &str, reason: &str) {
    state::ensure_loaded(cx, shared);
    let closed = with(shared, |state| state.store.fail_dispatch(bench_id, reason)).flatten();
    if closed.is_some() {
        state::persist(cx, shared);
        let body = cx.t_args("muster.notify.alert-body", &[("tail", reason)]);
        notify(cx, "muster.notify.alert-title", &body, None);
    }
}

/// Decide a limpeza quando um mutirão termina. Só bancadas que o mutirão criou
/// chegam ao plano; a remoção roda em tarefa de fundo, por `bench.store`.
pub(crate) fn maybe_cleanup(cx: &mut Ctx<'_>, shared: &Shared) {
    let busy = with(shared, |state| {
        state.store.assignments.iter().any(|assignment| {
            matches!(
                assignment.state,
                AssignmentState::Starting | AssignmentState::Running | AssignmentState::NeedsYou
            )
        }) || !state.store.retry_candidates().is_empty()
            || !state.store.pending_dispatches.is_empty()
    })
    .unwrap_or(true);
    if busy {
        return;
    }
    let policy = CleanupPolicy::parse(
        cx.setting("muster.cleanup.on_finish")
            .and_then(SettingValue::as_str)
            .unwrap_or("ask"),
    );
    let finished: Vec<String> = with(shared, |state| {
        state
            .store
            .runs
            .iter()
            .filter(|run| {
                matches!(run.state, RunState::Done | RunState::Failed) && !run.cleanup_done
            })
            .map(|run| run.id.clone())
            .collect()
    })
    .unwrap_or_default();
    for run_id in finished {
        if !with(shared, |state| state.store.take_cleanup(&run_id)).unwrap_or(false) {
            continue;
        }
        let owned: Vec<String> = with(shared, |state| {
            state
                .store
                .owned_benches
                .iter()
                .filter(|bench| bench.run == run_id)
                .map(|bench| bench.bench_id.clone())
                .collect()
        })
        .unwrap_or_default();
        if owned.is_empty() {
            continue;
        }
        match policy {
            CleanupPolicy::Ask => {
                with(shared, |state| {
                    state.notice = Some("muster-notice-cleanup-ask".to_owned())
                });
            }
            CleanupPolicy::Keep => {}
            CleanupPolicy::Remove => {
                let shared = Arc::clone(shared);
                cx.spawn(
                    move || (run_id, owned),
                    move |cx, (run_id, owned)| cleanup_benches(cx, &shared, &run_id, &owned),
                );
            }
        }
    }
    state::persist(cx, shared);
}

/// Aplica a decisão de limpeza. Não consulta git nem remove nada diretamente:
/// só pede `bench.store` para as bancadas que o mutirão possui. O módulo de
/// bancadas confere o git no work closure da própria tarefa, logo antes de
/// apagar, e publica o desfecho em `bench.store_outcome`.
pub(crate) fn cleanup_benches(cx: &mut Ctx<'_>, shared: &Shared, run_id: &str, ids: &[String]) {
    let plan = with(shared, |state| {
        cleanup::plan(&state.store, run_id, CleanupPolicy::Remove)
    })
    .unwrap_or_default();
    for id in ids.iter().filter(|id| plan.contains(id)) {
        let mut args = Args::new();
        args.insert("bench_id", json!(id));
        // Sem `force`: a própria API de bancadas barra bancada suja.
        let _ = cx.run_command("bench.store", args);
    }
    state::persist(cx, shared);
}

/// Publica o desfecho da remoção assíncrona no estado do Mutirão. Este callback
/// só aplica o resultado recebido; nenhuma checagem de git acontece aqui.
pub(crate) fn on_bench_store_outcome(cx: &mut Ctx<'_>, shared: &Shared, outcome: &BenchStoreOutcome) {
    let owned = with(shared, |state| {
        state
            .store
            .owned_benches
            .iter()
            .any(|bench| bench.bench_id == outcome.bench_id)
    })
    .unwrap_or(false);
    if !owned {
        return;
    }
    match outcome.outcome.as_str() {
        "stored" => {
            with(shared, |state| {
                state
                    .store
                    .owned_benches
                    .retain(|bench| bench.bench_id != outcome.bench_id)
            });
        }
        "kept_dirty" => {
            with(shared, |state| {
                state.notice = Some("muster-notice-cleanup-dirty".to_owned())
            });
        }
        _ => {}
    }
    state::persist(cx, shared);
}

