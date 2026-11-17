//! One sample muster for `forja-app --demo`.
//!
//! Five tasks with dependencies, mixed states, one live assignment and a note
//! waiting for the coordinator. Nothing here launches a helper: only the stored
//! plan changes, so the page has something real to show.

use crate::model::{
    Assignment, AssignmentState, MusterFile, MusterRun, Note, NoteKind, RunState, Task, TaskState,
};
use forja_kernel::Store;

/// Id of the demo run, also used by the tasks.
pub const RUN: &str = "run-demo";

/// Millisecond timestamp of the demo day the plan was written.
fn start_ms() -> i64 {
    1_767_000_000_000
}

fn task(id: &str, title: &str, spec: &str, deps: &[&str], state: TaskState) -> Task {
    Task {
        id: id.to_owned(),
        run: RUN.to_owned(),
        title: title.to_owned(),
        spec: spec.to_owned(),
        deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
        state,
        result: None,
        summary: None,
        evidence: None,
    }
}

/// Writes the demo run, its five tasks and the surrounding state.
pub fn demo_seed(store: &Store, bench_path: &str) -> Result<(), String> {
    let mut file = MusterFile::default();
    file.runs.push(MusterRun {
        id: RUN.to_owned(),
        title: "Relatório do trimestre".to_owned(),
        objective: "Fechar o relatório com números conferidos e a página pronta.".to_owned(),
        coordinator: "voce".to_owned(),
        state: RunState::Open,
        created_ms: start_ms(),
        bench_path: Some(bench_path.to_owned()),
        plan_approved: true,
        budget_usd: 12.0,
        spent_usd: 3.4,
    });

    let mut tasks = vec![
        task(
            "task-1",
            "Conferir os números do mês",
            "Some as planilhas e anote as diferenças no caderno.",
            &[],
            TaskState::Delivered,
        ),
        task(
            "task-2",
            "Escrever o resumo executivo",
            "Três parágrafos, sem jargão, para a primeira página.",
            &[],
            TaskState::Delivered,
        ),
        task(
            "task-3",
            "Montar os gráficos",
            "Um gráfico por trimestre, com a cor de destaque da casa.",
            &["task-1"],
            TaskState::Running,
        ),
        task(
            "task-4",
            "Revisar os textos",
            "Passar o corretor e conferir os nomes das seções.",
            &["task-2"],
            TaskState::Blocked,
        ),
        task(
            "task-5",
            "Fechar a página final",
            "Juntar resumo e gráficos e enviar o arquivo.",
            &["task-3", "task-4"],
            TaskState::Pending,
        ),
    ];
    tasks[0].summary = Some("Números conferidos; duas linhas corrigidas.".to_owned());
    tasks[1].summary = Some("Resumo pronto com 240 palavras.".to_owned());
    tasks[2].result = None;
    tasks[3].summary = Some("Parou esperando a resposta sobre o título.".to_owned());
    file.tasks = tasks;

    file.assignments.push(Assignment {
        id: "assign-1".to_owned(),
        task: "task-3".to_owned(),
        run: RUN.to_owned(),
        profile_id: "demo-helper".to_owned(),
        bench_path: bench_path.to_owned(),
        terminal_id: None,
        session_id: Some("demo-session-running".to_owned()),
        mode: "chat".to_owned(),
        state: AssignmentState::Running,
        last_status: "running".to_owned(),
        started_ms: start_ms() + 60_000,
        last_event_ms: start_ms() + 120_000,
        retries: 0,
        escalated: false,
    });

    file.notes.push(Note {
        id: "note-1".to_owned(),
        run: RUN.to_owned(),
        task: Some("task-3".to_owned()),
        assignment: Some("assign-1".to_owned()),
        from: "demo-helper".to_owned(),
        to: "voce".to_owned(),
        kind: NoteKind::Question,
        body: "Uso a cor de destaque da casa nos três gráficos ou só no último?".to_owned(),
        read: false,
        queued: false,
        delivered: false,
        no_helper: false,
        created_ms: start_ms() + 130_000,
    });

    store
        .save("muster", &file)
        .map_err(|error| format!("demo: could not write muster: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_five_tasks_with_dependencies_and_states() {
        let dir = std::env::temp_dir().join(format!("forja-muster-demo-{}", std::process::id()));
        let store = Store::new(&dir);
        demo_seed(&store, "C:/demo/relatorio").unwrap();
        let file: MusterFile = store.load("muster").into_option().expect("muster");
        assert_eq!(file.runs.len(), 1);
        assert_eq!(file.tasks.len(), 5);
        assert!(file.tasks.iter().any(|task| !task.deps.is_empty()));
        assert_eq!(
            file.tasks
                .iter()
                .filter(|task| task.state == TaskState::Delivered)
                .count(),
            2
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
