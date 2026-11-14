//! Right panel "MudanÃ§as" plus compact history and blame tabs.

use crate::engine;
use crate::state::Shared;
use crate::view_history_panel::{load_log, render_blame_panel, render_history};
use forja_contracts::GitStatusChanged;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map, Value};
use std::path::Path;

pub struct ChangesView {
    state: Shared,
    checked: bool,
    force: bool,
    blame_input: String,
}

impl ChangesView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            checked: false,
            force: false,
            blame_input: String::new(),
        }
    }
}

impl View for ChangesView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.checked {
            self.checked = true;
            ensure_context(cx, &self.state);
        }
        refresh_if_needed(cx, &self.state);
        ensure_fetch(cx, &self.state);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let mut tab = self.state.selected_tab();
        let labels = [
            cx.t("git.changes.tab.changes"),
            cx.t("git.changes.tab.history"),
            cx.t("git.changes.tab.blame"),
        ];
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let before = tab;
        forja_ui_kit::tabs(ui, &mut tab, &label_refs);
        if tab != before {
            self.state.set_selected_tab(tab);
        }
        ui.separator();
        match tab {
            1 => {
                load_log(cx, &self.state);
                render_history(ui, cx, &self.state, true);
            }
            2 => {
                render_blame_panel(ui, cx, &self.state, &mut self.blame_input);
            }
            _ => {
                render_changes(ui, cx, &self.state, &mut self.force);
            }
        }
        messages(ui, cx, &self.state, &tokens);
    }
}

pub(crate) fn ensure_context(cx: &mut Ctx<'_>, state: &Shared) {
    if state.path().is_some() {
        return;
    }
    let Ok(value) = cx.run_command("bench.current", Args::new()) else {
        return;
    };
    if let Some(path) = value.get("path").and_then(Value::as_str) {
        state.set_path(Some(path.to_owned()));
        refresh_async(cx, state);
    }
}

pub(crate) fn refresh_if_needed(cx: &mut Ctx<'_>, state: &Shared) {
    let Some(path) = state.path() else {
        return;
    };
    let interval = cx
        .setting("git.status.interval_s")
        .and_then(SettingValue::as_i64)
        .unwrap_or(5);
    if !state.begin_refresh(interval) {
        return;
    }
    let target = path.clone();
    let shared = state.clone();
    cx.spawn(
        move || engine::status(Path::new(&target)),
        move |cx, result| match result {
            Ok(report) => {
                let summary = report.summary.clone();
                shared.set_report(report);
                cx.emit(&GitStatusChanged {
                    bench_path: path,
                    summary,
                });
            }
            Err(error) => {
                shared.finish_refresh();
                shared.set_error(Some(error));
            }
        },
    );
}

pub(crate) fn ensure_fetch(cx: &mut Ctx<'_>, state: &Shared) {
    let Some(path) = state.path() else {
        return;
    };
    let interval = cx
        .setting("git.sync.fetch_interval_min")
        .and_then(SettingValue::as_i64)
        .unwrap_or(10);
    if !state.begin_fetch(interval) {
        return;
    }
    let auto_pull = cx
        .setting("git.sync.auto_pull")
        .and_then(SettingValue::as_str)
        .unwrap_or("never")
        .to_owned();
    let target = path.clone();
    let shared = state.clone();
    cx.spawn(
        move || engine::fetch(Path::new(&target)),
        move |cx, result| {
            shared.finish_fetch();
            if result.is_err() || auto_pull == "never" {
                return;
            }
            let clean = shared.report().map(clean_worktree).unwrap_or(false);
            if auto_pull == "always" || clean {
                let path = path.clone();
                let state = shared.clone();
                cx.spawn(
                    move || engine::pull(Path::new(&path)),
                    move |cx, _result| {
                        state.set_notice(Some(cx.t("git.notice.fetch-pull")));
                    },
                );
            }
        },
    );
}

fn clean_worktree(report: engine::StatusReport) -> bool {
    report.summary.staged == 0
        && report.summary.unstaged == 0
        && report.summary.untracked == 0
        && report.summary.conflicts == 0
}

