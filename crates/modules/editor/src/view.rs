//! Editor tab: file tabs, highlighted text, find/replace, preview and compare.

use crate::edit;
use crate::fonts;
use crate::highlight::HighlightCache;
use crate::model;
use crate::state::{Shared, Tab};
use forja_contracts::ViewerDocument;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

pub struct EditorView {
    state: Shared,
    caches: HashMap<String, HighlightCache>,
    last_find_key: u64,
    last_check: Instant,
    bootstrapped: bool,
    pub(crate) goto_open: bool,
    goto_text: String,
    preview_open: bool,
    text_focused: bool,
    auto_saved: Option<Instant>,
    /// Font choice requested from a task, so the file is read once.
    font_requested: Option<String>,
    font_ready: HashSet<String>,
    font_failed: HashSet<String>,
}

impl EditorView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            caches: HashMap::new(),
            last_find_key: 0,
            last_check: Instant::now(),
            bootstrapped: false,
            goto_open: false,
            goto_text: String::new(),
            preview_open: true,
            text_focused: false,
            auto_saved: None,
            font_requested: None,
            font_ready: HashSet::new(),
            font_failed: HashSet::new(),
        }
    }

    fn bootstrap(&mut self, cx: &mut Ctx<'_>) {
        if self.bootstrapped {
            return;
        }
        self.bootstrapped = true;
        if let Ok(path) = std::env::var("FORJA_EDITOR_FILE") {
            if !path.trim().is_empty() {
                let mut args = Args::new();
                args.insert("path", json!(path));
                let _ = cx.run_command("file.open", args);
            }
        }
        if let Ok(query) = std::env::var("FORJA_EDITOR_FIND") {
            if !query.trim().is_empty() {
                self.state.with(|state| {
                    state.find.open = true;
                    state.find.query = query;
                    state.pending_match = true;
                });
            }
            if let Ok(replace) = std::env::var("FORJA_EDITOR_REPLACE") {
                self.state.with(|state| {
                    state.find.replace_open = true;
                    state.find.replacement = replace;
                });
            }
        }
    }

    /// Installs the font chosen in `editor.font.family` once its file is read
    /// by a task. Missing files keep the monospaced family from the ui-kit.
    fn sync_font(&mut self, ui: &egui::Ui, cx: &mut Ctx<'_>) {
        if let Some(load) = self.state.take_font() {
            match load.bytes {
                Some(bytes) => {
                    if let Some(name) = fonts::family_name(&load.choice) {
                        ui.ctx().add_font(egui::epaint::text::FontInsert::new(
                            &name,
                            egui::FontData::from_owned(bytes),
                            vec![egui::epaint::text::InsertFontFamily {
                                family: egui::FontFamily::Name(name.clone().into()),
                                priority: egui::epaint::text::FontPriority::Highest,
                            }],
                        ));
                        self.font_ready.insert(load.choice);
                    }
                }
                None => {
                    self.font_failed.insert(load.choice);
                }
            }
        }
        let choice = self
            .setting(cx, "editor.font.family")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "inherit".to_owned());
        if choice == "inherit"
            || self.font_ready.contains(&choice)
            || self.font_failed.contains(&choice)
            || self.font_requested.as_deref() == Some(choice.as_str())
        {
            return;
        }
        self.font_requested = Some(choice.clone());
        let state = self.state.clone();
        let requested = choice.clone();
        cx.spawn(
            move || fonts::read(&requested),
            move |_cx, bytes| state.store_font(choice, bytes),
        );
    }

    fn setting(&self, cx: &Ctx<'_>, key: &str) -> Option<forja_kernel::SettingValue> {
        cx.setting(key).cloned()
    }

    fn font_family(&self, cx: &Ctx<'_>) -> Option<String> {
        let choice = self
            .setting(cx, "editor.font.family")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "inherit".to_owned());
        if self.font_ready.contains(&choice) {
            fonts::family_name(&choice)
        } else {
            None
        }
    }
}

impl View for EditorView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.bootstrap(cx);
        self.state.set_ui_context(ui.ctx().clone());
        self.sync_font(ui, cx);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        let danger = c32(tokens.color.danger);
        let accent = c32(tokens.color.accent);

        if self.last_check.elapsed() > Duration::from_secs(2) {
            self.last_check = Instant::now();
            let _ = cx.run_command("editor.check_external", Args::new());
        }

        self.tab_strip(ui, cx, muted, accent);
        if let Some(status) = self.state.status() {
            forja_ui_kit::text(ui, egui::RichText::new(status).small().color(muted));
        }
        let Some(mut tab) = self.state.active_tab() else {
            self.text_focused = false;
            ui.add_space(forja_ui_kit::Spacing::XL);
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("editor.view.empty"))
                    .color(muted)
                    .italics(),
            );
            return;
        };
        if !self.caches.contains_key(&tab.path) {
            self.caches
                .insert(tab.path.clone(), HighlightCache::default());
        }
        self.toolbar(ui, cx, &tab);
        if tab.external_changed {
            self.external_banner(ui, cx, &tab, muted);
        }
        if let Some(notice) = &tab.notice {
            forja_ui_kit::text(ui, egui::RichText::new(notice).small().color(muted));
        }
        ui.separator();
        let mut find = self
            .state
            .with(|state| state.find.clone())
            .unwrap_or_default();
        if find.open {
            self.find_bar(ui, cx, &tab, &mut find, muted, danger);
        }
        if ui.ctx().memory(|memory| memory.top_modal_layer().is_none()) {
            self.handle_keys(ui, cx, &tab, &mut find);
        }
        self.status_line(ui, cx, &tab, &find, muted);
        if tab.preview_mode && self.preview_open {
            self.text_focused = false;
            let document = tab.preview.clone();
            self.preview_area(ui, cx, document.as_ref());
        } else {
            self.text_area(ui, cx, &mut tab, &find, muted);
        }
        self.symbol_windows(ui, cx, &tab, muted);
        self.compare_window(ui, cx, &tab, muted);
        let _ = self.state.with(|state| {
            state.find = find;
        });
    }
}

