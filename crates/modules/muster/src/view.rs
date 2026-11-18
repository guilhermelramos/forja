//! Página "Mutirão" no trilho.

use crate::commands;
use crate::cost::{self, SpendBook};
use crate::model::{
    Assignment, AssignmentState, MusterRun, Note, NoteDeliveryState, NoteKind, Task, TaskState,
};
use crate::plan::{self, PlanGate};
use crate::state::{self, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, Locale};
use forja_ui_kit::{
    applied_tokens, button, icon_button, modal_explicit_close, progress_bar, text_field,
    wrapped_label, ButtonKind, CLOSE,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Chaves i18n usadas pela view.
pub const USED_KEYS: &[&str] = &[
    "muster.page.title",
    "muster.page.subtitle",
    "muster.page-plan",
    "muster.empty",
    "muster.new-run",
    "muster.run-title",
    "muster.run-objective",
    "muster.create",
    "muster.plan-approved",
    "muster.plan-pending",
    "muster.plan-approve",
    "muster.plan-confirm",
    "muster.plan-view",
    "muster.plan-summary",
    "muster.plan-review-title",
    "muster.plan-review-hint",
    "muster.plan-task-count",
    "muster.plan-dep-count",
    "muster.plan-helper",
    "muster.plan-helper-auto",
    "muster.plan-ceiling",
    "muster.plan-empty",
    "muster.plan-task-deps",
    "muster.plan-no-deps",
    "muster.budget",
    "muster.budget-hint",
    "muster.budget-none",
    "muster.budget-save",
    "muster.spend",
    "muster.spend-hint",
    "muster.spend-add",
    "muster.cost-title",
    "muster.cost-auto",
    "muster.cost-manual",
    "muster.cost-pending",
    "muster.cost-shared",
    "muster.cost-unpriced",
    "muster.cost-unavailable",
    "muster.cost-value",
    "muster.cost-now",
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
    "muster-error-choose-assignment",
    "muster-error-disabled",
    "muster-error-unavailable",
    "muster-error-not-found",
    "muster-error-command",
    "muster-notice-dismiss",
    "muster.profile-auto",
    "muster.profile-not-installed",
    "muster.sender-coordinator",
    "muster.note-state.delivered",
    "muster.note-state.queued",
    "muster.note-state.no-helper",
    "muster.note-target-no-helper",
    "muster.no-targets",
    "muster.error.empty-note",
];

#[derive(Clone)]
struct HelperOption {
    id: String,
    name: String,
    installed: bool,
}

#[derive(Clone)]
struct NoteTarget {
    id: String,
    is_assignment: bool,
    label: String,
}

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
    helper_options: Vec<HelperOption>,
    last_helper_sync: Option<Instant>,
    plan_gate: PlanGate,
    spend: SpendBook,
    spend_run: Option<String>,
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
            helper_options: Vec::new(),
            last_helper_sync: None,
            plan_gate: PlanGate::default(),
            spend: SpendBook::default(),
            spend_run: None,
        }
    }

    fn sync_helpers(&mut self, cx: &mut Ctx<'_>) {
        let should_refresh = self
            .last_helper_sync
            .is_none_or(|last| last.elapsed() >= Duration::from_secs(2));
        if !should_refresh {
            return;
        }
        self.last_helper_sync = Some(Instant::now());
        if cx.registry().command("helper.list").is_none() {
            return;
        }
        let Ok(value) = cx.run_command("helper.list", Args::new()) else {
            return;
        };
        let Some(items) = value.get("profiles").and_then(serde_json::Value::as_array) else {
            return;
        };
        let mut parsed: Vec<HelperOption> = items
            .iter()
            .filter_map(|item| {
                let id = item
                    .get("id")
                    .and_then(serde_json::Value::as_str)?
                    .to_owned();
                let name = item
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or(&id)
                    .to_owned();
                let installed = item
                    .get("installed")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                Some(HelperOption {
                    id,
                    name,
                    installed,
                })
            })
            .collect();
        parsed.sort_by_key(|option| (!option.installed, option.name.to_lowercase()));
        self.helper_options = parsed;
    }

    fn sync_spend(&mut self, cx: &mut Ctx<'_>, snap: &Snapshot, run: &MusterRun) {
        let benches = all_benches(run, &snap.assignments);
        let force = self.spend_run.as_deref() != Some(run.id.as_str());
        self.spend_run = Some(run.id.clone());
        self.spend.sync(cx, &benches, force);
    }

    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) -> forja_ui_kit::Page {
        let title = cx.t("muster.page.title");
        let subtitle = cx.t("muster.page.subtitle");
        let create = cx.t("muster.new-run");
        let refresh = cx.t("muster.page-state.refresh");
        let page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("muster.page", &title)
                .wide()
                .subtitle(&subtitle)
                .primary(&create)
                .refresh(&refresh),
        );
        if page.response.primary_clicked {
            self.new_run = !self.new_run;
        }
        if page.response.refresh_clicked {
            self.last_helper_sync = None;
            self.spend_run = None;
            self.sync_helpers(cx);
        }
        page
    }

    fn notices(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        if let Some(notice) = &snap.notice {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Error,
                &cx.t("muster.page-state.error"),
                Some(&cx.t(notice)),
            );
        }
        if let Some(notice) = self.notice.clone() {
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Error,
                    &cx.t("muster.page-state.error"),
                    Some(&notice),
                );
                if icon_button(ui, CLOSE, &cx.t("muster.notice.dismiss"))
                    .on_hover_text(cx.t("muster.notice.dismiss"))
                    .clicked()
                {
                    self.notice = None;
                }
            });
        }
        if !snap.loaded {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Loading,
                &cx.t("muster.page-state.loading"),
                None,
            );
        }
    }

    fn new_run_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.new_run {
            return;
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.run-title")).strong());
            text_field(ui, &mut self.run_title, &cx.t("muster.run-title"));
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.run-objective")).strong());
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
                            self.notice = None;
                            self.selected = value
                                .get("id")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned);
                            self.new_run = false;
                            self.run_title.clear();
                            self.run_objective.clear();
                            self.run_budget.clear();
                        }
                        Err(error) => self.set_error(cx, &error.to_string()),
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
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Empty,
                &cx.t("muster.page-state.empty"),
                Some(&cx.t("muster.empty")),
            );
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

    fn run_heading(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>, run: &MusterRun) {
        ui.add_space(forja_ui_kit::Spacing::SM);
        ui.horizontal_wrapped(|ui| {
            forja_ui_kit::text(ui, egui::RichText::new(&run.title).heading());
            forja_ui_kit::text(ui, 
                egui::RichText::new(run_state_label(cx, run.state))
                    .small()
                    .weak(),
            );
        });
        if !run.objective.is_empty() {
            let tokens = applied_tokens(ui.ctx());
            let metrics = tokens.metrics();
            wrapped_label(
                ui,
                &run.objective,
                egui::FontId::proportional(metrics.body),
                tokens.color.text_muted.to_color32(),
            );
        }
    }

    fn plan_column(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
        max_height: f32,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("muster-plan-column")
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.page-plan")).strong());
                    let (label, color) = plan_state(ui, cx, run);
                    forja_ui_kit::text(ui, egui::RichText::new(label).small().color(color));
                });
                self.plan_summary(ui, cx, snap, run);
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.tasks")).strong());
                    if button(ui, ButtonKind::Secondary, &cx.t("muster.new-task")).clicked() {
                        self.new_task = !self.new_task;
                    }
                });
                if self.new_task {
                    self.new_task_form(ui, cx, &run.id);
                }
                let tasks: Vec<&Task> = snap
                    .tasks
                    .iter()
                    .filter(|task| task.run == run.id)
                    .collect();
                if tasks.is_empty() {
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.no-tasks")).weak());
                }
                let shared = shared_benches(run, &snap.assignments);
                for task in tasks {
                    self.task_row(ui, cx, task, snap, &shared);
                }
                ui.add_space(6.0);
                self.task_pickers(ui, cx);
            });
    }

    fn plan_summary(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
    ) {
        let task_count = snap.tasks.iter().filter(|task| task.run == run.id).count();
        let dep_count: usize = snap
            .tasks
            .iter()
            .filter(|task| task.run == run.id)
            .map(|task| task.deps.len())
            .sum();
        let helper = self.helper_label(cx, self.profile.trim());
        let count = cx.t_args(
            "muster.plan-task-count",
            &[("count", &task_count.to_string())],
        );
        let deps = cx.t_args(
            "muster.plan-dep-count",
            &[("count", &dep_count.to_string())],
        );
        let ceiling = budget_label(cx, run.budget_usd);
        let title = cx.t("muster.plan-summary");
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            forja_ui_kit::text(ui, egui::RichText::new(&title).small().strong());
            forja_ui_kit::text(ui, 
                egui::RichText::new(format!("{count} · {deps}"))
                    .small()
                    .weak(),
            );
            forja_ui_kit::text(ui, 
                egui::RichText::new(format!("{}: {helper}", cx.t("muster.plan-helper")))
                    .small()
                    .weak(),
            );
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!("{}: ", cx.t("muster.plan-ceiling")))
                        .small()
                        .weak(),
                );
                forja_ui_kit::text(ui, egui::RichText::new(ceiling).small());
                if !run.plan_approved {
                    if button(ui, ButtonKind::Primary, &cx.t("muster.plan-approve")).clicked() {
                        self.plan_gate.open();
                    }
                } else if button(ui, ButtonKind::Ghost, &cx.t("muster.plan-view")).clicked() {
                    self.plan_gate.open();
                }
            });
        });
    }

    fn task_pickers(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ui.add_space(forja_ui_kit::Spacing::XS);
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.profile")).small().weak());
        let auto_label = cx.t("muster.profile-auto");
        let not_installed = cx.t("muster.profile-not-installed");
        let selected_label = if self.profile.trim().is_empty() {
            auto_label.clone()
        } else if let Some(found) = self
            .helper_options
            .iter()
            .find(|opt| opt.id == self.profile)
        {
            if found.installed {
                found.name.clone()
            } else {
                format!("{} ({not_installed})", found.name)
            }
        } else {
            self.profile.clone()
        };
        egui::ComboBox::from_id_salt("muster-profile-picker")
            .selected_text(selected_label)
            .width(ui.available_width().clamp(120.0, 190.0))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(self.profile.trim().is_empty(), &auto_label)
                    .clicked()
                {
                    self.profile.clear();
                }
                for option in &self.helper_options {
                    let label = if option.installed {
                        option.name.clone()
                    } else {
                        format!("{} ({not_installed})", option.name)
                    };
                    if ui
                        .selectable_label(self.profile == option.id, label)
                        .clicked()
                    {
                        self.profile = option.id.clone();
                    }
                }
            });
        ui.add_space(forja_ui_kit::Spacing::XS);
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.bench")).small().weak());
        ui.add(
            egui::TextEdit::singleline(&mut self.bench)
                .hint_text("C:/forja/pedido-123")
                .desired_width(ui.available_width().clamp(120.0, 220.0)),
        );
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut self.new_bench, cx.t("muster.new-bench")),
        )
        .on_hover_text(cx.t("muster.error.bench-pending"));
    }

    fn new_task_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, run: &str) {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            text_field(ui, &mut self.task_title, &cx.t("muster.task-title"));
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.task-spec")).strong());
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
                        self.notice = None;
                        self.new_task = false;
                        self.task_title.clear();
                        self.task_spec.clear();
                        self.task_deps.clear();
                    }
                    Err(error) => self.set_error(cx, &error.to_string()),
                }
            }
        });
    }

    fn task_row(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        task: &Task,
        snap: &Snapshot,
        shared: &BTreeMap<String, usize>,
    ) {
        let assignment = snap
            .assignments
            .iter()
            .rev()
            .find(|assignment| assignment.task == task.id);
        let dependencies = plan::dependency_titles(task, &snap.tasks);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(task_state_label(cx, task.state))
                        .small()
                        .color(task_state_color(ui, task.state)),
                );
                forja_ui_kit::text(ui, egui::RichText::new(&task.title).strong());
            });
            if !dependencies.is_empty() {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t_args(
                        "muster.plan-task-deps",
                        &[("deps", &dependencies.join(", "))],
                    ))
                    .small()
                    .weak(),
                );
            }
            if let Some(assignment) = assignment {
                ui.horizontal_wrapped(|ui| {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(assignment_state_label(cx, assignment.state))
                            .small()
                            .weak(),
                    );
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(self.helper_label(cx, &assignment.profile_id))
                            .small()
                            .monospace(),
                    );
                });
                if let Some(cost) = self.assignment_cost(cx, assignment, shared) {
                    forja_ui_kit::text(ui, egui::RichText::new(cost).small().weak());
                }
            }
            if !task.spec.is_empty() {
                forja_ui_kit::text(ui, egui::RichText::new(&task.spec).weak());
            }
            if let Some(summary) = &task.summary {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!(
                        "{} · {}",
                        task.result.as_deref().unwrap_or(""),
                        summary
                    ))
                    .small(),
                );
            }
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
            if can_start && button(ui, ButtonKind::Secondary, &cx.t("muster.start")).clicked() {
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
                    Err(error) => self.set_error(cx, &error.to_string()),
                }
            }
        });
    }

    fn assignment_cost(
        &self,
        cx: &Ctx<'_>,
        assignment: &Assignment,
        shared: &BTreeMap<String, usize>,
    ) -> Option<String> {
        let info = self.spend.get(&assignment.bench_path)?;
        if !info.available() {
            return None;
        }
        let mut label = cost_value(cx, info.usd);
        let count = shared
            .get(&cost::normalize(&assignment.bench_path))
            .copied()
            .unwrap_or(0);
        if count > 1 {
            label.push_str(&format!(" · {}", cx.t("muster.cost-shared")));
        }
        Some(label)
    }

    fn assignment_column(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
        max_height: f32,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("muster-assignments-column")
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.assignments")).strong());
                let active: Vec<&Assignment> = snap
                    .assignments
                    .iter()
                    .filter(|assignment| assignment.run == run.id)
                    .filter(|assignment| {
                        matches!(
                            assignment.state,
                            AssignmentState::Starting
                                | AssignmentState::Running
                                | AssignmentState::NeedsYou
                                | AssignmentState::Idle
                        )
                    })
                    .collect();
                if active.is_empty() {
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.no-assignments")).weak());
                    return;
                }
                let shared = shared_benches(run, &snap.assignments);
                let mut by_helper: BTreeMap<String, Vec<&Assignment>> = BTreeMap::new();
                for assignment in active {
                    by_helper
                        .entry(assignment.profile_id.clone())
                        .or_default()
                        .push(assignment);
                }
                for (profile, assignments) in by_helper {
                    ui.add_space(forja_ui_kit::Spacing::XS);
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(if profile.trim().is_empty() {
                            cx.t("muster.profile-auto")
                        } else {
                            self.helper_label(cx, &profile)
                        })
                        .strong(),
                    );
                    for assignment in assignments {
                        self.assignment_card(ui, cx, assignment, snap, &shared);
                    }
                }
            });
    }

    fn assignment_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        assignment: &Assignment,
        snap: &Snapshot,
        shared: &BTreeMap<String, usize>,
    ) {
        let task_title = snap
            .tasks
            .iter()
            .find(|task| task.id == assignment.task)
            .map(|task| task.title.as_str())
            .unwrap_or(&assignment.task);
        let bench_label = std::path::Path::new(&assignment.bench_path)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(&assignment.bench_path);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(assignment_state_label(cx, assignment.state))
                        .small()
                        .color(assignment_state_color(ui, assignment.state)),
                );
                forja_ui_kit::text(ui, egui::RichText::new(task_title).strong());
            });
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui, egui::RichText::new(bench_label).small().weak())
                    .on_hover_text(&assignment.bench_path);
                if let Some(terminal) = &assignment.terminal_id {
                    if button(ui, ButtonKind::Ghost, &cx.t("muster.focus")).clicked() {
                        let mut args = Args::new();
                        args.insert("terminal_id", json!(terminal));
                        match cx.run_command("helper.focus", args) {
                            Ok(_) => self.notice = None,
                            Err(error) => self.set_error(cx, &error.to_string()),
                        }
                    }
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(format!("{} {terminal}", cx.t("muster.terminal")))
                            .small()
                            .weak(),
                    );
                } else if assignment.session_id.is_some()
                    && button(ui, ButtonKind::Ghost, &cx.t("muster.focus")).clicked()
                {
                    if cx.registry().command("shell.bench.focus").is_some() {
                        let mut focus_bench = Args::new();
                        focus_bench.insert("path", json!(assignment.bench_path));
                        let _ = cx.run_command("shell.bench.focus", focus_bench);
                    }
                    let mut open_chat = Args::new();
                    open_chat.insert("view_id", json!("chat.tab"));
                    match cx.run_command("shell.tab.open", open_chat) {
                        Ok(_) => self.notice = None,
                        Err(error) => self.set_error(cx, &error.to_string()),
                    }
                }
            });
            if let Some(session) = &assignment.session_id {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!("{} {session}", cx.t("muster.session")))
                        .small()
                        .weak(),
                );
            }
            if assignment.retries > 0 {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("muster.retries"),
                        assignment.retries
                    ))
                    .small()
                    .weak(),
                );
            }
            if let Some(cost) = self.assignment_cost(cx, assignment, shared) {
                forja_ui_kit::text(ui, egui::RichText::new(cost).small());
            }
        });
    }

    fn inbox_column(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
        max_height: f32,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("muster-inbox-column")
            .auto_shrink([false, false])
            .max_height(max_height)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                self.cost_panel(ui, cx, snap, run);
                ui.add_space(6.0);
                ui.separator();
                self.inbox(ui, cx, &run.id);
            });
    }

    fn cost_panel(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
    ) {
        let benches = run_benches(run, &snap.assignments);
        let total = self.spend.total(&benches);
        let pending = total.is_none() && self.spend.any_pending(&benches);
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.cost-title")).strong());
        if let Some(total) = total {
            forja_ui_kit::text(ui, 
                egui::RichText::new(cost_value(cx, total.usd))
                    .size(forja_ui_kit::TextRole::Display.size())
                    .strong(),
            );
            self.budget_bar(ui, cx, run, total.usd);
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.cost-auto")).small().weak());
            if total.unpriced_tokens > 0 {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t("muster.cost-unpriced"))
                        .small()
                        .color(applied_tokens(ui.ctx()).color.spark.to_color32()),
                );
            }
            if total.pending {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t("muster.cost-pending"))
                        .small()
                        .weak(),
                );
            }
        } else if pending {
            forja_ui_kit::text(ui, 
                egui::RichText::new(cx.t("muster.cost-pending"))
                    .small()
                    .weak(),
            );
        } else {
            forja_ui_kit::text(ui, 
                egui::RichText::new(cx.t("muster.cost-manual"))
                    .small()
                    .weak(),
            );
            if !self.spend.contract_available() {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t("muster.cost-unavailable"))
                        .small()
                        .weak(),
                );
            }
            if run.spent_usd > 0.0 {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cost_value(cx, run.spent_usd))
                        .size(forja_ui_kit::TextRole::Display.size())
                        .strong(),
                );
                self.budget_bar(ui, cx, run, run.spent_usd);
            }
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.spend")).small().weak());
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.spend_input)
                        .hint_text(cx.t("muster.spend-hint"))
                        .desired_width(90.0),
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
                            Err(error) => self.set_error(cx, &error.to_string()),
                        }
                    }
                }
            });
        }
        ui.add_space(6.0);
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.budget")).small().weak());
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.budget_input)
                    .hint_text(cx.t("muster.budget-hint"))
                    .desired_width(90.0),
            );
            if button(ui, ButtonKind::Ghost, &cx.t("muster.budget-save")).clicked() {
                if let Ok(budget) = self.budget_input.trim().parse::<f64>() {
                    let mut args = Args::new();
                    args.insert("run", json!(run.id));
                    args.insert("budget_usd", json!(budget.max(0.0)));
                    match cx.run_command("muster.budget.set", args) {
                        Ok(_) => self.notice = None,
                        Err(error) => self.set_error(cx, &error.to_string()),
                    }
                }
            }
        });
    }

    fn budget_bar(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, run: &MusterRun, spent: f64) {
        if run.budget_usd > 0.0 {
            let ratio = (spent / run.budget_usd).clamp(0.0, 1.0) as f32;
            forja_ui_kit::text(ui, 
                egui::RichText::new(format!(
                    "{} / {}",
                    cost_value(cx, spent),
                    cost_value(cx, run.budget_usd)
                ))
                .small()
                .weak(),
            );
            progress_bar(ui, ratio);
        } else {
            forja_ui_kit::text(ui, 
                egui::RichText::new(cx.t("muster.budget-none"))
                    .small()
                    .weak(),
            );
        }
    }

    fn plan_review(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        snap: &Snapshot,
        run: &MusterRun,
    ) {
        if !self.plan_gate.is_open() {
            return;
        }
        let mut open = true;
        let mut approve = false;
        let mut cancel = false;
        let title = cx.t("muster.plan-review-title");
        let ctx = ui.ctx().clone();
        modal_explicit_close(&ctx, &mut open, &title, |ui| {
            ui.set_min_width(380.0);
            forja_ui_kit::text(ui, 
                egui::RichText::new(cx.t("muster.plan-review-hint"))
                    .small()
                    .weak(),
            );
            ui.add_space(6.0);
            self.plan_details(ui, cx, snap, run);
            ui.add_space(forja_ui_kit::Spacing::SM);
            ui.horizontal_wrapped(|ui| {
                if !run.plan_approved
                    && button(ui, ButtonKind::Primary, &cx.t("muster.plan-confirm")).clicked()
                {
                    approve = true;
                }
                if button(ui, ButtonKind::Ghost, &cx.t("muster.cancel")).clicked() {
                    cancel = true;
                }
            });
        });
        if cancel {
            open = false;
        }
        if !open {
            self.plan_gate.close();
        }
        if approve && self.plan_gate.can_approve() {
            let mut args = Args::new();
            args.insert("run", json!(run.id));
            match cx.run_command("muster.plan.approve", args) {
                Ok(_) => {
                    self.notice = None;
                    self.plan_gate.approved();
                }
                Err(error) => self.set_error(cx, &error.to_string()),
            }
        }
    }

    fn plan_details(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, snap: &Snapshot, run: &MusterRun) {
        forja_ui_kit::text(ui, egui::RichText::new(&run.title).strong());
        if !run.objective.is_empty() {
            let tokens = applied_tokens(ui.ctx());
            let metrics = tokens.metrics();
            wrapped_label(
                ui,
                &run.objective,
                egui::FontId::proportional(metrics.body),
                tokens.color.text_muted.to_color32(),
            );
        }
        let tasks: Vec<&Task> = snap
            .tasks
            .iter()
            .filter(|task| task.run == run.id)
            .collect();
        let dep_count: usize = tasks.iter().map(|task| task.deps.len()).sum();
        forja_ui_kit::text(ui, 
            egui::RichText::new(format!(
                "{} · {}",
                cx.t_args(
                    "muster.plan-task-count",
                    &[("count", &tasks.len().to_string())],
                ),
                cx.t_args(
                    "muster.plan-dep-count",
                    &[("count", &dep_count.to_string())],
                )
            ))
            .small()
            .weak(),
        );
        ui.separator();
        if tasks.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.plan-empty")).weak());
        }
        for (index, task) in tasks.iter().enumerate() {
            let dependencies = plan::dependency_titles(task, &snap.tasks);
            let suggested = plan::suggested_profile(task, &snap.assignments, self.profile.trim());
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(format!("{}.", index + 1))
                            .small()
                            .weak(),
                    );
                    forja_ui_kit::text(ui, egui::RichText::new(&task.title).strong());
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(task_state_label(cx, task.state))
                            .small()
                            .color(task_state_color(ui, task.state)),
                    );
                });
                let deps = if dependencies.is_empty() {
                    cx.t("muster.plan-no-deps")
                } else {
                    cx.t_args(
                        "muster.plan-task-deps",
                        &[("deps", &dependencies.join(", "))],
                    )
                };
                forja_ui_kit::text(ui, egui::RichText::new(deps).small().weak());
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("muster.plan-helper"),
                        self.helper_label(cx, suggested)
                    ))
                    .small()
                    .weak(),
                );
            });
        }
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            forja_ui_kit::text(ui, 
                egui::RichText::new(format!("{}:", cx.t("muster.plan-ceiling")))
                    .small()
                    .weak(),
            );
            forja_ui_kit::text(ui, egui::RichText::new(budget_label(cx, run.budget_usd)).small());
        });
        let benches = run_benches(run, &snap.assignments);
        if let Some(total) = self.spend.total(&benches) {
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!("{}:", cx.t("muster.cost-now")))
                        .small()
                        .weak(),
                );
                forja_ui_kit::text(ui, egui::RichText::new(cost_value(cx, total.usd)).small());
            });
        }
    }

    fn inbox(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, run: &str) {
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.inbox")).strong());
        let (notes, task_titles, assignments, tasks) = state::with(&self.shared, |state| {
            let titles: BTreeMap<String, String> = state
                .store
                .tasks
                .iter()
                .filter(|task| task.run == run)
                .map(|task| (task.id.clone(), task.title.clone()))
                .collect();
            let notes: Vec<(Note, Option<NoteDeliveryState>)> = state
                .store
                .conversation(Some(run))
                .into_iter()
                .map(|note| {
                    let delivery_state = state.store.note_delivery_state(&note);
                    (note, delivery_state)
                })
                .collect();
            let assignments: Vec<Assignment> = state
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
                .collect();
            let tasks: Vec<Task> = state
                .store
                .tasks
                .iter()
                .filter(|task| task.run == run)
                .cloned()
                .collect();
            (notes, titles, assignments, tasks)
        })
        .unwrap_or_default();

        if notes.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.no-notes")).weak());
        } else {
            egui::ScrollArea::vertical()
                .id_salt(("muster-inbox", run))
                .max_height(260.0)
                .show(ui, |ui| {
                    for (note, delivery_state) in &notes {
                        self.inbox_note_card(ui, cx, &task_titles, note, *delivery_state);
                    }
                });
        }
        ui.separator();
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.note")).strong());
        let targets = self.note_targets(cx, &task_titles, &assignments, &tasks);
        if targets.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("muster.no-targets")).weak());
            return;
        }
        let selected_target = self
            .note_target
            .as_ref()
            .and_then(|id| targets.iter().find(|target| &target.id == id))
            .cloned();
        let label = selected_target
            .as_ref()
            .map(|target| target.label.clone())
            .unwrap_or_else(|| cx.t("muster.note-target"));
        egui::ComboBox::from_id_salt("muster-note-target")
            .selected_text(label)
            .width(ui.available_width().clamp(120.0, 260.0))
            .truncate()
            .show_ui(ui, |ui| {
                for target in &targets {
                    ui.selectable_value(
                        &mut self.note_target,
                        Some(target.id.clone()),
                        &target.label,
                    );
                }
            });
        ui.horizontal_wrapped(|ui| {
            let hint = cx.t("muster.note-hint");
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.note_body)
                    .hint_text(&hint)
                    .desired_width((ui.available_width() - 96.0).max(90.0)),
            );
            input.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), hint.clone())
            });
            let enter = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if button(ui, ButtonKind::Primary, &cx.t("muster.send")).clicked() || enter {
                self.submit_note(cx, selected_target.as_ref());
            }
        });
    }

    fn inbox_note_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        task_titles: &BTreeMap<String, String>,
        note: &Note,
        delivery_state: Option<NoteDeliveryState>,
    ) {
        let tokens = applied_tokens(ui.ctx());
        let incoming = note.to == "coordinator";
        let sender = if note.from == "coordinator" {
            cx.t("muster.sender-coordinator")
        } else {
            self.helper_label(cx, &note.from)
        };
        let task_title = note
            .task
            .as_deref()
            .and_then(|id| task_titles.get(id))
            .map(String::as_str);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                if incoming && !note.read {
                    forja_ui_kit::icon(ui, forja_ui_kit::Icon::Info, 16.0);
                }
                forja_ui_kit::text(ui, egui::RichText::new(sender).strong());
                if let Some(title) = task_title {
                    forja_ui_kit::text(ui, egui::RichText::new(title).small().weak());
                }
                forja_ui_kit::text(ui, 
                    egui::RichText::new(note_kind_label(cx, note.kind))
                        .small()
                        .weak(),
                );
                if let Some(state) = delivery_state {
                    let (text, color) = match state {
                        NoteDeliveryState::Delivered => (
                            cx.t("muster.note-state.delivered"),
                            tokens.color.ok.to_color32(),
                        ),
                        NoteDeliveryState::Queued => (
                            cx.t("muster.note-state.queued"),
                            tokens.color.spark.to_color32(),
                        ),
                        NoteDeliveryState::NoHelper => (
                            cx.t("muster.note-state.no-helper"),
                            tokens.color.danger.to_color32(),
                        ),
                    };
                    forja_ui_kit::text(ui, egui::RichText::new(text).small().color(color));
                }
            });
            forja_ui_kit::text(ui, &note.body);
            if incoming && button(ui, ButtonKind::Ghost, &cx.t("muster.reply")).clicked() {
                self.reply_to = Some(note.id.clone());
            }
        });
        if incoming && self.reply_to.as_deref() == Some(note.id.as_str()) {
            text_field(ui, &mut self.reply_body, &cx.t("muster.reply-hint"));
            if button(ui, ButtonKind::Primary, &cx.t("muster.send")).clicked() {
                let mut args = Args::new();
                args.insert("note", json!(note.id));
                args.insert("body", json!(self.reply_body));
                match cx.run_command("muster.reply", args) {
                    Ok(_) => {
                        self.notice = None;
                        self.reply_to = None;
                        self.reply_body.clear();
                    }
                    Err(error) => self.set_error(cx, &error.to_string()),
                }
            }
        }
    }

    fn note_targets(
        &self,
        cx: &Ctx<'_>,
        task_titles: &BTreeMap<String, String>,
        assignments: &[Assignment],
        tasks: &[Task],
    ) -> Vec<NoteTarget> {
        let mut targets = Vec::new();
        let mut covered_tasks = std::collections::BTreeSet::new();
        for assignment in assignments {
            covered_tasks.insert(assignment.task.clone());
            let task_title = task_titles
                .get(&assignment.task)
                .map(String::as_str)
                .unwrap_or(&assignment.task);
            let helper_name = self.helper_display_name(cx, &assignment.profile_id);
            let state_text = if (assignment.terminal_id.is_none()
                && assignment.session_id.is_none())
                || matches!(assignment.state, AssignmentState::Stopped)
            {
                cx.t("muster.note-target-no-helper")
            } else {
                assignment_state_label(cx, assignment.state)
            };
            targets.push(NoteTarget {
                id: assignment.id.clone(),
                is_assignment: true,
                label: format!("{task_title} ({helper_name} · {state_text})"),
            });
        }
        for task in tasks {
            if !covered_tasks.contains(&task.id) {
                targets.push(NoteTarget {
                    id: task.id.clone(),
                    is_assignment: false,
                    label: format!("{} ({})", task.title, cx.t("muster.note-target-no-helper")),
                });
            }
        }
        targets
    }

    fn submit_note(&mut self, cx: &mut Ctx<'_>, target: Option<&NoteTarget>) {
        let Some(target) = target else {
            self.notice = Some(cx.t("muster.error.choose-assignment"));
            return;
        };
        if self.note_body.trim().is_empty() {
            self.notice = Some(cx.t("muster.error.empty-note"));
            return;
        }
        let mut args = Args::new();
        if target.is_assignment {
            args.insert("assignment", json!(target.id));
        } else {
            args.insert("task", json!(target.id));
        }
        args.insert("body", json!(self.note_body));
        args.insert("from", json!("coordinator"));
        match cx.run_command("muster.note", args) {
            Ok(_) => {
                self.notice = None;
                self.note_body.clear();
            }
            Err(error) => self.set_error(cx, &error.to_string()),
        }
    }

    fn helper_display_name(&self, cx: &Ctx<'_>, profile: &str) -> String {
        let profile = profile.trim();
        if profile.is_empty() {
            return cx.t("muster.plan-helper-auto");
        }
        self.helper_options
            .iter()
            .find(|option| option.id == profile)
            .map(|option| option.name.clone())
            .unwrap_or_else(|| profile.to_owned())
    }

    fn helper_label(&self, cx: &Ctx<'_>, profile: &str) -> String {
        let profile = profile.trim();
        if profile.is_empty() {
            return cx.t("muster.plan-helper-auto");
        }
        self.helper_options
            .iter()
            .find(|option| option.id == profile)
            .map(|option| {
                if option.installed {
                    option.name.clone()
                } else {
                    format!("{} ({})", option.name, cx.t("muster.profile-not-installed"))
                }
            })
            .unwrap_or_else(|| profile.to_owned())
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
        let column_height = (ui.available_height() - 200.0).clamp(240.0, 560.0);
        let mut page = self.header(ui, cx);
        if !snap.runs.is_empty() {
            page.filters(|ui| self.run_list(ui, cx, &snap));
        }
        page.body(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("muster.page.scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(ui.available_width().max(1.0));
                    self.notices(ui, cx, &snap);
                    self.new_run_form(ui, cx);
                    if snap.runs.is_empty() {
                        self.run_list(ui, cx, &snap);
                    }
                    let selected = snap
                        .runs
                        .iter()
                        .find(|run| self.selected.as_deref() == Some(run.id.as_str()));
                    if let Some(run) = selected {
                        self.run_heading(ui, cx, run);
                        self.sync_helpers(cx);
                        self.sync_spend(cx, &snap, run);
                        ui.add_space(4.0);
                        ui.columns(3, |columns| {
                            self.plan_column(&mut columns[0], cx, &snap, run, column_height);
                            self.assignment_column(&mut columns[1], cx, &snap, run, column_height);
                            self.inbox_column(&mut columns[2], cx, &snap, run, column_height);
                        });
                        self.plan_review(ui, cx, &snap, run);
                    }
                });
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

fn plan_state(ui: &egui::Ui, cx: &Ctx<'_>, run: &MusterRun) -> (String, egui::Color32) {
    let tokens = applied_tokens(ui.ctx());
    if run.plan_approved {
        (cx.t("muster.plan-approved"), tokens.color.ok.to_color32())
    } else {
        (cx.t("muster.plan-pending"), tokens.color.spark.to_color32())
    }
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
        AssignmentState::Starting | AssignmentState::Idle => tokens.color.text_muted.to_color32(),
        AssignmentState::Running => tokens.color.info.to_color32(),
        AssignmentState::NeedsYou => tokens.color.spark.to_color32(),
        AssignmentState::Delivered => tokens.color.ok.to_color32(),
        AssignmentState::Failed | AssignmentState::Blocked => tokens.color.danger.to_color32(),
        AssignmentState::Stopped => tokens.color.text_muted.to_color32(),
    }
}

