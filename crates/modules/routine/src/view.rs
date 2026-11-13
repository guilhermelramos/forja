//! A página "Rotinas" (aba da bancada) e o passo que mantém o agendador andando na barra de status.
//!
//! A tela copia o que precisa do estado e solta o lock antes de chamar qualquer comando.
//! O passo da barra de status roda mesmo com a aba fechada, porque é ele que dispara as rotinas.

use crate::actions;
use crate::commands;
use crate::draft::{ActionKind, Draft, PresetKind, ScheduleKind, TargetKind};
use crate::model::{Failure, Routine, Run, RunStatus, Skip, Trigger};
use crate::state::{self, with, Shared};
use chrono::{Local, TimeZone, Utc};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use forja_ui_kit::{button, segmented, text_field, ButtonKind};
use std::time::Duration;

/// Execuções mostradas no histórico da página.
const HISTORY_SHOWN: usize = 30;
/// Próximos disparos mostrados no editor.
const PREVIEW_COUNT: usize = 3;

/// Retrato do estado para uma pintura. Não guarda lock.
struct Snapshot {
    loaded: bool,
    routines: Vec<Routine>,
    runs: Vec<Run>,
    running: usize,
    queued: usize,
    notice: Option<String>,
}

fn snapshot(shared: &Shared) -> Snapshot {
    with(shared, |state| Snapshot {
        loaded: state.loaded,
        routines: state.store.routines.clone(),
        runs: state.store.runs.iter().rev().take(HISTORY_SHOWN).cloned().collect(),
        running: state.runner.running(),
        queued: state.runner.queued(),
        notice: state.notice.clone(),
    })
    .unwrap_or(Snapshot {
        loaded: false,
        routines: Vec::new(),
        runs: Vec::new(),
        running: 0,
        queued: 0,
        notice: None,
    })
}

pub struct RoutinesView {
    shared: Shared,
    draft: Option<Draft>,
    draft_error: Option<String>,
    selected: Option<String>,
    /// Em modo fixture (captura), abre o editor da primeira rotina uma vez para mostrar a prévia.
    auto_opened: bool,
}