impl EditorView {
    fn tab_strip(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        muted: egui::Color32,
        accent: egui::Color32,
    ) {
        let tabs = self.state.tabs();
        let active = self.state.active_index();
        ui.horizontal_wrapped(|ui| {
            for (index, tab) in tabs.iter().enumerate() {
                let label = if tab.dirty {
                    format!("{} \u{2022}", tab.title)
                } else {
                    tab.title.clone()
                };
                let selected = index == active;
                let response = ui.selectable_label(
                    selected,
                    if tab.pinned {
                        egui::RichText::new(label).color(if selected { accent } else { muted })
                    } else {
                        egui::RichText::new(label)
                            .color(if selected { accent } else { muted })
                            .italics()
                    },
                );
                if response.clicked() {
                    self.state.set_active(index);
                }
                if forja_ui_kit::icon_button(
                    ui,
                    forja_ui_kit::Icon::Close,
                    &cx.t("editor.tab.close"),
                )
                .on_hover_text(cx.t("editor.tab.close"))
                .clicked()
                {
                    self.state.close(index);
                }
            }
        });
        ui.separator();
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, tab: &Tab) {
        ui.horizontal_wrapped(|ui| {
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Primary,
                &cx.t("editor.action.save"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command("editor.save", args);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("editor.action.find"),
            )
            .clicked()
            {
                let _ = cx.run_command("editor.find", Args::new());
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("editor.action.replace"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("replace", json!(true));
                let _ = cx.run_command("editor.find", args);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("editor.action.goto"),
            )
            .clicked()
            {
                self.goto_open = !self.goto_open;
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("editor.action.format"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command("editor.format", args);
            }
            let wrap = if tab.wrap {
                cx.t("editor.action.wrap-on")
            } else {
                cx.t("editor.action.wrap-off")
            };
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &wrap).clicked() {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command("editor.wrap.toggle", args);
            }
            let preview = if tab.preview_mode {
                cx.t("editor.action.edit")
            } else {
                cx.t("editor.action.preview")
            };
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &preview).clicked() {
                if tab.preview_mode {
                    self.preview_open = false;
                    self.state.set_preview_mode(&tab.path, false);
                } else {
                    self.preview_open = true;
                    let mut args = Args::new();
                    args.insert("path", json!(tab.path));
                    let _ = cx.run_command("editor.preview", args);
                }
            }
        });
        if self.goto_open {
            let mut line = self.goto_text.clone();
            ui.horizontal(|ui| {
                let goto_label = cx.t("editor.goto.label");
                forja_ui_kit::text(ui, goto_label.clone());
                let full_width = ui.available_width();
                ui.set_width(80.0);
                forja_ui_kit::text_field_labeled(ui, &mut line, &goto_label, &goto_label);
                ui.set_width(full_width);
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("editor.action.goto-go"),
                )
                .clicked()
                {
                    if let Ok(line) = line.trim().parse::<usize>() {
                        self.state.with(|state| state.pending_line = Some(line));
                        self.goto_open = false;
                    }
                }
            });
            self.goto_text = line;
        }
    }

    fn external_banner(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        muted: egui::Color32,
    ) {
        ui.horizontal_wrapped(|ui| {
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("editor.banner.external"))
                    .color(c32(forja_ui_kit::applied_tokens(ui.ctx()).color.spark)),
            );
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("editor.banner.reload"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command("editor.reload", args);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("editor.banner.compare"),
            )
            .clicked()
            {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command("editor.compare", args);
            }
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("editor.banner.hint"))
                    .small()
                    .color(muted),
            );
        });
    }

    fn find_bar(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        find: &mut model::FindState,
        muted: egui::Color32,
        danger: egui::Color32,
    ) {
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            let find_hint = cx.t("editor.find.hint");
            let full_width = ui.available_width();
            ui.set_width(220.0);
            changed |=
                forja_ui_kit::text_field_labeled(ui, &mut find.query, &find_hint, &find_hint)
                    .changed();
            ui.set_width(full_width);
            changed |=
                forja_ui_kit::toggle(ui, &mut find.case_sensitive, &cx.t("editor.find.case"))
                    .changed();
            changed |=
                forja_ui_kit::toggle(ui, &mut find.whole_word, &cx.t("editor.find.word"))
                    .changed();
            changed |=
                forja_ui_kit::toggle(ui, &mut find.regex, &cx.t("editor.find.regex")).changed();
            let count = if find.matches.is_empty() {
                "0".to_owned()
            } else {
                format!("{}/{}", find.current + 1, find.matches.len())
            };
            forja_ui_kit::text(ui, egui::RichText::new(count).small().color(muted));
            if forja_ui_kit::icon_button(
                ui,
                forja_ui_kit::Icon::ArrowUp,
                &cx.t("editor.command.find_previous"),
            )
            .clicked()
            {
                find.previous();
                self.select_find_match(ui, tab, find);
            }
            if forja_ui_kit::icon_button(
                ui,
                forja_ui_kit::Icon::ArrowDown,
                &cx.t("editor.command.find_next"),
            )
            .clicked()
            {
                find.next();
                self.select_find_match(ui, tab, find);
            }
            if find.replace_open {
                ui.separator();
                let replace_hint = cx.t("editor.find.replace-hint");
                let full_width = ui.available_width();
                ui.set_width(180.0);
                changed |= forja_ui_kit::text_field_labeled(
                    ui, &mut find.replacement, &replace_hint, &replace_hint).changed();
                ui.set_width(full_width);
                if ui.button(cx.t("editor.find.replace-all")).clicked() {
                    let mut args = Args::new();
                    args.insert("path", json!(tab.path));
                    args.insert("find", json!(find.query));
                    args.insert("replace", json!(find.replacement));
                    let _ = cx.run_command("editor.replace.all", args);
                }
            }
            if forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::Close, &cx.t("editor.find.close"))
                .clicked()
            {
                find.open = false;
                find.replace_open = false;
            }
        });
        if let Some(error) = &find.error {
            forja_ui_kit::text(ui, egui::RichText::new(error).small().color(danger));
        }
        let key = find_key(&tab.path, &tab.content, find);
        if changed || key != self.last_find_key {
            find.refresh(&tab.content);
            self.last_find_key = key;
        }
    }

    fn select_find_match(&self, ui: &egui::Ui, tab: &Tab, find: &model::FindState) {
        let Some((start, end)) = find.matches.get(find.current) else {
            return;
        };
        let start = model::byte_to_char(&tab.content, *start);
        let end = model::byte_to_char(&tab.content, *end);
        let id = editor_id(&tab.path);
        if let Some(mut edit) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), id) {
            edit.cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(start),
                    egui::text::CCursor::new(end),
                )));
            edit.store(ui.ctx(), id);
            ui.memory_mut(|memory| memory.request_focus(id));
        }
    }

    fn handle_keys(
        &mut self,
        ui: &egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        find_state: &mut model::FindState,
    ) {
        let mut save = false;
        let mut find = false;
        let mut replace = false;
        let mut goto = false;
        let mut wrap = false;
        let mut delete_line = false;
        let mut zoom = 0i64;
        let mut undo = false;
        let mut redo = false;
        let mut cut = false;
        let mut copy = false;
        let mut paste = false;
        let mut select_all = false;
        let mut quick_fix = false;
        let mut rename = false;
        let mut goto_definition = false;
        let mut peek_definition = false;
        let mut completion = false;
        let mut parameter_hint = false;
        let mut escape = false;
        let ctrl_shift = egui::Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        ui.input_mut(|input| {
            save = input.consume_key(egui::Modifiers::CTRL, egui::Key::S);
            find = input.consume_key(egui::Modifiers::CTRL, egui::Key::F);
            replace = input.consume_key(egui::Modifiers::CTRL, egui::Key::H);
            goto = input.consume_key(egui::Modifiers::CTRL, egui::Key::G);
            wrap = input.consume_key(egui::Modifiers::ALT, egui::Key::Z);
            delete_line = input.consume_key(ctrl_shift, egui::Key::K);
            undo = input.consume_key(egui::Modifiers::CTRL, egui::Key::Z);
            redo = input.consume_key(egui::Modifiers::CTRL, egui::Key::Y);
            cut = input.consume_key(egui::Modifiers::CTRL, egui::Key::X);
            copy = input.consume_key(egui::Modifiers::CTRL, egui::Key::C);
            paste = input.consume_key(egui::Modifiers::CTRL, egui::Key::V);
            select_all = input.consume_key(egui::Modifiers::CTRL, egui::Key::A);
            quick_fix = input.consume_key(egui::Modifiers::CTRL, egui::Key::Period);
            rename = input.consume_key(egui::Modifiers::NONE, egui::Key::F2);
            goto_definition = input.consume_key(egui::Modifiers::NONE, egui::Key::F12);
            peek_definition = input.consume_key(egui::Modifiers::ALT, egui::Key::F12);
            completion = input.consume_key(egui::Modifiers::CTRL, egui::Key::Space);
            parameter_hint = input.consume_key(ctrl_shift, egui::Key::Space);
            escape = input.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::Plus)
                || input.consume_key(egui::Modifiers::CTRL, egui::Key::Equals)
            {
                zoom = 1;
            }
            if input.consume_key(egui::Modifiers::CTRL, egui::Key::Minus) {
                zoom = -1;
            }
        });
        if save {
            let mut args = Args::new();
            args.insert("path", json!(tab.path));
            let _ = cx.run_command("editor.save", args);
        }
        if find {
            let _ = cx.run_command("editor.find", Args::new());
        }
        if replace {
            let mut args = Args::new();
            args.insert("replace", json!(true));
            let _ = cx.run_command("editor.find", args);
        }
        if goto {
            self.goto_open = !self.goto_open;
            self.goto_text = self.state.cursor_line().to_string();
        }
        if wrap {
            let mut args = Args::new();
            args.insert("path", json!(tab.path));
            let _ = cx.run_command("editor.wrap.toggle", args);
        }
        if delete_line {
            let mut args = Args::new();
            args.insert("path", json!(tab.path));
            let _ = cx.run_command("editor.delete_line", args);
        }
        for (fired, id) in [
            (undo, "editor.undo"),
            (redo, "editor.redo"),
            (cut, "editor.cut"),
            (copy, "editor.copy"),
            (paste, "editor.paste"),
            (select_all, "editor.select_all"),
            (quick_fix, "editor.quick_fix"),
            (rename, "editor.rename"),
            (goto_definition, "editor.goto.definition"),
            (peek_definition, "editor.peek.definition"),
            (completion, "editor.completion"),
            (parameter_hint, "editor.parameter_hint"),
        ] {
            if fired {
                let mut args = Args::new();
                args.insert("path", json!(tab.path));
                let _ = cx.run_command(id, args);
            }
        }
        if escape {
            let mut dismissed = self.goto_open || find_state.open;
            self.goto_open = false;
            find_state.open = false;
            find_state.replace_open = false;
            self.state.with(|state| {
                if state.peek.is_some()
                    || state.completion.is_some()
                    || state.parameter_hint.is_some()
                    || state.rename.is_some()
                    || state.quick_fix
                {
                    dismissed = true;
                }
                state.peek = None;
                state.completion = None;
                state.parameter_hint = None;
                state.rename = None;
                state.quick_fix = false;
            });
            if !dismissed {
                let id = editor_id(&tab.path);
                let escaping_text = ui.memory(|memory| memory.has_focus(id))
                    || (self.text_focused && ui.memory(|memory| memory.focused().is_none()));
                if escaping_text {
                    forja_ui_kit::release_keyboard_focus(ui.ctx(), id);
                    let _ = cx.run_command("tab.focus", Args::new());
                    ui.ctx().request_repaint();
                }
            }
        }
        if zoom != 0 {
            let current = cx
                .setting("editor.font.size")
                .and_then(forja_kernel::SettingValue::as_i64)
                .unwrap_or(13);
            let next = (current + zoom).clamp(8, 32);
            let _ = cx.set_setting(
                "editor.font.size",
                forja_kernel::SettingValue::Integer(next),
            );
        }
    }

    fn text_area(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &mut Tab,
        find: &model::FindState,
        muted: egui::Color32,
    ) {
        let lines = model::line_count(&tab.content);
        let show_numbers = self
            .setting(cx, "editor.gutter.line_numbers")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let font_size = self
            .setting(cx, "editor.font.size")
            .and_then(|value| value.as_i64())
            .unwrap_or(13) as f32;
        let line_height = self
            .setting(cx, "editor.line.height")
            .and_then(|value| value.as_f64())
            .unwrap_or(1.4) as f32;
        let highlight_line = self
            .setting(cx, "editor.highlight.current_line")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let minimap = self
            .setting(cx, "editor.view.minimap")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let brackets = self
            .setting(cx, "editor.brackets.guides")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let whitespace = self
            .setting(cx, "editor.whitespace.show")
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "selection".to_owned());
        let indent_size = self
            .setting(cx, "editor.indent.size")
            .and_then(|value| value.as_i64())
            .unwrap_or(4)
            .clamp(1, 8) as usize;
        let family = self.font_family(cx);
        let dark = ui.visuals().dark_mode;
        let mut content = tab.content.clone();
        let canvas_label = cx.t_args("editor.canvas.label", &[("file", &tab.path)]);
        let id = editor_id(&tab.path);
        let mut response_focused = false;
        let mut changed = false;
        let selection = self.state.selection();
        let indents = line_indents(&tab.content, indent_size);
        let accent = c32(forja_ui_kit::applied_tokens(ui.ctx()).color.accent);

        if self
            .state
            .with(|state| std::mem::take(&mut state.pending_select_all))
            .unwrap_or(false)
        {
            if let Some(mut edit) = egui::widgets::text_edit::TextEditState::load(ui.ctx(), id) {
                edit.cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(content.chars().count()),
                    )));
                edit.store(ui.ctx(), id);
                ui.memory_mut(|memory| memory.request_focus(id));
            }
        }

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if show_numbers {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y =
                                (line_height - 1.0).max(0.0) * font_size;
                            ui.add_space(forja_ui_kit::Spacing::XS);
                            for line in 1..=lines {
                                forja_ui_kit::text(ui,
                                    egui::RichText::new(format!("{line:>4}"))
                                        .monospace()
                                        .size(font_size)
                                        .color(muted),
                                );
                            }
                        });
                        ui.separator();
                    }
                    let cache = self.caches.entry(tab.path.clone()).or_default();
                    let path = tab.path.clone();
                    let family_key = family.clone();
                    let mut layouter =
                        |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
                            let job = cache.get_or_build(
                                ui,
                                &path,
                                buffer.as_str(),
                                wrap_width,
                                crate::highlight::LayoutStyle {
                                    dark,
                                    font_size,
                                    family: family_key.as_deref(),
                                    line_height,
                                },
                            );
                            ui.fonts_mut(|fonts| fonts.layout_job(job))
                        };
                    let wrap_mode = cx
                        .setting("editor.wrap.mode")
                        .and_then(forja_kernel::SettingValue::as_str)
                        .unwrap_or("off");
                    let wrap_col = cx
                        .setting("editor.wrap.column")
                        .and_then(forja_kernel::SettingValue::as_i64)
                        .unwrap_or(100);
                    let width = wrap_desired_width(
                        wrap_mode,
                        wrap_col,
                        tab.wrap,
                        font_size,
                        ui.available_width(),
                    );
                    let output = egui::TextEdit::multiline(&mut content)
                        .id(id)
                        .code_editor()
                        .desired_width(width)
                        .margin(egui::Margin::ZERO)
                        .layouter(&mut layouter)
                        .show(ui);
                    let canvas_name = canvas_label.clone();
                    output.response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), canvas_name.clone())
                    });
                    changed = output.response.changed();
                    response_focused =
                        output.response.has_focus() || ui.memory(|memory| memory.has_focus(id));
                    if let Some(range) = output.cursor_range {
                        let primary = range.primary.index;
                        let secondary = range.secondary.index;
                        self.state.set_cursor(
                            primary,
                            Some((primary.min(secondary), primary.max(secondary))),
                        );
                        self.state
                            .set_cursor_line(model::line_of_char(&content, primary));
                    }
                    if highlight_line {
                        let cursor_line = self.state.cursor_line();
                        paint_current_line(ui, &output, &content, cursor_line, accent);
                    }
                    if brackets {
                        paint_guides(ui, &output, &indents, indent_size, muted, font_size);
                    }
                    if whitespace != "never" {
                        paint_whitespace(ui, &output, &whitespace, selection, muted);
                    }
                    if minimap {
                        paint_minimap(ui, &content, muted);
                    }
                    if let Some(mut edit) =
                        egui::widgets::text_edit::TextEditState::load(ui.ctx(), id)
                    {
                        let pending_match = self
                            .state
                            .with(|state| std::mem::take(&mut state.pending_match))
                            .unwrap_or(false);
                        let pending = self.state.with(|state| state.pending_line.take()).flatten();
                        if let Some(line) = pending {
                            let index = model::char_index_for_line(&content, line);
                            edit.cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(
                                    egui::text::CCursor::new(index),
                                )));
                            edit.store(ui.ctx(), id);
                            self.state.set_cursor_line(line);
                        }
                        if pending_match {
                            self.select_find_match(ui, tab, find);
                        }
                    }
                });
            });
        if changed {
            self.state.record_undo(&tab.path, tab.content.clone());
            self.state.set_content(&tab.path, content.clone());
            tab.content = content;
            tab.dirty = true;
            tab.last_edit = Some(Instant::now());
        }
        if tab.dirty {
            self.auto_save(cx, tab, response_focused);
        }
        self.text_focused = response_focused;
    }

    fn auto_save(&mut self, cx: &mut Ctx<'_>, tab: &Tab, focused: bool) {
        if !tab.dirty {
            return;
        }
        if self
            .auto_saved
            .map(|at| at.elapsed() < Duration::from_millis(500))
            .unwrap_or(false)
        {
            return;
        }
        let mode = cx
            .setting("editor.save.auto")
            .and_then(forja_kernel::SettingValue::as_str)
            .unwrap_or("off");
        let delay = cx
            .setting("editor.save.delay_ms")
            .and_then(forja_kernel::SettingValue::as_i64)
            .unwrap_or(1500) as u64;
        let due = match mode {
            "delay" => tab
                .last_edit
                .map(|edit| edit.elapsed() >= Duration::from_millis(delay))
                .unwrap_or(false),
            "focus_change" => {
                !focused
                    && self
                        .auto_saved
                        .map(|at| at.elapsed() > Duration::from_millis(300))
                        .unwrap_or(true)
            }
            _ => false,
        };
        if due {
            self.auto_saved = Some(Instant::now());
            let mut args = Args::new();
            args.insert("path", json!(tab.path));
            let _ = cx.run_command("editor.save", args);
        }
    }

    fn preview_area(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        document: Option<&ViewerDocument>,
    ) {
        let muted = c32(forja_ui_kit::applied_tokens(ui.ctx()).color.text_muted);
        let Some(document) = document else {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("editor.preview.loading")).color(muted));
            return;
        };
        if let Some(error) = &document.error {
            crate::view_error::banner(ui, cx, error);
            return;
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match document.kind.as_str() {
                "markdown" => {
                    if let Some(markdown) = &document.markdown {
                        ui.horizontal_top(|ui| {
                            if !markdown.toc.is_empty() {
                                ui.vertical(|ui| {
                                    ui.set_width(170.0);
                                    forja_ui_kit::text(ui,
                                        egui::RichText::new(cx.t("viewers.markdown.toc"))
                                            .small()
                                            .color(muted),
                                    );
                                    for entry in &markdown.toc {
                                        ui.horizontal(|ui| {
                                            ui.add_space(
                                                (entry.level.saturating_sub(1) as f32) * 8.0,
                                            );
                                            forja_ui_kit::text(ui, egui::RichText::new(&entry.text).small());
                                        });
                                    }
                                });
                                ui.separator();
                            }
                            ui.vertical(|ui| render_blocks(ui, cx, &markdown.blocks));
                        });
                    }
                }
                "table" => {
                    if let Some(table) = &document.table {
                        forja_ui_kit::text(ui, format!(
                            "{} {}",
                            table.rows.len(),
                            cx.t("viewers.table.rows")
                        ));
                        egui::Grid::new("editor.preview.table")
                            .striped(true)
                            .show(ui, |ui| {
                                for header in &table.headers {
                                    forja_ui_kit::text(ui, egui::RichText::new(header).strong());
                                }
                                ui.end_row();
                                for row in table.rows.iter().take(80) {
                                    for cell in row {
                                        forja_ui_kit::text(ui, cell);
                                    }
                                    ui.end_row();
                                }
                            });
                    }
                }
                "notebook" => {
                    if let Some(notebook) = &document.notebook {
                        for (index, cell) in notebook.cells.iter().enumerate() {
                            forja_ui_kit::text(ui,
                                egui::RichText::new(format!("[{}] {}", index + 1, cell.kind))
                                    .small()
                                    .color(muted),
                            );
                            if cell.kind == "markdown" {
                                forja_ui_kit::text(ui, &cell.source);
                            } else {
                                forja_ui_kit::text(ui, egui::RichText::new(&cell.source).monospace());
                                for output in &cell.outputs {
                                    forja_ui_kit::text(ui,
                                        egui::RichText::new(output)
                                            .small()
                                            .monospace()
                                            .color(muted),
                                    );
                                }
                            }
                            ui.separator();
                        }
                    }
                }
                "image" => {
                    if let Some(image) = &document.image {
                        forja_ui_kit::text(ui, format!("{} x {} px", image.width, image.height));
                        if image.frames > 1 {
                            forja_ui_kit::text(ui, cx.t_args(
                                "viewers.image.frames",
                                &[("frames", &image.frames.to_string())],
                            ));
                        }
                        forja_ui_kit::text(ui,
                            egui::RichText::new(&image.path)
                                .monospace()
                                .small()
                                .color(muted),
                        );
                    }
                }
                "pdf" => {
                    if let Some(pdf) = &document.pdf {
                        forja_ui_kit::text(ui,
                            cx.t_args("viewers.pdf.pages", &[("pages", &pdf.pages.to_string())]),
                        );
                        forja_ui_kit::text(ui,
                            egui::RichText::new(cx.t("viewers.pdf.hint"))
                                .small()
                                .color(muted),
                        );
                    }
                }
                _ => {}
            });
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Ghost,
            &cx.t("viewers.action.open-external"),
        )
        .clicked()
        {
            let mut args = Args::new();
            args.insert("path", json!(&document.path));
            let _ = cx.run_command("viewers.open_external", args);
        }
    }

    fn status_line(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        find: &model::FindState,
        muted: egui::Color32,
    ) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let eol = match tab.eol {
                model::Eol::Lf => "LF",
                model::Eol::Crlf => "CRLF",
            };
            let encoding = match tab.encoding {
                model::Encoding::Utf8 => "UTF-8",
                model::Encoding::Utf8Bom => "UTF-8 BOM",
                model::Encoding::Utf16Le => "UTF-16 LE",
            };
            let line = self.state.cursor_line();
            let text = cx.t_args(
                "editor.status.line",
                &[
                    ("line", &line.to_string()),
                    ("lines", &model::line_count(&tab.content).to_string()),
                ],
            );
            forja_ui_kit::text(ui, egui::RichText::new(text).small().color(muted));
            forja_ui_kit::text(ui,
                egui::RichText::new(format!("{eol} · {encoding}"))
                    .small()
                    .color(muted),
            );
            if !find.matches.is_empty() {
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t_args(
                        "editor.status.matches",
                        &[("count", &find.matches.len().to_string())],
                    ))
                    .small()
                    .color(muted),
                );
            }
            if tab.dirty {
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t("editor.status.unsaved"))
                        .small()
                        .color(muted),
                );
            }
            forja_ui_kit::text(ui, egui::RichText::new(&tab.path).small().color(muted));
        });
    }

    fn symbol_windows(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        muted: egui::Color32,
    ) {
        let peek = self.state.with(|state| state.peek.clone()).flatten();
        if let Some(peek) = peek {
            let mut open = true;
            let title = cx.t("editor.peek.title");
            let close_label = cx.t("editor.peek.close");
            egui::Window::new(&title)
                .id(egui::Id::new("forja.editor.peek"))
                .title_bar(false)
                .default_width(460.0)
                .show(ui.ctx(), |ui| {
                    if forja_ui_kit::window_header(ui, &title, &close_label) {
                        open = false;
                    }
                    forja_ui_kit::text(ui,
                        egui::RichText::new(cx.t_args(
                            "editor.peek.line",
                            &[("word", &peek.word), ("line", &peek.line.to_string())],
                        ))
                        .small()
                        .color(muted),
                    );
                    forja_ui_kit::text(ui,
                        egui::RichText::new(&peek.text)
                            .monospace()
                            .color(c32(forja_ui_kit::applied_tokens(ui.ctx()).color.text)),
                    );
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("editor.peek.close"),
                    )
                    .clicked()
                    {
                        self.state.with(|state| state.peek = None);
                    }
                });
            if !open {
                self.state.with(|state| state.peek = None);
            }
        }

        let completion = self.state.with(|state| state.completion.clone()).flatten();
        if let Some(completion) = completion {
            let mut open = true;
            let title = cx.t("editor.completion.title");
            let close_label = cx.t("editor.completion.close");
            egui::Window::new(&title)
                .id(egui::Id::new("forja.editor.completion"))
                .title_bar(false)
                .default_width(260.0)
                .show(ui.ctx(), |ui| {
                    if forja_ui_kit::window_header(ui, &title, &close_label) {
                        open = false;
                    }
                    let mut chosen: Option<String> = None;
                    for candidate in &completion.candidates {
                        if ui.selectable_label(false, candidate).clicked() {
                            chosen = Some(candidate.clone());
                        }
                    }
                    if let Some(word) = chosen {
                        let mut args = Args::new();
                        args.insert("path", json!(tab.path));
                        args.insert("word", json!(word));
                        let _ = cx.run_command("editor.completion.apply", args);
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("editor.completion.close"),
                    )
                    .clicked()
                    {
                        self.state.with(|state| state.completion = None);
                    }
                });
            if !open {
                self.state.with(|state| state.completion = None);
            }
        }

        let hint = self
            .state
            .with(|state| state.parameter_hint.clone())
            .flatten();
        if let Some(hint) = hint {
            let mut open = true;
            let title = cx.t("editor.parameter.title");
            let close_label = cx.t("editor.parameter.close");
            egui::Window::new(&title)
                .id(egui::Id::new("forja.editor.parameter"))
                .title_bar(false)
                .default_width(420.0)
                .show(ui.ctx(), |ui| {
                    if forja_ui_kit::window_header(ui, &title, &close_label) {
                        open = false;
                    }
                    forja_ui_kit::text(ui,
                        egui::RichText::new(
                            cx.t_args("editor.parameter.line", &[("line", &hint.line.to_string())]),
                        )
                        .small()
                        .color(muted),
                    );
                    forja_ui_kit::text(ui, egui::RichText::new(&hint.text).monospace());
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("editor.parameter.close"),
                    )
                    .clicked()
                    {
                        self.state.with(|state| state.parameter_hint = None);
                    }
                });
            if !open {
                self.state.with(|state| state.parameter_hint = None);
            }
        }

        let rename = self.state.with(|state| state.rename.clone()).flatten();
        if let Some(mut rename) = rename {
            let mut open = true;
            let mut applied = false;
            let title = cx.t("editor.rename.title");
            let close_label = cx.t("editor.peek.close");
            egui::Window::new(&title)
                .id(egui::Id::new("forja.editor.rename"))
                .title_bar(false)
                .default_width(320.0)
                .show(ui.ctx(), |ui| {
                    if forja_ui_kit::window_header(ui, &title, &close_label) {
                        open = false;
                    }
                    forja_ui_kit::text(ui, egui::RichText::new(&rename.word).monospace().color(muted));
                    let rename_hint = cx.t("editor.rename.hint");
                    forja_ui_kit::text_field_labeled(ui, &mut rename.text, &rename_hint, &rename_hint);
                    let apply = forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Primary,
                        &cx.t("editor.action.rename-apply"),
                    )
                    .clicked();
                    if apply && !rename.text.trim().is_empty() {
                        let mut args = Args::new();
                        args.insert("path", json!(tab.path));
                        args.insert("word", json!(&rename.word));
                        args.insert("replacement", json!(rename.text.trim()));
                        let _ = cx.run_command("editor.rename", args);
                        applied = true;
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("editor.peek.close"),
                    )
                    .clicked()
                    {
                        self.state.with(|state| state.rename = None);
                    }
                });
            if !applied {
                self.state.with(|state| state.rename = Some(rename));
            }
            if !open {
                self.state.with(|state| state.rename = None);
            }
        }

        if self.state.with(|state| state.quick_fix).unwrap_or(false) {
            let mut open = true;
            let title = cx.t("editor.quick-fix.title");
            let close_label = cx.t("editor.quick-fix.close");
            egui::Window::new(&title)
                .id(egui::Id::new("forja.editor.quick_fix"))
                .title_bar(false)
                .default_width(320.0)
                .show(ui.ctx(), |ui| {
                    if forja_ui_kit::window_header(ui, &title, &close_label) {
                        open = false;
                    }
                    for fix in ["trim", "newline", "spaces"] {
                        if ui
                            .selectable_label(false, cx.t(edit::quick_fix_label_key(fix)))
                            .clicked()
                        {
                            let mut args = Args::new();
                            args.insert("path", json!(tab.path));
                            args.insert("fix", json!(fix));
                            let _ = cx.run_command("editor.quick_fix.apply", args);
                        }
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("editor.quick-fix.close"),
                    )
                    .clicked()
                    {
                        self.state.with(|state| state.quick_fix = false);
                    }
                });
            if !open {
                self.state.with(|state| state.quick_fix = false);
            }
        }
    }

    fn compare_window(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tab: &Tab,
        muted: egui::Color32,
    ) {
        let Some((disk, buffer)) = self.state.with(|state| state.compare.clone()).flatten() else {
            return;
        };
        let mut open = true;
        let title = cx.t("editor.compare.title");
        let close_label = cx.t("editor.peek.close");
        egui::Window::new(&title)
            .id(egui::Id::new("forja.editor.compare"))
            .title_bar(false)
            .default_width(760.0)
            .show(ui.ctx(), |ui| {
                if forja_ui_kit::window_header(ui, &title, &close_label) {
                    open = false;
                }
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t("editor.compare.hint"))
                        .small()
                        .color(muted),
                );
                let diff = similar::TextDiff::from_lines(&disk, &buffer);
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for change in diff.iter_all_changes() {
                            let (sign, color) = match change.tag() {
                                similar::ChangeTag::Delete => (
                                    "-",
                                    c32(forja_ui_kit::applied_tokens(ui.ctx()).color.danger),
                                ),
                                similar::ChangeTag::Insert => {
                                    ("+", c32(forja_ui_kit::applied_tokens(ui.ctx()).color.ok))
                                }
                                similar::ChangeTag::Equal => (" ", muted),
                            };
                            forja_ui_kit::text(ui,
                                egui::RichText::new(format!(
                                    "{sign} {}",
                                    change.value().trim_end_matches('\n')
                                ))
                                .monospace()
                                .small()
                                .color(color),
                            );
                        }
                    });
                forja_ui_kit::text(ui, egui::RichText::new(&tab.path).small().color(muted));
            });
        if !open {
            self.state.with(|state| state.compare = None);
        }
    }
}

