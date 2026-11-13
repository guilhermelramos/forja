//! Modelo persistido do Mutirão: mutirões, tarefas, atribuições e recados.
//!
//! A máquina de estados não faz IO. Ela decide transições e devolve os recados;
//! a camada de comandos conversa com `bench`, `helper` e `terminal`.

use forja_contracts::HelperStatus;
use serde::{Deserialize, Serialize};

/// Estado de um mutirão.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Aceita tarefas e atribuições.
    #[default]
    Open,
    /// Todas as tarefas entregues.
    Done,
    /// Fechado com falha ou bloqueio.
    Failed,
}

/// Estado de uma tarefa.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Aguardando dependências ou atribuição.
    #[default]
    Pending,
    /// Um ajudante está trabalhando nela.
    Running,
    /// Entregue.
    Delivered,
    /// Falhou.
    Failed,
    /// O ajudante parou sem entregar.
    Blocked,
}

/// Estado de uma atribuição.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssignmentState {
    /// Processo sendo aberto.
    #[default]
    Starting,
    /// Ajudante trabalhando.
    Running,
    /// Ajudante esperando aprovação.
    NeedsYou,
    /// Ajudante ocioso, ainda sem entrega.
    Idle,
    /// Entregou.
    Delivered,
    /// Falhou.
    Failed,
    /// Bloqueou.
    Blocked,
    /// Encerrado sem entrega.
    Stopped,
}

/// Tipo de recado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// Andamento.
    #[default]
    Status,
    /// Pergunta do ajudante.
    Question,
    /// Resposta do coordenador.
    Reply,
    /// Entrega da tarefa.
    Handoff,
    /// Alerta do sistema.
    Alert,
}

/// Resultado de uma entrega.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandoffResult {
    /// Feito.
    Done,
    /// Falhou.
    Failed,
    /// Bloqueado.
    Blocked,
}

impl HandoffResult {
    /// Lê `feito`, `falhou`, `bloqueado` e os equivalentes em inglês.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "feito" | "done" | "succeeded" | "success" => Ok(Self::Done),
            "falhou" | "failed" | "failure" => Ok(Self::Failed),
            "bloqueado" | "blocked" => Ok(Self::Blocked),
            other => Err(format!(
                "resultado `{other}` não é feito, falhou ou bloqueado"
            )),
        }
    }

    /// Valor estável mostrado no JSON e no CLI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Done => "feito",
            Self::Failed => "falhou",
            Self::Blocked => "bloqueado",
        }
    }

    /// Estado da tarefa correspondente.
    pub fn task_state(self) -> TaskState {
        match self {
            Self::Done => TaskState::Delivered,
            Self::Failed => TaskState::Failed,
            Self::Blocked => TaskState::Blocked,
        }
    }

    /// Estado da atribuição correspondente.
    pub fn assignment_state(self) -> AssignmentState {
        match self {
            Self::Done => AssignmentState::Delivered,
            Self::Failed => AssignmentState::Failed,
            Self::Blocked => AssignmentState::Blocked,
        }
    }
}

/// Um mutirão.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MusterRun {
    /// Id.
    pub id: String,
    /// Título.
    pub title: String,
    /// Objetivo escrito pelo coordenador.
    pub objective: String,
    /// Quem coordena.
    pub coordinator: String,
    /// Estado.
    pub state: RunState,
    /// Criação em ms.
    pub created_ms: i64,
    /// Bancada principal, quando escolhida.
    #[serde(default)]
    pub bench_path: Option<String>,
    /// Plano revisado e liberado para iniciar tarefas.
    #[serde(default = "default_true")]
    pub plan_approved: bool,
    /// Teto de custo em dólares; 0 desliga.
    #[serde(default)]
    pub budget_usd: f64,
    /// Custo acumulado em dólares.
    #[serde(default)]
    pub spent_usd: f64,
}

