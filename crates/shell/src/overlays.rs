//! Settings window. The Bigorna lives in `anvil.rs`.

use crate::app::ForjaApp;
use eframe::egui::{self, Color32, RichText, Sense, TextEdit};
use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue};
use forja_ui_kit::text_field;
use std::collections::BTreeMap;

impl ForjaApp {
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        let focus_id = egui::Id::new("forja.settings.was_open");
        let pending_focus = egui::Id::new("forja.settings.focus_pending");
        if !self.settings_open {
            ctx.data_mut(|data| {
                data.remove_temp::<bool>(focus_id);
                data.remove_temp::<bool>(pending_focus);
            });
            return;
        }
        // The window must exist for one frame before the search takes the
        // keyboard; focusing in the same frame the window is created loses it
        // to the window itself.
        let focus_search = ctx.data_mut(|data| {
            let was_open = data.get_temp::<bool>(focus_id).unwrap_or(false);
            data.insert_temp(focus_id, true);
            if was_open {
                data.remove_temp::<bool>(pending_focus).unwrap_or(false)
            } else {
                data.insert_temp(pending_focus, true);
                false
            }
        });
        let layer = egui::Id::new("forja.settings.keyboard");
        if forja_ui_kit::consume_layer_escape(ctx, layer) {
            self.settings_open = false;
            self.settings_section = None;
            ctx.data_mut(|data| {
                data.remove_temp::<bool>(focus_id);
                data.remove_temp::<bool>(pending_focus);
            });
            return;
        }
        let mut open = self.settings_open;
        let title = self.text("shell.settings.title");
        let close_label = self.text("shell.window.close-dialog");
        let modal = egui::Window::new(&title)
            .id(egui::Id::new("forja.shell.settings"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .max_height((ctx.content_rect().height() - 120.0).max(240.0))
            .show(ctx, |ui| {
                forja_ui_kit::register_keyboard_layer(ui, layer);
                if forja_ui_kit::window_header(ui, &title, &close_label) {
                    open = false;
                }
                self.settings_contents(ui, focus_search);
            });
        if let Some(modal) = modal {
            self.slot_rects
                .insert("settings".into(), modal.response.rect);
        }
        self.settings_open = open;
        if !self.settings_open {
            self.settings_section = None;
            ctx.data_mut(|data| {
                data.remove_temp::<bool>(focus_id);
                data.remove_temp::<bool>(pending_focus);
            });
        }
    }

    /// Window opened by `help.keymap`; lists every resolved binding.
    pub(crate) fn keymap_window(&mut self, ctx: &egui::Context) {
        let focused_id = egui::Id::new("forja.keymap.initial_focus");
        if !self.keymap_open {
            ctx.data_mut(|data| data.remove_temp::<bool>(focused_id));
            return;
        }
        let layer = egui::Id::new("forja.keymap.keyboard");
        if forja_ui_kit::consume_layer_escape(ctx, layer) {
            self.keymap_open = false;
            return;
        }
        let focus_close = forja_ui_kit::keyboard_layer_active(ctx, layer)
            && ctx.data_mut(|data| {
                if data.get_temp::<bool>(focused_id).unwrap_or(false) {
                    false
                } else {
                    data.insert_temp(focused_id, true);
                    true
                }
            });
        let mut open = self.keymap_open;
        let title = self.text("shell.keymap.title");
        let close_label = self.text("shell.window.close-dialog");
        egui::Window::new(&title)
            .id(egui::Id::new("forja.shell.keymap"))
            .title_bar(false)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| {
                forja_ui_kit::register_keyboard_layer(ui, layer);
                if forja_ui_kit::window_header_with_focus(ui, &title, &close_label, focus_close) {
                    open = false;
                }
                let muted = self.paint(self.tokens.color.text_muted);
                ui.label(
                    RichText::new(self.text("shell.keymap.hint"))
                        .small()
                        .color(muted),
                );
                ui.separator();
                let mut rows: Vec<(String, String, String)> = self
                    .runtime
                    .keymap()
                    .bindings()
                    .iter()
                    .map(|resolved| {
                        let command = resolved.binding.command.as_str().to_owned();
                        let title = self
                            .runtime
                            .registry()
                            .command(&command)
                            .map(|spec| self.text(&spec.title_key))
                            .unwrap_or_else(|| command.clone());
                        let scope = match resolved.binding.scope.as_str() {
                            "global" => self.text("shell.keymap.scope-global"),
                            "editor" => self.text("shell.keymap.scope-editor"),
                            "terminal" => self.text("shell.keymap.scope-terminal"),
                            "bench" => self.text("shell.keymap.scope-bench"),
                            _ => self.text("shell.keymap.scope-other"),
                        };
                        (resolved.binding.chord.to_string(), title, scope)
                    })
                    .collect();
                rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
                if rows.is_empty() {
                    ui.label(RichText::new(self.text("shell.keymap.empty")).color(muted));
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(520.0)
                    .show(ui, |ui| {
                        for (chord, title, scope) in rows {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(chord).monospace());
                                ui.label(title);
                                ui.label(RichText::new(scope).small().color(muted));
                            });
                        }
                    });
            });
        self.keymap_open = open;
    }

    fn settings_contents(&mut self, ui: &mut egui::Ui, focus_search: bool) {
        // `--settings <módulo>` (captura): só a seção desse módulo; sem o flag, tudo como antes.
        let section = self.settings_section.clone();
        self.settings_body(ui, section.as_deref(), focus_search);
    }

    fn settings_body(&mut self, ui: &mut egui::Ui, section: Option<&str>, focus_search: bool) {
        // Com `--settings`, a seção própria do módulo (se houver) substitui as linhas dele.
        let hint = self.text("shell.settings.search");
        let search = text_field(ui, &mut self.settings_query, &hint);
        if focus_search {
            search.request_focus();
        }
        ui.add_space(6.0);

        // Only the rows scroll: the search stays on top and the window fits the screen.
        let query = self.settings_query.trim().to_lowercase();
        let section_has_view = query.is_empty() && section
            .is_some_and(|module| self.settings_views.iter().any(|view| view.module == module));
        let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, true]);
        if search.changed() {
            scroll = scroll.vertical_scroll_offset(0.0);
        }
        scroll
            .show(ui, |ui| {
                let specs = self.runtime.registry().settings().to_vec();
                let modules = self.runtime.registry().modules().to_vec();
                let values: BTreeMap<String, SettingValue> =
                    self.runtime.settings().values().clone();

                let mut by_module: BTreeMap<String, Vec<SettingSpec>> = BTreeMap::new();
                for spec in specs {
                    let label = self.text(&spec.label_key).to_lowercase();
                    let matches = setting_matches(&query, &spec.key, &label);
                    let in_section =
                        section.is_none_or(|module| spec.module() == module && !section_has_view);
                    if matches && in_section {
                        by_module
                            .entry(spec.module().to_owned())
                            .or_default()
                            .push(spec);
                    }
                }

                // Com `--settings`, a lista pode estar vazia de propósito: a seção do módulo é que aparece.
                if by_module.is_empty() && (section.is_none() || !query.is_empty()) {
                    ui.label(
                        RichText::new(self.text("shell.settings.empty"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                }

                for (module_id, mut module_specs) in by_module {
                    module_specs
                        .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.key.cmp(&b.key)));
                    let name = modules
                        .iter()
                        .find(|manifest| manifest.id == module_id)
                        .map(|manifest| self.text(&manifest.name_key))
                        .unwrap_or_else(|| module_id.clone());
                    egui::CollapsingHeader::new(name)
                        .default_open(true)
                        .show(ui, |ui| {
                            for spec in &module_specs {
                                self.setting_row(ui, spec, values.get(&spec.key));
                            }
                        });
                }

                if query.is_empty() && !self.settings_views.is_empty() {
                    ui.separator();
                    for view in self.settings_views.iter_mut() {
                        if section.is_some_and(|only| view.module != only) {
                            continue;
                        }
                        let module = view.module.clone();
                        let mut cx = self.runtime.ctx(&module);
                        ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                    }
                }
                reveal_focused_control(ui);
                // PageUp/PageDown scroll this list. Keep it last so a nested
                // scroll area from a module section cannot take the delta.
                apply_page_scroll(ui);
            });
    }

    fn setting_row(
        &mut self,
        ui: &mut egui::Ui,
        spec: &SettingSpec,
        current: Option<&SettingValue>,
    ) {
        ui.push_id(&spec.key, |ui| {
            ui.horizontal(|ui| {
                let label_text = self.text(&spec.label_key);
                let label = ui
                    .label(&label_text)
                    .on_hover_text(self.text(&spec.help_key));
                let mut changed: Option<SettingValue> = None;
                match &spec.kind {
                    SettingKind::Bool => {
                        let mut value = current.and_then(SettingValue::as_bool).unwrap_or(false);
                        let checkbox = ui.checkbox(&mut value, "");
                        let state = value;
                        checkbox.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                true,
                                state,
                                &label_text,
                            )
                        });
                        if checkbox.changed() {
                            changed = Some(SettingValue::Bool(value));
                        }
                    }
                    SettingKind::Integer { min, max } => {
                        let mut value = current.and_then(SettingValue::as_i64).unwrap_or(0);
                        let mut drag = egui::DragValue::new(&mut value);
                        if min.is_some() || max.is_some() {
                            drag = drag.range(min.unwrap_or(i64::MIN)..=max.unwrap_or(i64::MAX));
                        }
                        if ui.add(drag).labelled_by(label.id).changed() {
                            changed = Some(SettingValue::Integer(value));
                        }
                        if let Some(hint) = self.integer_bounds_hint(*min, *max) {
                            let muted = self.paint(self.tokens.color.text_muted);
                            ui.label(RichText::new(hint).small().color(muted));
                        }
                    }
                    SettingKind::Number { min, max } => {
                        let mut value = current.and_then(SettingValue::as_f64).unwrap_or(0.0);
                        let mut drag = egui::DragValue::new(&mut value).speed(0.05);
                        if min.is_some() || max.is_some() {
                            drag = drag.range(min.unwrap_or(f64::MIN)..=max.unwrap_or(f64::MAX));
                        }
                        if ui.add(drag).labelled_by(label.id).changed() {
                            changed = Some(SettingValue::Number(value));
                        }
                        if let Some(hint) = self.number_bounds_hint(*min, *max) {
                            let muted = self.paint(self.tokens.color.text_muted);
                            ui.label(RichText::new(hint).small().color(muted));
                        }
                    }
                    SettingKind::Text { .. } | SettingKind::Path | SettingKind::Color => {
                        changed = self.text_setting(ui, spec, current);
                    }
                    SettingKind::List { .. } => {
                        let original = current
                            .and_then(SettingValue::as_list)
                            .map(|values| values.join(", "))
                            .unwrap_or_default();
                        let mut value = self
                            .setting_drafts
                            .get(&spec.key)
                            .cloned()
                            .unwrap_or(original);
                        if ui
                            .add(TextEdit::singleline(&mut value).desired_width(220.0))
                            .changed()
                        {
                            let list = value
                                .split(',')
                                .map(str::trim)
                                .filter(|item| !item.is_empty())
                                .map(str::to_owned)
                                .collect();
                            self.setting_drafts.insert(spec.key.clone(), value);
                            changed = Some(SettingValue::List(list));
                        }
                    }
                    SettingKind::Choice { options } => {
                        let original = current
                            .and_then(SettingValue::as_str)
                            .unwrap_or("")
                            .to_owned();
                        let mut index = options
                            .iter()
                            .position(|option| option == &original)
                            .unwrap_or(0);
                        let labels: Vec<String> = options
                            .iter()
                            .map(|option| self.choice_label(&spec.key, option))
                            .collect();
                        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                        if forja_ui_kit::select(ui, &mut index, &label_refs).changed() {
                            if let Some(value) = options.get(index) {
                                changed = Some(SettingValue::Text(value.clone()));
                            }
                        }
                    }
                }
                if let Some(value) = changed {
                    self.apply_setting(&spec.key, value);
                }
            });
        });
    }

    fn text_setting(
        &mut self,
        ui: &mut egui::Ui,
        spec: &SettingSpec,
        current: Option<&SettingValue>,
    ) -> Option<SettingValue> {
        let original = current
            .and_then(SettingValue::as_str)
            .unwrap_or("")
            .to_owned();
        let mut value = self
            .setting_drafts
            .get(&spec.key)
            .cloned()
            .unwrap_or_else(|| original.clone());
        let width = if matches!(spec.kind, SettingKind::Color) {
            110.0
        } else {
            240.0
        };
        let response = ui.add(TextEdit::singleline(&mut value).desired_width(width));
        if matches!(spec.kind, SettingKind::Color) {
            if let Some(rgb) = parse_hex_color(&value) {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(3),
                    Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                );
            }
        }
        if response.changed() {
            self.setting_drafts.insert(spec.key.clone(), value.clone());
            Some(SettingValue::Text(value))
        } else {
            None
        }
    }

    pub(crate) fn choice_label(&self, setting_key: &str, option: &str) -> String {
        choice_label_for(self.runtime.i18n(), setting_key, option)
    }

    fn integer_bounds_hint(&self, min: Option<i64>, max: Option<i64>) -> Option<String> {
        bounds_hint_for(
            self.runtime.i18n(),
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )
    }

    fn number_bounds_hint(&self, min: Option<f64>, max: Option<f64>) -> Option<String> {
        bounds_hint_for(
            self.runtime.i18n(),
            min.map(format_number_limit),
            max.map(format_number_limit),
        )
    }

    fn apply_setting(&mut self, key: &str, value: SettingValue) {
        match self.runtime.set_setting(key, value) {
            Ok(()) => self.clear_status(),
            Err(error) => {
                let raw = error.to_string();
                let summary = humanize_setting_error_for(
                    self.runtime.i18n(),
                    self.runtime.registry(),
                    key,
                    &raw,
                );
                self.status = Some(forja_kernel::HumanError::with_detail(summary, raw));
                self.status_details_open = false;
            }
        }
    }
}

