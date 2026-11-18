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
        messages(ui, cx, &self.state, &tokens);
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
            forja_ui_kit::ButtonKind::Secondary,
            &cx.t("git.changes.commit"),
        )
        .clicked()
        {
            if report.summary.staged == 0 {
                state.set_error(Some("nothing-staged".to_owned()));
            } else {
                let mut args = Map::new();
                args.insert("path".to_owned(), json!(path));
                args.insert("message".to_owned(), json!(message));
                args.insert("clear_message".to_owned(), json!(true));
                let _ = run(cx, "git.commit", args);
            }
        }
        if forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::Refresh, &cx.t("git.changes.sync"))
            .clicked()
        {
            let _ = run(
                cx,
                "git.sync",
                Map::from_iter([("path".to_owned(), json!(path))]),
            );
        }
        let more =
            forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::More, &cx.t("git.changes.more"));
        egui::Popup::from_response(&more)
            .kind(egui::PopupKind::Menu)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .open_memory(more.clicked().then_some(egui::SetOpenCommand::Toggle))
            .show(|ui| {
                for (key, command) in [
                    ("git.changes.refresh", "git.refresh"),
                    ("git.changes.fetch", "git.fetch"),
                    ("git.changes.pull", "git.pull"),
                    ("git.changes.publish", "git.publish"),
                    ("git.changes.push", "git.push"),
                    ("git.changes.abort", "git.abort"),
                    ("git.changes.rebase", "git.rebase"),
                    ("git.changes.generate", "generate"),
                    ("git.changes.commit-all", "commit-all"),
                    ("git.changes.stage-all", "git.stage_all"),
                    ("git.changes.unstage-all", "git.unstage_all"),
                ] {
                    if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t(key))
                        .clicked()
                    {
                        let mut args = Map::from_iter([("path".to_owned(), json!(path))]);
                        if command == "generate" {
                            if let Err(error) = generate_message(cx, &path, state) {
                                state.set_error(Some(error));
                            }
                        } else {
                            if command == "git.push" {
                                args.insert("force_with_lease".to_owned(), json!(*force));
                            }
                            if command == "commit-all" {
                                args.insert("message".to_owned(), json!(message));
                                args.insert("all".to_owned(), json!(true));
                                args.insert("clear_message".to_owned(), json!(true));
                            }
                            let _ = run(
                                cx,
                                if command == "commit-all" {
                                    "git.commit"
                                } else {
                                    command
                                },
                                args,
                            );
                        }
                        ui.close();
                    }
                }
                ui.separator();
                ui.checkbox(force, cx.t("git.changes.force-with-lease"));
            });
    });
    ui.separator();

    if report.files.is_empty() {
        forja_ui_kit::empty_state(ui, forja_ui_kit::ARCHIVE, &cx.t("git.changes.empty"), None);
    } else {
        egui::ScrollArea::vertical()
            .id_salt("git.changes.files")
            .auto_shrink([false, false])
            .max_height((ui.available_height() - 10.0).max(60.0))
            .show(ui, |ui| {
                for file in &report.files {
                    ui.horizontal(|ui| {
                        ui.set_max_width(ui.available_width().max(1.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if forja_ui_kit::icon_button(
                                ui,
                                forja_ui_kit::Icon::Delete,
                                &cx.t("git.changes.discard"),
                            )
                            .clicked()
                            {
                                state.set_discard(Some(vec![file.path.clone()]));
                            }
                            if file.staged
                                && forja_ui_kit::icon_button(
                                    ui,
                                    forja_ui_kit::Icon::Back,
                                    &cx.t("git.changes.unstage"),
                                )
                                .clicked()
                            {
                                unstage(cx, &path, std::slice::from_ref(&file.path));
                            }
                            if !file.staged
                                && !file.conflict
                                && !file.untracked
                                && forja_ui_kit::icon_button(
                                    ui,
                                    forja_ui_kit::Icon::Add,
                                    &cx.t("git.changes.stage"),
                                )
                                .clicked()
                            {
                                stage(cx, &path, std::slice::from_ref(&file.path));
                            }
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
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
                                    let response = ui
                                        .add(
                                            egui::Label::new(
                                                egui::RichText::new(&file.path)
                                                    .small()
                                                    .underline()
                                                    .color(c32(tokens.color.text)),
                                            )
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                        )
                                        .on_hover_text(&file.path);
                                    if response.clicked() {
                                        open_file_diff(
                                            cx,
                                            &path,
                                            &file.path,
                                            file.staged && !file.unstaged,
                                        );
                                    }
                                },
                            );
                        });
                    });
                }
            });
    }

    if let Some(files) = state.discard() {
        let layer = egui::Id::new("forja.git.discard.keyboard");
        if forja_ui_kit::consume_layer_escape(ui.ctx(), layer) {
            state.set_discard(None);
        } else {
            let mut open = true;
            egui::Window::new(cx.t("git.changes.discard-title"))
                .id(egui::Id::new("forja.git.discard"))
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .show(ui.ctx(), |ui| {
                    forja_ui_kit::register_keyboard_layer(ui, layer);
                    ui.label(
                        cx.t_args("git.changes.discard-body", &[("files", &files.join(", "))]),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::DangerConfirm,
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
        let human = humanize_error(cx, &error);
        let dismiss = cx.t("git.error.dismiss");
        if forja_ui_kit::error_notice(
            ui,
            &human.summary,
            human.detail.as_deref(),
            &cx.t("kernel.error.details"),
            Some(&dismiss),
        ) {
            state.set_error(None);
        }
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

fn open_file_diff(cx: &mut Ctx<'_>, path: &str, file: &str, staged: bool) {
    let mut args = Args::new();
    args.insert("path", json!(path));
    args.insert("file", json!(file));
    args.insert("staged", json!(staged));
    if cx.run_command("diff.open", args).is_err() {
        return;
    }
    let mut args = Args::new();
    args.insert("view_id", json!("diff.tab"));
    let _ = cx.run_command("shell.tab.open", args);
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

pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    if trimmed == "nothing-staged" {
        return forja_kernel::HumanError::simple(cx.t("git.error.nothing-to-commit"));
    }
    if trimmed == "no-remote" {
        return forja_kernel::HumanError::simple(cx.t("git.error.no-remote"));
    }
    if trimmed == "nothing-to-abort" {
        return forja_kernel::HumanError::simple(cx.t("git.error.nothing-to-abort"));
    }
    if let Some(branch) = trimmed.strip_prefix("protected:") {
        return forja_kernel::HumanError::simple(
            cx.t_args("git.error.protected", &[("branch", branch)]),
        );
    }
    if trimmed == "no file selected" {
        return forja_kernel::HumanError::simple(cx.t("git.error.no-file"));
    }
    let protected_template = cx.t_args("git.error.protected", &[("branch", "\x00")]);
    if let Some((head, tail)) = protected_template.split_once('\x00') {
        if trimmed.starts_with(head) && trimmed.ends_with(tail) {
            return forja_kernel::HumanError::simple(trimmed);
        }
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("not a git repository") {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.not-repo"), trimmed);
    }
    if lower.contains("nothing to commit") || lower.contains("no changes added to commit") {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.nothing-to-commit"), trimmed);
    }
    if lower.contains("nothing added to commit") {
        return forja_kernel::HumanError::simple(cx.t("git.error.nothing-to-commit"));
    }
    if lower.contains("conflict") || lower.contains("unmerged files") {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.conflict"), trimmed);
    }
    if lower.contains("rejected")
        || lower.contains("failed to push")
        || lower.contains("non-fast-forward")
    {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.push-rejected"), trimmed);
    }
    if lower.contains("no upstream branch")
        || lower.contains("has no upstream")
        || lower.contains("no tracking information")
    {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.no-upstream"), trimmed);
    }
    if lower.contains("authentication failed")
        || lower.contains("could not read username")
        || lower.contains("terminal prompts disabled")
    {
        return forja_kernel::HumanError::with_detail(cx.t("git.error.auth"), trimmed);
    }
    let human = forja_kernel::human_error::humanize_ctx(cx, trimmed);
    if human.summary == cx.t("kernel.error.generic") {
        if lower.contains("git failed to start") || lower.contains("failed to execute") {
            return forja_kernel::HumanError::with_detail(cx.t("git.error.exec"), trimmed);
        }
        return forja_kernel::HumanError::with_detail(cx.t("git.error.generic"), trimmed);
    }
    human
}

pub(crate) fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
    use std::sync::{Arc, Mutex};

    struct CommandProbe {
        module_id: &'static str,
        command_id: &'static str,
        calls: Arc<Mutex<Vec<Value>>>,
    }

    impl Module for CommandProbe {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new(self.module_id, "git.name", "0.0.0")
        }

        fn register(&mut self, registrar: &mut Registrar<'_>) {
            let calls = self.calls.clone();
            registrar.command(self.command_id, "git.command.commit", move |_cx, args| {
                calls.lock().expect("probe calls").push(args.into_json());
                Ok(json!({}))
            });
        }
    }

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
        {
            let cx = runtime.ctx("git");
            let msg = humanize_error(&cx, "fatal: not a git repository (or any parent)");
            assert!(msg.summary.contains("repositório Git"), "{}", msg.summary);
            assert!(!msg.summary.contains("fatal:"), "{}", msg.summary);
            assert_eq!(
                msg.detail.as_deref(),
                Some("fatal: not a git repository (or any parent)")
            );
            let upstream = humanize_error(
                &cx,
                "git pull --ff-only failed: There is no tracking information for the current branch.",
            );
            assert!(upstream.summary.contains("branch remoto"));
            assert!(!upstream.summary.contains("failed"));
            let dir_err = humanize_error(
                &cx,
                "could not run git: The directory name is invalid. (os error 267)",
            );
            assert!(!dir_err.summary.contains("os error"));
            assert!(!dir_err.summary.to_lowercase().contains("failed"));
            assert_eq!(
                dir_err.detail.as_deref(),
                Some("could not run git: The directory name is invalid. (os error 267)")
            );
            assert!(humanize_error(&cx, "no-remote")
                .summary
                .contains("remote configurado"));
            assert_eq!(
                humanize_error(&cx, "no file selected").summary,
                "Selecione ao menos um arquivo."
            );
            let no_stage = humanize_error(&cx, "nothing-staged");
            assert!(
                no_stage.summary.contains("Use Preparar tudo"),
                "{}",
                no_stage.summary
            );
            assert!(no_stage.detail.is_none());
            let untracked_only = humanize_error(
                &cx,
                "git commit failed: nothing added to commit but untracked files present",
            );
            assert_eq!(untracked_only.summary, no_stage.summary);
            assert!(untracked_only.detail.is_none());
        }
        runtime.set_locale(forja_kernel::Locale::En);
        let cx = runtime.ctx("git");
        let en_err = humanize_error(&cx, "git status failed: some technical issue");
        assert!(!en_err.summary.to_lowercase().contains("failed"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn clicking_a_changed_file_opens_its_diff_tab() {
        let dir = std::env::temp_dir().join(format!("forja-git-open-diff-{}", std::process::id()));
        let diff_calls = Arc::new(Mutex::new(Vec::new()));
        let shell_calls = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(crate::GitModule {
                    state: Shared::new(),
                }) as Box<dyn Module>,
                Box::new(CommandProbe {
                    module_id: "diff",
                    command_id: "diff.open",
                    calls: diff_calls.clone(),
                }),
                Box::new(CommandProbe {
                    module_id: "shell",
                    command_id: "shell.tab.open",
                    calls: shell_calls.clone(),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime");

        let mut cx = runtime.ctx("git");
        open_file_diff(&mut cx, "C:/repo", "src/main.rs", true);
        drop(cx);

        assert_eq!(
            diff_calls.lock().expect("diff calls").as_slice(),
            &[json!({"path": "C:/repo", "file": "src/main.rs", "staged": true})]
        );
        assert_eq!(
            shell_calls.lock().expect("shell calls").as_slice(),
            &[json!({"view_id": "diff.tab"})]
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
