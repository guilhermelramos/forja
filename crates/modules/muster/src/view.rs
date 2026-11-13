//! Página "Mutirão" no trilho.

use crate::commands;
use crate::model::{
    Assignment, AssignmentState, MusterRun, Note, NoteKind, Task, TaskState,
};
use crate::state::{self, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use forja_ui_kit::{applied_tokens, button, progress_bar, text_field, ButtonKind};
use serde_json::json;
use std::time::{Duration, Instant};

/// Chaves i18n usadas pela view.
pub const USED_KEYS: &[&str] = &[
    "muster.page.title",
    "muster.page.subtitle",
    "muster.empty",
    "muster.new-run",
    "muster.run-title",
    "muster.run-objective",
    "muster.create",
    "muster.plan-approved",
    "muster.plan-pending",
    "muster.plan-approve",
    "muster.budget",
    "muster.budget-hint",
    "muster.budget-none",
    "muster.budget-save",
    "muster.spend",
    "muster.spend-hint",
    "muster.spend-add",
    "muster.cancel",
    "muster.tasks",
    "muster.no-tasks",
    "muster.new-task",
    "muster.task-title",
    "muster.task-spec",
    "muster.task-deps",
    "muster.start",
    "muster.assignments",
    "muster.no-assignments",
    "muster.focus",
    "muster.inbox",
    "muster.no-notes",
    "muster.reply",
    "muster.reply-hint",
    "muster.send",
    "muster.note",
    "muster.note-hint",
    "muster.profile",
    "muster.bench",
    "muster.new-bench",
    "muster.status",
    "muster.retries",
    "muster.terminal",
    "muster.session",
    "muster.run-open",
    "muster.run-done",
    "muster.run-failed",
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
    "muster-kind-status",
    "muster-kind-question",
    "muster-kind-reply",
    "muster-kind-handoff",
    "muster-kind-alert",
];

#[derive(Default)]
struct Snapshot {
    runs: Vec<MusterRun>,
    tasks: Vec<Task>,
    assignments: Vec<Assignment>,
    loaded: bool,
    notice: Option<String>,
}

fn snapshot(shared: &Shared) -> Snapshot {
    state::with(shared, |state| Snapshot {
        runs: state.store.runs.clone(),
        tasks: state.store.tasks.clone(),
        assignments: state.store.assignments.clone(),
        loaded: state.loaded,
        notice: state.notice.clone(),
    })
    .unwrap_or_default()
}

/// Página do Mutirão.
pub struct MusterView {
    shared: Shared,
    selected: Option<String>,
    new_run: bool,
    run_title: String,
    run_objective: String,
    run_budget: String,
    budget_input: String,
    spend_input: String,
    new_task: bool,
    task_title: String,
    task_spec: String,
    task_deps: String,
    profile: String,
    bench: String,
    new_bench: bool,
    note_target: Option<String>,
    note_body: String,
    reply_to: Option<String>,
    reply_body: String,
    notice: Option<String>,
}

impl MusterView {
    /// Cria a página ligada ao estado.
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            selected: None,
            new_run: false,
            run_title: String::new(),
            run_objective: String::new(),
            run_budget: String::new(),
            budget_input: String::new(),
            spend_input: String::new(),
            new_task: false,
            task_title: String::new(),
            task_spec: String::new(),
            task_deps: String::new(),
            profile: String::new(),
            bench: String::new(),
            new_bench: false,
            note_target: None,
            note_body: String::new(),
            reply_to: None,
            reply_body: String::new(),
            notice: None,
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        ui.horizontal(|ui| {
            ui.heading(cx.t("muster.page.title"));
            ui.label(
                egui::RichText::new(cx.t("muster.page.subtitle"))
                    .small()
                    .weak(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if button(ui, ButtonKind::Primary, &cx.t("muster.new-run")).clicked() {
                    self.new_run = !self.new_run;
                }
            });
        });
        if let Some(notice) = &snap.notice {
            ui.label(egui::RichText::new(cx.t(notice)).weak());
        }
        if let Some(notice) = &self.notice {
            ui.label(
                egui::RichText::new(notice).color(
                    applied_tokens(ui.ctx()).color.danger.to_color32(),
                ),
            );
        }
        if !snap.loaded {
            ui.label(egui::RichText::new(cx.t("muster.page-loading")).weak());
        }
    }

    fn new_run_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.new_run {
            return;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(egui::RichText::new(cx.t("muster.run-title")).strong());
            text_field(ui, &mut self.run_title, &cx.t("muster.run-title"));
            ui.label(egui::RichText::new(cx.t("muster.run-objective")).strong());
            ui.add(
                egui::TextEdit::multiline(&mut self.run_objective)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            text_field(ui, &mut self.run_budget, &cx.t("muster.budget-hint"));
            ui.horizontal(|ui| {
                if button(ui, ButtonKind::Primary, &cx.t("muster.create")).clicked() {
                    let mut args = Args::new();
                    args.insert("title", json!(self.run_title));
                    args.insert("objective", json!(self.run_objective));
                    if let Ok(budget) = self.run_budget.trim().parse::<f64>() {
                        args.insert("budget_usd", json!(budget.max(0.0)));
                    }
                    match cx.run_command("muster.run", args) {
                        Ok(value) => {
                            self.selected = value
                                .get("id")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned);
                            self.new_run = false;
                            self.run_title.clear();
                            self.run_objective.clear();
                            self.run_budget.clear();
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
                if button(ui, ButtonKind::Ghost, &cx.t("muster.cancel")).clicked() {
                    self.new_run = false;
                }
            });
        });
    }

    fn run_list(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        if snap.runs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(cx.t("muster.empty")).weak());
            });
            return;
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            for run in &snap.runs {
                let selected = self.selected.as_deref() == Some(run.id.as_str());
                let label = format!("{}  ·  {}", run.title, run_state_label(cx, run.state));
                if ui.selectable_label(selected, label).clicked() {
                    self.selected = Some(run.id.clone());
                }
            }
        });
    }

    fn run_body(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        let Some(run) = snap.runs.iter().find(|run| {
            self.selected.as_deref() == Some(run.id.as_str())
        }) else {
            return;
        };
        ui.add_space(8.0);
        ui.label(egui::RichText::new(&run.title).heading());
        if !run.objective.is_empty() {
            ui.label(egui::RichText::new(&run.objective).weak());
        }
        self.run_controls(ui, cx, run);
        let tasks: Vec<&Task> = snap.tasks.iter().filter(|task| task.run == run.id).collect();
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(cx.t("muster.tasks")).strong());
            if button(ui, ButtonKind::Secondary, &cx.t("muster.new-task")).clicked() {
                self.new_task = !self.new_task;
            }
        });
        if self.new_task {
            self.new_task_form(ui, cx, &run.id);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(cx.t("muster.profile")).small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.profile)
                    .hint_text("claude-code / codex / auto")
                    .desired_width(130.0),
            );
            ui.label(egui::RichText::new(cx.t("muster.bench")).small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.bench)
                    .hint_text("C:/forja/pedido-123")
                    .desired_width(170.0),
            );
            ui.add_enabled(false, egui::Checkbox::new(&mut self.new_bench, cx.t("muster.new-bench")))
                .on_hover_text(cx.t("muster.error.bench-pending"));
        });
        if tasks.is_empty() {
            ui.label(egui::RichText::new(cx.t("muster.no-tasks")).weak());
        }
        for task in tasks {
            let assignment = snap
                .assignments
                .iter()
                .rev()
                .find(|assignment| assignment.task == task.id);
            self.task_row(ui, cx, task, assignment);
        }
        ui.separator();
        ui.label(egui::RichText::new(cx.t("muster.assignments")).strong());
        let assignments: Vec<&Assignment> = snap
            .assignments
            .iter()
            .filter(|assignment| {
                assignment.run == run.id
                    && matches!(
                        assignment.state,
                        AssignmentState::Starting
                            | AssignmentState::Running
                            | AssignmentState::NeedsYou
                            | AssignmentState::Idle
                    )
            })
            .collect();
        if assignments.is_empty() {
            ui.label(egui::RichText::new(cx.t("muster.no-assignments")).weak());
        }
        for assignment in assignments {
            self.assignment_row(ui, cx, assignment);
        }
        ui.separator();
        self.inbox(ui, cx, &run.id);
    }

    fn run_controls(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, run: &MusterRun) {
        ui.horizontal_wrapped(|ui| {
            let (label, color) = if run.plan_approved {
                (
                    cx.t("muster.plan-approved"),
                    applied_tokens(ui.ctx()).color.ok.to_color32(),
                )
            } else {
                (
                    cx.t("muster.plan-pending"),
                    applied_tokens(ui.ctx()).color.spark.to_color32(),
                )
            };
            ui.label(egui::RichText::new(label).color(color));
            if !run.plan_approved
                && button(ui, ButtonKind::Secondary, &cx.t("muster.plan-approve")).clicked()
            {
                let mut args = Args::new();
                args.insert("run", json!(run.id));
                match cx.run_command("muster.plan.approve", args) {
                    Ok(_) => self.notice = None,
                    Err(error) => self.notice = Some(error.to_string()),
                }
            }
            if run.budget_usd > 0.0 {
                let ratio = (run.spent_usd / run.budget_usd).clamp(0.0, 1.0) as f32;
                ui.label(
                    egui::RichText::new(format!(
                        "US$ {:.2} / {:.2}",
                        run.spent_usd, run.budget_usd
                    ))
                    .weak(),
                );
                progress_bar(ui, ratio);
            } else {
                ui.label(egui::RichText::new(cx.t("muster.budget-none")).weak());
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(cx.t("muster.budget")).small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.budget_input)
                    .hint_text(cx.t("muster.budget-hint"))
                    .desired_width(100.0),
            );
            if button(ui, ButtonKind::Ghost, &cx.t("muster.budget-save")).clicked() {
                if let Ok(budget) = self.budget_input.trim().parse::<f64>() {
                    let mut args = Args::new();
                    args.insert("run", json!(run.id));
                    args.insert("budget_usd", json!(budget.max(0.0)));
                    match cx.run_command("muster.budget.set", args) {
                        Ok(_) => self.notice = None,
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
            }
            ui.label(egui::RichText::new(cx.t("muster.spend")).small().weak());
            ui.add(
                egui::TextEdit::singleline(&mut self.spend_input)
                    .hint_text(cx.t("muster.spend-hint"))
                    .desired_width(100.0),
            );
            if button(ui, ButtonKind::Ghost, &cx.t("muster.spend-add")).clicked() {
                if let Ok(usd) = self.spend_input.trim().parse::<f64>() {
                    let mut args = Args::new();
                    args.insert("run", json!(run.id));
                    args.insert("usd", json!(usd));
                    match cx.run_command("muster.spend", args) {
                        Ok(_) => {
                            self.notice = None;
                            self.spend_input.clear();
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
            }
        });
    }

    fn new_task_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, run: &str) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            text_field(ui, &mut self.task_title, &cx.t("muster.task-title"));
            ui.label(egui::RichText::new(cx.t("muster.task-spec")).strong());
            ui.add(
                egui::TextEdit::multiline(&mut self.task_spec)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            text_field(ui, &mut self.task_deps, &cx.t("muster.task-deps"));
            if button(ui, ButtonKind::Primary, &cx.t("muster.create")).clicked() {
                let mut args = Args::new();
                args.insert("run", json!(run));
                args.insert("title", json!(self.task_title));
                args.insert("spec", json!(self.task_spec));
                args.insert("deps", json!(self.task_deps));
                match cx.run_command("muster.task", args) {
                    Ok(_) => {
                        self.new_task = false;
                        self.task_title.clear();
                        self.task_spec.clear();
                        self.task_deps.clear();
                    }
                    Err(error) => self.notice = Some(error.to_string()),
                }
            }
        });
    }

    fn task_row(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        task: &Task,
        assignment: Option<&Assignment>,
    ) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(task_state_label(cx, task.state))
                        .color(task_state_color(ui, task.state)),
                );
                ui.label(egui::RichText::new(&task.title).strong());
                if !task.deps.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            cx.t("muster.task-deps"),
                            task.deps.join(", ")
                        ))
                        .weak(),
                    );
                }
                if let Some(assignment) = assignment {
                    ui.label(
                        egui::RichText::new(assignment_state_label(cx, assignment.state))
                            .weak(),
                    );
                }
            });
            let can_start = !matches!(task.state, TaskState::Delivered)
                && assignment.is_none_or(|assignment| {
                    matches!(
                        assignment.state,
                        AssignmentState::Delivered
                            | AssignmentState::Failed
                            | AssignmentState::Blocked
                            | AssignmentState::Stopped
                    )
                });
            if can_start
                && button(ui, ButtonKind::Secondary, &cx.t("muster.start")).clicked()
            {
                let mut args = Args::new();
                args.insert("task", json!(task.id));
                if !self.profile.trim().is_empty() {
                    args.insert("profile_id", json!(self.profile.trim()));
                }
                if !self.bench.trim().is_empty() {
                    args.insert("bench_path", json!(self.bench.trim()));
                }
                args.insert("new_bench", json!(self.new_bench));
                match cx.run_command("muster.start", args) {
                    Ok(_) => self.notice = None,
                    Err(error) => self.notice = Some(error.to_string()),
                }
            }
            if !task.spec.is_empty() {
                ui.label(egui::RichText::new(&task.spec).weak());
            }
            if let Some(summary) = &task.summary {
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        task.result.as_deref().unwrap_or(""),
                        summary
                    ))
                    .small(),
                );
            }
        });
    }

    fn assignment_row(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, assignment: &Assignment) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(assignment_state_label(cx, assignment.state))
                    .color(assignment_state_color(ui, assignment.state)),
            );
            ui.label(egui::RichText::new(&assignment.profile_id).monospace());
            ui.label(egui::RichText::new(&assignment.bench_path).weak());
            if let Some(terminal) = &assignment.terminal_id {
                ui.label(
                    egui::RichText::new(format!("{} {terminal}", cx.t("muster.terminal")))
                        .weak(),
                );
                if button(ui, ButtonKind::Ghost, &cx.t("muster.focus")).clicked() {
                    let mut args = Args::new();
                    args.insert("terminal_id", json!(terminal));
                    if let Err(error) = cx.run_command("helper.focus", args) {
                        self.notice = Some(error.to_string());
                    }
                }
            }
            if let Some(session) = &assignment.session_id {
                ui.label(
                    egui::RichText::new(format!("{} {session}", cx.t("muster.session")))
                        .weak(),
                );
            }
            if assignment.retries > 0 {
                ui.label(
                    egui::RichText::new(format!("{}: {}", cx.t("muster.retries"), assignment.retries))
                        .weak(),
                );
            }
        });
    }

    fn inbox(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, run: &str) {
        ui.label(egui::RichText::new(cx.t("muster.inbox")).strong());
        let notes: Vec<Note> = state::with(&self.shared, |state| {
            state
                .store
                .inbox(Some(run), true)
        })
        .unwrap_or_default();
        if notes.is_empty() {
            ui.label(egui::RichText::new(cx.t("muster.no-notes")).weak());
        }
        for note in notes {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if !note.read {
                        ui.label(egui::RichText::new("●").color(
                            applied_tokens(ui.ctx()).color.spark.to_color32(),
                        ));
                    }
                    ui.label(egui::RichText::new(&note.from).strong());
                    ui.label(egui::RichText::new(note_kind_label(cx, note.kind)).weak());
                });
                ui.label(&note.body);
                if button(ui, ButtonKind::Ghost, &cx.t("muster.reply")).clicked() {
                    self.reply_to = Some(note.id.clone());
                }
            });
            if self.reply_to.as_deref() == Some(note.id.as_str()) {
                text_field(ui, &mut self.reply_body, &cx.t("muster.reply-hint"));
                if button(ui, ButtonKind::Primary, &cx.t("muster.send")).clicked() {
                    let mut args = Args::new();
                    args.insert("note", json!(note.id));
                    args.insert("body", json!(self.reply_body));
                    match cx.run_command("muster.reply", args) {
                        Ok(_) => {
                            self.reply_to = None;
                            self.reply_body.clear();
                        }
                        Err(error) => self.notice = Some(error.to_string()),
                    }
                }
            }
        }
        ui.separator();
        ui.label(egui::RichText::new(cx.t("muster.note")).strong());
        let assignments: Vec<Assignment> = state::with(&self.shared, |state| {
            state
                .store
                .assignments
                .iter()
                .filter(|assignment| {
                    assignment.run == run
                        && !matches!(
                            assignment.state,
                            AssignmentState::Delivered
                                | AssignmentState::Failed
                                | AssignmentState::Blocked
                        )
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
        if assignments.is_empty() {
            ui.label(egui::RichText::new(cx.t("muster.no-assignments")).weak());
            return;
        }
        let label = self
            .note_target
            .as_ref()
            .and_then(|id| assignments.iter().find(|assignment| &assignment.id == id))
            .map(|assignment| assignment.profile_id.clone())
            .unwrap_or_else(|| cx.t("muster.note-target"));
        egui::ComboBox::from_id_salt("muster-note-target")
            .selected_text(label)
            .show_ui(ui, |ui| {
                for assignment in &assignments {
                    ui.selectable_value(
                        &mut self.note_target,
                        Some(assignment.id.clone()),
                        format!("{} · {}", assignment.profile_id, assignment.id),
                    );
                }
            });
        text_field(ui, &mut self.note_body, &cx.t("muster.note-hint"));
        if button(ui, ButtonKind::Primary, &cx.t("muster.send")).clicked() {
            let Some(target) = self.note_target.clone() else {
                self.notice = Some("escolha uma atribuição".to_owned());
                return;
            };
            let mut args = Args::new();
            args.insert("assignment", json!(target));
            args.insert("body", json!(self.note_body));
            args.insert("from", json!("coordinator"));
            match cx.run_command("muster.note", args) {
                Ok(_) => self.note_body.clear(),
                Err(error) => self.notice = Some(error.to_string()),
            }
        }
    }
}

impl View for MusterView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        state::ensure_loaded(cx, &self.shared);
        state::seed_preview(&self.shared);
        if self.selected.is_none() {
            self.selected = state::with(&self.shared, |state| {
                state.store.runs.last().map(|run| run.id.clone())
            })
            .flatten();
        }
        let snap = snapshot(&self.shared);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.header(ui, cx, &snap);
                self.new_run_form(ui, cx);
                self.run_list(ui, cx, &snap);
                self.run_body(ui, cx, &snap);
            });
    }
}

