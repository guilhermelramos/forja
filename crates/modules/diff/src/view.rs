//! Diff tab: file list, side-by-side/unified/combined rows and review notes.

use crate::images;
use crate::model::{self, Row, RowKind};
use crate::state::{Mode, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub struct DiffView {
    state: Shared,
    mode: Mode,
    initialized: bool,
    ignore_whitespace: bool,
    collapse: bool,
    inline_notes: bool,
    rows: Vec<Row>,
    rows_version: u64,
    bootstrapped: bool,
    images: Option<ImagePair>,
    image_slot: Arc<Mutex<Option<ImagePair>>>,
    image_loading: bool,
    image_key: Option<String>,
    scroll_target: Option<usize>,
}

struct ImagePair {
    old: Option<egui::TextureHandle>,
    new: Option<egui::TextureHandle>,
    old_size: (u32, u32),
    new_size: (u32, u32),
    old_bytes: usize,
    new_bytes: usize,
}

impl DiffView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            mode: Mode::Split,
            initialized: false,
            ignore_whitespace: false,
            collapse: true,
            inline_notes: true,
            rows: Vec::new(),
            rows_version: u64::MAX,
            bootstrapped: false,
            images: None,
            image_slot: Arc::new(Mutex::new(None)),
            image_loading: false,
            image_key: None,
            scroll_target: None,
        }
    }
}

impl View for DiffView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.bootstrap(cx);
        if !self.initialized {
            self.initialized = true;
            let setting = cx
                .setting("diff.view.mode")
                .and_then(SettingValue::as_str)
                .unwrap_or("split");
            self.mode = Mode::from_setting(setting);
            if let Ok(mode) = std::env::var("FORJA_DIFF_MODE") {
                self.mode = Mode::from_setting(&mode);
            }
            self.ignore_whitespace = setting_bool(cx, "diff.whitespace.ignore", false);
            self.collapse = setting_bool(cx, "diff.view.collapse_unchanged", true);
            self.inline_notes = setting_bool(cx, "diff.notes.inline", true);
        }
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        self.toolbar(ui, cx, muted);
        if let Some(status) = self.state.status() {
            crate::view_error::banner(ui, cx, &status);
        }
        if let Some(notice) = self.state.notice() {
            forja_ui_kit::text(ui,
                egui::RichText::new(notice)
                    .small()
                    .color(c32(tokens.color.ok)),
            );
        }
        if self.state.stale() {
            let _ = cx.run_command("diff.refresh", Args::new());
        }
        ui.separator();
        ui.horizontal_top(|ui| {
            self.file_list(ui, cx, muted);
            ui.separator();
            self.detail(ui, cx, muted);
        });
    }
}

