use crate::log;
use crate::state::Shared;
use crate::view_changes::{c32, small};
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;
use std::path::Path;
pub(crate) fn render_history(ui: &mut egui::Ui, cx: &Ctx<'_>, state: &Shared, compact: bool) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let Some(path) = state.path() else {
        forja_ui_kit::empty_state(ui, forja_ui_kit::BOARD, &cx.t("git.changes.empty"), None);
        return;
    };
    let lines = state.commits();
    if lines.is_empty() {
        forja_ui_kit::empty_state(ui, forja_ui_kit::ARCHIVE, &cx.t("git.history.empty"), None);
        return;
    }
    let selected = state.selected_commit();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in lines.iter().take(if compact { 80 } else { 300 }) {
                let Some(commit) = &line.commit else {
                    ui.label(
                        egui::RichText::new(&line.graph)
                            .monospace()
                            .small()
                            .color(c32(tokens.color.border)),
                    );
                    continue;
                };
                let active = selected.as_deref() == Some(commit.hash.as_str());
                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "{}{} {}",
                            line.graph, commit.short, commit.subject
                        ))
                        .monospace()
                        .color(c32(if active {
                            tokens.color.accent
                        } else {
                            tokens.color.text
                        })),
                    )
                    .truncate()
                    .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    state.set_selected_commit(Some(commit.hash.clone()));
                }
                ui.label(
                    egui::RichText::new(format!(
                        "   {} \u{00b7} {}",
                        commit.author,
                        format_time(commit.time)
                    ))
                    .small()
                    .color(c32(tokens.color.text_muted)),
                );
            }
        });
    if let Some(hash) = selected {
        if let Some(commit) = lines
            .iter()
            .filter_map(|line| line.commit.as_ref())
            .find(|commit| commit.hash == hash)
        {
            ui.separator();
            ui.label(
                egui::RichText::new(format!("{} {}", commit.short, commit.subject))
                    .strong()
                    .color(c32(tokens.color.text)),
            );
            ui.label(
                egui::RichText::new(format!("{} <{}>", commit.author, commit.email))
                    .small()
                    .color(c32(tokens.color.text_muted)),
            );
            ui.label(
                egui::RichText::new(format_time(commit.time))
                    .small()
                    .color(c32(tokens.color.text_muted)),
            );
        }
    }
    let _ = path;
}

pub(crate) fn render_blame_panel(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    blame_input: &mut String,
) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let Some(path) = state.path() else {
        forja_ui_kit::empty_state(ui, forja_ui_kit::BOARD, &cx.t("git.changes.empty"), None);
        return;
    };
    let file = state.blame_file();
    if blame_input.is_empty() {
        *blame_input = file;
    }
    if let Ok(env_file) = std::env::var("FORJA_GIT_BLAME") {
        if !env_file.trim().is_empty() {
            if blame_input.is_empty() {
                *blame_input = env_file.clone();
                state.set_blame_file(env_file.clone());
            }
            load_blame(cx, state, &path, &env_file);
        }
    }
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(blame_input)
                .hint_text(cx.t("git.changes.blame-file"))
                .desired_width(ui.available_width() - 70.0),
        );
        if small(ui, &cx.t("git.changes.blame-run")) {
            state.set_blame_file(blame_input.clone());
            load_blame(cx, state, &path, blame_input);
        }
    });
    let lines = state.blame();
    if lines.is_empty() {
        forja_ui_kit::empty_state(ui, forja_ui_kit::ARCHIVE, &cx.t("git.changes.blame-empty"), None);
        return;
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for line in &lines {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{:>4}", line.line))
                            .monospace()
                            .small()
                            .color(c32(tokens.color.text_muted)),
                    );
                    ui.label(
                        egui::RichText::new(&line.short)
                            .monospace()
                            .small()
                            .color(c32(tokens.color.info)),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&line.content)
                                .monospace()
                                .small()
                                .color(c32(tokens.color.text)),
                        )
                        .truncate(),
                    );
                });
            }
        });
}

pub(crate) fn load_log(cx: &mut Ctx<'_>, state: &Shared) {
    let _ = cx;
    let Some(path) = state.path() else {
        return;
    };
    if state.log_for(&path) {
        return;
    }
    match log::log(Path::new(&path), 200) {
        Ok(lines) => {
            state.set_error(None);
            state.set_commits(&path, lines);
        }
        Err(error) => state.set_error(Some(error)),
    }
}

fn load_blame(cx: &mut Ctx<'_>, state: &Shared, path: &str, file: &str) {
    let _ = cx;
    if file.trim().is_empty() || state.blame_for(path, file) {
        return;
    }
    match log::blame(Path::new(path), file) {
        Ok(lines) => {
            state.set_error(None);
            state.set_blame(path, file, lines);
        }
        Err(error) => state.set_error(Some(error)),
    }
}

pub(crate) fn format_time(epoch: i64) -> String {
    chrono::DateTime::from_timestamp(epoch, 0)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| epoch.to_string())
}