impl RoutinesView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            draft: None,
            draft_error: None,
            selected: None,
            auto_opened: false,
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        let title = cx.t("routine-tab-title");
        let running = snap.running + snap.queued;
        let subtitle = (running > 0).then(|| {
            cx.t_args("routine-tick-running", &[("count", &running.to_string())])
        });
        forja_ui_kit::page_heading(ui, &title, subtitle.as_deref(), |ui| {
            if button(ui, ButtonKind::Primary, &cx.t("routine-new")).clicked() {
                let target = target_default(cx);
                self.draft = Some(Draft::new(target));
                self.draft_error = None;
            }
        });
        if let Some(notice) = &snap.notice {
            ui.colored_label(egui::Color32::from_rgb(0xFF, 0xC1, 0x4D), cx.t(notice));
        }
        if !snap.loaded {
            ui.label(cx.t("routine-loading"));
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        if snap.routines.is_empty() && snap.loaded {
            // A ação principal, "Nova rotina", fica logo acima, na barra.
            forja_ui_kit::empty_state(ui, forja_ui_kit::ROUTINES, &cx.t("routine-empty"), None);
            return;
        }
        let utc = state::schedule_in_utc(cx);
        let now = state::now_unix();
        for routine in &snap.routines {
            let selected = self.selected.as_deref() == Some(routine.id.as_str());
            ui.horizontal(|ui| {
                if ui.selectable_label(selected, routine.name.as_str()).clicked() {
                    self.selected = Some(routine.id.clone());
                }
                let next = commands::next_fire(&routine.schedule, utc, now)
                    .map(|at| format_time(at, utc))
                    .unwrap_or_else(|| cx.t("routine-no-next"));
                let last = snap
                    .runs
                    .iter()
                    .find(|run| run.routine_id == routine.id)
                    .map(|run| cx.t(state::status_key(&run.status)))
                    .unwrap_or_else(|| cx.t("routine-never-ran"));
                let mut line = format!("{}  ·  {}  ·  {}", routine.schedule.expression(), next, last);
                if !routine.enabled {
                    line.push_str("  ·  ");
                    line.push_str(&cx.t("routine-disabled"));
                }
                ui.label(
                    egui::RichText::new(line)
                        .small()
                        .weak(),
                );
            });
            ui.horizontal(|ui| {
                if button(ui, ButtonKind::Ghost, &cx.t("routine-edit")).clicked() {
                    self.draft = Some(Draft::from_routine(routine));
                    self.draft_error = None;
                }
                if button(ui, ButtonKind::Secondary, &cx.t("routine-run-now")).clicked() {
                    let _ = commands::run_now(cx, &self.shared, &routine.id);
                }
                if button(ui, ButtonKind::Danger, &cx.t("routine-delete")).clicked() {
                    let _ = commands::delete(cx, &self.shared, &routine.id);
                    if self.selected.as_deref() == Some(routine.id.as_str()) {
                        self.selected = None;
                    }
                }
            });
            ui.add_space(4.0);
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let Some(mut draft) = self.draft.take() else {
            return;
        };
        ui.separator();
        ui.label(egui::RichText::new(cx.t("routine-editor-title")).strong());
        text_field(ui, &mut draft.name, &cx.t("routine-field-name"));
        forja_ui_kit::toggle(ui, &mut draft.enabled, &cx.t("routine-field-enabled"));

        let kinds = [cx.t("routine-kind-preset"), cx.t("routine-kind-cron")];
        let mut kind = usize::from(draft.schedule_kind == ScheduleKind::Cron);
        if segmented(ui, &mut kind, &[kinds[0].as_str(), kinds[1].as_str()]).changed() {
            draft.schedule_kind = if kind == 1 { ScheduleKind::Cron } else { ScheduleKind::Preset };
        }
        match draft.schedule_kind {
            ScheduleKind::Cron => {
                text_field(ui, &mut draft.cron, &cx.t("routine-field-cron"));
            }
            ScheduleKind::Preset => {
                let labels = [
                    cx.t("routine-preset-hourly"),
                    cx.t("routine-preset-daily"),
                    cx.t("routine-preset-weekdays"),
                    cx.t("routine-preset-weekly"),
                ];
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                let mut preset = match draft.preset {
                    PresetKind::Hourly => 0,
                    PresetKind::Daily => 1,
                    PresetKind::Weekdays => 2,
                    PresetKind::Weekly => 3,
                };
                if segmented(ui, &mut preset, &refs).changed() {
                    draft.preset = [PresetKind::Hourly, PresetKind::Daily, PresetKind::Weekdays, PresetKind::Weekly][preset];
                }
                if draft.preset != PresetKind::Hourly {
                    text_field(ui, &mut draft.time, &cx.t("routine-field-time"));
                }
                if draft.preset == PresetKind::Weekly {
                    let days: Vec<String> = (0..7).map(|day| cx.t(WEEKDAY_KEYS[day])).collect();
                    let refs: Vec<&str> = days.iter().map(String::as_str).collect();
                    segmented(ui, &mut draft.weekday, &refs);
                }
            }
        }

        let targets = [cx.t("routine-target-ask"), cx.t("routine-target-fixed"), cx.t("routine-target-new")];
        let mut target = match draft.target {
            TargetKind::Ask => 0,
            TargetKind::Fixed => 1,
            TargetKind::New => 2,
        };
        if segmented(ui, &mut target, &[targets[0].as_str(), targets[1].as_str(), targets[2].as_str()]).changed() {
            draft.target = [TargetKind::Ask, TargetKind::Fixed, TargetKind::New][target];
        }
        match draft.target {
            TargetKind::Ask => {}
            TargetKind::Fixed => {
                text_field(ui, &mut draft.bench_path, &cx.t("routine-field-bench_path"));
            }
            TargetKind::New => {
                text_field(ui, &mut draft.project_id, &cx.t("routine-field-project_id"));
                text_field(ui, &mut draft.project_path, &cx.t("routine-field-project_path"));
                text_field(ui, &mut draft.name_template, &cx.t("routine-field-name_template"));
            }
        }

        let actions = [cx.t("routine-action-command"), cx.t("routine-action-prompt")];
        let mut action = usize::from(draft.action == ActionKind::Prompt);
        if segmented(ui, &mut action, &[actions[0].as_str(), actions[1].as_str()]).changed() {
            draft.action = if action == 1 { ActionKind::Prompt } else { ActionKind::Command };
        }
        match draft.action {
            ActionKind::Command => {
                text_field(ui, &mut draft.command, &cx.t("routine-field-command"));
            }
            ActionKind::Prompt => {
                text_field(ui, &mut draft.profile, &cx.t("routine-field-profile"));
                ui.add(
                    egui::TextEdit::multiline(&mut draft.prompt)
                        .hint_text(cx.t("routine-field-prompt"))
                        .desired_width(f32::INFINITY)
                        .desired_rows(3),
                );
            }
        }
        text_field(ui, &mut draft.precheck, &cx.t("routine-field-precheck"));
        text_field(ui, &mut draft.tolerance, &cx.t("routine-field-tolerance"));

        self.preview(ui, cx, &draft);

        if let Some(error) = &self.draft_error {
            ui.colored_label(egui::Color32::from_rgb(0xE5, 0x53, 0x4B), cx.t(error));
        }
        let mut keep = true;
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Primary, &cx.t("routine-save")).clicked() {
                match draft.to_routine(0, None) {
                    Err(key) => self.draft_error = Some(format!("routine-error-{key}")),
                    Ok(routine) => match serde_json::to_value(&routine) {
                        Err(_) => self.draft_error = Some("routine-error-invalid".to_owned()),
                        Ok(value) => {
                            let mut args = Args::new();
                            args.insert("routine", value);
                            match cx.run_command("routine.save", args) {
                                Ok(result) if result["saved"] == true => {
                                    self.draft_error = None;
                                    keep = false;
                                }
                                Ok(result) => {
                                    self.draft_error = result["error"].as_str().map(str::to_owned);
                                }
                                Err(_) => self.draft_error = Some("routine-error-invalid".to_owned()),
                            }
                        }
                    },
                }
            }
            if button(ui, ButtonKind::Ghost, &cx.t("routine-cancel")).clicked() {
                keep = false;
                self.draft_error = None;
            }
        });
        if keep {
            self.draft = Some(draft);
        }
    }

    /// Os próximos disparos com a agenda como está digitada.
    fn preview(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, draft: &Draft) {
        let utc = state::schedule_in_utc(cx);
        let text = match draft.schedule() {
            Err(_) => cx.t("routine-error-time"),
            Ok(schedule) => {
                let runs = if utc {
                    schedule.next_runs(&Utc::now(), PREVIEW_COUNT)
                        .map(|runs| runs.into_iter().map(|at| format_time(at.timestamp(), true)).collect::<Vec<_>>())
                } else {
                    schedule.next_runs(&Local::now(), PREVIEW_COUNT)
                        .map(|runs| runs.into_iter().map(|at| format_time(at.timestamp(), false)).collect::<Vec<_>>())
                };
                match runs {
                    Err(message) => cx.t_args("routine-error-schedule-detail", &[("message", message.as_str())]),
                    Ok(list) if list.is_empty() => cx.t("routine-no-next"),
                    Ok(list) => list.join("  ·  "),
                }
            }
        };
        ui.label(egui::RichText::new(format!("{}: {text}", cx.t("routine-next-runs"))).small().weak());
    }

    fn history(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snapshot) {
        ui.separator();
        ui.label(egui::RichText::new(cx.t("routine-history")).strong());
        let utc = state::schedule_in_utc(cx);
        let runs: Vec<&Run> = snap
            .runs
            .iter()
            .filter(|run| self.selected.as_deref().is_none_or(|id| run.routine_id == id))
            .collect();
        if runs.is_empty() {
            forja_ui_kit::empty_state(ui, forja_ui_kit::ROUTINES, &cx.t("routine-history-empty"), None);
            return;
        }
        egui::ScrollArea::vertical().id_salt("routine.history").max_height(260.0).show(ui, |ui| {
            for run in runs {
                let when = format_time(run.started, utc);
                let trigger = cx.t(trigger_key(run.trigger));
                let status = cx.t(state::status_key(&run.status));
                let reason = reason_text(cx, &run.status).map(|text| format!(" · {text}")).unwrap_or_default();
                ui.label(format!("{when}  {}  ·  {trigger}  ·  {status}{reason}", run.routine_name));
                if !run.summary.is_empty() {
                    // Saída de terminal é dado: aparece como texto, sem interpretação.
                    ui.label(egui::RichText::new(run.summary.as_str()).small().monospace().weak());
                }
                ui.add_space(4.0);
            }
        });
    }
}

