//! Comandos, RPC e efeitos externos do Mutirão.

use crate::delivery::{self, Delivery};
use crate::model::{
    AssignmentState, HandoffResult, MusterRun, NoteKind, TaskState,
};
use crate::preamble::helper_preamble;
use crate::state::{self, with, Shared};
use forja_contracts::{HelperStatus, HelperStatusChanged, TerminalCommandFinished, TerminalExited};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{Args, Ctx, KernelError, Registrar, SettingValue};
use serde_json::{json, Value};
use std::sync::Arc;

/// Chaves i18n usadas pelos comandos e notificações.
pub const USED_KEYS: &[&str] = &[
    "muster.name",
    "muster.command.run",
    "muster.command.task",
    "muster.command.start",
    "muster.command.note",
    "muster.command.reply",
    "muster.command.handoff",
    "muster.command.inbox",
    "muster.command.status",
    "muster.command.open",
    "muster.command.plan",
    "muster.command.budget",
    "muster.command.spend",
    "muster.method.run.description",
    "muster.method.task.description",
    "muster.method.start.description",
    "muster.method.note.description",
    "muster.method.reply.description",
    "muster.method.handoff.description",
    "muster.method.inbox.description",
    "muster.method.status.description",
    "muster.method.plan.description",
    "muster.method.plan.result",
    "muster.method.budget.description",
    "muster.method.budget.result",
    "muster.method.spend.description",
    "muster.method.spend.result",
    "muster.notify.held-title",
    "muster.notify.held-body",
    "muster.notify.alert-title",
    "muster.notify.alert-body",
    "muster.notify.handoff-title",
    "muster.notify.handoff-body",
    "muster.error.bench-pending",
    "muster-task-pending",
    "muster-task-running",
    "muster-task-delivered",
    "muster-task-failed",
    "muster-task-blocked",
    "muster-assignment-starting",
    "muster-assignment-running",
    "muster-assignment-needs-you",
    "muster-assignment-idle",
    "muster-assignment-delivered",
    "muster-assignment-failed",
    "muster-assignment-blocked",
    "muster-assignment-stopped",
    "muster-notice-load-failed",
    "muster.page-loading",
];