fn cost_value(cx: &Ctx<'_>, usd: f64) -> String {
    let amount = match cx.i18n().locale() {
        Locale::PtBr => format!("{usd:.2}").replace('.', ","),
        Locale::En => format!("{usd:.2}"),
    };
    cx.t_args("muster.cost-value", &[("amount", &amount)])
}

fn budget_label(cx: &Ctx<'_>, usd: f64) -> String {
    if usd > 0.0 {
        cost_value(cx, usd)
    } else {
        cx.t("muster.budget-none")
    }
}

fn all_benches(run: &MusterRun, assignments: &[Assignment]) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(bench) = &run.bench_path {
        paths.push(bench.clone());
    }
    for assignment in assignments
        .iter()
        .filter(|assignment| assignment.run == run.id)
    {
        paths.push(assignment.bench_path.clone());
    }
    cost::distinct_benches(paths)
}

fn run_benches(run: &MusterRun, assignments: &[Assignment]) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(bench) = &run.bench_path {
        paths.push(bench.clone());
    }
    for assignment in assignments
        .iter()
        .filter(|assignment| assignment.run == run.id)
    {
        paths.push(assignment.bench_path.clone());
    }
    cost::unique_benches(paths)
}

/// Quantas atribuições usam cada bancada, para avisar quando o custo é da
/// bancada inteira e não só daquela tarefa.
fn shared_benches(run: &MusterRun, assignments: &[Assignment]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for assignment in assignments
        .iter()
        .filter(|assignment| assignment.run == run.id)
    {
        *counts
            .entry(cost::normalize(&assignment.bench_path))
            .or_insert(0) += 1;
    }
    counts
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

impl MusterView {
    fn set_error(&mut self, cx: &Ctx<'_>, raw: &str) {
        self.notice = Some(humanize_error(cx, raw));
    }
}

fn strip_command_wrappers(raw: &str) -> String {
    let mut text = raw.trim().to_owned();
    loop {
        let mut changed = false;
        if let Some(rest) = text.strip_prefix("command `") {
            if let Some(pos) = rest.find("` failed: ") {
                text = rest[pos + "` failed: ".len()..].trim().to_owned();
                changed = true;
            }
        }
        if let Some(idx) = text.find(": command `") {
            let after = &text[idx + ": command `".len()..];
            if let Some(pos) = after.find("` failed: ") {
                let prefix = &text[..idx];
                let inner = after[pos + "` failed: ".len()..].trim();
                text = if prefix.trim().is_empty() {
                    inner.to_owned()
                } else {
                    format!("{prefix}: {inner}")
                };
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    text
}

pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> String {
    let stripped = strip_command_wrappers(raw);
    let trimmed = stripped.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("disabled") || lower.contains("desativad") {
        return cx.t("muster.error.disabled");
    }
    if lower.contains("unknown command")
        || lower.contains("unavailable")
        || lower.contains("indispon")
    {
        return cx.t_args("muster.error.unavailable", &[("detail", trimmed)]);
    }
    if lower.contains("not found")
        || lower.contains("nao encontrado")
        || lower.contains("não encontrado")
    {
        return cx.t_args("muster.error.not-found", &[("detail", trimmed)]);
    }
    if lower.contains("ajudante")
        || lower.contains("plano")
        || lower.contains("teto")
        || lower.contains("bancada")
    {
        return trimmed.to_owned();
    }
    cx.t_args("muster.error.command", &[("detail", trimmed)])
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
