//! Standard settings controls and inline validation.
use crate::app::ForjaApp;
use crate::overlays::{bounds_hint_for, humanize_setting_error_for, parse_hex_color};
use crate::settings_catalog::catalog;
use eframe::egui::{self, TextEdit};
use forja_kernel::{SettingKind, SettingSpec, SettingValue};

impl ForjaApp {
    pub(crate) fn settings_row(&mut self, ui: &mut egui::Ui, spec: &SettingSpec) {
        let value = self
            .runtime
            .settings()
            .user_get(&spec.key)
            .unwrap_or(&spec.default)
            .clone();
        let label = self.text(&spec.label_key);
        let mut help = self.text(&spec.help_key);
        let limits = match spec.kind {
            SettingKind::Integer { min, max } => {
                (min.map(|v| v.to_string()), max.map(|v| v.to_string()))
            }
            SettingKind::Number { min, max } => {
                (min.map(|v| v.to_string()), max.map(|v| v.to_string()))
            }
            _ => (None, None),
        };
        if let Some(bounds) = bounds_hint_for(self.runtime.i18n(), limits.0, limits.1) {
            help = format!("{help} {bounds}");
        }
        let modified_label = self.text("shell.settings.modified");
        let restore_label = self.text("shell.settings.restore");
        let notice = catalog()
            .restart(&spec.key)
            .then(|| self.text("shell.settings.restart"));
        let error = self.settings_page.errors.get(&spec.key).cloned();
        let query = self.settings_query.clone();
        ui.push_id(&spec.key, |ui| {
            let (changed, restore) = forja_ui_kit::setting_row(
                ui,
                forja_ui_kit::SettingRow {
                    label: &label,
                    help: &help,
                    query: &query,
                    modified: value != spec.default,
                    modified_label: &modified_label,
                    restore_label: &restore_label,
                    notice: notice.as_deref(),
                    error: error.as_deref(),
                },
                |ui| self.setting_control(ui, spec, &value, &label),
            );
            if restore {
                self.setting_drafts.remove(&spec.key);
                self.apply_setting(&spec.key, spec.default.clone());
            } else if let Some(value) = changed {
                self.apply_setting(&spec.key, value);
            }
        });
    }

    fn setting_control(
        &mut self,
        ui: &mut egui::Ui,
        spec: &SettingSpec,
        current: &SettingValue,
        label: &str,
    ) -> Option<SettingValue> {
        match &spec.kind {
            SettingKind::Bool => {
                let mut value = current.as_bool().unwrap_or(false);
                let response = forja_ui_kit::toggle_labeled(ui, &mut value, label);
                response.changed().then_some(SettingValue::Bool(value))
            }
            SettingKind::Integer { min, max } => {
                let mut value = current.as_i64().unwrap_or(0);
                let response = ui.add(
                    egui::DragValue::new(&mut value)
                        .range(min.unwrap_or(i64::MIN)..=max.unwrap_or(i64::MAX)),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::DragValue, ui.is_enabled(), label)
                });
                response.changed().then_some(SettingValue::Integer(value))
            }
            SettingKind::Number { min, max } => {
                let mut value = current.as_f64().unwrap_or(0.0);
                let response = ui.add(
                    egui::DragValue::new(&mut value)
                        .speed(0.05)
                        .range(min.unwrap_or(f64::MIN)..=max.unwrap_or(f64::MAX)),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::DragValue, ui.is_enabled(), label)
                });
                response.changed().then_some(SettingValue::Number(value))
            }
            SettingKind::Path
                if matches!(
                    spec.key.as_str(),
                    "app.storage.data_dir" | "app.storage.content_dir"
                ) =>
            {
                self.storage_path_setting(ui, spec, current, label);
                None
            }
            SettingKind::Text { .. } | SettingKind::Path | SettingKind::Color => {
                self.text_setting(ui, spec, current, label)
            }
            SettingKind::Choice { options } => {
                let mut index = options
                    .iter()
                    .position(|option| Some(option.as_str()) == current.as_str())
                    .unwrap_or(0);
                let labels: Vec<String> = options
                    .iter()
                    .map(|option| {
                        crate::overlays::choice_label_for(self.runtime.i18n(), &spec.key, option)
                    })
                    .collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                let response = forja_ui_kit::select(ui, &mut index, &refs);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), label)
                });
                if response.changed() {
                    options
                        .get(index)
                        .map(|value| SettingValue::Text(value.clone()))
                } else {
                    None
                }
            }
            SettingKind::List { .. } => {
                let original = current
                    .as_list()
                    .map(|values| values.join(", "))
                    .unwrap_or_default();
                let mut value = self
                    .setting_drafts
                    .get(&spec.key)
                    .cloned()
                    .unwrap_or(original);
                let response = ui.add(
                    TextEdit::singleline(&mut value).desired_width(ui.available_width().min(190.0)),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label)
                });
                if response.changed() {
                    self.setting_drafts.insert(spec.key.clone(), value.clone());
                    Some(SettingValue::List(
                        value
                            .split(',')
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(str::to_owned)
                            .collect(),
                    ))
                } else {
                    None
                }
            }
        }
    }

    fn text_setting(
        &mut self,
        ui: &mut egui::Ui,
        spec: &SettingSpec,
        current: &SettingValue,
        label: &str,
    ) -> Option<SettingValue> {
        let original = current.as_str().unwrap_or("");
        let mut value = self
            .setting_drafts
            .get(&spec.key)
            .cloned()
            .unwrap_or_else(|| original.to_owned());
        let width = (ui.available_width() - 24.0).clamp(70.0, 180.0);
        if matches!(spec.kind, SettingKind::Color) {
            if let Some(rgb) = parse_hex_color(&value) {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    rect,
                    3.0,
                    egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                );
            }
        }
        let response = ui.add(TextEdit::singleline(&mut value).desired_width(width));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label)
        });
        if response.changed() {
            self.setting_drafts.insert(spec.key.clone(), value.clone());
            Some(SettingValue::Text(value))
        } else {
            None
        }
    }

    pub(crate) fn apply_setting(&mut self, key: &str, value: SettingValue) {
        match self.runtime.set_setting(key, value) {
            Ok(()) => {
                self.settings_page.errors.remove(key);
            }
            Err(error) => {
                let message = humanize_setting_error_for(
                    self.runtime.i18n(),
                    self.runtime.registry(),
                    key,
                    &error.to_string(),
                );
                self.settings_page.errors.insert(key.to_owned(), message);
            }
        }
    }
}
