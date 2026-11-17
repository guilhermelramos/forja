//! Keymap overlay and shared settings copy/keyboard helpers.
use crate::app::ForjaApp;
use eframe::egui::{self, RichText};
#[cfg(test)]
use forja_kernel::settings::{SettingKind, SettingValue};

impl ForjaApp {
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
}

/// PageUp/PageDown scroll one screen of the settings list, like the wheel.
pub(crate) fn apply_page_scroll(ui: &egui::Ui) {
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
pub(crate) fn reveal_focused_control(ui: &egui::Ui) {
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
        (Some(min), Some(max)) => {
            Some(i18n.t_args("shell.settings.range", &[("min", &min), ("max", &max)]))
        }
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

#[cfg(test)]
fn setting_matches(query: &str, key: &str, label: &str) -> bool {
    query.is_empty()
        || key.to_lowercase().contains(query)
        || label.match_indices(query).any(|(index, _)| {
            label[..index]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_alphanumeric())
        })
}

pub(crate) fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
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
        assert!(!setting_matches(
            "tema",
            "browser.system",
            "login no sistema"
        ));
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
                    let output = egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .show(ui, |ui| {
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
        assert!(
            missing.is_empty(),
            "untranslated choice options: {missing:?}"
        );
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
        assert!(
            !color_err.to_ascii_lowercase().contains("invalid setting"),
            "{color_err}"
        );
        assert!(
            !color_err.contains("appearance.colors.accent"),
            "{color_err}"
        );

        let min_err = humanize_setting_error_for(
            runtime.i18n(),
            runtime.registry(),
            "appearance.sidebar.width",
            "invalid setting: `appearance.sidebar.width` must be at least 160",
        );
        assert!(min_err.contains("160"), "{min_err}");
        assert!(
            !min_err.to_ascii_lowercase().contains("invalid setting"),
            "{min_err}"
        );
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
            !color_err_en
                .to_ascii_lowercase()
                .contains("invalid setting"),
            "{color_err_en}"
        );
        assert!(
            !color_err_en.contains("appearance.colors.accent"),
            "{color_err_en}"
        );
    }
}