impl DiffView {
    fn bootstrap(&mut self, cx: &mut Ctx<'_>) {
        if self.bootstrapped {
            return;
        }
        self.bootstrapped = true;
        if let Ok(file) = std::env::var("FORJA_DIFF_FILE") {
            if !file.trim().is_empty() {
                let mut args = Args::new();
                args.insert("file", json!(file));
                if let Ok(staged) = std::env::var("FORJA_DIFF_STAGED") {
                    args.insert("staged", json!(staged == "1"));
                }
                let _ = cx.run_command("diff.open", args);
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        ui.horizontal_wrapped(|ui| {
            if let Some(comparison) = self.state.comparison() {
                forja_ui_kit::text(ui,
                    egui::RichText::new(format!(
                        "{}: {comparison}",
                        cx.t("diff.comparison.heading")
                    ))
                    .small()
                    .color(muted),
                );
            }
            for mode in [Mode::Split, Mode::Unified, Mode::Combined] {
                let selected = self.mode == mode;
                if ui
                    .selectable_label(selected, cx.t(mode.label_key()))
                    .clicked()
                {
                    self.mode = mode;
                }
            }
            ui.separator();
            let before = (self.ignore_whitespace, self.collapse);
            let whitespace = cx.t("diff.toolbar.whitespace");
            ui.checkbox(&mut self.ignore_whitespace, whitespace);
            let collapse = cx.t("diff.toolbar.collapse");
            ui.checkbox(&mut self.collapse, collapse);
            if before != (self.ignore_whitespace, self.collapse) {
                self.rows_version = u64::MAX;
            }
            if forja_ui_kit::button_with_icon(
                ui,
                forja_ui_kit::Icon::Refresh,
                &cx.t("diff.toolbar.refresh"),
                forja_ui_kit::ButtonKind::Ghost,
            )
            .clicked()
            {
                let _ = cx.run_command("diff.refresh", Args::new());
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("diff.toolbar.previous-change"),
            )
            .clicked()
            {
                self.scroll_target = previous_change(&self.rows, self.scroll_target);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("diff.toolbar.next-change"),
            )
            .clicked()
            {
                self.scroll_target = next_change(&self.rows, self.scroll_target);
            }
        });
        ui.horizontal_wrapped(|ui| {
            let count = self.state.notes().len();
            if count > 0 {
                forja_ui_kit::text(ui,
                    egui::RichText::new(
                        cx.t_args("diff.notes.count", &[("count", &count.to_string())]),
                    )
                    .small()
                    .color(muted),
                );
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("diff.notes.send"),
            )
            .clicked()
            {
                let _ = cx.run_command("diff.send_notes", Args::new());
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("diff.notes.copy"),
            )
            .clicked()
            {
                let heading = cx.t("diff.notes.heading");
                let text = model::format_notes(&self.state.notes(), &heading);
                ui.ctx().copy_text(text);
            }
            if ui
                .add_enabled(
                    !self.state.notes().is_empty(),
                    egui::Button::new(cx.t("diff.notes.clear")),
                )
                .clicked()
            {
                self.state.clear_notes();
            }
        });
    }

    fn file_list(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        let changes = self.state.with(|state| state.changes.clone()).flatten();
        let Some(changes) = changes else {
            ui.vertical(|ui| {
                ui.set_width(220.0);
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("diff.empty")).small().color(muted));
            });
            return;
        };
        let filter = self
            .state
            .with(|state| state.filter.clone())
            .unwrap_or_default();
        let mut filter_text = filter.clone();
        ui.vertical(|ui| {
            ui.set_width(240.0);
            ui.add(
                egui::TextEdit::singleline(&mut filter_text)
                    .hint_text(cx.t("diff.filter.hint"))
                    .desired_width(220.0),
            );
            self.state.with(|state| state.filter = filter_text.clone());
            let filter = filter_text.to_ascii_lowercase();
            let selected = self.state.with(|state| state.selected.clone()).flatten();
            let bench = self.state.bench().unwrap_or_default();
            let mut grouped: BTreeMap<String, Vec<&model::GitFile>> = BTreeMap::new();
            for file in changes.files.iter().filter(|file| {
                filter.is_empty() || file.path.to_ascii_lowercase().contains(&filter)
            }) {
                let directory = std::path::Path::new(&file.path)
                    .parent()
                    .and_then(|parent| parent.to_str())
                    .unwrap_or("")
                    .replace('\\', "/");
                grouped.entry(directory).or_default().push(file);
            }
            egui::ScrollArea::vertical()
                .id_salt("diff-files-list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (directory, files) in grouped {
                        if !directory.is_empty() {
                            forja_ui_kit::text(ui, egui::RichText::new(directory).small().color(muted));
                        }
                        for file in files {
                            let name = std::path::Path::new(&file.path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or(&file.path);
                            let status = status_letters(file);
                            let selected_here = selected.as_deref() == Some(file.path.as_str());
                            let label = format!("{status}  {name}");
                            if ui.selectable_label(selected_here, label).clicked() {
                                let mut args = Args::new();
                                args.insert("path", json!(bench));
                                args.insert("file", json!(file.path));
                                args.insert("staged", json!(!file.unstaged && file.staged));
                                if let Some(comparison) = self.state.comparison() {
                                    args.insert("comparison", json!(comparison));
                                }
                                let _ = cx.run_command("diff.open", args);
                            }
                        }
                    }
                });
        });
    }

    fn detail(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        let Some(diff) = self.state.with(|state| state.diff.clone()).flatten() else {
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("diff.select-file"))
                    .color(muted)
                    .italics(),
            );
            return;
        };
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                forja_ui_kit::text(ui, egui::RichText::new(&diff.path).strong());
                if diff.staged {
                    forja_ui_kit::text(ui,
                        egui::RichText::new(cx.t("diff.staged"))
                            .small()
                            .color(muted),
                    );
                }
            });
            if diff.binary {
                if images::is_image(&diff.path) {
                    if self.image_key.as_deref() != Some(diff.path.as_str()) && !self.image_loading
                    {
                        self.start_image(ui, cx, &diff);
                    }
                    self.image_area(ui, cx, muted);
                } else {
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("diff.binary")).color(muted));
                }
                return;
            }
            let version = self.state.version();
            if self.rows_version != version {
                self.rows_version = version;
                let context = setting_usize(cx, "diff.context.lines", 3);
                self.rows = model::rows(&diff, context, self.ignore_whitespace, self.collapse);
            }
            let notes = self.state.notes_for(&diff.path);
            let rows = std::mem::take(&mut self.rows);
            egui::ScrollArea::both()
                .id_salt("diff-rows")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (index, row) in rows.iter().enumerate() {
                        let response = ui.scope(|ui| {
                            self.row(ui, cx, &diff.path, row, &notes, muted);
                        });
                        if self.scroll_target == Some(index) {
                            ui.scroll_to_rect(response.response.rect, Some(egui::Align::TOP));
                            self.scroll_target = None;
                        }
                    }
                });
            self.rows = rows;
        });
    }

    fn row(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        path: &str,
        row: &Row,
        notes: &[model::Note],
        muted: egui::Color32,
    ) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let danger = c32(tokens.color.danger);
        let ok = c32(tokens.color.ok);
        match row.kind {
            RowKind::Header => {
                forja_ui_kit::text(ui,
                    egui::RichText::new(&row.header)
                        .monospace()
                        .small()
                        .color(c32(tokens.color.info)),
                );
            }
            RowKind::Gap => {
                forja_ui_kit::text(ui,
                    egui::RichText::new(
                        cx.t_args("diff.collapsed", &[("count", &row.skipped.to_string())]),
                    )
                    .small()
                    .color(muted),
                );
            }
            RowKind::Context => match self.mode {
                Mode::Split => {
                    split_line(ui, row.old.as_ref(), row.new.as_ref(), muted, muted);
                }
                Mode::Unified => {
                    let text = row
                        .new
                        .as_ref()
                        .map(|cell| format!(" {:>4}   {}", cell.number, cell.text))
                        .unwrap_or_default();
                    forja_ui_kit::text(ui, egui::RichText::new(text).monospace().small().color(muted));
                }
                Mode::Combined => {
                    forja_ui_kit::text(ui,
                        egui::RichText::new(
                            row.new
                                .as_ref()
                                .map(|cell| format!(" {:>4}   {}", cell.number, cell.text))
                                .unwrap_or_default(),
                        )
                        .monospace()
                        .small()
                        .color(muted),
                    );
                }
            },
            RowKind::Change => {
                if self.mode == Mode::Unified {
                    if let Some(old) = &row.old {
                        forja_ui_kit::text(ui,
                            egui::RichText::new(format!("-{:>4}   {}", old.number, old.text))
                                .monospace()
                                .small()
                                .color(danger),
                        );
                    }
                    if let Some(new) = &row.new {
                        forja_ui_kit::text(ui,
                            egui::RichText::new(format!("+{:>4}   {}", new.number, new.text))
                                .monospace()
                                .small()
                                .color(ok),
                        );
                    }
                } else if self.mode == Mode::Split {
                    split_line(ui, row.old.as_ref(), row.new.as_ref(), danger, ok);
                } else {
                    combined_line(ui, row.old.as_ref(), row.new.as_ref(), danger, ok);
                }
            }
        }
        if row.kind == RowKind::Change && self.inline_notes {
            if let Some(line) = row.note_line() {
                for note in notes.iter().filter(|note| note.line == line) {
                    ui.horizontal(|ui| {
                        ui.add_space(forja_ui_kit::Spacing::XL);
                        forja_ui_kit::text(ui,
                            egui::RichText::new(format!("{}: {}", line, note.text))
                                .small()
                                .color(c32(tokens.color.spark)),
                        );
                    });
                }
                let selected = self.state.with(|state| state.note_line) == Some(Some(line));
                ui.horizontal(|ui| {
                    ui.add_space(forja_ui_kit::Spacing::XL);
                    let label = cx.t("diff.notes.add");
                    if ui.small_button(label).clicked() {
                        self.state.with(|state| {
                            state.note_line = if state.note_line == Some(line) {
                                None
                            } else {
                                Some(line)
                            };
                        });
                    }
                    if selected {
                        let mut draft = self
                            .state
                            .with(|state| state.note_draft.clone())
                            .unwrap_or_default();
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut draft)
                                .hint_text(cx.t("diff.notes.hint"))
                                .desired_width(360.0),
                        );
                        let enter = response.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter));
                        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cx.t("diff.notes.confirm")).clicked() || enter {
                            self.state.add_note(path, line, &draft);
                        } else {
                            self.state.with(|state| state.note_draft = draft);
                        }
                    }
                });
            }
        }
    }

    fn start_image(&mut self, ui: &egui::Ui, cx: &mut Ctx<'_>, diff: &model::FileDiff) {
        let bench = self.state.bench().unwrap_or_default();
        let file = diff.path.clone();
        let staged = diff.staged;
        let comparison_base = self
            .state
            .comparison()
            .as_deref()
            .and_then(crate::commands::comparison_base)
            .map(str::to_owned);
        let context = ui.ctx().clone();
        let slot = Arc::clone(&self.image_slot);
        self.images = None;
        self.image_loading = true;
        self.image_key = Some(file.clone());
        cx.spawn(
            move || match comparison_base {
                Some(base) => images::load_comparison(&bench, &file, &base),
                None => images::load(&bench, &file, staged),
            },
            move |_cx, pair| {
                let old = pair.old.as_ref().map(|image| {
                    context.load_texture("diff-old", image.clone(), egui::TextureOptions::LINEAR)
                });
                let new = pair.new.as_ref().map(|image| {
                    context.load_texture("diff-new", image.clone(), egui::TextureOptions::LINEAR)
                });
                let old_size = pair
                    .old
                    .as_ref()
                    .map(|image| (image.size[0] as u32, image.size[1] as u32))
                    .unwrap_or((0, 0));
                let new_size = pair
                    .new
                    .as_ref()
                    .map(|image| (image.size[0] as u32, image.size[1] as u32))
                    .unwrap_or((0, 0));
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(ImagePair {
                        old,
                        new,
                        old_size,
                        new_size,
                        old_bytes: pair.old_bytes,
                        new_bytes: pair.new_bytes,
                    });
                }
            },
        );
    }

    fn image_area(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        if self.images.is_none() {
            if let Ok(mut slot) = self.image_slot.lock() {
                self.images = slot.take();
            }
        }
        if self.images.is_some() {
            self.image_loading = false;
        }
        let Some(pair) = &self.images else {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("diff.image.loading")).color(muted));
            return;
        };
        ui.horizontal_top(|ui| {
            image_cell(ui, cx, &pair.old, pair.old_size, pair.old_bytes, muted);
            ui.separator();
            image_cell(ui, cx, &pair.new, pair.new_size, pair.new_bytes, muted);
        });
    }
}

