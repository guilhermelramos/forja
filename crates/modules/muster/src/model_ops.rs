//! Operações do Mutirão sobre o estado persistido, extraídas de `model.rs`.

use crate::model::*;
use forja_contracts::HelperStatus;
use serde::{Deserialize, Serialize};

/// Bancada criada pelo mutirão. Só bancadas com esta marca podem ser limpas.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchOwnership {
    pub bench_id: String,
    pub run: String,
    pub task: String,
}

/// Criação assíncrona de bancada esperada por uma tarefa. O `bench_id` é a
/// identidade que liga o evento `bench.created` (ou a falha) a esta tarefa.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDispatch {
    pub id: String,
    pub task: String,
    pub run: String,
    pub bench_id: String,
    pub bench_name: String,
    pub profile_id: Option<String>,
    pub permission: Option<String>,
}

/// Reenvio seguro: a tarefa mantém a identidade e a atribuição anterior já
/// encerrou.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryRequest {
    pub task: String,
    pub run: String,
    pub profile_id: String,
    pub bench_path: String,
}

impl MusterFile {
    /// Registra a criação assíncrona de bancada pedida para uma tarefa.
    pub fn queue_dispatch(
        &mut self,
        task: &str,
        run: &str,
        bench_id: &str,
        bench_name: &str,
        profile_id: Option<String>,
        permission: Option<String>,
    ) -> PendingDispatch {
        let pending = PendingDispatch {
            id: self.id("dispatch-"),
            task: task.to_owned(),
            run: run.to_owned(),
            bench_id: bench_id.to_owned(),
            bench_name: bench_name.to_owned(),
            profile_id,
            permission,
        };
        self.pending_dispatches.push(pending.clone());
        pending
    }

    /// Fecha a criação pelo `bench_id`. O evento de outra bancada não casa.
    pub fn resolve_dispatch(&mut self, bench_id: &str) -> Option<PendingDispatch> {
        let at = self
            .pending_dispatches
            .iter()
            .position(|item| item.bench_id == bench_id)?;
        Some(self.pending_dispatches.remove(at))
    }