fn render_blocks(ui: &mut egui::Ui, cx: &mut Ctx<'_>, blocks: &[forja_contracts::MarkdownBlock]) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    for block in blocks {
        match block.kind.as_str() {
            "heading" => {
                let size = match block.level {
                    1 => tokens.metrics().title + 4.0,
                    2 => tokens.metrics().title,
                    3 => tokens.metrics().body + 3.0,
                    _ => tokens.metrics().body + 1.0,
                };
                ui.add_space(forja_ui_kit::Spacing::XS);
                forja_ui_kit::text(ui,
                    egui::RichText::new(&block.text)
                        .size(size)
                        .strong()
                        .color(c32(tokens.color.text)),
                );
            }
            "paragraph" | "quote" => {
                forja_ui_kit::text(ui, egui::RichText::new(&block.text).color(c32(tokens.color.text)));
            }
            "list_item" => {
                forja_ui_kit::text(ui,
                    egui::RichText::new(format!("\u{2022} {}", block.text))
                        .color(c32(tokens.color.text)),
                );
            }
            "code" => {
                egui::Frame::new()
                    .fill(c32(tokens.color.surface_alt))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        forja_ui_kit::text(ui, egui::RichText::new(&block.text).monospace());
                    });
            }
            "callout" => {
                let color = match block.callout.as_deref() {
                    Some("warning") | Some("caution") => tokens.color.spark,
                    Some("tip") => tokens.color.ok,
                    _ => tokens.color.info,
                };
                egui::Frame::new()
                    .fill(c32(tokens.color.surface_alt))
                    .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        forja_ui_kit::text(ui,
                            egui::RichText::new(cx.t(&format!(
                                "viewers.callout.{}",
                                block.callout.as_deref().unwrap_or("note")
                            )))
                            .strong()
                            .color(c32(color)),
                        );
                        forja_ui_kit::text(ui, &block.text);
                    });
            }
            "rule" => {
                ui.separator();
            }
            _ => {
                if !block.text.is_empty() {
                    forja_ui_kit::text(ui, &block.text);
                }
            }
        }
        ui.add_space(2.0);
    }
}

