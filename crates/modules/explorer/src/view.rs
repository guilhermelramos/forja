//! The Files panel: toolbar, status line, tree, context menu, and dialogs.
//!
//! The view draws from the cache and collects what the user asked for into
//! [`Request`]s. The requests run after drawing, so the state lock is never held
//! while a command runs.

use crate::commands;
use crate::dialogs::{confirm_trash_window, name_window, new_entry_window, Outcome};
use crate::state::{lock, Dialog, Shared};
use crate::tree::Entry;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, Result as KResult};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

/// Indentation per tree level, in points.
const INDENT: f32 = 14.0;

pub struct ExplorerView {
    pub shared: Shared,
}

/// Something the user asked for during a frame.
enum Request {
    Toggle(PathBuf),
    Select(PathBuf),
    Open(PathBuf),
    Load(PathBuf),
    NewEntry { dir: PathBuf, is_dir: bool },
    Trash(PathBuf),
    Copy { path: PathBuf, relative: bool },
    Command(&'static str, PathBuf),
    Undo,
    Redo,
    Refresh,
}

/// Snapshot of the state the frame needs, taken once under the lock.
struct Frame {
    root: Option<PathBuf>,
    bench_name: Option<String>,
    notice: Option<String>,
    can_undo: bool,
    can_redo: bool,
    busy: bool,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    changed: BTreeSet<String>,
}

impl View for ExplorerView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        commands::sync(cx, &self.shared);

        let (frame, pending_copy) = {
            let mut state = lock(&self.shared);
            let frame = Frame {
                root: state.tree.root().map(Path::to_path_buf),
                bench_name: state.bench_name.clone(),
                notice: state.notice.clone(),
                can_undo: state.history.can_undo(),
                can_redo: state.history.can_redo(),
                busy: state.busy,
                expanded: state.expanded.clone(),
                selected: state.selected.clone(),
                changed: state.changed_paths.clone(),
            };
            (frame, state.pending_copy.take())
        };
        if let Some(text) = pending_copy {
            ui.ctx().copy_text(text);
        }

        let mut requests = Vec::new();
        // O título do painel já está na aba: não repetimos aqui.
        match frame.root.clone() {
            None => {
                forja_ui_kit::empty_state(
                    ui,
                    forja_ui_kit::BOARD,
                    &cx.t("explorer.panel.empty"),
                    None,
                );
            }
            Some(root) => {
                self.toolbar(ui, cx, &root, &frame, &mut requests);
                if let Some(name) = &frame.bench_name {
                    forja_ui_kit::text(ui, egui::RichText::new(name).strong());
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(
                            forja_kernel::paths::clean_path(&root).display().to_string(),
                        )
                        .weak(),
                    )
                    .truncate(),
                );
                render_notice(ui, cx, &self.shared, frame.notice.as_deref());
                ui.separator();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.render_dir(ui, cx, &root, 0, &frame, &mut requests);
                    });
            }
        }
        if frame.root.is_none() {
            render_notice(ui, cx, &self.shared, frame.notice.as_deref());
        }

        self.dialogs(ui.ctx(), cx);
        for request in requests {
            self.run(cx, request);
        }
    }
}

fn render_notice(ui: &mut egui::Ui, cx: &Ctx<'_>, shared: &Shared, notice: Option<&str>) {
    let Some(notice) = notice else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        forja_ui_kit::text(ui, egui::RichText::new(notice).weak());
        if forja_ui_kit::icon_button(ui, forja_ui_kit::CLOSE, &cx.t("explorer.notice.dismiss"))
            .on_hover_text(cx.t("explorer.notice.dismiss"))
            .clicked()
        {
            crate::state::lock(shared).notice = None;
        }
    });
}