    /// Fecha uma criação que falhou, marca a tarefa como falha e avisa.
    pub fn fail_dispatch(&mut self, bench_id: &str, reason: &str) -> Option<PendingDispatch> {
        let pending = self.resolve_dispatch(bench_id)?;
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == pending.task) {
            if !matches!(task.state, TaskState::Delivered) {
                task.state = TaskState::Failed;
            }
        }
        self.note(
            &pending.run,
            Some(pending.task.clone()),
            None,
            "muster",
            "coordinator",
            NoteKind::Alert,
            &format!("bancada não criada: {}", tail(reason)),
            false,
            false,
        );
        Some(pending)
    }

    /// Tarefas prontas para reenvio: a anterior encerrou e não há execução
    /// simultânea da mesma tarefa.
    pub fn retry_candidates(&self) -> Vec<RetryRequest> {
        self.tasks
            .iter()
            .filter(|task| task.state == TaskState::Failed)
            .filter_map(|task| {
                let previous = self.assignments.iter().rev().find(|a| a.task == task.id)?;
                if !previous.escalated
                    || !matches!(
                        previous.state,
                        AssignmentState::Failed
                            | AssignmentState::Stopped
                            | AssignmentState::Blocked
                            | AssignmentState::Delivered
                    )
                    || self.assignments.iter().any(|a| {
                        a.task == task.id
                            && matches!(
                                a.state,
                                AssignmentState::Starting
                                    | AssignmentState::Running
                                    | AssignmentState::NeedsYou
                            )
                    })
                {
                    return None;
                }
                Some(RetryRequest {
                    task: task.id.clone(),
                    run: task.run.clone(),
                    profile_id: previous.profile_id.clone(),
                    bench_path: previous.bench_path.clone(),
                })
            })
            .collect()
    }

    /// Reabre a mesma tarefa. Recusa quando a anterior ainda não encerrou; a
    /// checagem e a criação da nova atribuição acontecem juntas no estado.
    pub fn retry(
        &mut self,
        task_id: &str,
        profile_id: &str,
        bench_path: &str,
    ) -> Result<Assignment, String> {
        let previous = self
            .assignments
            .iter()
            .rev()
            .find(|assignment| assignment.task == task_id)
            .ok_or_else(|| "a tarefa não tem atribuição anterior".to_owned())?;
        if !matches!(
            previous.state,
            AssignmentState::Failed
                | AssignmentState::Stopped
                | AssignmentState::Blocked
                | AssignmentState::Delivered
        ) {
            return Err("a atribuição anterior ainda não encerrou".to_owned());
        }
        let retries = previous.retries.saturating_add(1);
        let mut assignment = self.start(task_id, profile_id, bench_path)?;
        if let Some(open) = self
            .assignments
            .iter_mut()
            .find(|item| item.id == assignment.id)
        {
            open.retries = retries;
        }
        assignment.retries = retries;
        Ok(assignment)
    }

    /// Marca que o mutirão criou esta bancada.
    pub fn own_bench(&mut self, run: &str, task: &str, bench_id: &str) {
        if !self
            .owned_benches
            .iter()
            .any(|item| item.bench_id == bench_id)
        {
            self.owned_benches.push(BenchOwnership {
                bench_id: bench_id.to_owned(),
                run: run.to_owned(),
                task: task.to_owned(),
            });
        }
    }

    /// Marca a limpeza de um mutirão terminado como decidida, uma única vez.
    pub fn take_cleanup(&mut self, run_id: &str) -> bool {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == run_id) else {
            return false;
        };
        let finished = matches!(run.state, RunState::Done | RunState::Failed);
        if finished && !run.cleanup_done {
            run.cleanup_done = true;
            return true;
        }
        false
    }

    /// Atualiza o status de uma atribuição e devolve um alerta quando ela para
    /// sem entregar. Sem limite configurado, a tarefa fica bloqueada.
    #[cfg(test)]
    pub fn update_status(
        &mut self,
        assignment_id: &str,
        status: HelperStatus,
        terminal_tail: &str,
    ) -> Option<Note> {
        self.update_status_with_retries(assignment_id, status, terminal_tail, 0)
    }

    /// Como `update_status`, mas com reenvio enquanto couber no limite. Só
    /// reenvia depois que o ajudante anterior encerrou (`Finished`).
    pub fn update_status_with_retries(
        &mut self,
        assignment_id: &str,
        status: HelperStatus,
        terminal_tail: &str,
        max_retries: u32,
    ) -> Option<Note> {
        let (run, task_id, was_running, delivered, escalated) = {
            let assignment = self.assignment(assignment_id)?;
            (
                assignment.run.clone(),
                assignment.task.clone(),
                matches!(
                    assignment.state,
                    AssignmentState::Running | AssignmentState::NeedsYou
                ),
                matches!(
                    assignment.state,
                    AssignmentState::Delivered
                        | AssignmentState::Failed
                        | AssignmentState::Blocked
                ),
                assignment.escalated,
            )
        };
        {
            let assignment = self
                .assignments
                .iter_mut()
                .find(|assignment| assignment.id == assignment_id)?;
            assignment.last_event_ms = now_ms();
            assignment.last_status = status_key(status).to_owned();
            if !delivered {
                assignment.state = match status {
                    HelperStatus::Working => AssignmentState::Running,
                    HelperStatus::NeedsYou => AssignmentState::NeedsYou,
                    HelperStatus::Idle => AssignmentState::Idle,
                    HelperStatus::Finished => AssignmentState::Stopped,
                    HelperStatus::Error => AssignmentState::Stopped,
                    HelperStatus::Unknown => AssignmentState::Idle,
                };
            }
        }
        let stopped = matches!(
            status,
            HelperStatus::Idle | HelperStatus::Finished | HelperStatus::Error
        );
        if stopped && was_running && !delivered && !escalated {
            let assignment = self
                .assignments
                .iter_mut()
                .find(|assignment| assignment.id == assignment_id)?;
            assignment.escalated = true;
            // A atribuição anterior encerrou de verdade: o reenvio não pode
            // dividir a bancada com uma execução ainda viva.
            let can_retry = assignment.retries < max_retries;
            if can_retry {
                assignment.state = AssignmentState::Failed;
            }
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
                if !matches!(task.state, TaskState::Delivered) {
                    task.state = if can_retry {
                        TaskState::Failed
                    } else {
                        TaskState::Blocked
                    };
                }
            }
            let message = if can_retry {
                "parou sem entregar; reenviando a mesma tarefa"
            } else {
                "parou sem entregar; limite de reenvios atingido"
            };
            let alert = self.note(
                &run,
                Some(task_id.clone()),
                Some(assignment_id.to_owned()),
                "muster",
                "coordinator",
                NoteKind::Alert,
                &format!("{message}: {}", tail(terminal_tail)),
                false,
                false,
            );
            return Some(alert);
        }
        None
    }
}
