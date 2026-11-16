//! The Search panel (right panel): the query form, streamed results grouped by
//! file, and the replace section with its preview and confirmation.
//!
//! The view takes a snapshot under the lock, draws, and collects user requests as
//! [`Action`]s. The actions run after drawing, so no lock is held while `Ctx` runs.
//! The results list is drawn under the lock, which is safe because drawing calls
//! no `Ctx` and no other lock.

use crate::commands;
use crate::engine::Hit;
use crate::state::{lock, Form, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use std::path::PathBuf;

/// Hits drawn per frame. Further hits are counted but not drawn, so a big result stays responsive.
const MAX_RENDER_HITS: usize = 400;

pub struct SearchView {
    pub shared: Shared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Form(Form),
    Search,
    Stop,
    Preview,
    Apply,
    Open(PathBuf, u32),
}

impl View for SearchView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        commands::sync_root(cx, &self.shared);

        let mut actions: Vec<Action> = Vec::new();
        let (form, focus, has_root) = {
            let state = lock(&self.shared);
            (state.form.clone(), state.focus_query, state.root.is_some())
        };

        // O título do painel já está na aba: não repetimos aqui.
        let mut edited = form.clone();
        let query = forja_ui_kit::text_field(ui, &mut edited.pattern, &cx.t("search.query.hint"));
        if focus {
            query.request_focus();
            lock(&self.shared).focus_query = false;
        }
        if query.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            actions.push(Action::Search);
        }
        ui.horizontal_wrapped(|ui| {
            forja_ui_kit::toggle(ui, &mut edited.regex, &cx.t("search.option.regex"));
            forja_ui_kit::toggle(ui, &mut edited.case_sensitive, &cx.t("search.option.case"));
            forja_ui_kit::toggle(ui, &mut edited.whole_word, &cx.t("search.option.word"));
        });
        forja_ui_kit::text_field(ui, &mut edited.include, &cx.t("search.include.hint"));
        forja_ui_kit::text_field(ui, &mut edited.exclude, &cx.t("search.exclude.hint"));
        if edited != form {
            actions.push(Action::Form(edited.clone()));
        }

        let running = lock(&self.shared).running;
        ui.horizontal(|ui| {
            let can_search = has_root && !edited.pattern.is_empty() && !running;
            // `add_enabled_ui` desliga o clique; a condição no `if` cobre o caso sem depender disso.
            ui.add_enabled_ui(can_search, |ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &cx.t("search.button.search"))
                    .clicked()
                    && can_search
                {
                    actions.push(Action::Search);
                }
            });
            ui.add_enabled_ui(running, |ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cx.t("search.button.stop"))
                    .clicked()
                    && running
                {
                    actions.push(Action::Stop);
                }
            });
        });

        // Results: drawn under the lock (no Ctx calls inside), then the actions run.
        {
            let state = lock(&self.shared);
            let status = if !has_root {
                Some(cx.t("search.notice.no_bench"))
            } else if state.running {
                Some(cx.t("search.status.running"))
            } else if !state.searched {
                Some(cx.t("search.status.idle"))
            } else if state.error.is_some() {
                None
            } else if state.files.is_empty() {
                Some(cx.t("search.status.none"))
            } else {
                Some(cx.t_args(
                    "search.status.results",
                    &[
                        ("matches", &state.matches.to_string()),
                        ("files", &state.files.len().to_string()),
                    ],
                ))
            };
            if let Some(status) = status {
                ui.label(egui::RichText::new(status).weak());
            }
            if let Some(error) = state.error.as_deref() {
                let human = humanize_error(cx, error);
                let details_label = cx.t("kernel.error.details");
                forja_ui_kit::error_banner(
                    ui,
                    &human.summary,
                    human.detail.as_deref(),
                    &details_label,
                );
            }
            if state.truncated {
                ui.label(egui::RichText::new(cx.t("search.status.truncated")).weak());
            }
            if let Some(notice) = &state.notice {
                ui.label(egui::RichText::new(notice).weak());
            }
            draw_results(ui, cx, &state.files, &mut actions);
        }

        ui.separator();
        ui.label(egui::RichText::new(cx.t("search.replace.title")).strong());
        let mut replacement = form.replacement.clone();
        let replace_edit = forja_ui_kit::text_field(ui, &mut replacement, &cx.t("search.replace.hint"));
        if replace_edit.changed() {
            let mut next = form.clone();
            next.replacement = replacement;
            actions.push(Action::Form(next));
        }
        let (planning, applying, confirm, plan_ready) = {
            let state = lock(&self.shared);
            (
                state.planning,
                state.applying,
                state.confirm,
                state.plan.is_some() && state.plan_form.as_ref() == Some(&form),
            )
        };
        ui.horizontal(|ui| {
            let can_preview = has_root && !edited.pattern.is_empty() && !planning && !applying;
            ui.add_enabled_ui(can_preview, |ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cx.t("search.button.preview"))
                    .clicked()
                    && can_preview
                {
                    actions.push(Action::Preview);
                }
            });
            let label = if confirm {
                cx.t("search.button.confirm")
            } else {
                cx.t("search.button.apply")
            };
            let can_apply = plan_ready && !applying;
            ui.add_enabled_ui(can_apply, |ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &label).clicked() && can_apply {
                    actions.push(Action::Apply);
                }
            });
        });
        draw_plan(ui, cx, &self.shared, planning);

        for action in actions {
            self.run(cx, action);
        }
    }
}

