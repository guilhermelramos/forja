//! Review tab: create a PR, watch checks, answer threads and merge.

mod sections;

use crate::api::Provider;
use crate::parse::Check;
use crate::state::{Generated, Shared, State};
use chrono::TimeZone;
use forja_kernel::ui_contract::{View, egui};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::time::{Duration, Instant};

pub struct ReviewView {
    state: Shared,
    bootstrapped: bool,
    selected_check: Option<u64>,
    comparison_debounce: Option<Instant>,
    last_comparison_check: Option<Instant>,
}

#[derive(Clone, Default)]
struct Form {
    title: String,
    body: String,
    base: String,
    draft: bool,
    reviewers: String,
    labels: String,
}

impl ReviewView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            bootstrapped: false,
            selected_check: None,
            comparison_debounce: None,
            last_comparison_check: None,
        }
    }

    fn bootstrap(&mut self, cx: &mut Ctx<'_>) {
        if self.bootstrapped {
            return;
        }
        self.bootstrapped = true;
        let mut args = Args::new();
        if let Ok(bench) = std::env::var("FORJA_REVIEW_BENCH") {
            args.insert("bench_path", json!(bench));
        }
        let _ = cx.run_command("review.refresh", args);
    }
}

impl View for ReviewView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.bootstrap(cx);
        let snapshot = self.state.with(|state| state.clone()).unwrap_or_default();
        let now = Instant::now();
        if self
            .last_comparison_check
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(5))
        {
            self.last_comparison_check = Some(now);
            if let Some(bench) = &snapshot.bench {
                crate::commands::start_comparison(
                    cx,
                    &self.state,
                    bench,
                    (!snapshot.base.trim().is_empty()).then_some(snapshot.base.as_str()),
                );
            }
        }
        if self
            .comparison_debounce
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.comparison_debounce = None;
            if let Some(bench) = &snapshot.bench {
                crate::commands::start_comparison(
                    cx,
                    &self.state,
                    bench,
                    (!snapshot.base.trim().is_empty()).then_some(snapshot.base.as_str()),
                );
            }
        }
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        self.header(ui, cx, &snapshot, muted);
        if let Some(status) = &snapshot.status {
            let human = crate::commands::humanize_error(cx, status);
            forja_ui_kit::error_banner(
                ui,
                &human.summary,
                human.detail.as_deref(),
                &cx.t("kernel.error.details"),
            );
        }
        if let Some(notice) = &snapshot.notice {
            forja_ui_kit::text(ui, egui::RichText::new(notice).color(c32(tokens.color.ok)));
        }
        egui::ScrollArea::vertical()
            .id_salt("review-body")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(pr) = &snapshot.pr {
                    self.pr_card(ui, cx, &snapshot, pr, muted);
                    self.checks(ui, cx, &snapshot, muted);
                    self.threads(ui, cx, &snapshot, muted);
                    self.merge(ui, cx, &snapshot, muted);
                } else {
                    self.create_form(ui, cx, &snapshot, muted);
                }
                self.generated(ui, cx, &snapshot, muted);
            });
    }
}