impl View for RoutinesView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        actions::ensure_loaded(cx, &self.shared);
        ui.ctx().request_repaint_after(Duration::from_secs(1));
        let snap = snapshot(&self.shared);
        if !self.auto_opened && state::fixtures_enabled() && self.draft.is_none() {
            if let Some(first) = snap.routines.first() {
                self.auto_opened = true;
                self.selected = Some(first.id.clone());
                self.draft = Some(Draft::from_routine(first));
            }
        }
        self.header(ui, cx, &snap);
        egui::ScrollArea::vertical().id_salt("routine.page").show(ui, |ui| {
            self.list(ui, cx, &snap);
            self.editor(ui, cx);
            self.history(ui, cx, &snap);
        });
    }
}

/// Passo da barra de status: avança o agendador a cada segundo e mostra quantas rodam.
pub struct TickView {
    pub shared: Shared,
}

impl View for TickView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        actions::tick(cx, &self.shared);
        ui.ctx().request_repaint_after(Duration::from_secs(1));
        let busy = with(&self.shared, |state| state.runner.running() + state.runner.queued()).unwrap_or(0);
        if busy > 0 {
            ui.label(cx.t_args("routine-tick-running", &[("count", &busy.to_string())]));
        }
    }
}

fn target_default(cx: &Ctx<'_>) -> TargetKind {
    match cx.setting("routine.run.target").and_then(SettingValue::as_str) {
        Some("fixed") => TargetKind::Fixed,
        Some("new") => TargetKind::New,
        _ => TargetKind::Ask,
    }
}