impl SearchView {
    fn run(&self, cx: &mut Ctx<'_>, action: Action) {
        match action {
            Action::Form(form) => commands::set_form(&self.shared, form),
            Action::Search => {
                let _ = commands::start_search(cx, &self.shared);
            }
            Action::Stop => commands::stop_search(&self.shared),
            Action::Preview => {
                let _ = commands::preview(cx, &self.shared);
            }
            Action::Apply => {
                let _ = commands::apply(cx, &self.shared);
            }
            Action::Open(path, line) => commands::open_hit(cx, &self.shared, path, line),
        }
    }
}

/// Draws the match groups. Each matching line is clickable and opens the file there.
fn draw_results(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    files: &[crate::engine::FileHits],
    actions: &mut Vec<Action>,
) {
    let mut drawn = 0usize;
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for file in files {
                if drawn >= MAX_RENDER_HITS {
                    break;
                }
                let title = format!("{} ({})", file.rel, file.hits.len());
                egui::CollapsingHeader::new(title)
                    .id_salt(&file.rel)
                    .default_open(true)
                    .show(ui, |ui| {
                        for hit in &file.hits {
                            if drawn >= MAX_RENDER_HITS {
                                break;
                            }
                            drawn += 1;
                            draw_hit(ui, hit, &file.path, actions);
                        }
                    });
            }
            if drawn >= MAX_RENDER_HITS {
                ui.label(egui::RichText::new(cx.t("search.status.more")).weak());
            }
        });
}

fn draw_hit(ui: &mut egui::Ui, hit: &Hit, path: &std::path::Path, actions: &mut Vec<Action>) {
    for line in &hit.lines {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{:>5}", line.number))
                    .monospace()
                    .weak(),
            );
            let text = egui::RichText::new(&line.text).monospace();
            let text = if line.is_match {
                text.strong()
            } else {
                text.weak()
            };
            let response = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
            if line.is_match && response.clicked() {
                actions.push(Action::Open(path.to_path_buf(), line.number));
            }
        });
    }
    ui.add_space(4.0);
}

/// Draws the replacement preview: a summary and the changed lines of each file.
fn draw_plan(ui: &mut egui::Ui, cx: &Ctx<'_>, shared: &Shared, planning: bool) {
    if planning {
        ui.label(egui::RichText::new(cx.t("search.preview.planning")).weak());
        return;
    }
    let state = lock(shared);
    if let Some(error) = state.plan_error.as_deref() {
        let human = humanize_error(cx, error);
        let details_label = cx.t("kernel.error.details");
        forja_ui_kit::error_banner(ui, &human.summary, human.detail.as_deref(), &details_label);
    }
    if let Some(notice) = &state.notice {
        if state.plan.is_none() {
            ui.label(egui::RichText::new(notice).weak());
        }
    }
    let Some(plan) = &state.plan else {
        return;
    };
    ui.label(
        egui::RichText::new(cx.t_args(
            "search.preview.summary",
            &[
                ("matches", &plan.matches.to_string()),
                ("files", &plan.files.len().to_string()),
            ],
        ))
        .strong(),
    );
    egui::ScrollArea::vertical()
        .max_height(240.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for file in &plan.files {
                egui::CollapsingHeader::new(format!("{} ({})", file.rel, file.count))
                    .id_salt(("preview", &file.rel))
                    .default_open(false)
                    .show(ui, |ui| {
                        if file.multiline {
                            ui.label(egui::RichText::new(cx.t("search.preview.multiline")).weak());
                        }
                        for change in &file.changes {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:>5} - {}",
                                    change.line, change.before
                                ))
                                .monospace()
                                .weak(),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "{:>5} + {}",
                                    change.line, change.after
                                ))
                                .monospace(),
                            );
                        }
                    });
            }
        });
}

pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("regex parse error")
        || lower.starts_with("regex error")
        || lower.starts_with("compiled regex exceeds")
    {
        return forja_kernel::HumanError::with_detail(cx.t("search.error.invalid-regex"), trimmed);
    }
    forja_kernel::human_error::humanize_ctx(cx, trimmed)
}