/// Leading whitespace columns per line, with tabs expanded by `indent_size`.
fn line_indents(text: &str, indent_size: usize) -> Vec<usize> {
    text.lines()
        .map(|line| {
            let mut columns = 0;
            for character in line.chars() {
                match character {
                    ' ' => columns += 1,
                    '\t' => columns += indent_size,
                    _ => break,
                }
            }
            columns
        })
        .collect()
}

fn paint_current_line(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    content: &str,
    line: usize,
    color: egui::Color32,
) {
    if line == 0 {
        return;
    }
    let index = model::char_index_for_line(content, line);
    let rect = output
        .galley
        .pos_from_cursor(egui::text::CCursor::new(index));
    let top = output.galley_pos.y + rect.top();
    let bottom = output.galley_pos.y + rect.bottom();
    if !ui.clip_rect().intersects(egui::Rect::from_min_max(
        egui::pos2(output.galley_pos.x, top),
        egui::pos2(output.galley_pos.x + 2.0, bottom),
    )) {
        return;
    }
    let area = egui::Rect::from_min_max(
        egui::pos2(output.galley_pos.x - 4.0, top),
        egui::pos2(
            ui.clip_rect().right().max(output.galley_pos.x + 24.0),
            bottom,
        ),
    );
    ui.painter().rect_filled(
        area,
        0.0,
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 22),
    );
}