/// PageUp/PageDown scroll one screen of the settings list, like the wheel.
fn apply_page_scroll(ui: &egui::Ui) {
    let direction = ui.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown) {
            Some(1.0)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp) {
            Some(-1.0)
        } else {
            None
        }
    });
    if let Some(direction) = direction {
        let page = ui.clip_rect().height() * 0.85;
        ui.scroll_with_delta(egui::vec2(0.0, -direction * page));
    }
}

/// Scrolls the settings list when Tab/Shift+Tab moves the focus to a control
/// that is outside the visible area. Only a real focus change scrolls, so a
/// manual scroll is never fought.
fn reveal_focused_control(ui: &egui::Ui) {
    let Some(focused) = ui.memory(|memory| memory.focused()) else {
        return;
    };
    let last_focus = egui::Id::new("forja.settings.last_focus");
    if ui.data(|data| data.get_temp::<egui::Id>(last_focus)) == Some(focused) {
        return;
    }
    ui.data_mut(|data| data.insert_temp(last_focus, focused));
    let Some(response) = ui.ctx().read_response(focused) else {
        return;
    };
    // Controls outside this scroll area (search, close) must not move it.
    if ui.min_rect().contains(response.rect.center()) {
        response.scroll_to_me(None);
    }
}

pub(crate) fn choice_label_for(
    i18n: &forja_kernel::I18n,
    setting_key: &str,
    option: &str,
) -> String {
    let locale = i18n.locale();
    let normalized = option.replace('.', "_");
    let specific = format!("setting.{setting_key}.option.{normalized}");
    if i18n.has(locale, &specific) || i18n.has(forja_kernel::i18n::Locale::En, &specific) {
        return i18n.t(&specific);
    }
    let generic = format!("setting.option.{normalized}");
    if i18n.has(locale, &generic) || i18n.has(forja_kernel::i18n::Locale::En, &generic) {
        return i18n.t(&generic);
    }
    option.to_owned()
}