pub(super) fn register(r: &mut Registrar<'_>, shared: &Shared) {
    let run_shared = Arc::clone(shared);
    r.command("muster.run", "muster.command.run", move |cx, args| {
        run(cx, &run_shared, &args)
    });
    let task_shared = Arc::clone(shared);
    r.command("muster.task", "muster.command.task", move |cx, args| {
        task(cx, &task_shared, &args)
    });
    let start_shared = Arc::clone(shared);
    r.command("muster.start", "muster.command.start", move |cx, args| {
        start(cx, &start_shared, &args)
    });
    let note_shared = Arc::clone(shared);
    r.command("muster.note", "muster.command.note", move |cx, args| {
        note(cx, &note_shared, &args)
    });
    let reply_shared = Arc::clone(shared);
    r.command("muster.reply", "muster.command.reply", move |cx, args| {
        reply(cx, &reply_shared, &args)
    });
    let handoff_shared = Arc::clone(shared);
    r.command("muster.handoff", "muster.command.handoff", move |cx, args| {
        handoff(cx, &handoff_shared, &args)
    });
    let inbox_shared = Arc::clone(shared);
    r.command("muster.inbox", "muster.command.inbox", move |cx, args| {
        inbox(cx, &inbox_shared, &args)
    });
    let status_shared = Arc::clone(shared);
    r.command("muster.status", "muster.command.status", move |cx, args| {
        status(cx, &status_shared, &args)
    });
    let plan_shared = Arc::clone(shared);
    r.command("muster.plan.approve", "muster.command.plan", move |cx, args| {
        plan_approve(cx, &plan_shared, &args)
    });
    let budget_shared = Arc::clone(shared);
    r.command("muster.budget.set", "muster.command.budget", move |cx, args| {
        budget_set(cx, &budget_shared, &args)
    });
    let spend_shared = Arc::clone(shared);
    r.command("muster.spend", "muster.command.spend", move |cx, args| {
        spend(cx, &spend_shared, &args)
    });
    r.command("muster.open", "muster.command.open", |cx, _| {
        let mut args = Args::new();
        args.insert("view_id", json!("muster.page"));
        cx.run_command("shell.rail.open", args)
            .map_err(|error| KernelError::Message(format!("não foi possível abrir o Mutirão: {error}")))?;
        Ok(json!({ "open": true }))
    });

    register_rpc(r, shared);
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let run_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.run", "muster.method.run.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("title", ParamKind::String))
            .param(ParamSchema::optional("objective", ParamKind::String))
            .param(ParamSchema::optional("coordinator", ParamKind::String))
            .param(ParamSchema::optional("bench_path", ParamKind::String).alias("bench"))
            .param(ParamSchema::optional("budget_usd", ParamKind::Number))
            .result(ResultSchema::new("muster.method.run.result")),
        move |cx, params| rpc(run(cx, &run_shared, &Args::from_map(params))),
    );

    let task_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.task", "muster.method.task.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("run", ParamKind::String))
            .param(ParamSchema::required("title", ParamKind::String))
            .param(ParamSchema::optional("spec", ParamKind::String))
            .param(ParamSchema::optional("deps", ParamKind::String))
            .result(ResultSchema::new("muster.method.task.result")),
        move |cx, params| rpc(task(cx, &task_shared, &Args::from_map(params))),
    );

    let start_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.start", "muster.method.start.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("task", ParamKind::String))
            .param(ParamSchema::optional("profile_id", ParamKind::String).alias("profile"))
            .param(ParamSchema::optional("bench_path", ParamKind::String).alias("bench"))
            .param(ParamSchema::optional("new_bench", ParamKind::Boolean))
            .param(ParamSchema::optional("permission", ParamKind::String))
            .result(ResultSchema::new("muster.method.start.result")),
        move |cx, params| rpc(start(cx, &start_shared, &Args::from_map(params))),
    );

    let note_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.note", "muster.method.note.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("body", ParamKind::String))
            .param(ParamSchema::optional("task", ParamKind::String))
            .param(ParamSchema::optional("assignment", ParamKind::String))
            .param(ParamSchema::optional("kind", ParamKind::String))
            .param(ParamSchema::optional("from", ParamKind::String))
            .result(ResultSchema::new("muster.method.note.result")),
        move |cx, params| rpc(note(cx, &note_shared, &Args::from_map(params))),
    );

    let reply_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.reply", "muster.method.reply.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("note", ParamKind::String))
            .param(ParamSchema::required("body", ParamKind::String))
            .result(ResultSchema::new("muster.method.reply.result")),
        move |cx, params| rpc(reply(cx, &reply_shared, &Args::from_map(params))),
    );

    let handoff_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.handoff", "muster.method.handoff.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("summary", ParamKind::String))
            .param(ParamSchema::optional("result", ParamKind::String))
            .param(ParamSchema::optional("evidence", ParamKind::String))
            .param(ParamSchema::optional("task", ParamKind::String))
            .param(ParamSchema::optional("assignment", ParamKind::String))
            .result(ResultSchema::new("muster.method.handoff.result")),
        move |cx, params| rpc(handoff(cx, &handoff_shared, &Args::from_map(params))),
    );

    let inbox_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.inbox", "muster.method.inbox.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("run", ParamKind::String))
            .param(ParamSchema::optional("all", ParamKind::Boolean))
            .result(ResultSchema::new("muster.method.inbox.result")),
        move |cx, params| rpc(inbox(cx, &inbox_shared, &Args::from_map(params))),
    );

    let status_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.status", "muster.method.status.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("run", ParamKind::String))
            .result(ResultSchema::new("muster.method.status.result")),
        move |cx, params| rpc(status(cx, &status_shared, &Args::from_map(params))),
    );

    let plan_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.plan.approve", "muster.method.plan.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("run", ParamKind::String))
            .result(ResultSchema::new("muster.method.plan.result")),
        move |cx, params| rpc(plan_approve(cx, &plan_shared, &Args::from_map(params))),
    );
    let budget_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.budget.set", "muster.method.budget.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("run", ParamKind::String))
            .param(ParamSchema::required("budget_usd", ParamKind::Number))
            .result(ResultSchema::new("muster.method.budget.result")),
        move |cx, params| rpc(budget_set(cx, &budget_shared, &Args::from_map(params))),
    );
    let spend_shared = Arc::clone(shared);
    r.rpc(
        Method::new("muster.spend", "muster.method.spend.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("run", ParamKind::String))
            .param(ParamSchema::required("usd", ParamKind::Number))
            .result(ResultSchema::new("muster.method.spend.result")),
        move |cx, params| rpc(spend(cx, &spend_shared, &Args::from_map(params))),
    );
}