fn image_cell(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    texture: &Option<egui::TextureHandle>,
    size: (u32, u32),
    bytes: usize,
    muted: egui::Color32,
) {
    ui.vertical(|ui| {
        if let Some(texture) = texture {
            let scale = (240.0 / texture.size_vec2().x.max(1.0)).min(1.0);
            ui.add(egui::Image::new((
                texture.id(),
                texture.size_vec2() * scale,
            )));
        } else {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("diff.image.missing")).color(muted));
        }
        forja_ui_kit::text(ui,
            egui::RichText::new(format!("{} x {} · {} B", size.0, size.1, bytes))
                .small()
                .color(muted),
        );
    });
}

fn split_line(
    ui: &mut egui::Ui,
    old: Option<&model::Cell>,
    new: Option<&model::Cell>,
    old_color: egui::Color32,
    new_color: egui::Color32,
) {
    let muted = c32(forja_ui_kit::applied_tokens(ui.ctx()).color.text_muted);
    ui.columns(2, |columns| {
        cell_text(&mut columns[0], old, old_color, muted);
        cell_text(&mut columns[1], new, new_color, muted);
    });
}

fn cell_text(
    ui: &mut egui::Ui,
    cell: Option<&model::Cell>,
    color: egui::Color32,
    muted: egui::Color32,
) {
    let text = cell
        .map(|cell| format!("{:>4}  {}", cell.number, cell.text))
        .unwrap_or_default();
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .monospace()
                .small()
                .color(if cell.is_some() { color } else { muted }),
        )
        .wrap(),
    );
}