fn paint_guides(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    indents: &[usize],
    indent_size: usize,
    color: egui::Color32,
    font_size: f32,
) {
    let char_width = (font_size * 0.62).max(4.0);
    let stroke = egui::Stroke::new(
        1.0_f32,
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46),
    );
    let mut line_index = 0usize;
    for row in &output.galley.rows {
        let row_rect = row.rect();
        let top = output.galley_pos.y + row_rect.top();
        let bottom = output.galley_pos.y + row_rect.bottom();
        let probe = egui::Rect::from_min_max(
            egui::pos2(output.galley_pos.x, top),
            egui::pos2(output.galley_pos.x + 1.0, bottom),
        );
        if ui.clip_rect().intersects(probe) {
            let columns = indents.get(line_index).copied().unwrap_or(0);
            let levels = columns / indent_size.max(1);
            for level in 1..=levels {
                let x = output.galley_pos.x + ((level * indent_size) as f32 * char_width) + 2.0;
                ui.painter().vline(x, top..=bottom, stroke);
            }
        }
        if row.row.ends_with_newline {
            line_index += 1;
        }
    }
}

fn paint_whitespace(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    mode: &str,
    selection: Option<(usize, usize)>,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0_f32, color);
    let mut char_cursor = 0usize;
    for row in &output.galley.rows {
        let row_rect = row.rect();
        let top = output.galley_pos.y + row_rect.top();
        let bottom = output.galley_pos.y + row_rect.bottom();
        let probe = egui::Rect::from_min_max(
            egui::pos2(output.galley_pos.x, top),
            egui::pos2(output.galley_pos.x + 1.0, bottom),
        );
        if !ui.clip_rect().intersects(probe) {
            char_cursor += row.row.char_count_including_newline();
            continue;
        }
        for (offset, glyph) in row.row.glyphs.iter().enumerate() {
            let index = char_cursor + offset;
            let visible = mode == "always"
                || selection.is_some_and(|(start, end)| index >= start && index < end);
            if !visible {
                continue;
            }
            let x = output.galley_pos.x + row.pos.x + glyph.pos.x;
            match glyph.chr {
                ' ' => {
                    ui.painter().circle_filled(
                        egui::pos2(x + glyph.advance_width * 0.5, bottom - 2.0),
                        0.9,
                        color,
                    );
                }
                '\t' => {
                    let y = bottom - 2.0;
                    let end = x + glyph.advance_width.max(4.0) - 1.0;
                    ui.painter()
                        .line_segment([egui::pos2(x + 1.0, y), egui::pos2(end, y)], stroke);
                    ui.painter()
                        .line_segment([egui::pos2(end, y), egui::pos2(end - 3.0, y - 3.0)], stroke);
                }
                _ => {}
            }
        }
        char_cursor += row.row.char_count_including_newline();
    }
}