fn run(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    if !cx
        .setting("muster.mode.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true)
    {
        return Err(KernelError::Message("muster is disabled".to_owned()));
    }
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let title = args.required_str("title")?;
    let objective = args.str("objective").unwrap_or_default();
    let coordinator = args.str("coordinator").unwrap_or("coordinator");
    let bench_path = args.str("bench_path").map(str::to_owned);
    let require_approval = cx
        .setting("muster.plan.require_approval")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let default_budget = cx
        .setting("muster.budget.max_usd")
        .and_then(SettingValue::as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let budget = args
        .get("budget_usd")
        .and_then(Value::as_f64)
        .unwrap_or(default_budget)
        .max(0.0);
    let run = with(shared, |state| {
        let mut run = state
            .store
            .create_run(title, objective, coordinator, bench_path);
        run.plan_approved = !require_approval;
        run.budget_usd = budget;
        if let Some(stored) = state.store.runs.iter_mut().find(|item| item.id == run.id) {
            *stored = run.clone();
        }
        run
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?;
    state::persist(cx, shared);
    Ok(serde_json::to_value(run)?)
}

fn task(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let run_id = args.required_str("run")?.to_owned();
    let title = args.required_str("title")?.to_owned();
    let spec = args.str("spec").unwrap_or_default().to_owned();
    let deps = parse_deps(args.str("deps").unwrap_or_default());
    let max_tasks = cx
        .setting("muster.plan.max_tasks")
        .and_then(SettingValue::as_i64)
        .unwrap_or(20)
        .clamp(1, 100) as usize;
    let count = with(shared, |state| {
        state
            .store
            .tasks
            .iter()
            .filter(|task| task.run == run_id)
            .count()
    })
    .unwrap_or(0);
    if count >= max_tasks {
        return Err(KernelError::Message(format!(
            "o plano já tem {max_tasks} tarefas, o limite configurado"
        )));
    }
    let task = with(shared, |state| {
        state.store.add_task(&run_id, &title, &spec, deps)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    state::persist(cx, shared);
    Ok(serde_json::to_value(task)?)
}

fn start(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    if !cx
        .setting("muster.mode.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true)
    {
        return Err(KernelError::Message("muster is disabled".to_owned()));
    }
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let task_id = args.required_str("task")?.to_owned();
    let profile = args.str("profile_id").map(str::to_owned);
    let permission = args.str("permission").map(str::to_owned);
    let new_bench = args.bool("new_bench").unwrap_or(false);
    let bench_arg = args.str("bench_path").map(str::to_owned);
    if new_bench && bench_arg.is_none() {
        return Err(KernelError::Message(cx.t("muster.error.bench-pending")));
    }

    let run_id = with(shared, |state| {
        state
            .store
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .map(|task| task.run.clone())
    })
    .flatten()
    .ok_or_else(|| KernelError::Message(format!("tarefa `{task_id}` não encontrada")))?;
    let run = with(shared, |state| {
        state.store.runs.iter().find(|run| run.id == run_id).cloned()
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("mutirão não encontrado".to_owned()))?;
    if !run.plan_approved {
        return Err(KernelError::Message(
            "o plano do mutirão precisa ser aprovado antes de iniciar".to_owned(),
        ));
    }
    if run.budget_usd > 0.0 && run.spent_usd >= run.budget_usd {
        return Err(KernelError::Message(format!(
            "o teto de US$ {:.2} do mutirão já foi atingido",
            run.budget_usd
        )));
    }

    let bench_path = match bench_arg {
        Some(path) => path,
        None => current_bench_path(cx)
            .ok_or_else(|| KernelError::Message("sem bancada atual; informe bench_path".to_owned()))?,
    };

    let task_for_preamble = with(shared, |state| {
        state.store.task(&task_id).cloned()
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("tarefa sumiu".to_owned()))?;
    let max_parallel = cx
        .setting("muster.dispatch.max_parallel")
        .and_then(|value| value.as_i64())
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
    })
    .unwrap_or(0);
    if active >= max_parallel {
        return Err(KernelError::Message(format!(
            "o limite de {max_parallel} ajudantes em paralelo já foi atingido"
        )));
    }
    let preamble = helper_preamble(&run, &task_for_preamble, &bench_path);
    let assignment = with(shared, |state| {
        state.store.start(&task_id, profile.as_deref().unwrap_or(""), &bench_path)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;

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
            with(shared, |state| {
                state
                    .store
                    .launch_failed(&assignment.id, &error.to_string())
            });
            state::persist(cx, shared);
            return Err(KernelError::Message(format!(
                "não foi possível abrir o ajudante: {error}"
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

fn note(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let body = args.required_str("body")?.to_owned();
    let task = args.str("task").map(str::to_owned);
    let assignment = args.str("assignment").map(str::to_owned);
    let from = args.str("from").unwrap_or("helper");
    let kind = parse_kind(args.str("kind").unwrap_or("status"));

    let assignment_id = with(shared, |state| {
        state
            .store
            .resolve_assignment(assignment.as_deref(), task.as_deref())
            .map(|assignment| assignment.id.clone())
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;

    if from == "coordinator" {
        let note = with(shared, |state| {
            state.store.note_to_helper(&assignment_id, &body, kind)
        })
        .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
        .map_err(KernelError::Message)?;
        let delivery = deliver(cx, shared, &note.id, &assignment_id)?;
        return Ok(json!({
            "note": note,
            "queued": delivery.queued,
            "delivered": delivery.delivered,
            "reason": delivery.reason
        }));
    }

    let note = with(shared, |state| {
        state.store.note_from_helper(&assignment_id, &body, kind)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    state::persist(cx, shared);
    Ok(json!({ "note": note, "queued": false, "delivered": true }))
}

fn reply(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let note_id = args.required_str("note")?.to_owned();
    let body = args.required_str("body")?.to_owned();
    let (note, assignment_id) = with(shared, |state| {
        state.store.reply(&note_id, &body).map(|note| {
            let assignment = note.assignment.clone();
            (note, assignment)
        })
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    let delivery = match assignment_id {
        Some(assignment) => deliver(cx, shared, &note.id, &assignment)?,
        None => DeliveryResult {
            queued: false,
            delivered: true,
            reason: None,
        },
    };
    Ok(json!({
        "note": note,
        "queued": delivery.queued,
        "delivered": delivery.delivered,
        "reason": delivery.reason
    }))
}

fn handoff(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let summary = args.required_str("summary")?.to_owned();
    let result = HandoffResult::parse(args.str("result").unwrap_or("feito"))
        .map_err(KernelError::Message)?;
    let evidence = args.str("evidence").unwrap_or_default().to_owned();
    let require_evidence = cx
        .setting("muster.handoff.require_evidence")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if require_evidence && result == HandoffResult::Done && evidence.trim().is_empty() {
        return Err(KernelError::Message(
            "a entrega exige --evidence com testes, arquivos ou saída".to_owned(),
        ));
    }
    let assignment_id = with(shared, |state| {
        state
            .store
            .resolve_assignment(
                args.str("assignment"),
                args.str("task"),
            )
            .map(|assignment| assignment.id.clone())
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    let note = with(shared, |state| {
        state
            .store
            .handoff(&assignment_id, result, &summary, &evidence)
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?
    .map_err(KernelError::Message)?;
    state::persist(cx, shared);
    let body = cx.t_args(
        "muster.notify.handoff-body",
        &[("result", result.as_str()), ("summary", &summary)],
    );
    notify(cx, "muster.notify.handoff-title", &body, None);
    Ok(json!({
        "note": note,
        "result": result.as_str(),
        "delivered": true
    }))
}

fn plan_approve(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let run_id = args.required_str("run")?.to_owned();
    let run = with(shared, |state| {
        let run = state.store.runs.iter_mut().find(|run| run.id == run_id)?;
        run.plan_approved = true;
        Some(run.clone())
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("mutirão não encontrado".to_owned()))?;
    state::persist(cx, shared);
    Ok(serde_json::to_value(run)?)
}

fn budget_set(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let run_id = args.required_str("run")?.to_owned();
    let budget = args
        .get("budget_usd")
        .and_then(Value::as_f64)
        .ok_or_else(|| KernelError::Message("informe budget_usd".to_owned()))?
        .max(0.0);
    let run = with(shared, |state| {
        let run = state.store.runs.iter_mut().find(|run| run.id == run_id)?;
        run.budget_usd = budget;
        Some(run.clone())
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("mutirão não encontrado".to_owned()))?;
    state::persist(cx, shared);
    Ok(serde_json::to_value(run)?)
}

fn spend(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    ensure_writable(shared)?;
    let run_id = args.required_str("run")?.to_owned();
    let usd = args
        .get("usd")
        .and_then(Value::as_f64)
        .ok_or_else(|| KernelError::Message("informe usd".to_owned()))?;
    if usd <= 0.0 {
        return Err(KernelError::Message(
            "o custo registrado precisa ser maior que zero".to_owned(),
        ));
    }
    let run = with(shared, |state| {
        let run = state.store.runs.iter_mut().find(|run| run.id == run_id)?;
        run.spent_usd += usd;
        Some(run.clone())
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("mutirão não encontrado".to_owned()))?;
    state::persist(cx, shared);
    Ok(serde_json::to_value(run)?)
}

fn inbox(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    let run = args.str("run");
    let all = args.bool("all").unwrap_or(false);
    let notes = with(shared, |state| state.store.inbox(run, all)).unwrap_or_default();
    Ok(json!({ "notes": notes }))
}

fn status(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, KernelError> {
    state::ensure_loaded(cx, shared);
    let run = args.str("run");
    let snapshot = with(shared, |state| {
        let runs: Vec<MusterRun> = state
            .store
            .runs
            .iter()
            .filter(|run_item| run.is_none_or(|id| run_item.id == id))
            .cloned()
            .collect();
        let tasks: Vec<Value> = state
            .store
            .tasks
            .iter()
            .filter(|task| run.is_none_or(|id| task.run == id))
            .map(|task| {
                json!({
                    "id": task.id,
                    "run": task.run,
                    "title": task.title,
                    "state": task.state,
                    "result": task.result,
                    "summary": task.summary,
                    "evidence": task.evidence,
                    "ready": state.store.ready_tasks(&task.run).iter().any(|ready| ready.id == task.id),
                })
            })
            .collect();
        let assignments: Vec<Value> = state
            .store
            .assignments
            .iter()
            .filter(|assignment| run.is_none_or(|id| assignment.run == id))
            .map(|assignment| {
                json!({
                    "id": assignment.id,
                    "task": assignment.task,
                    "run": assignment.run,
                    "profile_id": assignment.profile_id,
                    "bench_path": assignment.bench_path,
                    "terminal_id": assignment.terminal_id,
                    "session_id": assignment.session_id,
                    "mode": assignment.mode,
                    "state": assignment.state,
                    "last_status": assignment.last_status,
                    "retries": assignment.retries,
                })
            })
            .collect();
        let notes: Vec<Value> = state
            .store
            .inbox(run, false)
            .into_iter()
            .map(|note| json!(note))
            .collect();
        json!({
            "runs": runs,
            "tasks": tasks,
            "assignments": assignments,
            "inbox": notes,
            "loaded": state.loaded,
            "notice": state.notice
        })
    })
    .ok_or_else(|| KernelError::Message("muster state is unavailable".to_owned()))?;
    Ok(snapshot)
}

#[derive(Clone, Debug)]
struct DeliveryResult {
    queued: bool,
    delivered: bool,
    reason: Option<String>,
}

fn deliver(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    note_id: &str,
    assignment_id: &str,
) -> Result<DeliveryResult, KernelError> {
    let snapshot = with(shared, |state| {
        state.store.assignment(assignment_id).map(|assignment| {
            (
                assignment.state,
                assignment.terminal_id.clone(),
                assignment.profile_id.clone(),
                assignment.task.clone(),
            )
        })
    })
    .flatten()
    .ok_or_else(|| KernelError::Message("atribuição não encontrada".to_owned()))?;
    let (assignment_state, terminal_id, profile, task) = snapshot;
    let body = with(shared, |state| {
        state
            .store
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .map(|note| note.body.clone())
    })
    .flatten()
    .unwrap_or_default();
    let recent = terminal_id
        .as_deref()
        .and_then(|terminal| read_terminal(cx, terminal))
        .unwrap_or_default();
    let decision = delivery::decide(assignment_state, &delivery::recent_lines(&recent));
    if let (Delivery::Send, Some(terminal_id)) = (decision, terminal_id.as_deref()) {
        let sent = send_terminal(cx, terminal_id, &body);
        if sent {
            with(shared, |state| state.store.mark_delivered(note_id));
            state::persist(cx, shared);
            return Ok(DeliveryResult {
                queued: false,
                delivered: true,
                reason: None,
            });
        }
    }
    let reason = match decision {
        Delivery::Send => "o ajudante não tem terminal para receber o recado".to_owned(),
        Delivery::Hold(reason) => reason.to_owned(),
    };
    with(shared, |state| mark_queued(state, note_id));
    state::persist(cx, shared);
    let body = cx.t_args(
        "muster.notify.held-body",
        &[
            ("profile", if profile.is_empty() { "auto" } else { &profile }),
            ("task", &task),
            ("reason", &reason),
        ],
    );
    notify(cx, "muster.notify.held-title", &body, None);
    Ok(DeliveryResult {
        queued: true,
        delivered: false,
        reason: Some(reason),
    })
}

fn mark_queued(state: &mut crate::state::MusterState, note_id: &str) {
    if let Some(note) = state.store.notes.iter_mut().find(|note| note.id == note_id) {
        note.queued = true;
        note.delivered = false;
    }
}

fn send_terminal(cx: &mut Ctx<'_>, terminal_id: &str, text: &str) -> bool {
    let mut args = Args::new();
    args.insert("terminal_id", json!(terminal_id));
    args.insert("text", json!(text));
    args.insert("enter", json!(true));
    cx.run_command("terminal.send", args).is_ok()
}

fn read_terminal(cx: &mut Ctx<'_>, terminal_id: &str) -> Option<String> {
    let mut args = Args::new();
    args.insert("terminal_id", json!(terminal_id));
    args.insert("lines", json!(60));
    cx.run_command("terminal.read", args)
        .ok()
        .and_then(|value| value.get("text").and_then(Value::as_str).map(str::to_owned))
}

fn current_bench(cx: &mut Ctx<'_>) -> Option<Value> {
    cx.run_command("bench.current", Args::new()).ok()
}

fn current_bench_path(cx: &mut Ctx<'_>) -> Option<String> {
    current_bench(cx)
        .and_then(|value| value.get("path").and_then(Value::as_str).map(str::to_owned))
}

fn ensure_writable(shared: &Shared) -> Result<(), KernelError> {
    let (loaded, failed) = with(shared, |state| (state.loaded, state.load_failed))
        .unwrap_or((false, false));
    if !loaded {
        return Err(KernelError::Message("o Mutirão ainda está carregando".to_owned()));
    }
    if failed {
        return Err(KernelError::Message(
            "o arquivo do Mutirão não pôde ser lido; nada foi gravado".to_owned(),
        ));
    }
    Ok(())
}

fn parse_deps(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_kind(text: &str) -> NoteKind {
    match text.trim().to_ascii_lowercase().as_str() {
        "pergunta" | "question" => NoteKind::Question,
        "resposta" | "reply" => NoteKind::Reply,
        "entrega" | "handoff" => NoteKind::Handoff,
        "alerta" | "alert" => NoteKind::Alert,
        _ => NoteKind::Status,
    }
}

pub(super) fn on_status(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    terminal_id: &str,
    status: HelperStatus,
) {
    state::ensure_loaded(cx, shared);
    let assignment = with(shared, |state| {
        state
            .store
            .assignments
            .iter()
            .rev()
            .find(|assignment| assignment.terminal_id.as_deref() == Some(terminal_id))
            .map(|assignment| assignment.id.clone())
    })
    .flatten();
    let Some(assignment) = assignment else {
        return;
    };
    let terminal_tail = read_terminal(cx, terminal_id).unwrap_or_default();
    let alert = with(shared, |state| {
        state
            .store
            .update_status(&assignment, status, &terminal_tail)
    })
    .flatten();
    if let Some(alert) = alert {
        let enabled = cx
            .setting("muster.notifications.task_failed")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if enabled {
            let body = cx.t_args("muster.notify.alert-body", &[("tail", &alert.body)]);
            notify(cx, "muster.notify.alert-title", &body, None);
        }
    }
    if matches!(
        status,
        HelperStatus::Idle | HelperStatus::Unknown | HelperStatus::Finished
    ) {
        flush_queued(cx, shared, &assignment, status);
    } else {
        state::persist(cx, shared);
    }
}

fn flush_queued(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    assignment_id: &str,
    status: HelperStatus,
) {
    let (state, terminal_id, notes) = with(shared, |state| {
        state.store.assignment(assignment_id).map(|assignment| {
            (
                assignment.state,
                assignment.terminal_id.clone(),
                state.store.queued_notes(assignment_id),
            )
        })
    })
    .flatten()
    .unwrap_or((AssignmentState::Idle, None, Vec::new()));
    let terminal = match terminal_id {
        Some(terminal) => terminal,
        None => {
            state::persist(cx, shared);
            return;
        }
    };
    let tail = read_terminal(cx, &terminal).unwrap_or_default();
    if !matches!(status, HelperStatus::Idle | HelperStatus::Unknown)
        || !matches!(delivery::decide(state, &delivery::recent_lines(&tail)), Delivery::Send)
    {
        state::persist(cx, shared);
        return;
    }
    for note in notes {
        if send_terminal(cx, &terminal, &note.body) {
            with(shared, |state| state.store.mark_delivered(&note.id));
        }
    }
    state::persist(cx, shared);
}

/// Reenvia recados que já podem sair. O passo da barra de status usa o
/// intervalo `muster.notes.poll_s`.
pub(super) fn flush_pending(cx: &mut Ctx<'_>, shared: &Shared) {
    let retention_days = cx
        .setting("muster.notes.retention_days")
        .and_then(SettingValue::as_i64)
        .unwrap_or(30)
        .clamp(1, 365);
    let cutoff = crate::model::now_ms() - retention_days * 86_400_000;
    if with(shared, |state| state.store.prune_notes(cutoff)).unwrap_or(0) > 0 {
        state::persist(cx, shared);
    }
    let pending: Vec<(String, String)> = with(shared, |state| {
        state
            .store
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.terminal_id.is_some()
                    && !state.store.queued_notes(&assignment.id).is_empty()
            })
            .map(|assignment| (assignment.id.clone(), assignment.last_status.clone()))
            .collect()
    })
    .unwrap_or_default();
    for (assignment, last_status) in pending {
        let status = match last_status.as_str() {
            "working" => HelperStatus::Working,
            "needs_you" => HelperStatus::NeedsYou,
            "finished" => HelperStatus::Finished,
            "idle" => HelperStatus::Idle,
            _ => HelperStatus::Unknown,
        };
        if matches!(status, HelperStatus::Idle | HelperStatus::Unknown) {
            flush_queued(cx, shared, &assignment, status);
        }
    }
}

pub(super) fn subscribe_events(r: &mut Registrar<'_>, shared: &Shared) {
    r.contract_event::<HelperStatusChanged>(forja_contracts::names::HELPER_STATUS_CHANGED);
    r.contract_event::<TerminalExited>(forja_contracts::names::TERMINAL_EXITED);
    r.contract_event::<TerminalCommandFinished>(forja_contracts::names::TERMINAL_COMMAND_FINISHED);

    let status_shared = Arc::clone(shared);
    r.subscribe(move |cx, event: &HelperStatusChanged| {
        if let Some(terminal) = &event.terminal_id {
            on_status(cx, &status_shared, terminal, event.status);
        }
    });
    let exited_shared = Arc::clone(shared);
    r.subscribe(move |cx, event: &TerminalExited| {
        on_status(
            cx,
            &exited_shared,
            &event.terminal.terminal_id,
            HelperStatus::Finished,
        );
    });
    let finished_shared = Arc::clone(shared);
    r.subscribe(move |cx, event: &TerminalCommandFinished| {
        on_status(
            cx,
            &finished_shared,
            &event.terminal.terminal_id,
            HelperStatus::Idle,
        );
    });
}

fn notify(cx: &mut Ctx<'_>, title_key: &str, body: &str, bench_path: Option<String>) {
    let mut args = Args::new();
    args.insert("title", json!(cx.t(title_key)));
    args.insert("body", json!(body));
    args.insert("level", json!("attention"));
    if let Some(bench) = bench_path {
        args.insert("bench_path", json!(bench));
    }
    let _ = cx.run_command("notify.show", args);
}

/// Status de uma tarefa em chave i18n.
pub fn task_key(state: TaskState) -> &'static str {
    match state {
        TaskState::Pending => "muster-task-pending",
        TaskState::Running => "muster-task-running",
        TaskState::Delivered => "muster-task-delivered",
        TaskState::Failed => "muster-task-failed",
        TaskState::Blocked => "muster-task-blocked",
    }
}

/// Status de uma atribuição em chave i18n.
fn rpc(result: Result<Value, KernelError>) -> Result<Value, RpcError> {
    result.map_err(|error| RpcError::app(error.to_string()))
}

pub fn assignment_key(state: AssignmentState) -> &'static str {
    match state {
        AssignmentState::Starting => "muster-assignment-starting",
        AssignmentState::Running => "muster-assignment-running",
        AssignmentState::NeedsYou => "muster-assignment-needs-you",
        AssignmentState::Idle => "muster-assignment-idle",
        AssignmentState::Delivered => "muster-assignment-delivered",
        AssignmentState::Failed => "muster-assignment-failed",
        AssignmentState::Blocked => "muster-assignment-blocked",
        AssignmentState::Stopped => "muster-assignment-stopped",
    }
}