pub(crate) fn bounds_hint_for(
    i18n: &forja_kernel::I18n,
    min: Option<String>,
    max: Option<String>,
) -> Option<String> {
    match (min, max) {
        (Some(min), Some(max)) => Some(i18n.t_args("shell.settings.range", &[("min", &min), ("max", &max)])),
        (Some(min), None) => Some(i18n.t_args("shell.settings.range-min", &[("min", &min)])),
        (None, Some(max)) => Some(i18n.t_args("shell.settings.range-max", &[("max", &max)])),
        (None, None) => None,
    }
}

pub(crate) fn humanize_setting_error_for(
    i18n: &forja_kernel::I18n,
    registry: &forja_kernel::Registry,
    key: &str,
    raw: &str,
) -> String {
    let label = registry
        .settings()
        .iter()
        .find(|spec| spec.key == key)
        .map(|spec| i18n.t(&spec.label_key))
        .unwrap_or_else(|| key.to_owned());
    if raw.contains("must be a color in #RRGGBB form") {
        return i18n.t_args("shell.settings.error-color", &[("label", &label)]);
    }
    if let Some(limit) = raw.split("must be at least ").nth(1) {
        return i18n.t_args(
            "shell.settings.error-min",
            &[("label", &label), ("limit", limit.trim())],
        );
    }
    if let Some(limit) = raw.split("must be at most ").nth(1) {
        return i18n.t_args(
            "shell.settings.error-max",
            &[("label", &label), ("limit", limit.trim())],
        );
    }
    if let Some(limit) = raw
        .split("must have at most ")
        .nth(1)
        .and_then(|rest| rest.strip_suffix(" characters"))
    {
        return i18n.t_args(
            "shell.settings.error-max-len",
            &[("label", &label), ("limit", limit.trim())],
        );
    }
    if raw.contains("list items cannot be empty") {
        return i18n.t_args("shell.settings.error-list-empty", &[("label", &label)]);
    }
    if raw.contains("list repeats ") {
        return i18n.t_args("shell.settings.error-list-repeat", &[("label", &label)]);
    }
    i18n.t_args("shell.settings.error-generic", &[("label", &label)])
}