fn run_state_label(cx: &Ctx<'_>, state: crate::model::RunState) -> String {
    cx.t(match state {
        crate::model::RunState::Open => "muster.run-open",
        crate::model::RunState::Done => "muster.run-done",
        crate::model::RunState::Failed => "muster.run-failed",
    })
}

fn task_state_label(cx: &Ctx<'_>, state: TaskState) -> String {
    cx.t(commands::task_key(state))
}

fn task_state_color(ui: &egui::Ui, state: TaskState) -> egui::Color32 {
    let tokens = applied_tokens(ui.ctx());
    match state {
        TaskState::Pending => tokens.color.text_muted.to_color32(),
        TaskState::Running => tokens.color.info.to_color32(),
        TaskState::Delivered => tokens.color.ok.to_color32(),
        TaskState::Failed => tokens.color.danger.to_color32(),
        TaskState::Blocked => tokens.color.spark.to_color32(),
    }
}

fn assignment_state_label(cx: &Ctx<'_>, state: AssignmentState) -> String {
    cx.t(commands::assignment_key(state))
}

fn assignment_state_color(ui: &egui::Ui, state: AssignmentState) -> egui::Color32 {
    let tokens = applied_tokens(ui.ctx());
    match state {
        AssignmentState::Starting | AssignmentState::Idle => {
            tokens.color.text_muted.to_color32()
        }
        AssignmentState::Running => tokens.color.info.to_color32(),
        AssignmentState::NeedsYou => tokens.color.spark.to_color32(),
        AssignmentState::Delivered => tokens.color.ok.to_color32(),
        AssignmentState::Failed | AssignmentState::Blocked => {
            tokens.color.danger.to_color32()
        }
        AssignmentState::Stopped => tokens.color.text_muted.to_color32(),
    }
}

/// Passo da barra de status: tenta entregar recados na fila no intervalo
/// configurado, mesmo com a página fechada.
pub struct MusterTick {
    shared: Shared,
    last: Instant,
}

impl MusterTick {
    /// Cria o passo ligado ao estado.
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            last: Instant::now(),
        }
    }
}

impl View for MusterTick {
    fn ui(&mut self, _ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let seconds = cx
            .setting("muster.notes.poll_s")
            .and_then(|value| value.as_i64())
            .unwrap_or(3)
            .clamp(1, 60) as u64;
        if self.last.elapsed() < Duration::from_secs(seconds) {
            return;
        }
        self.last = Instant::now();
        commands::flush_pending(cx, &self.shared);
    }
}

fn note_kind_label(cx: &Ctx<'_>, kind: NoteKind) -> String {
    cx.t(match kind {
        NoteKind::Status => "muster.kind.status",
        NoteKind::Question => "muster.kind.question",
        NoteKind::Reply => "muster.kind.reply",
        NoteKind::Handoff => "muster.kind.handoff",
        NoteKind::Alert => "muster.kind.alert",
    })
}