fn combined_line(
    ui: &mut egui::Ui,
    old: Option<&model::Cell>,
    new: Option<&model::Cell>,
    old_color: egui::Color32,
    new_color: egui::Color32,
) {
    ui.horizontal_wrapped(|ui| {
        if let Some(old) = old {
            forja_ui_kit::text(ui,
                egui::RichText::new(format!("-{:>4}  {}", old.number, old.text))
                    .monospace()
                    .small()
                    .color(old_color),
            );
        }
        if old.is_some() && new.is_some() {
            forja_ui_kit::text(ui, egui::RichText::new("\u{2192}").small());
        }
        if let Some(new) = new {
            forja_ui_kit::text(ui,
                egui::RichText::new(format!("+{:>4}  {}", new.number, new.text))
                    .monospace()
                    .small()
                    .color(new_color),
            );
        }
    });
}

fn next_change(rows: &[Row], current: Option<usize>) -> Option<usize> {
    let start = current.map(|index| index + 1).unwrap_or(0);
    rows.iter()
        .enumerate()
        .skip(start)
        .find(|(_, row)| row.kind == RowKind::Change)
        .map(|(index, _)| index)
        .or(current)
}

fn previous_change(rows: &[Row], current: Option<usize>) -> Option<usize> {
    let end = current.unwrap_or(rows.len());
    rows.iter()
        .enumerate()
        .take(end)
        .rev()
        .find(|(_, row)| row.kind == RowKind::Change)
        .map(|(index, _)| index)
        .or(current)
}

fn status_letters(file: &model::GitFile) -> String {
    if file.comparison {
        return "Δ".to_owned();
    }
    let mut letters = String::new();
    letters.push(if file.staged {
        file.index.to_ascii_uppercase()
    } else {
        '.'
    });
    letters.push(if file.unstaged || file.conflict {
        file.worktree.to_ascii_uppercase()
    } else {
        '.'
    });
    if file.untracked {
        letters.push('U');
    }
    letters
}

fn setting_bool(cx: &Ctx<'_>, key: &str, fallback: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(fallback)
}

fn setting_usize(cx: &Ctx<'_>, key: &str, fallback: usize) -> usize {
    cx.setting(key)
        .and_then(SettingValue::as_i64)
        .map(|value| value.clamp(0, 20) as usize)
        .unwrap_or(fallback)
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}

