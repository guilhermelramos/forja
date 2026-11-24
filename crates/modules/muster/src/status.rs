//! Acompanhamento do status dos ajudantes e entrega de recados pendentes.

use crate::commands::{notify, read_terminal, send_terminal};
use crate::delivery::{self, Delivery};
use crate::dispatch::{maybe_cleanup, retry_task};
use crate::model::AssignmentState;
use crate::state::{self, with, Shared};
use forja_contracts::HelperStatus;
use forja_kernel::{Ctx, SettingValue};
use std::sync::Arc;

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
    let max_retries = cx
        .setting("muster.escalation.max_retries")
        .and_then(SettingValue::as_i64)
        .unwrap_or(2)
        .clamp(0, 10) as u32;
    // Só `Finished` confirma o fim do processo; `Idle` ainda pode voltar.
    let limit = if status == HelperStatus::Finished {
        max_retries
    } else {
        0
    };
    let alert = with(shared, |state| {
        state
            .store
            .update_status_with_retries(&assignment, status, &terminal_tail, limit)
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
        HelperStatus::Idle | HelperStatus::Unknown | HelperStatus::Finished | HelperStatus::Error
    ) {
        flush_queued(cx, shared, &assignment, status);
    } else {
        state::persist(cx, shared);
    }
    let retries = with(shared, |state| state.store.retry_candidates()).unwrap_or_default();
    for request in retries {
        let shared = Arc::clone(shared);
        cx.spawn(
            move || request,
            move |cx, request| {
                if let Err(error) = retry_task(cx, &shared, &request) {
                    let body =
                        cx.t_args("muster.error.unavailable", &[("detail", &error.to_string())]);
                    notify(cx, "muster.notify.alert-title", &body, None);
                }
            },
        );
    }
    maybe_cleanup(cx, shared);
}
pub(crate) fn flush_queued(
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
            "error" => HelperStatus::Error,
            "idle" => HelperStatus::Idle,
            _ => HelperStatus::Unknown,
        };
        if matches!(status, HelperStatus::Idle | HelperStatus::Unknown) {
            flush_queued(cx, shared, &assignment, status);
        }
    }
}
