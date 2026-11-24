//! Testes das operações de despacho, reenvio e limpeza do estado.

use crate::model::*;
use forja_contracts::HelperStatus;

fn run_with_task(file: &mut MusterFile) -> (MusterRun, Task) {
    let run = file.create_run("Mutirão", "Objetivo", "coordinator", None);
    let task = file
        .add_task(&run.id, "Tarefa", "Spec", Vec::new())
        .unwrap();
    (run, task)
}

#[test]
fn max_retries_resends_the_same_task_and_then_blocks() {
    let mut file = MusterFile::default();
    let (_run, task) = run_with_task(&mut file);
    let first = file.start(&task.id, "codex", "C:/bench").unwrap();
    file.launched(&first.id, "codex", Some("1".into()), None, "tui")
        .unwrap();

    assert!(file
        .update_status_with_retries(&first.id, HelperStatus::Finished, "fim", 1)
        .is_some());
    assert_eq!(file.task(&task.id).unwrap().state, TaskState::Failed);
    let candidates = file.retry_candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].task, task.id);

    let second = file.retry(&task.id, "codex", "C:/bench").unwrap();
    assert_eq!(second.task, task.id);
    assert_eq!(second.retries, 1);
    file.launched(&second.id, "codex", Some("2".into()), None, "tui")
        .unwrap();
    // Duas execuções da mesma tarefa nunca coexistem.
    assert!(file.retry(&task.id, "codex", "C:/bench").is_err());
    assert!(file.retry_candidates().is_empty());

    assert!(file
        .update_status_with_retries(&second.id, HelperStatus::Finished, "fim", 1)
        .is_some());
    assert_eq!(file.task(&task.id).unwrap().state, TaskState::Blocked);
    assert!(file.retry_candidates().is_empty());
}

#[test]
fn retry_requires_the_previous_assignment_to_have_ended() {
    let mut file = MusterFile::default();
    let (_run, task) = run_with_task(&mut file);
    let first = file.start(&task.id, "codex", "C:/bench").unwrap();
    file.launched(&first.id, "codex", Some("1".into()), None, "tui")
        .unwrap();
    assert!(file.retry(&task.id, "codex", "C:/bench").is_err());
    assert_eq!(file.assignments.len(), 1);
}
