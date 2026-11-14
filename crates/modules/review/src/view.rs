//! Review tab: create a PR, watch checks, answer threads and merge.

use crate::api::Provider;
use crate::parse::Check;
use crate::state::{Generated, Shared, State};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;

pub struct ReviewView {
    state: Shared,
    bootstrapped: bool,
    selected_check: Option<u64>,
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
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        self.header(ui, cx, &snapshot, muted);
        if let Some(status) = &snapshot.status {
            ui.label(egui::RichText::new(status).color(c32(tokens.color.danger)));
        }
        if let Some(notice) = &snapshot.notice {
            ui.label(egui::RichText::new(notice).color(c32(tokens.color.ok)));
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
    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &State, muted: egui::Color32) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(cx.t("review.tab.title"));
            if let Some(target) = &state.target {
                ui.label(
                    egui::RichText::new(cx.t(target.provider.label_key()))
                        .small()
                        .color(muted),
                );
                ui.label(
                    egui::RichText::new(format!("{} · {}", target.repo, state.branch))
                        .small()
                        .color(muted),
                );
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("review.command.refresh"),
            )
            .clicked()
            {
                let _ = cx.run_command("review.refresh", Args::new());
            }
            if state.loading {
                ui.spinner();
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
        ui.add_space(8.0);
        ui.label(egui::RichText::new(cx.t("review.create.title")).strong());
        ui.label(
            egui::RichText::new(cx.t("review.create.source-label"))
                .small()
                .color(muted),
        );
        let mut source = state.branch.clone();
        ui.add_enabled(
            false,
            egui::TextEdit::singleline(&mut source).desired_width(f32::INFINITY),
        );
        ui.label(
            egui::RichText::new(cx.t("review-pr-title"))
                .small()
                .color(muted),
        );
        ui.add(
            egui::TextEdit::singleline(&mut form.title)
                .hint_text(cx.t("review.pr.title"))
                .desired_width(f32::INFINITY),
        );
        ui.label(
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
            ui.label(cx.t("review.create.base"));
            ui.add(egui::TextEdit::singleline(&mut form.base).desired_width(140.0));
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
            ui.label(cx.t("review.create.reviewers"));
            ui.add(egui::TextEdit::singleline(&mut form.reviewers).desired_width(220.0));
            ui.label(cx.t("review.create.labels"));
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
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Primary,
            &cx.t("review.create.open"),
        )
        .clicked()
        {
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
                    self.state.set_status(Some(error.to_string()));
                }
            }
        }
    }

    fn pr_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        state: &State,
        pr: &crate::parse::PullRequest,
        muted: egui::Color32,
    ) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        egui::Frame::new()
            .fill(c32(tokens.color.surface_alt))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!("#{} {}", pr.number, pr.title))
                            .strong()
                            .color(c32(tokens.color.text)),
                    );
                    let state_label = if pr.draft && pr.is_open() {
                        cx.t("review.pr.draft")
                    } else {
                        pr.display_state().to_owned()
                    };
                    ui.label(egui::RichText::new(state_label).small().color(muted));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("review.pr.open-browser"),
                        )
                        .clicked()
                            && !pr.url.is_empty()
                        {
                            let mut args = Args::new();
                            args.insert("url", json!(pr.url));
                            let _ = cx.run_command("browser.open", args);
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(format!("{} → {}", pr.source, pr.target))
                        .small()
                        .color(muted),
                );
                if !pr.reviewers.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            cx.t("review.pr.reviewers"),
                            pr.reviewers.join(", ")
                        ))
                        .small()
                        .color(muted),
                    );
                }
                if !pr.labels.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {}",
                            cx.t("review.pr.labels"),
                            pr.labels.join(", ")
                        ))
                        .small()
                        .color(muted),
                    );
                }
            });
        let _ = state;
        ui.add_space(6.0);
    }

    fn checks(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &State, muted: egui::Color32) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        ui.label(egui::RichText::new(cx.t("review.checks.title")).strong());
        if state.checks.is_empty() {
            ui.label(
                egui::RichText::new(cx.t("review.checks.empty"))
                    .small()
                    .color(muted),
            );
            return;
        }
        for check in &state.checks {
            let color = if check.failed() {
                tokens.color.danger
            } else if check.passed() {
                tokens.color.ok
            } else {
                tokens.color.spark
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(&check.name).monospace());
                ui.label(egui::RichText::new(&check.state).small().color(c32(color)));
                if ui
                    .selectable_label(
                        self.selected_check == Some(check.id),
                        cx.t("review.checks.log"),
                    )
                    .clicked()
                {
                    self.selected_check = Some(check.id);
                    if check.log.is_empty() {
                        let mut args = Args::new();
                        args.insert("id", json!(check.id));
                        let _ = cx.run_command("review.checks.log", args);
                    }
                }
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("review.checks.rerun"),
                )
                .clicked()
                {
                    let mut args = Args::new();
                    args.insert("id", json!(check.id));
                    let _ = cx.run_command("review.checks.rerun", args);
                }
                if check.failed()
                    && forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Secondary,
                        &cx.t("review.checks.fix"),
                    )
                    .clicked()
                {
                    let _ = cx.run_command("review.fix", Args::new());
                }
            });
            if self.selected_check == Some(check.id) {
                let log = state
                    .logs
                    .get(&check.id.to_string())
                    .cloned()
                    .unwrap_or_else(|| check.log.clone());
                egui::Frame::new()
                    .fill(c32(tokens.color.bg))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt(("review-log", check.id))
                            .max_height(220.0)
                            .show(ui, |ui| {
                                if log.is_empty() {
                                    ui.label(
                                        egui::RichText::new(cx.t("review.checks.pending"))
                                            .small()
                                            .color(muted),
                                    );
                                } else {
                                    ui.label(egui::RichText::new(log).monospace().small());
                                }
                            });
                    });
            }
        }
        ui.add_space(6.0);
    }

    fn threads(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        state: &State,
        muted: egui::Color32,
    ) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        ui.label(egui::RichText::new(cx.t("review.threads.title")).strong());
        if state.threads.is_empty() {
            ui.label(
                egui::RichText::new(cx.t("review.threads.empty"))
                    .small()
                    .color(muted),
            );
            return;
        }
        for thread in &state.threads {
            egui::Frame::new()
                .fill(c32(tokens.color.surface))
                .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}:{}", thread.path, thread.line))
                                .small()
                                .color(muted),
                        );
                        if thread.resolved {
                            ui.label(
                                egui::RichText::new(cx.t("review.threads.resolved"))
                                    .small()
                                    .color(c32(tokens.color.ok)),
                            );
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t(if thread.resolved {
                                "review.threads.unresolve"
                            } else {
                                "review.threads.resolve"
                            }),
                        )
                        .clicked()
                        {
                            let mut args = Args::new();
                            args.insert("thread_id", json!(thread.id));
                            args.insert("resolved", json!(!thread.resolved));
                            let _ = cx.run_command("review.thread.resolve", args);
                        }
                    });
                    for comment in &thread.comments {
                        ui.label(
                            egui::RichText::new(format!("{}: {}", comment.author, comment.body))
                                .small(),
                        );
                    }
                    let mut draft = self.state.reply_draft(&thread.id);
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut draft)
                            .hint_text(cx.t("review.threads.reply"))
                            .desired_width(f32::INFINITY),
                    );
                    if response.changed() {
                        self.state.set_reply_draft(&thread.id, draft.clone());
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Secondary,
                        &cx.t("review.threads.send"),
                    )
                    .clicked()
                    {
                        let mut args = Args::new();
                        args.insert("thread_id", json!(thread.id));
                        args.insert("body", json!(draft));
                        let _ = cx.run_command("review.thread.reply", args);
                    }
                });
            ui.add_space(4.0);
        }
    }

    fn merge(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &State, muted: egui::Color32) {
        let Some(pr) = &state.pr else {
            return;
        };
        ui.add_space(6.0);
        ui.label(egui::RichText::new(cx.t("review.merge.title")).strong());
        ui.horizontal_wrapped(|ui| {
            for (index, key) in [
                (0, "review.merge.merge"),
                (1, "review.merge.squash"),
                (2, "review.merge.rebase"),
            ] {
                if ui
                    .selectable_label(state.merge_method == index, cx.t(key))
                    .clicked()
                {
                    self.state.with(|state| state.merge_method = index);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut delete = state.delete_branch;
            let mut auto = state.auto_merge;
            if ui
                .checkbox(&mut delete, cx.t("review.merge.delete-branch"))
                .changed()
            {
                self.state.with(|state| state.delete_branch = delete);
            }
            if ui.checkbox(&mut auto, cx.t("review.merge.auto")).changed() {
                self.state.with(|state| state.auto_merge = auto);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Primary,
                &cx.t("review.merge.action"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("method", json!(method_name(state.merge_method)));
                args.insert("delete_branch", json!(state.delete_branch));
                args.insert("auto_merge", json!(state.auto_merge));
                let _ = cx.run_command("review.merge", args);
            }
        });
        ui.label(
            egui::RichText::new(cx.t_args("review.pr.state", &[("state", &pr.merge_state)]))
                .small()
                .color(muted),
        );
    }

    fn generated(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        state: &State,
        muted: egui::Color32,
    ) {
        let Some(generated) = &state.generated else {
            return;
        };
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(cx.t("review.generated.title"))
                    .strong()
                    .color(c32(tokens.color.text)),
            );
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("review.command.copy"),
            )
            .clicked()
            {
                ui.ctx().copy_text(generated.text.clone());
            }
        });
        egui::Frame::new()
            .fill(c32(tokens.color.surface_alt))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&generated.text).monospace().small());
                if let (Some(title), Some(body)) = (&generated.title, &generated.body) {
                    ui.label(egui::RichText::new(title).strong());
                    ui.label(egui::RichText::new(body).small().color(muted));
                }
            });
        if generated.kind == "pr"
            && forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("review.generated.use"),
            )
            .clicked()
        {
            self.state.with(|state| {
                if let Some(Generated {
                    title: Some(title),
                    body: Some(body),
                    ..
                }) = &state.generated
                {
                    state.title = title.clone();
                    state.body = body.clone();
                }
            });
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
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[allow(dead_code)]
fn check_name(check: &Check) -> &str {
    &check.name
}

#[allow(dead_code)]
fn provider_name(provider: Provider) -> &'static str {
    provider.id()
}