impl ReviewView {
    fn summary(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        state: &State,
        base: &mut String,
        muted: egui::Color32,
    ) {
        ui.group(|ui| {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("review.summary.title")).strong());
            if state.comparison_loading {
                ui.horizontal(|ui| {
                    forja_ui_kit::spinner_if_active(ui);
                    forja_ui_kit::text(ui, cx.t("review.summary.loading"));
                });
            }
            if let Some(error) = &state.comparison_error {
                let human = crate::commands::humanize_error(cx, error);
                forja_ui_kit::error_banner(
                    ui,
                    &human.summary,
                    human.detail.as_deref(),
                    &cx.t("kernel.error.details"),
                );
            }
            let mut changed_base = false;
            if let Some(comparison) = &state.comparison {
                if !comparison.remote {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("review.summary.remote-missing")).color(muted),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    forja_ui_kit::text(ui, cx.t("review.summary.base"));
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(
                            comparison
                                .base
                                .clone()
                                .unwrap_or_else(|| cx.t("review.summary.choose-base-label")),
                        )
                        .strong(),
                    );
                    if let Some(commit) = &comparison.base_commit {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(commit.chars().take(12).collect::<String>())
                                .monospace()
                                .small()
                                .color(muted),
                        );
                    }
                    if !comparison.branches.is_empty() {
                        egui::ComboBox::from_id_salt("review-base-branch")
                            .selected_text(cx.t("review.summary.choose-base-label"))
                            .show_ui(ui, |ui| {
                                for branch in &comparison.branches {
                                    if ui.selectable_label(base == branch, branch).clicked() {
                                        *base = branch.clone();
                                        changed_base = true;
                                    }
                                }
                            });
                    }
                });
                if !comparison.head.is_empty() {
                    forja_ui_kit::text(ui, cx.t_args(
                        "review.summary.head",
                        &[(
                            "commit",
                            &comparison.head.chars().take(12).collect::<String>(),
                        )],
                    ));
                }
                if comparison.base.is_none() {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("review.summary.no-base"))
                            .small()
                            .color(muted),
                    );
                } else {
                    forja_ui_kit::text(ui, cx.t_args(
                        "review.summary.ahead",
                        &[("count", &comparison.ahead.to_string())],
                    ));
                }
                if comparison.ahead == 0 && comparison.base.is_some() {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("review.summary.nothing-to-send")).color(muted),
                    );
                }
                ui.add_space(forja_ui_kit::Spacing::XS);
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("review.summary.commits")).strong());
                if comparison.commits.is_empty() {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("review.summary.no-commits"))
                            .small()
                            .color(muted),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("review-summary-commits")
                        .max_height(180.0)
                        .show_rows(ui, 22.0, comparison.commits.len(), |ui, rows| {
                            for index in rows {
                                let commit = &comparison.commits[index];
                                ui.horizontal_wrapped(|ui| {
                                    forja_ui_kit::text(ui, egui::RichText::new(&commit.short).monospace());
                                    forja_ui_kit::text(ui, &commit.title);
                                    forja_ui_kit::text(ui, 
                                        egui::RichText::new(&commit.author).small().color(muted),
                                    );
                                    forja_ui_kit::text(ui, 
                                        egui::RichText::new(format_commit_date(commit.time, cx))
                                            .small()
                                            .color(muted),
                                    );
                                });
                            }
                        });
                }
                ui.add_space(forja_ui_kit::Spacing::XS);
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("review.summary.files")).strong());
                if comparison.files.is_empty() {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("review.summary.no-files"))
                            .small()
                            .color(muted),
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("review-summary-files")
                        .max_height(180.0)
                        .show_rows(ui, 22.0, comparison.files.len(), |ui, rows| {
                            for index in rows {
                                let file = &comparison.files[index];
                                let additions = file
                                    .additions
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|| cx.t("review.summary.binary"));
                                let deletions = file
                                    .deletions
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|| cx.t("review.summary.binary"));
                                ui.horizontal_wrapped(|ui| {
                                    forja_ui_kit::text(ui, 
                                        egui::RichText::new(format!("+{additions} / -{deletions}"))
                                            .monospace()
                                            .small()
                                            .color(muted),
                                    );
                                    forja_ui_kit::text(ui, egui::RichText::new(&file.path).monospace());
                                });
                            }
                        });
                }
                forja_ui_kit::text(ui, cx.t_args(
                    "review.summary.total",
                    &[
                        ("files", &comparison.files.len().to_string()),
                        ("additions", &comparison.additions.to_string()),
                        ("deletions", &comparison.deletions.to_string()),
                    ],
                ));
                if let Some(base) = comparison
                    .base
                    .as_ref()
                    .filter(|branch| branch.as_str() == base.trim())
                    .filter(|_| !state.comparison_loading)
                {
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Secondary,
                        &cx.t("review.summary.view-diff"),
                    )
                    .clicked()
                    {
                        if let Some(bench) = &state.bench {
                            let mut args = Args::new();
                            args.insert("path", json!(bench));
                            args.insert("comparison", json!(format!("{base}...HEAD")));
                            if cx.run_command("diff.open", args).is_ok() {
                                let mut focus = Args::new();
                                focus.insert("view_id", json!("diff.tab"));
                                let _ = cx.run_command("shell.tab.open", focus);
                            }
                        }
                    }
                }
            } else if !state.comparison_loading {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t("review.summary.no-base"))
                        .small()
                        .color(muted),
                );
            }
            if changed_base {
                self.comparison_debounce = Some(Instant::now() + Duration::from_millis(250));
                forja_ui_kit::request_repaint_after_active(ui.ctx(), Duration::from_millis(260));
            }
        });
        ui.add_space(forja_ui_kit::Spacing::SM);
    }

    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &State, muted: egui::Color32) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(cx.t("review.tab.title"));
            if let Some(target) = &state.target {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t(target.provider.label_key()))
                        .small()
                        .color(muted),
                );
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!("{} · {}", target.repo, state.branch))
                        .small()
                        .color(muted),
                );
            }
            if state.loading {
                forja_ui_kit::spinner_if_active(ui);
            }
        });
        ui.separator();
    }

    fn create_form(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        state: &State,
        muted: egui::Color32,
    ) {
        let mut form = Form {
            title: state.title.clone(),
            body: state.body.clone(),
            base: state.base.clone(),
            draft: state.draft,
            reviewers: state.reviewers.clone(),
            labels: state.labels.clone(),
        };
        ui.add_space(forja_ui_kit::Spacing::SM);
        let previous_base = form.base.clone();
        self.summary(ui, cx, state, &mut form.base, muted);
        if form.base != previous_base {
            self.comparison_debounce = Some(Instant::now() + Duration::from_millis(250));
            forja_ui_kit::request_repaint_after_active(ui.ctx(), Duration::from_millis(260));
        }
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("review.create.title")).strong());
        forja_ui_kit::text(ui, 
            egui::RichText::new(cx.t("review.create.source-label"))
                .small()
                .color(muted),
        );
        let mut source = state.branch.clone();
        ui.add_enabled(
            false,
            egui::TextEdit::singleline(&mut source).desired_width(f32::INFINITY),
        );
        forja_ui_kit::text(ui, 
            egui::RichText::new(cx.t("review-pr-title"))
                .small()
                .color(muted),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.title)
                .hint_text(cx.t("review.pr.title"))
                .desired_width(f32::INFINITY),
        );
        forja_ui_kit::text(ui, 
            egui::RichText::new(cx.t("review.create.body"))
                .small()
                .color(muted),
        );
        ui.add(
            egui::TextEdit::multiline(&mut form.body)
                .hint_text(cx.t("review.create.body"))
                .desired_rows(8)
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            forja_ui_kit::text(ui, cx.t("review.create.base"));
            let response = ui.add(egui::TextEdit::singleline(&mut form.base).desired_width(140.0));
            if response.changed() {
                self.comparison_debounce = Some(Instant::now() + Duration::from_millis(250));
                forja_ui_kit::request_repaint_after_active(ui.ctx(), Duration::from_millis(260));
            }
            ui.checkbox(&mut form.draft, cx.t("review.create.draft"));
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("review.create.generate"),
            )
            .clicked()
            {
                let _ = cx.run_command("review.generate", args_kind("pr"));
            }
        });
        ui.horizontal(|ui| {
            forja_ui_kit::text(ui, cx.t("review.create.reviewers"));
            ui.add(egui::TextEdit::singleline(&mut form.reviewers).desired_width(220.0));
            forja_ui_kit::text(ui, cx.t("review.create.labels"));
            ui.add(egui::TextEdit::singleline(&mut form.labels).desired_width(180.0));
        });
        self.state.with(|state| {
            state.title = form.title.clone();
            state.body = form.body.clone();
            state.base = form.base.clone();
            state.draft = form.draft;
            state.reviewers = form.reviewers.clone();
            state.labels = form.labels.clone();
        });
        let disabled_reason = create_disabled_reason(state, cx);
        let can_open = disabled_reason.is_none();
        let response = ui.add_enabled_ui(can_open, |ui| {
            forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Primary,
                &cx.t("review.create.open"),
            )
        });
        if let Some(reason) = disabled_reason {
            forja_ui_kit::text(ui, egui::RichText::new(reason).small().color(muted));
        }
        if response.inner.clicked() {
            if form.title.trim().is_empty() {
                self.state
                    .set_status(Some(cx.t("review.error.title-required")));
            } else {
                let mut args = Args::new();
                args.insert("title", json!(form.title));
                args.insert("body", json!(form.body));
                args.insert("base", json!(form.base));
                args.insert("draft", json!(form.draft));
                args.insert("reviewers", json!(form.reviewers));
                args.insert("labels", json!(form.labels));
                if let Err(error) = cx.run_command("review.pr.create", args) {
                    let message = match error {
                        forja_kernel::KernelError::Command { message, .. } => message,
                        other => other.to_string(),
                    };
                    self.state.set_status(Some(message));
                }
            }
        }
    }
}

