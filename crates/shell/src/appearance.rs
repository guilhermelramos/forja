//! Appearance, language and zoom read from the catalog settings.

use crate::app::ForjaApp;
use crate::system;
use crate::theme::tokens_for;
use eframe::egui;
use forja_kernel::settings::SettingValue;
use forja_kernel::{Args, Locale};
use forja_ui_kit::{Color, Tokens};
use serde_json::json;
use std::time::{Duration, Instant};

impl ForjaApp {
    pub(crate) fn refresh_look(&mut self, ctx: &egui::Context) {
        if self.system_probed.elapsed() > Duration::from_secs(1) {
            self.system_dark = system::system_is_dark();
            self.system_probed = Instant::now();
        }
        let mode = self
            .theme_override
            .clone()
            .unwrap_or_else(|| self.setting_text("appearance.theme.mode", "system"));
        let density = self
            .density_override
            .clone()
            .unwrap_or_else(|| self.setting_text("appearance.ui.density", "comfortable"));
        let dark_name = self.setting_text("appearance.theme.dark_name", "brasa_escuro");
        let light_name = self.setting_text("appearance.theme.light_name", "brasa_claro");
        let motion = self.setting_text("appearance.motion.reduce", "auto");
        let reduce = match motion.as_str() {
            "always" => true,
            "never" => false,
            _ => system::system_reduce_motion(),
        };
        let mut tokens = tokens_for(
            &mode,
            &density,
            reduce,
            self.system_dark,
            &dark_name,
            &light_name,
        );
        if self.theme_checked.elapsed() > Duration::from_secs(2) {
            self.theme_checked = Instant::now();
            let mut args = Args::new();
            args.insert("mode", json!(if tokens.mode == forja_ui_kit::Mode::Light { "light" } else { "dark" }));
            self.interface_theme = self
                .runtime
                .run_command("content.themes.ui", args)
                .ok()
                .and_then(|value| {
                    value
                        .get("toml")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_owned)
                });
        }
        if let Some(toml) = &self.interface_theme {
            if let Ok(mut themed) = Tokens::from_toml(toml) {
                themed.density = tokens.density;
                themed.reduce_motion = tokens.reduce_motion;
                tokens = themed;
            }
        }
        self.tokens = tokens;
        let accent = self.setting_text("appearance.colors.accent", "#F2762E");
        apply_accent(&mut self.tokens, &accent);
        if !self.fonts_ready {
            let _ = forja_ui_kit::install_fonts(ctx);
            self.fonts_ready = true;
        }
        forja_ui_kit::apply(ctx, &self.tokens);
        let scale = self
            .setting_number("appearance.ui.scale", 1.0)
            .clamp(0.8, 1.5) as f32;
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        ctx.set_pixels_per_point(native * scale);

        let fallback = Locale::parse(&self.setting_text("i18n.language.fallback", "en"));
        if self.runtime.i18n().fallback() != fallback {
            self.runtime.set_fallback_locale(fallback);
        }
        if !self.locale_locked {
            let mode = self.setting_text("i18n.language.mode", "auto");
            let locale = match mode.as_str() {
                "en" => Locale::En,
                "pt_BR" => Locale::PtBr,
                _ => system::system_locale(),
            };
            if self.runtime.i18n().locale() != locale {
                self.runtime.set_locale(locale);
            }
        }
    }

    pub(crate) fn set_theme_mode(&mut self, mode: &str) {
        if self.theme_override.is_some() {
            return;
        }
        let _ = self
            .runtime
            .set_setting("appearance.theme.mode", SettingValue::Text(mode.to_owned()));
    }

    pub(crate) fn theme_label_key(&self) -> &'static str {
        let mode = if let Some(mode) = &self.theme_override {
            mode.clone()
        } else {
            self.setting_text("appearance.theme.mode", "system")
        };
        match mode.as_str() {
            "dark" => "shell.theme.dark",
            "light" => "shell.theme.light",
            _ => "shell.theme.system",
        }
    }

    pub(crate) fn setting_text(&self, key: &str, default: &str) -> String {
        self.runtime
            .settings()
            .values()
            .get(key)
            .and_then(SettingValue::as_str)
            .unwrap_or(default)
            .to_owned()
    }

    pub(crate) fn setting_bool(&self, key: &str, default: bool) -> bool {
        self.runtime
            .settings()
            .values()
            .get(key)
            .and_then(SettingValue::as_bool)
            .unwrap_or(default)
    }

    pub(crate) fn setting_number(&self, key: &str, default: f64) -> f64 {
        self.runtime
            .settings()
            .values()
            .get(key)
            .and_then(SettingValue::as_f64)
            .unwrap_or(default)
    }

    pub(crate) fn paint(&self, color: Color) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
    }
}

fn apply_accent(tokens: &mut forja_ui_kit::Tokens, value: &str) {
    if let Ok(accent) = Color::parse(value) {
        tokens.color.accent = accent;
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        tokens.color.on_accent = if forja_ui_kit::contrast_ratio(accent, black)
            >= forja_ui_kit::contrast_ratio(accent, white) { black } else { white };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn accent_changes_palette_and_selects_a_readable_foreground() {
        let mut tokens = forja_ui_kit::Tokens::brasa_dark();
        super::apply_accent(&mut tokens, "#0000FF");
        assert_eq!(tokens.color.accent, forja_ui_kit::Color::rgb(0, 0, 255));
        assert_eq!(tokens.color.on_accent, forja_ui_kit::Color::rgb(255, 255, 255));
    }
}