fn refresh_async(cx: &mut Ctx<'_>, state: &Shared) {
    let Some(path) = state.path() else {
        return;
    };
    if !state.begin_refresh(0) {
        return;
    }
    let target = path.clone();
    let shared = state.clone();
    cx.spawn(
        move || engine::status(Path::new(&target)),
        move |cx, result| match result {
            Ok(report) => {
                let summary = report.summary.clone();
                shared.set_report(report);
                cx.emit(&GitStatusChanged {
                    bench_path: path,
                    summary,
                });
            }
            Err(error) => {
                shared.finish_refresh();
                shared.set_error(Some(error));
            }
        },
    );
}

fn render_changes(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared, force: &mut bool) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let Some(path) = state.path() else {
        let message = format!(
            "{}\n{}",
            cx.t("git.changes.empty"),
            cx.t("git.changes.empty-hint")
        );
        forja_ui_kit::empty_state(ui, forja_ui_kit::BOARD, &message, None);
        return;
    };
    let Some(report) = state.report() else {
        ui.label(
            egui::RichText::new(cx.t("git.changes.busy"))
                .small()
                .color(c32(tokens.color.text_muted)),
        );
        return;
    };
    let branch = report.summary.branch.clone().unwrap_or_else(|| {
        if report.detached {
            cx.t("git.changes.detached")
        } else {
            "\u{2014}".to_owned()
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(branch)
                .strong()
                .color(c32(tokens.color.text)),
        );
        if report.summary.ahead > 0 {
            ui.label(
                egui::RichText::new(format!("\u{2191}{}", report.summary.ahead))
                    .small()
                    .color(c32(tokens.color.ok)),
            );
        }
        if report.summary.behind > 0 {
            ui.label(
                egui::RichText::new(format!("\u{2193}{}", report.summary.behind))
                    .small()
                    .color(c32(tokens.color.spark)),
            );
        }
        for (label, value, color) in [
            ("git.changes.staged", report.summary.staged, tokens.color.ok),
            (
                "git.changes.unstaged",
                report.summary.unstaged,
                tokens.color.accent,
            ),
            (
                "git.changes.untracked",
                report.summary.untracked,
                tokens.color.info,
            ),
            (
                "git.changes.conflicts",
                report.summary.conflicts,
                tokens.color.danger,
            ),
        ] {
            if value > 0 {
                ui.label(
                    egui::RichText::new(format!("{} {value}", cx.t(label)))
                        .small()
                        .color(c32(color)),
                );
            }
        }
    });

    ui.horizontal_wrapped(|ui| {
        if small(ui, &cx.t("git.changes.refresh")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            let _ = run(cx, "git.refresh", args);
        }
        for (label, command) in [
            ("git.changes.fetch", "git.fetch"),
            ("git.changes.pull", "git.pull"),
            ("git.changes.sync", "git.sync"),
            ("git.changes.publish", "git.publish"),
            ("git.changes.abort", "git.abort"),
        ] {
            if small(ui, &cx.t(label)) {
                let mut args = Map::new();
                args.insert("path".to_owned(), json!(path));
                let _ = run(cx, command, args);
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        if small(ui, &cx.t("git.changes.push")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            args.insert("force_with_lease".to_owned(), json!(*force));
            let _ = run(cx, "git.push", args);
        }
        ui.checkbox(force, cx.t("git.changes.force-with-lease"));
        if small(ui, &cx.t("git.changes.rebase")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            let _ = run(cx, "git.rebase", args);
        }
    });

    ui.add_space(4.0);
    let mut message = state.commit_message();
    let changed = ui
        .add(
            egui::TextEdit::singleline(&mut message)
                .hint_text(cx.t("git.changes.commit-message"))
                .desired_width(ui.available_width()),
        )
        .changed();
    if changed {
        state.set_commit_message(message.clone());
    }
    ui.horizontal(|ui| {
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Primary,
            &cx.t("git.changes.commit"),
        )
        .clicked()
        {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            args.insert("message".to_owned(), json!(message));
            let _ = run(cx, "git.commit", args);
            state.set_commit_message(String::new());
        }
        if small(ui, &cx.t("git.changes.generate")) {
            if let Err(error) = generate_message(cx, &path, state) {
                state.set_error(Some(error));
            }
        }
        if small(ui, &cx.t("git.changes.commit-all")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            args.insert("message".to_owned(), json!(message));
            args.insert("all".to_owned(), json!(true));
            let _ = run(cx, "git.commit", args);
            state.set_commit_message(String::new());
        }
    });
    ui.horizontal_wrapped(|ui| {
        if small(ui, &cx.t("git.changes.stage-all")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            let _ = run(cx, "git.stage_all", args);
        }
        if small(ui, &cx.t("git.changes.unstage-all")) {
            let mut args = Map::new();
            args.insert("path".to_owned(), json!(path));
            let _ = run(cx, "git.unstage_all", args);
        }
    });
    ui.separator();

    if report.files.is_empty() {
        forja_ui_kit::empty_state(ui, forja_ui_kit::ARCHIVE, &cx.t("git.changes.empty"), None);
    } else {
        egui::ScrollArea::vertical()
            .id_salt("git.changes.files")
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 10.0)
            .show(ui, |ui| {
                for file in &report.files {
                    ui.horizontal(|ui| {
                        let code = format!("{}{}", file.index, file.worktree);
                        let color = if file.conflict {
                            tokens.color.danger
                        } else if file.untracked {
                            tokens.color.info
                        } else if file.staged {
                            tokens.color.ok
                        } else {
                            tokens.color.accent
                        };
                        ui.label(
                            egui::RichText::new(code)
                                .monospace()
                                .small()
                                .color(c32(color)),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&file.path)
                                    .small()
                                    .color(c32(tokens.color.text)),
                            )
                            .truncate(),
                        );
                        if !file.staged && !file.conflict && !file.untracked && small(ui, "+") {
                            stage(cx, &path, std::slice::from_ref(&file.path));
                        }
                        if file.staged && small(ui, "\u{2212}") {
                            unstage(cx, &path, std::slice::from_ref(&file.path));
                        }
                        if small(ui, "\u{21ba}") {
                            state.set_discard(Some(vec![file.path.clone()]));
                        }
                    });
                }
            });
    }

    if let Some(files) = state.discard() {
        let mut open = true;
        egui::Window::new(cx.t("git.changes.discard-title"))
            .id(egui::Id::new("forja.git.discard"))
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                ui.label(cx.t_args("git.changes.discard-body", &[("files", &files.join(", "))]));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Danger,
                        &cx.t("git.changes.discard-confirm"),
                    )
                    .clicked()
                    {
                        let mut args = Map::new();
                        args.insert("path".to_owned(), json!(path));
                        args.insert("files".to_owned(), json!(files));
                        args.insert("confirm".to_owned(), json!(true));
                        let _ = run(cx, "git.discard", args);
                        state.set_discard(None);
                        open = false;
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("git.changes.cancel"),
                    )
                    .clicked()
                    {
                        state.set_discard(None);
                        open = false;
                    }
                });
            });
        if !open {
            state.set_discard(None);
        }
    }
}