impl ExplorerView {
    fn toolbar(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        root: &Path,
        frame: &Frame,
        requests: &mut Vec<Request>,
    ) {
        let enabled = !frame.busy;
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(enabled, |ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("explorer.command.new_file"),
                )
                .clicked()
                    && enabled
                {
                    requests.push(Request::NewEntry {
                        dir: root.to_path_buf(),
                        is_dir: false,
                    });
                }
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("explorer.command.new_folder"),
                )
                .clicked()
                    && enabled
                {
                    requests.push(Request::NewEntry {
                        dir: root.to_path_buf(),
                        is_dir: true,
                    });
                }
            });
            let can_undo = enabled && frame.can_undo;
            ui.add_enabled_ui(can_undo, |ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("explorer.command.undo"),
                )
                .clicked()
                    && can_undo
                {
                    requests.push(Request::Undo);
                }
            });
            let can_redo = enabled && frame.can_redo;
            ui.add_enabled_ui(can_redo, |ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("explorer.command.redo"),
                )
                .clicked()
                    && can_redo
                {
                    requests.push(Request::Redo);
                }
            });
            if forja_ui_kit::button_with_icon(
                ui,
                forja_ui_kit::Icon::Refresh,
                &cx.t("explorer.command.refresh"),
                forja_ui_kit::ButtonKind::Ghost,
            )
            .clicked()
            {
                requests.push(Request::Refresh);
            }
        });
    }

    fn render_dir(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        dir: &Path,
        depth: usize,
        frame: &Frame,
        requests: &mut Vec<Request>,
    ) {
        let children = lock(&self.shared).tree.children(dir);
        let Some(children) = children else {
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * INDENT);
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("explorer.panel.loading")).weak());
            });
            requests.push(Request::Load(dir.to_path_buf()));
            return;
        };
        if children.is_empty() && depth == 0 {
            forja_ui_kit::empty_state(
                ui,
                forja_ui_kit::ARCHIVE,
                &cx.t("explorer.panel.empty_folder"),
                None,
            );
        }
        for entry in children.iter() {
            self.render_entry(ui, cx, entry, depth, frame, requests);
            if entry.is_dir && frame.expanded.contains(&entry.path) {
                self.render_dir(ui, cx, &entry.path, depth + 1, frame, requests);
            }
        }
    }

    fn render_entry(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        entry: &Entry,
        depth: usize,
        frame: &Frame,
        requests: &mut Vec<Request>,
    ) {
        let is_selected = frame.selected.as_deref() == Some(entry.path.as_path());
        let parent = entry
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| entry.path.clone());
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * INDENT);
            if entry.is_dir {
                let open = frame.expanded.contains(&entry.path);
                if forja_ui_kit::icon_button(
                    ui,
                    if open {
                        forja_ui_kit::Icon::ChevronDown
                    } else {
                        forja_ui_kit::Icon::ChevronRight
                    },
                    &cx.t(if open {
                        "explorer.tree.collapse"
                    } else {
                        "explorer.tree.expand"
                    }),
                )
                .clicked()
                {
                    requests.push(Request::Toggle(entry.path.clone()));
                }
            } else {
                ui.add_space(INDENT + 6.0);
            }
            let glyph = if entry.is_dir {
                forja_ui_kit::Icon::Folder
            } else {
                forja_ui_kit::Icon::for_extension(
                    entry
                        .path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or(""),
                )
            };
            forja_ui_kit::icon(ui, glyph, 16.0);
            let mut text = egui::RichText::new(&entry.name);
            if entry.ignored {
                text = text.weak();
            }
            let response = ui.add(egui::Button::selectable(is_selected, text).truncate());
            crate::changed::render(
                ui,
                cx,
                frame.root.as_deref().is_some_and(|root| {
                    crate::changed::is_changed(root, &entry.path, &frame.changed)
                }),
            );
            if response.clicked() {
                requests.push(Request::Select(entry.path.clone()));
                if entry.is_dir {
                    requests.push(Request::Toggle(entry.path.clone()));
                } else {
                    requests.push(Request::Open(entry.path.clone()));
                }
            }
            response.context_menu(|ui| {
                if !entry.is_dir && forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.menu.open")).clicked() {
                    requests.push(Request::Open(entry.path.clone()));
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.new_file")).clicked() {
                    requests.push(Request::NewEntry {
                        dir: if entry.is_dir {
                            entry.path.clone()
                        } else {
                            parent.clone()
                        },
                        is_dir: false,
                    });
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.new_folder")).clicked() {
                    requests.push(Request::NewEntry {
                        dir: if entry.is_dir {
                            entry.path.clone()
                        } else {
                            parent.clone()
                        },
                        is_dir: true,
                    });
                    ui.close();
                }
                ui.separator();
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.rename")).clicked() {
                    requests.push(Request::Command("explorer.rename", entry.path.clone()));
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.duplicate")).clicked() {
                    requests.push(Request::Command("explorer.duplicate", entry.path.clone()));
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.trash")).clicked() {
                    requests.push(Request::Trash(entry.path.clone()));
                    ui.close();
                }
                ui.separator();
                if ui
                    .button(cx.t("explorer.command.copy_absolute_path"))
                    .clicked()
                {
                    requests.push(Request::Copy {
                        path: entry.path.clone(),
                        relative: false,
                    });
                    ui.close();
                }
                if ui
                    .button(cx.t("explorer.command.copy_relative_path"))
                    .clicked()
                {
                    requests.push(Request::Copy {
                        path: entry.path.clone(),
                        relative: true,
                    });
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.reveal")).clicked() {
                    requests.push(Request::Command("explorer.reveal", entry.path.clone()));
                    ui.close();
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("explorer.command.open_terminal")).clicked() {
                    let dir = if entry.is_dir {
                        entry.path.clone()
                    } else {
                        parent.clone()
                    };
                    requests.push(Request::Command("explorer.open_terminal", dir));
                    ui.close();
                }
            });
        });
    }

    fn dialogs(&self, ctx: &egui::Context, cx: &mut Ctx<'_>) {
        let Some(dialog) = lock(&self.shared).dialog.take() else {
            ctx.data_mut(|data| {
                data.remove_temp::<bool>(egui::Id::new("forja.explorer.new_entry.focus"));
                data.remove_temp::<bool>(egui::Id::new("forja.explorer.rename.focus"));
            });
            return;
        };
        match dialog {
            Dialog::NewEntry {
                dir,
                is_dir,
                mut name,
                mut template,
            } => {
                let title = if is_dir {
                    cx.t("explorer.dialog.new_folder.title")
                } else {
                    cx.t("explorer.dialog.new_file.title")
                };
                let templates = if is_dir { Vec::new() } else { templates(cx) };
                let outcome =
                    new_entry_window(ctx, cx, &title, &mut name, &templates, &mut template);
                match outcome {
                    Outcome::Open => {
                        lock(&self.shared).dialog = Some(Dialog::NewEntry {
                            dir,
                            is_dir,
                            name,
                            template,
                        });
                    }
                    Outcome::Cancel => {}
                    Outcome::Submit => {
                        let result = commands::new_entry(
                            cx,
                            &self.shared,
                            dir.clone(),
                            Some(name.clone()),
                            is_dir,
                            template.clone(),
                        );
                        self.keep_dialog_on_error(
                            result,
                            Dialog::NewEntry {
                                dir,
                                is_dir,
                                name,
                                template,
                            },
                        );
                    }
                }
            }
            Dialog::Rename { path, mut name } => {
                let title = cx.t("explorer.dialog.rename.title");
                let outcome = name_window(ctx, cx, &title, &mut name);
                match outcome {
                    Outcome::Open => {
                        lock(&self.shared).dialog = Some(Dialog::Rename { path, name });
                    }
                    Outcome::Cancel => {}
                    Outcome::Submit => {
                        let result =
                            commands::rename(cx, &self.shared, path.clone(), Some(name.clone()));
                        self.keep_dialog_on_error(result, Dialog::Rename { path, name });
                    }
                }
            }
            Dialog::ConfirmTrash { path } => match confirm_trash_window(ctx, cx, &path) {
                Some(true) => {
                    let result = commands::trash(cx, &self.shared, path);
                    report(&self.shared, result);
                }
                Some(false) => {}
                None => lock(&self.shared).dialog = Some(Dialog::ConfirmTrash { path }),
            },
        }
    }

    /// Reopens the dialog with the typed name when the name was refused.
    fn keep_dialog_on_error(&self, result: KResult<Value>, dialog: Dialog) {
        match result {
            Ok(_) => {}
            Err(error) => {
                let mut state = lock(&self.shared);
                state.notice = Some(error.to_string());
                state.dialog = Some(dialog);
            }
        }
    }

    fn run(&self, cx: &mut Ctx<'_>, request: Request) {
        match request {
            Request::Toggle(path) => {
                let opening = {
                    let mut state = lock(&self.shared);
                    if state.expanded.remove(&path) {
                        false
                    } else {
                        state.expanded.insert(path.clone());
                        true
                    }
                };
                if opening {
                    commands::request_children(cx, &self.shared, &path);
                }
            }
            Request::Select(path) => lock(&self.shared).selected = Some(path),
            Request::Open(path) => commands::open_file(cx, &self.shared, path, None),
            Request::Load(path) => commands::request_children(cx, &self.shared, &path),
            Request::NewEntry { dir, is_dir } => {
                let result = commands::new_entry(cx, &self.shared, dir, None, is_dir, None);
                report(&self.shared, result);
            }
            Request::Trash(path) => lock(&self.shared).dialog = Some(Dialog::ConfirmTrash { path }),
            Request::Copy { path, relative } => {
                let result = commands::copy_path(cx, &self.shared, &path, relative);
                report(&self.shared, result);
            }
            Request::Command(id, path) => {
                let mut args = Args::new();
                args.insert("path", json!(path.display().to_string()));
                let result = cx.run_command(id, args);
                report(&self.shared, result);
            }
            Request::Undo => {
                let result = commands::undo(cx, &self.shared);
                report(&self.shared, result);
            }
            Request::Redo => {
                let result = commands::redo(cx, &self.shared);
                report(&self.shared, result);
            }
            Request::Refresh => {
                let result = commands::refresh(&self.shared);
                report(&self.shared, result);
            }
        }
    }
}

/// Shows an error from a command in the status line.
fn report(shared: &Shared, result: KResult<Value>) {
    if let Err(error) = result {
        lock(shared).notice = Some(error.to_string());
    }
}

/// Modelos de arquivo oferecidos pelo módulo `content`.
fn templates(cx: &mut Ctx<'_>) -> Vec<(String, String)> {
    cx.run_command("content.templates.list", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("templates")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            let id = item.get("id").and_then(Value::as_str)?;
                            let name = item.get("name").and_then(Value::as_str).unwrap_or(id);
                            Some((id.to_owned(), name.to_owned()))
                        })
                        .collect()
                })
        })
        .unwrap_or_default()
}
