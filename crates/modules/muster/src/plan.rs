//! Resumo do plano e trava de aprovação.
//!
//! A página só aprova um plano depois de abrir a revisão. A trava fica num
//! tipo pequeno e testável para que "aprovar sem ver" continue impossível se a
//! interface mudar de lugar.

use crate::model::{Assignment, Task};

/// Revisão do plano aberta pelo coordenador.
#[derive(Debug, Default)]
pub struct PlanGate {
    open: bool,
}

impl PlanGate {
    /// Abre a revisão; o botão de aprovar só aparece dentro dela.
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Fecha a revisão sem aprovar.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// A revisão está aberta.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Só é possível aprovar enquanto a revisão está aberta.
    pub fn can_approve(&self) -> bool {
        self.open
    }

    /// Fecha a revisão depois do aceite.
    pub fn approved(&mut self) {
        self.open = false;
    }
}

/// Títulos das dependências, na ordem em que a tarefa as declara. Um id que
/// não existe mais aparece como o próprio id, sem esconder o problema.
pub fn dependency_titles(task: &Task, tasks: &[Task]) -> Vec<String> {
    task.deps
        .iter()
        .map(|dep| {
            tasks
                .iter()
                .find(|candidate| candidate.id == *dep)
                .map(|candidate| candidate.title.clone())
                .unwrap_or_else(|| dep.clone())
        })
        .collect()
}

/// Ajudante sugerido para a tarefa: a atribuição existente manda; depois, o
/// perfil escolhido na página; por último, o automático.
pub fn suggested_profile<'a>(
    task: &Task,
    assignments: &'a [Assignment],
    selected: &'a str,
) -> &'a str {
    assignments
        .iter()
        .rev()
        .find(|assignment| assignment.task == task.id && !assignment.profile_id.trim().is_empty())
        .map(|assignment| assignment.profile_id.as_str())
        .unwrap_or(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, title: &str, deps: &[&str]) -> Task {
        Task {
            id: id.to_owned(),
            title: title.to_owned(),
            deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
            ..Task::default()
        }
    }

    #[test]
    fn dependency_titles_follow_the_declared_order_and_keep_unknown_ids() {
        let tasks = vec![
            task("task-1", "Primeira", &[]),
            task("task-2", "Segunda", &[]),
        ];
        let dependent = task("task-3", "Terceira", &["task-2", "task-1", "task-9"]);
        assert_eq!(
            dependency_titles(&dependent, &tasks),
            ["Segunda", "Primeira", "task-9"]
        );
    }

    #[test]
    fn suggested_profile_prefers_an_assignment_then_the_picker() {
        let first = task("task-1", "Primeira", &[]);
        let second = task("task-2", "Segunda", &[]);
        let assignments = vec![Assignment {
            id: "assignment-3".to_owned(),
            task: "task-1".to_owned(),
            profile_id: "claude-code".to_owned(),
            ..Assignment::default()
        }];
        assert_eq!(
            suggested_profile(&first, &assignments, "codex"),
            "claude-code"
        );
        assert_eq!(suggested_profile(&second, &assignments, "codex"), "codex");
        assert_eq!(suggested_profile(&second, &assignments, ""), "");
    }

    #[test]
    fn approval_requires_the_review_to_be_open() {
        let mut gate = PlanGate::default();
        assert!(!gate.can_approve());
        gate.open();
        assert!(gate.is_open());
        assert!(gate.can_approve());
        gate.close();
        assert!(!gate.can_approve());
        gate.open();
        gate.approved();
        assert!(!gate.is_open());
    }
}
