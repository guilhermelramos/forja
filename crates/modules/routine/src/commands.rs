//! Comandos e métodos RPC das rotinas, e as fotos (snapshots) que o CLI e a tela leem.

use crate::actions::{execute, persist};
use crate::model::{self, Routine, Trigger};
use crate::schedule::Schedule;
use crate::state::{self, now_unix, with, Shared};
use chrono::{Local, TimeZone, Utc};
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{json, Value};
use std::time::SystemTime;

/// Próximo disparo de uma rotina, em unix, ou `None` se a agenda não tem nenhum.
pub fn next_fire(schedule: &Schedule, utc: bool, now: i64) -> Option<i64> {
    if utc {
        let at = Utc.timestamp_opt(now, 0).single()?;
        schedule.next_runs(&at, 1).ok()?.first().map(|t| t.timestamp())
    } else {
        let at = Local.timestamp_opt(now, 0).single()?;
        schedule.next_runs(&at, 1).ok()?.first().map(|t| t.timestamp())
    }
}

/// Lista para o CLI e a tela: rotinas, próximo disparo, último status e contagem da fila.
pub fn list_snapshot(cx: &Ctx<'_>, shared: &Shared) -> Value {
    let utc = state::schedule_in_utc(cx);
    let now = now_unix();
    with(shared, |state| {
        let routines: Vec<Value> = state
            .store
            .routines
            .iter()
            .map(|routine| {
                let last = state.store.runs_of(&routine.id, 1).into_iter().next();
                json!({
                    "id": routine.id,
                    "name": routine.name,
                    "enabled": routine.enabled,
                    "schedule": routine.schedule.expression(),
                    "next_run": if routine.enabled { next_fire(&routine.schedule, utc, now) } else { None },
                    "last_status": last.map(|run| run.status.clone()),
                    "last_run_at": last.map(|run| run.started),
                })
            })
            .collect();
        json!({
            "routines": routines,
            "running": state.runner.running(),
            "queued": state.runner.queued(),
            "loaded": state.loaded,
        })
    })
    .unwrap_or_else(|| json!({ "routines": [] }))
}

/// Histórico, do mais recente para o mais antigo. Saída resumida é dado, não instrução.
pub fn runs_snapshot(shared: &Shared, routine_id: Option<&str>, limit: usize) -> Value {
    with(shared, |state| {
        let runs: Vec<&crate::model::Run> = match routine_id {
            Some(id) => state.store.runs_of(id, limit),
            None => state.store.runs.iter().rev().take(limit).collect(),
        };
        json!({ "runs": runs })
    })
    .unwrap_or_else(|| json!({ "runs": [] }))
}

/// Grava uma rotina nova ou editada. Erros de validação voltam como código, não como exceção.
pub fn save(cx: &mut Ctx<'_>, shared: &Shared, value: Value) -> Result<Value> {
    // Antes do carregamento, gravar seria perdido quando o arquivo chegasse por cima; se a
    // leitura falhou, gravar apagaria o que existe. Nos dois casos, nada é mudado.
    match writable(shared) {
        Writable::Loading => return Ok(json!({ "saved": false, "error": "routine-error-loading" })),
        Writable::Unreadable => return Ok(json!({ "saved": false, "error": "routine-notice-load-failed" })),
        Writable::Yes => {}
    }
    let mut routine: Routine = match serde_json::from_value(value) {
        Ok(routine) => routine,
        Err(_) => return Ok(json!({ "saved": false, "error": "routine-error-invalid" })),
    };
    if let Err(problem) = model::validate(&routine) {
        return Ok(json!({ "saved": false, "error": problem.key() }));
    }
    let now = now_unix();
    let id = with(shared, |state| {
        if routine.id.is_empty() {
            routine.id = new_id();
            routine.created_at = now;
            state.store.routines.push(routine.clone());
        } else if let Some(existing) = state.store.routine_mut(&routine.id) {
            routine.created_at = existing.created_at;
            routine.last_fire = existing.last_fire;
            *existing = routine.clone();
        } else {
            routine.created_at = now;
            state.store.routines.push(routine.clone());
        }
        routine.id.clone()
    })
    .unwrap_or_default();
    persist(cx, shared);
    Ok(json!({ "saved": true, "id": id }))
}

/// Apaga a rotina. O histórico dela fica, com o nome que ela tinha.
pub fn delete(cx: &mut Ctx<'_>, shared: &Shared, id: &str) -> Result<Value> {
    match writable(shared) {
        Writable::Yes => {}
        Writable::Loading => return Err(KernelError::message("routines are still loading")),
        Writable::Unreadable => return Err(KernelError::message("the routines file could not be read")),
    }
    let removed = with(shared, |state| {
        let before = state.store.routines.len();
        state.store.routines.retain(|routine| routine.id != id);
        before != state.store.routines.len()
    })
    .unwrap_or(false);
    if !removed {
        return Err(KernelError::message(format!("unknown routine `{id}`")));
    }
    persist(cx, shared);
    Ok(json!({ "deleted": true }))
}

/// Roda agora (botão ou CLI). A execução entra na fila e segue a mesma máquina de estados.
pub fn run_now(cx: &mut Ctx<'_>, shared: &Shared, id: &str) -> Result<Value> {
    let settings = state::settings(cx);
    let now = now_unix();
    let started = with(shared, |state| {
        if !state.loaded {
            return Err("routines are still loading");
        }
        if state.load_failed {
            return Err("the routines file could not be read");
        }
        let routine = state.store.routine(id).cloned().ok_or("unknown routine")?;
        let effects = state
            .runner
            .enqueue(&mut state.store, &routine, Trigger::Manual, now, &settings);
        let run_id = state.store.runs_of(id, 1).first().map(|run| run.id.clone());
        Ok((run_id, effects))
    });
    match started {
        None => Err(KernelError::message("the routine state is unavailable")),
        Some(Err(message)) => Err(KernelError::message(message)),
        Some(Ok((run_id, effects))) => {
            execute(cx, shared, effects);
            persist(cx, shared);
            Ok(json!({ "run_id": run_id }))
        }
    }
}

/// Pode-se gravar? Só depois de carregar, e só se a leitura do arquivo deu certo.
enum Writable {
    Yes,
    Loading,
    Unreadable,
}

fn writable(shared: &Shared) -> Writable {
    with(shared, |state| match (state.loaded, state.load_failed) {
        (false, _) => Writable::Loading,
        (true, true) => Writable::Unreadable,
        (true, false) => Writable::Yes,
    })
    .unwrap_or(Writable::Loading)
}

/// Id novo de rotina: estável o bastante para o arquivo, sem repetir dentro do app.
fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("routine-{nanos}")
}

/// Argumentos de `routine.runs`.
pub fn runs_args(args: &Args) -> (Option<String>, usize) {
    let routine_id = args.str("routine_id").map(str::to_owned);
    let limit = args.i64("limit").unwrap_or(20).clamp(1, 200) as usize;
    (routine_id, limit)
}