/// Uma tarefa do mutirão.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Id.
    pub id: String,
    /// Mutirão.
    pub run: String,
    /// Título.
    pub title: String,
    /// Spec entregue ao ajudante.
    pub spec: String,
    /// Dependências por id.
    #[serde(default)]
    pub deps: Vec<String>,
    /// Estado.
    pub state: TaskState,
    /// Resultado da última entrega.
    #[serde(default)]
    pub result: Option<String>,
    /// Resumo da última entrega.
    #[serde(default)]
    pub summary: Option<String>,
    /// Evidências da última entrega.
    #[serde(default)]
    pub evidence: Option<String>,
}

/// Uma atribuição de tarefa a um ajudante.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assignment {
    /// Id.
    pub id: String,
    /// Tarefa.
    pub task: String,
    /// Mutirão.
    pub run: String,
    /// Perfil do ajudante.
    pub profile_id: String,
    /// Bancada.
    pub bench_path: String,
    /// Terminal, quando a TUI foi aberta.
    #[serde(default)]
    pub terminal_id: Option<String>,
    /// Sessão, quando o modo chat foi usado.
    #[serde(default)]
    pub session_id: Option<String>,
    /// `tui` ou `chat`.
    #[serde(default)]
    pub mode: String,
    /// Estado.
    pub state: AssignmentState,
    /// Último status do ajudante.
    #[serde(default)]
    pub last_status: String,
    /// Início em ms.
    pub started_ms: i64,
    /// Último evento em ms.
    #[serde(default)]
    pub last_event_ms: i64,
    /// Reenvios.
    #[serde(default)]
    pub retries: u32,
    /// Já gerou alerta por parar sem entregar.
    #[serde(default)]
    pub escalated: bool,
}

/// Um recado.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Id.
    pub id: String,
    /// Mutirão.
    pub run: String,
    /// Tarefa, quando houver.
    #[serde(default)]
    pub task: Option<String>,
    /// Atribuição, quando houver.
    #[serde(default)]
    pub assignment: Option<String>,
    /// Remetente.
    pub from: String,
    /// Destinatário.
    pub to: String,
    /// Tipo.
    pub kind: NoteKind,
    /// Corpo.
    pub body: String,
    /// Lido.
    #[serde(default)]
    pub read: bool,
    /// Aguardando o ajudante ficar disponível.
    #[serde(default)]
    pub queued: bool,
    /// Já foi digitado no terminal.
    #[serde(default)]
    pub delivered: bool,
    /// Criação em ms.
    pub created_ms: i64,
}

/// Arquivo persistido.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MusterFile {
    /// Mutirões.
    pub runs: Vec<MusterRun>,
    /// Tarefas.
    pub tasks: Vec<Task>,
    /// Atribuições.
    pub assignments: Vec<Assignment>,
    /// Recados.
    pub notes: Vec<Note>,
    next_id: u64,
}

impl MusterFile {
    /// Remove expired notes, retaining those still queued for delivery.
    pub fn prune_notes(&mut self, cutoff_ms: i64) -> usize {
        let before = self.notes.len();
        self.notes
            .retain(|note| note.queued || note.created_ms >= cutoff_ms);
        before - self.notes.len()
    }
    /// Cria um mutirão.
    pub fn create_run(
        &mut self,
        title: &str,
        objective: &str,
        coordinator: &str,
        bench_path: Option<String>,
    ) -> MusterRun {
        let run = MusterRun {
            id: self.id("run-"),
            title: non_empty(title, "Mutirão"),
            objective: objective.trim().to_owned(),
            coordinator: non_empty(coordinator, "coordinator"),
            state: RunState::Open,
            created_ms: now_ms(),
            bench_path,
            plan_approved: true,
            budget_usd: 0.0,
            spent_usd: 0.0,
        };
        self.runs.push(run.clone());
        run
    }

