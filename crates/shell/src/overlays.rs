//! Settings window. The Bigorna lives in `anvil.rs`.

use crate::app::ForjaApp;
use eframe::egui::{self, Color32, RichText, Sense, TextEdit};
use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue};
use forja_ui_kit::text_field;
use std::collections::BTreeMap;

impl ForjaApp {
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = self.settings_open;
        let title = self.text("shell.settings.title");
        let modal = egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(680.0)
            .show(ctx, |ui| self.settings_contents(ui));
        if let Some(modal) = modal {
            self.slot_rects
                .insert("settings".into(), modal.response.rect);
        }
        self.settings_open = open;
        if !open {
            self.settings_section = None;
        }
    }

    /// Window opened by `help.keymap`; lists every resolved binding.
    pub(crate) fn keymap_window(&mut self, ctx: &egui::Context) {
        if !self.keymap_open {
            return;
        }
        let mut open = self.keymap_open;
        egui::Window::new(self.text("shell.keymap.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(560.0)
            .show(ctx, |ui| {
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
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
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

    fn settings_contents(&mut self, ui: &mut egui::Ui) {
        // `--settings <módulo>` (captura): só a seção desse módulo; sem o flag, tudo como antes.
        let section = self.settings_section.clone();
        self.settings_body(ui, section.as_deref());
    }

    fn settings_body(&mut self, ui: &mut egui::Ui, section: Option<&str>) {
        // Com `--settings`, a seção própria do módulo (se houver) substitui as linhas dele.
        let section_has_view =
            section.is_some_and(|module| self.settings_views.iter().any(|view| view.module == module));
        let hint = self.text("shell.settings.search");
        text_field(ui, &mut self.settings_query, &hint);
        ui.add_space(6.0);

        let query = self.settings_query.trim().to_lowercase();
        let specs = self.runtime.registry().settings().to_vec();
        let modules = self.runtime.registry().modules().to_vec();
        let values: BTreeMap<String, SettingValue> = self.runtime.settings().values().clone();

        let mut by_module: BTreeMap<String, Vec<SettingSpec>> = BTreeMap::new();
        for spec in specs {
            let label = self.text(&spec.label_key).to_lowercase();
            let help = self.text(&spec.help_key).to_lowercase();
            let matches = query.is_empty()
                || spec.key.to_lowercase().contains(&query)
                || label.contains(&query)
                || help.contains(&query);
            let in_section = section.is_none_or(|module| spec.module() == module && !section_has_view);
            if matches && in_section {
                by_module
                    .entry(spec.module().to_owned())
                    .or_default()
                    .push(spec);
            }
        }

        // Com `--settings`, a lista pode estar vazia de propósito: a seção do módulo é que aparece.
        if by_module.is_empty() && section.is_none() {
            ui.label(
                RichText::new(self.text("shell.settings.empty"))
                    .color(self.paint(self.tokens.color.text_muted)),
            );
        }

        for (module_id, mut module_specs) in by_module {
            module_specs.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.key.cmp(&b.key)));
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

        if !self.settings_views.is_empty() {
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
    }

    fn setting_row(&mut self, ui: &mut egui::Ui, spec: &SettingSpec, current: Option<&SettingValue>) {
        ui.push_id(&spec.key, |ui| {
            ui.horizontal(|ui| {
                ui.label(self.text(&spec.label_key))
                    .on_hover_text(self.text(&spec.help_key));
                let mut changed: Option<SettingValue> = None;
                match &spec.kind {
                    SettingKind::Bool => {
                        let mut value = current.and_then(SettingValue::as_bool).unwrap_or(false);
                        if ui.checkbox(&mut value, "").changed() {
                            changed = Some(SettingValue::Bool(value));
                        }
                    }
                    SettingKind::Integer { min, max } => {
                        let mut value = current.and_then(SettingValue::as_i64).unwrap_or(0);
                        let mut drag = egui::DragValue::new(&mut value);
                        if min.is_some() || max.is_some() {
                            drag = drag.range(min.unwrap_or(i64::MIN)..=max.unwrap_or(i64::MAX));
                        }
                        if ui.add(drag).changed() {
                            changed = Some(SettingValue::Integer(value));
                        }
                    }
                    SettingKind::Number { min, max } => {
                        let mut value = current.and_then(SettingValue::as_f64).unwrap_or(0.0);
                        let mut drag = egui::DragValue::new(&mut value).speed(0.05);
                        if min.is_some() || max.is_some() {
                            drag = drag.range(min.unwrap_or(f64::MIN)..=max.unwrap_or(f64::MAX));
                        }
                        if ui.add(drag).changed() {
                            changed = Some(SettingValue::Number(value));
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
                        let mut value = original.clone();
                        egui::ComboBox::from_id_salt(&spec.key)
                            .selected_text(value.clone())
                            .show_ui(ui, |ui| {
                                for option in options {
                                    ui.selectable_value(&mut value, option.clone(), option);
                                }
                            });
                        if value != original {
                            changed = Some(SettingValue::Text(value));
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

    fn apply_setting(&mut self, key: &str, value: SettingValue) {
        if let Err(error) = self.runtime.set_setting(key, value) {
            let message = error.to_string();
            let text = self.text_args("shell.settings.save-failed", &[("message", &message)]);
            self.status = Some(text);
        }
    }
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