fn paint_minimap(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    let height = ui.clip_rect().height().clamp(120.0, 2000.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(
        rect,
        2,
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 12),
    );
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return;
    }
    let longest = lines
        .iter()
        .map(|line| line.trim_end().chars().count())
        .max()
        .unwrap_or(1)
        .max(1);
    let step = lines.len().div_ceil(300).max(1);
    let slots = lines.len().div_ceil(step);
    let stroke = egui::Stroke::new(
        1.0_f32,
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 120),
    );
    for (slot, line) in lines.iter().step_by(step).enumerate() {
        let y = rect.top() + (slot as f32 / slots.max(1) as f32) * (rect.height() - 1.0);
        let ratio = line.trim_end().chars().count() as f32 / longest as f32;
        let width = (rect.width() - 3.0) * ratio;
        if width > 0.5 {
            painter.hline(rect.left() + 1.5..=rect.left() + 1.5 + width, y, stroke);
        }
    }
}

fn find_key(path: &str, text: &str, find: &model::FindState) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    text.hash(&mut hasher);
    find.query.hash(&mut hasher);
    find.regex.hash(&mut hasher);
    find.case_sensitive.hash(&mut hasher);
    find.whole_word.hash(&mut hasher);
    hasher.finish()
}

fn editor_id(path: &str) -> egui::Id {
    egui::Id::new(("forja-editor", path))
}

fn wrap_desired_width(
    mode: &str,
    column: i64,
    tab_wrap: bool,
    font_size: f32,
    available_width: f32,
) -> f32 {
    if !tab_wrap {
        return 4000.0;
    }
    let window_width = (available_width - 8.0).max(160.0);
    if mode == "column" {
        let cols = column.clamp(40, 200) as f32;
        (cols * font_size * 0.62 + 12.0).clamp(160.0, window_width)
    } else {
        window_width
    }
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}

#[cfg(test)]
mod tests {
    use super::wrap_desired_width;

    #[test]
    fn column_wrap_clamps_to_column_budget() {
        assert_eq!(
            wrap_desired_width("window", 80, false, 13.0, 1200.0),
            4000.0
        );
        let col = wrap_desired_width("column", 80, true, 13.0, 1200.0);
        assert!(col < 1192.0 && col > 500.0);
    }
}