    /// Adiciona uma tarefa. As dependências precisam existir no mesmo mutirão.
    pub fn add_task(
        &mut self,
        run: &str,
        title: &str,
        spec: &str,
        deps: Vec<String>,
    ) -> Result<Task, String> {
        if !self.runs.iter().any(|item| item.id == run) {
            return Err("mutirão desconhecido".to_owned());
        }
        for dep in &deps {
            if !self.tasks.iter().any(|task| task.run == run && &task.id == dep) {
                return Err(format!("dependência `{dep}` não existe neste mutirão"));
            }
        }
        let task = Task {
            id: self.id("task-"),
            run: run.to_owned(),
            title: non_empty(title, "Tarefa"),
            spec: spec.trim().to_owned(),
            deps,
            state: TaskState::Pending,
            ..Task::default()
        };
        self.tasks.push(task.clone());
        Ok(task)
    }

    /// Tarefa por id.
    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|task| task.id == id)
    }

    /// Atribuição por id.
    pub fn assignment(&self, id: &str) -> Option<&Assignment> {
        self.assignments.iter().find(|item| item.id == id)
    }

    /// Atribuição ativa de uma tarefa.
    pub fn assignment_for_task(&self, task: &str) -> Option<&Assignment> {
        self.assignments
            .iter()
            .rev()
            .find(|assignment| assignment.task == task)
    }

    /// Resolve a atribuição por id, por tarefa ou pela única ativa.
    pub fn resolve_assignment(
        &self,
        assignment: Option<&str>,
        task: Option<&str>,
    ) -> Result<&Assignment, String> {
        if let Some(id) = assignment {
            return self
                .assignment(id)
                .ok_or_else(|| format!("atribuição `{id}` não encontrada"));
        }
        if let Some(task) = task {
            return self
                .assignment_for_task(task)
                .ok_or_else(|| format!("tarefa `{task}` não tem atribuição"));
        }
        let active: Vec<&Assignment> = self
            .assignments
            .iter()
            .filter(|assignment| {
                !matches!(
                    assignment.state,
                    AssignmentState::Delivered
                        | AssignmentState::Failed
                        | AssignmentState::Blocked
                )
            })
            .collect();
        match active.as_slice() {
            [only] => Ok(only),
            [] => Err("nenhuma atribuição ativa; informe --task".to_owned()),
            _ => Err("mais de uma atribuição ativa; informe --task".to_owned()),
        }
    }

    /// Abre uma atribuição para a tarefa.
    pub fn start(
        &mut self,
        task_id: &str,
        profile_id: &str,
        bench_path: &str,
    ) -> Result<Assignment, String> {
        let task = self
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| format!("tarefa `{task_id}` não encontrada"))?;
        if matches!(task.state, TaskState::Delivered) {
            return Err("tarefa já entregue".to_owned());
        }
        let run = task.run.clone();
        if let Some(existing) = self.assignments.iter().find(|assignment| {
            assignment.task == task_id
                && matches!(
                    assignment.state,
                    AssignmentState::Starting | AssignmentState::Running
                )
        }) {
            return Err(format!(
                "tarefa já está com o ajudante na atribuição `{}`",
                existing.id
            ));
        }
        let assignment = Assignment {
            id: self.id("assignment-"),
            task: task_id.to_owned(),
            run: run.clone(),
            profile_id: profile_id.to_owned(),
            bench_path: bench_path.to_owned(),
            terminal_id: None,
            session_id: None,
            mode: "tui".to_owned(),
            state: AssignmentState::Starting,
            last_status: "starting".to_owned(),
            started_ms: now_ms(),
            last_event_ms: now_ms(),
            retries: 0,
            escalated: false,
        };
        self.assignments.push(assignment.clone());
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
            task.state = TaskState::Running;
        }
        Ok(assignment)
    }

    /// Registra o processo aberto pelo helper.
    pub fn launched(
        &mut self,
        assignment_id: &str,
        profile_id: &str,
        terminal_id: Option<String>,
        session_id: Option<String>,
        mode: &str,
    ) -> Result<(), String> {
        let assignment = self
            .assignments
            .iter_mut()
            .find(|assignment| assignment.id == assignment_id)
            .ok_or_else(|| "atribuição sumiu".to_owned())?;
        assignment.profile_id = profile_id.to_owned();
        assignment.terminal_id = terminal_id;
        assignment.session_id = session_id;
        assignment.mode = mode.to_owned();
        assignment.state = AssignmentState::Running;
        assignment.last_status = "working".to_owned();
        assignment.last_event_ms = now_ms();
        Ok(())
    }

    /// Marca a atribuição como falha de abertura.
    pub fn launch_failed(&mut self, assignment_id: &str, message: &str) {
        if let Some(assignment) = self
            .assignments
            .iter_mut()
            .find(|assignment| assignment.id == assignment_id)
        {
            assignment.state = AssignmentState::Failed;
            assignment.last_status = message.to_owned();
            assignment.last_event_ms = now_ms();
        }
        if let Some(task_id) = self
            .assignments
            .iter()
            .find(|assignment| assignment.id == assignment_id)
            .map(|assignment| assignment.task.clone())
        {
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
                task.state = TaskState::Failed;
            }
        }
    }

    /// Atualiza o status de uma atribuição e devolve um alerta quando ela para
    /// sem entregar.
    pub fn update_status(
        &mut self,
        assignment_id: &str,
        status: HelperStatus,
        terminal_tail: &str,
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
                    HelperStatus::Unknown => AssignmentState::Idle,
                };
            }
        }
        let stopped = matches!(status, HelperStatus::Idle | HelperStatus::Finished);
        if stopped && was_running && !delivered && !escalated {
            let assignment = self
                .assignments
                .iter_mut()
                .find(|assignment| assignment.id == assignment_id)?;
            assignment.escalated = true;
            assignment.retries = assignment.retries.saturating_add(1);
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
                if !matches!(task.state, TaskState::Delivered) {
                    task.state = TaskState::Blocked;
                }
            }
            let alert = self.note(
                &run,
                Some(task_id.clone()),
                Some(assignment_id.to_owned()),
                "muster",
                "coordinator",
                NoteKind::Alert,
                &format!("parou sem entregar: {}", tail(terminal_tail)),
                false,
                false,
            );
            return Some(alert);
        }
        None
    }

    /// Recado para um ajudante. Fica na fila quando ele está ocupado ou pedindo
    /// aprovação.
    pub fn note_to_helper(
        &mut self,
        assignment_id: &str,
        body: &str,
        kind: NoteKind,
    ) -> Result<Note, String> {
        let assignment = self
            .assignment(assignment_id)
            .ok_or_else(|| "atribuição não encontrada".to_owned())?;
        if matches!(
            assignment.state,
            AssignmentState::Delivered | AssignmentState::Failed | AssignmentState::Blocked
        ) {
            return Err("a atribuição já terminou".to_owned());
        }
        let queued = assignment.terminal_id.is_none()
            || matches!(
                assignment.state,
                AssignmentState::Starting
                    | AssignmentState::Running
                    | AssignmentState::NeedsYou
                    | AssignmentState::Stopped
            );
        let run = assignment.run.clone();
        let task = assignment.task.clone();
        Ok(self.note(
            &run,
            Some(task),
            Some(assignment_id.to_owned()),
            "coordinator",
            "helper",
            kind,
            body,
            false,
            queued,
        ))
    }

    /// Recado de um ajudante para o coordenador.
    pub fn note_from_helper(
        &mut self,
        assignment_id: &str,
        body: &str,
        kind: NoteKind,
    ) -> Result<Note, String> {
        let assignment = self
            .assignment(assignment_id)
            .ok_or_else(|| "atribuição não encontrada".to_owned())?;
        let run = assignment.run.clone();
        let task = assignment.task.clone();
        let profile = assignment.profile_id.clone();
        Ok(self.note(
            &run,
            Some(task),
            Some(assignment_id.to_owned()),
            &profile,
            "coordinator",
            kind,
            body,
            false,
            false,
        ))
    }

    /// Entrega de uma tarefa.
    pub fn handoff(
        &mut self,
        assignment_id: &str,
        result: HandoffResult,
        summary: &str,
        evidence: &str,
    ) -> Result<Note, String> {
        let (run, task_id, profile) = {
            let assignment = self
                .assignment(assignment_id)
                .ok_or_else(|| "atribuição não encontrada".to_owned())?;
            (
                assignment.run.clone(),
                assignment.task.clone(),
                assignment.profile_id.clone(),
            )
        };
        if result == HandoffResult::Done
            && self
                .task(&task_id)
                .is_some_and(|task| task.state == TaskState::Delivered)
        {
            return Err("tarefa já entregue".to_owned());
        }
        if let Some(assignment) = self
            .assignments
            .iter_mut()
            .find(|assignment| assignment.id == assignment_id)
        {
            assignment.state = result.assignment_state();
            assignment.last_status = result.as_str().to_owned();
            assignment.last_event_ms = now_ms();
        }
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) {
            task.state = result.task_state();
            task.result = Some(result.as_str().to_owned());
            task.summary = Some(summary.trim().to_owned());
            task.evidence = (!evidence.trim().is_empty()).then(|| evidence.trim().to_owned());
        }
        self.refresh_run(&run);
        Ok(self.note(
            &run,
            Some(task_id),
            Some(assignment_id.to_owned()),
            &profile,
            "coordinator",
            NoteKind::Handoff,
            &format!("{}: {}", result.as_str(), summary.trim()),
            false,
            false,
        ))
    }

    /// Resposta do coordenador a um recado. Devolve o novo recado.
    pub fn reply(&mut self, note_id: &str, body: &str) -> Result<Note, String> {
        let original = self
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .cloned()
            .ok_or_else(|| "recado não encontrado".to_owned())?;
        if let Some(note) = self.notes.iter_mut().find(|note| note.id == note_id) {
            note.read = true;
        }
        if let Some(assignment) = original.assignment.as_deref() {
            return self.note_to_helper(assignment, body, NoteKind::Reply);
        }
        Ok(self.note(
            &original.run,
            original.task.clone(),
            None,
            "coordinator",
            &original.from,
            NoteKind::Reply,
            body,
            true,
            false,
        ))
    }

    /// Recados ainda na fila de uma atribuição.
    pub fn queued_notes(&self, assignment_id: &str) -> Vec<Note> {
        self.notes
            .iter()
            .filter(|note| {
                note.assignment.as_deref() == Some(assignment_id)
                    && note.queued
                    && !note.delivered
                    && note.to == "helper"
            })
            .cloned()
            .collect()
    }

    /// Marca um recado como digitado.
    pub fn mark_delivered(&mut self, note_id: &str) {
        if let Some(note) = self.notes.iter_mut().find(|note| note.id == note_id) {
            note.delivered = true;
            note.queued = false;
        }
    }

    /// Caixa de entrada do coordenador. `all` inclui recados já lidos.
    pub fn inbox(&self, run: Option<&str>, all: bool) -> Vec<Note> {
        let mut notes: Vec<Note> = self
            .notes
            .iter()
            .filter(|note| note.to == "coordinator")
            .filter(|note| run.is_none_or(|run| note.run == run))
            .filter(|note| all || !note.read)
            .cloned()
            .collect();
        notes.sort_by_key(|note| std::cmp::Reverse(note.created_ms));
        notes
    }

    /// Tarefas prontas: pendentes e com todas as dependências entregues.
    pub fn ready_tasks(&self, run: &str) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|task| task.run == run && task.state == TaskState::Pending)
            .filter(|task| {
                task.deps.iter().all(|dep| {
                    self.task(dep)
                        .is_some_and(|dep| dep.state == TaskState::Delivered)
                })
            })
            .collect()
    }

    fn refresh_run(&mut self, run_id: &str) {
        let tasks: Vec<TaskState> = self
            .tasks
            .iter()
            .filter(|task| task.run == run_id)
            .map(|task| task.state)
            .collect();
        if tasks.is_empty() {
            return;
        }
        let state = if tasks.iter().all(|state| *state == TaskState::Delivered) {
            RunState::Done
        } else if tasks
            .iter()
            .any(|state| matches!(state, TaskState::Failed | TaskState::Blocked))
            && tasks
                .iter()
                .all(|state| matches!(state, TaskState::Delivered | TaskState::Failed | TaskState::Blocked))
        {
            RunState::Failed
        } else {
            RunState::Open
        };
        if let Some(run) = self.runs.iter_mut().find(|run| run.id == run_id) {
            run.state = state;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn note(
        &mut self,
        run: &str,
        task: Option<String>,
        assignment: Option<String>,
        from: &str,
        to: &str,
        kind: NoteKind,
        body: &str,
        read: bool,
        queued: bool,
    ) -> Note {
        let note = Note {
            id: self.id("note-"),
            run: run.to_owned(),
            task,
            assignment,
            from: from.to_owned(),
            to: to.to_owned(),
            kind,
            body: body.trim().to_owned(),
            read,
            queued,
            delivered: false,
            created_ms: now_ms(),
        };
        self.notes.push(note.clone());
        note
    }

    fn id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}{}", self.next_id)
    }
}