const WEEKDAY_KEYS: [&str; 7] = [
    "routine-weekday-sun",
    "routine-weekday-mon",
    "routine-weekday-tue",
    "routine-weekday-wed",
    "routine-weekday-thu",
    "routine-weekday-fri",
    "routine-weekday-sat",
];

fn trigger_key(trigger: Trigger) -> &'static str {
    match trigger {
        Trigger::Schedule => "routine-trigger-schedule",
        Trigger::CatchUp => "routine-trigger-catch_up",
        Trigger::Manual => "routine-trigger-manual",
    }
}

/// Motivo curto de uma falha ou de um pulo, para a linha do histórico.
fn reason_text(cx: &Ctx<'_>, status: &RunStatus) -> Option<String> {
    match status {
        RunStatus::Failed { reason } => Some(match reason {
            Failure::Timeout => cx.t("routine-reason-timeout"),
            Failure::CommandExit { code } => cx.t_args("routine-reason-exit", &[("code", &code.to_string())]),
            Failure::TerminalUnavailable => cx.t("routine-reason-terminal_unavailable"),
            Failure::HelperUnavailable => cx.t("routine-reason-helper_unavailable"),
            Failure::BenchUnavailable => cx.t("routine-reason-bench_unavailable"),
            Failure::TerminalClosed => cx.t("routine-reason-terminal_closed"),
            Failure::Interrupted => cx.t("routine-reason-interrupted"),
            Failure::Detail { message } => cx.t_args("routine-reason-detail", &[("message", message.as_str())]),
        }),
        RunStatus::Skipped { reason } => Some(match reason {
            Skip::PrecheckFailed { code } => cx.t_args("routine-reason-precheck", &[("code", &code.to_string())]),
            Skip::NeedsTarget => cx.t("routine-reason-needs_target"),
            Skip::MissedSkipped => cx.t("routine-reason-missed_skipped"),
        }),
        _ => None,
    }
}

/// Data e hora curtas, no fuso escolhido.
fn format_time(unix: i64, utc: bool) -> String {
    if utc {
        Utc.timestamp_opt(unix, 0)
            .single()
            .map(|at| at.format("%d/%m %H:%M UTC").to_string())
            .unwrap_or_default()
    } else {
        Local
            .timestamp_opt(unix, 0)
            .single()
            .map(|at| at.format("%d/%m %H:%M").to_string())
            .unwrap_or_default()
    }
}

/// Chaves de texto da página e da barra de status, para o teste de contrato.
pub const USED_KEYS: &[&str] = &[
    "routine-tab-title",
    "routine-tick-running",
    "routine-new",
    "routine-loading",
    "routine-empty",
    "routine-no-next",
    "routine-never-ran",
    "routine-disabled",
    "routine-edit",
    "routine-run-now",
    "routine-delete",
    "routine-editor-title",
    "routine-field-name",
    "routine-field-enabled",
    "routine-kind-preset",
    "routine-kind-cron",
    "routine-field-cron",
    "routine-preset-hourly",
    "routine-preset-daily",
    "routine-preset-weekdays",
    "routine-preset-weekly",
    "routine-field-time",
    "routine-weekday-sun",
    "routine-weekday-mon",
    "routine-weekday-tue",
    "routine-weekday-wed",
    "routine-weekday-thu",
    "routine-weekday-fri",
    "routine-weekday-sat",
    "routine-target-ask",
    "routine-target-fixed",
    "routine-target-new",
    "routine-field-bench_path",
    "routine-field-project_id",
    "routine-field-project_path",
    "routine-field-name_template",
    "routine-action-command",
    "routine-action-prompt",
    "routine-field-command",
    "routine-field-profile",
    "routine-field-prompt",
    "routine-field-precheck",
    "routine-field-tolerance",
    "routine-next-runs",
    "routine-error-schedule-detail",
    "routine-save",
    "routine-cancel",
    "routine-history",
    "routine-history-empty",
    "routine-trigger-schedule",
    "routine-trigger-catch_up",
    "routine-trigger-manual",
    "routine-status-queued",
    "routine-status-running",
    "routine-status-succeeded",
    "routine-status-failed",
    "routine-status-skipped",
    "routine-status-missed",
    "routine-reason-timeout",
    "routine-reason-exit",
    "routine-reason-terminal_unavailable",
    "routine-reason-helper_unavailable",
    "routine-reason-bench_unavailable",
    "routine-reason-terminal_closed",
    "routine-reason-interrupted",
    "routine-reason-detail",
    "routine-reason-precheck",
    "routine-reason-needs_target",
    "routine-reason-missed_skipped",
];