fn method_name(index: usize) -> &'static str {
    match index {
        1 => "squash",
        2 => "rebase",
        _ => "merge",
    }
}

fn args_kind(kind: &str) -> Args {
    let mut args = Args::new();
    args.insert("kind", json!(kind));
    args
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}

fn create_disabled_reason(state: &State, cx: &Ctx<'_>) -> Option<String> {
    if state.comparison_loading {
        return Some(cx.t("review.summary.loading"));
    }
    let Some(comparison) = &state.comparison else {
        return Some(cx.t("review.summary.choose-base"));
    };
    if comparison.base.is_none() || comparison.base.as_deref() != Some(state.base.trim()) {
        return Some(cx.t("review.summary.choose-base"));
    }
    if comparison.ahead == 0 {
        return Some(cx.t("review.summary.nothing-to-send"));
    }
    if !comparison.remote {
        return Some(cx.t("review.summary.remote-missing"));
    }
    if state.target.is_none() {
        return Some(cx.t("review.summary.provider-unavailable"));
    }
    None
}

fn format_commit_date(timestamp: i64, cx: &Ctx<'_>) -> String {
    let Some(date) = chrono::Local.timestamp_opt(timestamp, 0).single() else {
        return timestamp.to_string();
    };
    let format = match cx.i18n().locale() {
        forja_kernel::i18n::Locale::PtBr => "%d/%m/%Y %H:%M",
        forja_kernel::i18n::Locale::En => "%b %e, %Y %H:%M",
    };
    date.format(format).to_string()
}

#[allow(dead_code)]
fn check_name(check: &Check) -> &str {
    &check.name
}

#[allow(dead_code)]
fn provider_name(provider: Provider) -> &'static str {
    provider.id()
}