/// Status do ajudante em texto estável.
pub fn status_key(status: HelperStatus) -> &'static str {
    match status {
        HelperStatus::Unknown => "unknown",
        HelperStatus::Idle => "idle",
        HelperStatus::Working => "working",
        HelperStatus::NeedsYou => "needs_you",
        HelperStatus::Finished => "finished",
    }
}

fn default_true() -> bool {
    true
}

fn non_empty(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(30);
    lines[start..].join("\n").chars().take(4000).collect()
}

/// Hora atual em ms.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_prunes_expired_notes_but_keeps_queued_delivery() {
        let mut file = MusterFile { notes: vec![
            Note { created_ms: 10, ..Note::default() },
            Note { created_ms: 10, queued: true, ..Note::default() },
            Note { created_ms: 20, ..Note::default() },
        ], ..MusterFile::default() };
        assert_eq!(file.prune_notes(20), 1);
        assert_eq!(file.notes.len(), 2);
        assert!(file.notes[0].queued);
    }

    fn run_with_task(file: &mut MusterFile) -> (MusterRun, Task) {
        let run = file.create_run("Mutirão", "Objetivo", "coordinator", None);
        let task = file
            .add_task(&run.id, "Tarefa", "Spec", Vec::new())
            .unwrap();
        (run, task)
    }

    #[test]
    fn start_and_handoff_move_task_and_run() {
        let mut file = MusterFile::default();
        let (run, task) = run_with_task(&mut file);
        let assignment = file
            .start(&task.id, "claude-code", "C:/bench")
            .unwrap();
        assert_eq!(file.task(&task.id).unwrap().state, TaskState::Running);
        file.launched(
            &assignment.id,
            "claude-code",
            Some("1".into()),
            None,
            "tui",
        )
        .unwrap();
        let note = file
            .handoff(&assignment.id, HandoffResult::Done, "feito", "testes")
            .unwrap();
        assert_eq!(note.kind, NoteKind::Handoff);
        assert_eq!(file.task(&task.id).unwrap().state, TaskState::Delivered);
        assert_eq!(file.runs[0].state, RunState::Done);
        assert_eq!(run.state, RunState::Open);
        assert_eq!(file.inbox(Some(&run.id), false).len(), 1);
    }

    #[test]
    fn note_queues_when_the_helper_needs_you_and_sends_when_idle() {
        let mut file = MusterFile::default();
        let (_, task) = run_with_task(&mut file);
        let assignment = file.start(&task.id, "codex", "C:/bench").unwrap();
        file.launched(&assignment.id, "codex", Some("2".into()), None, "tui")
            .unwrap();
        file.update_status(&assignment.id, HelperStatus::NeedsYou, "");
        let note = file
            .note_to_helper(&assignment.id, "responda", NoteKind::Status)
            .unwrap();
        assert!(note.queued);
        assert!(!note.delivered);
        assert_eq!(file.queued_notes(&assignment.id).len(), 1);
        file.update_status(&assignment.id, HelperStatus::Idle, "done");
        let note = file
            .note_to_helper(&assignment.id, "obrigado", NoteKind::Status)
            .unwrap();
        assert!(!note.queued);
        assert!(!note.delivered);
        file.mark_delivered(&note.id);
        assert!(file
            .notes
            .iter()
            .find(|candidate| candidate.id == note.id)
            .is_some_and(|candidate| candidate.delivered));
    }

    #[test]
    fn stopping_without_handoff_creates_one_alert_with_the_tail() {
        let mut file = MusterFile::default();
        let (run, task) = run_with_task(&mut file);
        let assignment = file.start(&task.id, "claude-code", "C:/bench").unwrap();
        file.launched(&assignment.id, "claude-code", Some("3".into()), None, "tui")
            .unwrap();
        let alert = file
            .update_status(
                &assignment.id,
                HelperStatus::Idle,
                "line one\nlast line before the prompt",
            )
            .expect("alert");
        assert_eq!(alert.kind, NoteKind::Alert);
        assert!(alert.body.contains("last line"));
        assert_eq!(file.task(&task.id).unwrap().state, TaskState::Blocked);
        assert!(file
            .update_status(&assignment.id, HelperStatus::Idle, "again")
            .is_none());
        assert_eq!(file.inbox(Some(&run.id), false).len(), 1);
    }

    #[test]
    fn dependencies_gate_ready_tasks_until_delivery() {
        let mut file = MusterFile::default();
        let run = file.create_run("Run", "", "coordinator", None);
        let first = file.add_task(&run.id, "First", "", Vec::new()).unwrap();
        let second = file
            .add_task(&run.id, "Second", "", vec![first.id.clone()])
            .unwrap();
        assert_eq!(file.ready_tasks(&run.id).len(), 1);
        let assignment = file.start(&first.id, "codex", "C:/bench").unwrap();
        file.launched(&assignment.id, "codex", None, None, "tui")
            .unwrap();
        file.handoff(&assignment.id, HandoffResult::Done, "ok", "")
            .unwrap();
        let ready = file.ready_tasks(&run.id);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, second.id);
    }

    #[test]
    fn replies_route_to_the_original_assignment() {
        let mut file = MusterFile::default();
        let (_, task) = run_with_task(&mut file);
        let assignment = file.start(&task.id, "claude-code", "C:/bench").unwrap();
        file.launched(&assignment.id, "claude-code", Some("4".into()), None, "tui")
            .unwrap();
        let question = file
            .note_from_helper(&assignment.id, "qual branch?", NoteKind::Question)
            .unwrap();
        let reply = file.reply(&question.id, "main").unwrap();
        assert_eq!(reply.from, "coordinator");
        assert_eq!(reply.to, "helper");
        assert_eq!(reply.assignment.as_deref(), Some(assignment.id.as_str()));
    }

    #[test]
    fn handoff_results_accept_our_words_and_keep_state() {
        assert_eq!(HandoffResult::parse("feito").unwrap(), HandoffResult::Done);
        assert_eq!(
            HandoffResult::parse("falhou").unwrap(),
            HandoffResult::Failed
        );
        assert_eq!(
            HandoffResult::parse("bloqueado").unwrap(),
            HandoffResult::Blocked
        );
        assert_eq!(HandoffResult::Done.as_str(), "feito");
        assert!(HandoffResult::parse("talvez").is_err());
    }

    #[test]
    fn store_roundtrips_through_json() {
        let mut file = MusterFile::default();
        let (_, task) = run_with_task(&mut file);
        let assignment = file.start(&task.id, "codex", "C:/bench").unwrap();
        file.launched(&assignment.id, "codex", Some("5".into()), None, "tui")
            .unwrap();
        let text = serde_json::to_string(&file).unwrap();
        let restored: MusterFile = serde_json::from_str(&text).unwrap();
        assert_eq!(restored.runs.len(), 1);
        assert_eq!(restored.tasks.len(), 1);
        assert_eq!(restored.assignments[0].terminal_id.as_deref(), Some("5"));
    }
}