pub(crate) fn messages(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    state: &Shared,
    tokens: &forja_ui_kit::Tokens,
) {
    if state.busy() {
        ui.label(
            egui::RichText::new(cx.t("git.changes.busy"))
                .small()
                .color(c32(tokens.color.text_muted)),
        );
    }
    if let Some(error) = state.error() {
        let message = humanize_error(cx, &error);
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(message)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
            if forja_ui_kit::icon_button(ui, forja_ui_kit::CLOSE)
                .on_hover_text(cx.t("git.error.dismiss"))
                .clicked()
            {
                state.set_error(None);
            }
        });
    }
    if let Some(notice) = state.notice() {
        ui.label(
            egui::RichText::new(notice)
                .small()
                .color(c32(tokens.color.info)),
        );
    }
}

pub(crate) fn small(ui: &mut egui::Ui, label: &str) -> bool {
    forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, label).clicked()
}

pub(crate) fn run(
    cx: &mut Ctx<'_>,
    id: &str,
    map: Map<String, Value>,
) -> forja_kernel::Result<Value> {
    cx.run_command(id, Args::from_map(map))
}

/// Asks the review module to generate a commit message, but explains first
/// when there is no installed helper to run the generation.
pub(crate) fn generate_message(cx: &mut Ctx<'_>, path: &str, state: &Shared) -> Result<(), String> {
    let mut helper_args = Map::new();
    helper_args.insert("bench_path".to_owned(), json!(path));
    let profiles = cx
        .run_command("helper.list", Args::from_map(helper_args))
        .ok()
        .and_then(|value| value.get("profiles").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    let installed = profiles.iter().any(|profile| {
        profile
            .get("installed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && profile
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
    });
    if !installed {
        return Err(cx.t("git.generate.no-helper"));
    }
    let mut args = Map::new();
    args.insert("kind".to_owned(), json!("commit"));
    args.insert("bench_path".to_owned(), json!(path));
    match run(cx, "review.generate", args) {
        Ok(_) => {
            state.set_notice(Some(cx.t("git.generate.running")));
            Ok(())
        }
        Err(forja_kernel::KernelError::UnknownCommand(_)) => Err(cx.t("git.generate.no-review")),
        Err(error) => Err(error.to_string()),
    }
}

fn stage(cx: &mut Ctx<'_>, path: &str, files: &[String]) {
    let mut args = Map::new();
    args.insert("path".to_owned(), json!(path));
    args.insert("files".to_owned(), json!(files));
    let _ = run(cx, "git.stage", args);
}

fn unstage(cx: &mut Ctx<'_>, path: &str, files: &[String]) {
    let mut args = Map::new();
    args.insert("path".to_owned(), json!(path));
    args.insert("files".to_owned(), json!(files));
    let _ = run(cx, "git.unstage", args);
}

pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed == "no-remote" {
        return cx.t("git.error.no-remote");
    }
    if trimmed == "nothing-to-abort" {
        return cx.t("git.error.nothing-to-abort");
    }
    if let Some(branch) = trimmed.strip_prefix("protected:") {
        return cx.t_args("git.error.protected", &[("branch", branch)]);
    }
    if trimmed == "no file selected" {
        return cx.t("git.error.no-file");
    }
    let protected_template = cx.t_args("git.error.protected", &[("branch", "\x00")]);
    if let Some((head, tail)) = protected_template.split_once('\x00') {
        if trimmed.starts_with(head) && trimmed.ends_with(tail) {
            return trimmed.to_owned();
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("not a git repository") {
        return cx.t_args("git.error.not-repo", &[("detail", trimmed)]);
    }
    if lower.contains("nothing to commit") || lower.contains("no changes added to commit") {
        return cx.t_args("git.error.nothing-to-commit", &[("detail", trimmed)]);
    }
    if lower.contains("conflict") || lower.contains("unmerged files") {
        return cx.t_args("git.error.conflict", &[("detail", trimmed)]);
    }
    if lower.contains("rejected") || lower.contains("failed to push") || lower.contains("non-fast-forward") {
        return cx.t_args("git.error.push-rejected", &[("detail", trimmed)]);
    }
    if lower.contains("no upstream branch") || lower.contains("has no upstream") || lower.contains("no tracking information") {
        return cx.t_args("git.error.no-upstream", &[("detail", trimmed)]);
    }
    if lower.contains("authentication failed")
        || lower.contains("permission denied")
        || lower.contains("could not read username")
    {
        return cx.t_args("git.error.auth", &[("detail", trimmed)]);
    }
    if lower.contains("git failed to start") || lower.contains("failed to execute") {
        return cx.t_args("git.error.exec", &[("detail", trimmed)]);
    }
    cx.t_args("git.error.generic", &[("detail", trimmed)])
}

pub(crate) fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Module, Runtime, RuntimeOptions};

    #[test]
    fn raw_git_errors_are_humanized_with_technical_detail() {
        let dir = std::env::temp_dir().join(format!("forja-git-err-{}", std::process::id()));
        let mut runtime = Runtime::new(
            vec![Box::new(crate::GitModule {
                state: Shared::new(),
            }) as Box<dyn Module>],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime");
        let cx = runtime.ctx("git");
        let msg = humanize_error(&cx, "fatal: not a git repository (or any parent)");
        assert!(msg.contains("repositório Git"), "{msg}");
        assert!(msg.contains("not a git repository"), "{msg}");
        assert!(humanize_error(
            &cx,
            "git pull --ff-only failed: There is no tracking information for the current branch."
        )
        .contains("branch remoto"));
        assert_eq!(humanize_error(&cx, "no file selected"), "Selecione ao menos um arquivo.");
        let _ = std::fs::remove_dir_all(dir);
    }
}