fn setting_matches(query: &str, key: &str, label: &str) -> bool {
    query.is_empty()
        || key.to_lowercase().contains(query)
        || label.match_indices(query).any(|(index, _)| {
            label[..index].chars().next_back().is_none_or(|c| !c.is_alphanumeric())
        })
}

fn format_number_limit(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([red, green, blue])
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::i18n::Locale;

    #[test]
    fn settings_search_matches_labels_and_keys_at_word_boundaries() {
        assert!(setting_matches("tema", "appearance.theme", "tema escuro"));
        assert!(setting_matches("fonte", "editor.font", "tamanho da fonte"));
        assert!(!setting_matches("tema", "browser.system", "login no sistema"));
        assert!(!setting_matches("tema", "integration.jira", "jira"));
    }

    #[test]
    fn focused_row_inside_the_scroll_area_is_revealed() {
        let ctx = egui::Context::default();
        let mut offset = 0.0;
        for _ in 0..4 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .show(ui, |ui| {
                            for row in 0..40 {
                                let response = ui.button(format!("row {row}"));
                                if row == 35 {
                                    response.request_focus();
                                }
                            }
                            reveal_focused_control(ui);
                        });
                    offset = output.state.offset.y;
                });
            });
        }
        assert!(offset > 0.0, "the focused row stayed out of view");
    }

    #[test]
    fn page_keys_scroll_the_settings_list() {
        let ctx = egui::Context::default();
        let mut offset = 0.0;
        for frame in 0..6 {
            let mut raw = egui::RawInput::default();
            if frame == 1 {
                raw.events.push(egui::Event::Key {
                    key: egui::Key::PageDown,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let output = egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                        for row in 0..40 {
                            ui.label(format!("row {row}"));
                        }
                        apply_page_scroll(ui);
                    });
                    offset = output.state.offset.y;
                });
            });
        }
        assert!(offset > 0.0, "PageDown did not move the settings list");
    }

    struct TempGuard(std::path::PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn every_choice_option_has_translated_label_in_both_locales() {
        let dir = TempGuard(
            std::env::temp_dir().join(format!("forja-shell-choices-{}", std::process::id())),
        );
        let mut runtime = crate::app::build_runtime(
            Some("pt-BR".into()),
            Some(dir.0.clone()),
            Some(dir.0.join("u")),
            true,
        )
        .expect("runtime");
        assert!(
            runtime.warnings().is_empty(),
            "unexpected i18n/runtime warnings: {:?}",
            runtime.warnings()
        );
        let mut missing = Vec::new();
        for locale in [Locale::PtBr, Locale::En] {
            runtime
                .set_setting(
                    "i18n.language.mode",
                    SettingValue::Text(match locale {
                        Locale::PtBr => "pt_BR".into(),
                        Locale::En => "en".into(),
                    }),
                )
                .ok();
            let specs: Vec<_> = runtime.registry().settings().to_vec();
            for spec in specs {
                if let SettingKind::Choice { options } = &spec.kind {
                    for option in options {
                        let label = choice_label_for(runtime.i18n(), &spec.key, option);
                        if label == *option {
                            missing.push(format!("{locale:?}: {} -> {option}", spec.key));
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "untranslated choice options: {missing:?}");
    }

    #[test]
    fn bounds_hint_and_humanized_setting_errors_show_limits() {
        let dir = TempGuard(
            std::env::temp_dir().join(format!("forja-shell-bounds-{}", std::process::id())),
        );
        let mut runtime = crate::app::build_runtime(
            Some("pt-BR".into()),
            Some(dir.0.clone()),
            Some(dir.0.join("u")),
            true,
        )
        .expect("runtime");
        assert_eq!(
            bounds_hint_for(runtime.i18n(), Some("8".into()), Some("32".into())),
            Some("Aceito: 8 a 32".to_owned())
        );
        assert_eq!(
            bounds_hint_for(runtime.i18n(), Some("10".into()), None),
            Some("Mínimo: 10".to_owned())
        );
        let color_err = humanize_setting_error_for(
            runtime.i18n(),
            runtime.registry(),
            "appearance.colors.accent",
            "invalid setting: `appearance.colors.accent` must be a color in #RRGGBB form",
        );
        assert!(color_err.contains("#RRGGBB"), "{color_err}");
        assert!(!color_err.to_ascii_lowercase().contains("invalid setting"), "{color_err}");
        assert!(!color_err.contains("appearance.colors.accent"), "{color_err}");

        let min_err = humanize_setting_error_for(
            runtime.i18n(),
            runtime.registry(),
            "appearance.sidebar.width",
            "invalid setting: `appearance.sidebar.width` must be at least 160",
        );
        assert!(min_err.contains("160"), "{min_err}");
        assert!(!min_err.to_ascii_lowercase().contains("invalid setting"), "{min_err}");
        assert!(!min_err.contains('`'), "{min_err}");

        runtime.set_locale(forja_kernel::Locale::En);
        let color_err_en = humanize_setting_error_for(
            runtime.i18n(),
            runtime.registry(),
            "appearance.colors.accent",
            "invalid setting: `appearance.colors.accent` must be a color in #RRGGBB form",
        );
        assert!(color_err_en.contains("#RRGGBB"), "{color_err_en}");
        assert!(
            !color_err_en.to_ascii_lowercase().contains("invalid setting"),
            "{color_err_en}"
        );
        assert!(!color_err_en.contains("appearance.colors.accent"), "{color_err_en}");
    }
}
