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

/// Everything the theme depends on, cheap to compare every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Look {
    settings: u64,
    theme: u64,
    system_dark: bool,
    scale: u32,
    native: u32,
}

impl Look {
    pub(crate) fn new(settings: u64, theme: u64, system_dark: bool, scale: f32, native: f32) -> Self {
        Self {
            settings,
            theme,
            system_dark,
            scale: scale.to_bits(),
            native: native.to_bits(),
        }
    }
}

/// Remembers the look that was applied so the theme is rebuilt only when one
/// of its inputs changed.
#[derive(Default)]
pub(crate) struct LookGate {
    applied: Option<Look>,
}

impl LookGate {
    /// True when the theme must be rebuilt; records `look` as applied.
    pub(crate) fn accepts(&mut self, look: Look) -> bool {
        if self.applied == Some(look) {
            return false;
        }
        self.applied = Some(look);
        true
    }
}

impl ForjaApp {
    pub(crate) fn refresh_look(&mut self, ctx: &egui::Context) {
        if self.system_probed.elapsed() > Duration::from_secs(1) {
            self.system_dark = system::system_is_dark();
            self.system_probed = Instant::now();
        }
        self.poll_interface_theme();
        let scale = self
            .setting_number("appearance.ui.scale", 1.0)
            .clamp(0.8, 1.5) as f32;
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let look = Look::new(
            self.runtime.settings_generation(),
            self.theme_revision,
            self.system_dark,
            scale,
            native,
        );
        if self.look_gate.accepts(look) {
            self.apply_look(ctx, scale, native);
        }

        let fallback = Locale::parse(&self.setting_text("i18n.language.fallback", "en"));
        if self.runtime.i18n().fallback() != fallback {
            self.runtime.set_fallback_locale(fallback);
        }
        if self.runtime.take_locale_setting_changed() {
            self.locale_locked = false;
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

    /// Reads the user's interface theme file every couple of seconds. Only a
    /// real change is recorded, so unchanged themes never rebuild the style.
    fn poll_interface_theme(&mut self) {
        if self.theme_checked.elapsed() <= Duration::from_secs(2) {
            return;
        }
        self.theme_checked = Instant::now();
        let mode = self
            .theme_override
            .clone()
            .unwrap_or_else(|| self.setting_text("appearance.theme.mode", "system"));
        let dark = match mode.as_str() {
            "light" => false,
            "dark" => true,
            _ => self.system_dark,
        };
        let mut args = Args::new();
        args.insert("mode", json!(if dark { "dark" } else { "light" }));
        let theme = self
            .runtime
            .run_command("content.themes.ui", args)
            .ok()
            .and_then(|value| {
                value
                    .get("toml")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            });
        if theme != self.interface_theme {
            self.interface_theme = theme;
            self.theme_revision = self.theme_revision.wrapping_add(1);
        }
    }

    /// Rebuilds the tokens from the settings and applies the whole egui style.
    /// Runs only when [`LookGate`] says something changed.
    fn apply_look(&mut self, ctx: &egui::Context, scale: f32, native: f32) {
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
        if let Some(toml) = &self.interface_theme {
            if let Ok(mut themed) = Tokens::from_toml(toml) {
                themed.density = tokens.density;
                themed.reduce_motion = tokens.reduce_motion;
                tokens = themed;
            }
        }
        tokens.motion_duration_ms = self.setting_number("appearance.motion.duration_ms", 150.0).clamp(0.0, 250.0) as f32;
        tokens.tooltips_enabled = self.setting_bool("appearance.tooltips.enabled", true);
        tokens.deterministic = cfg!(test) || self.driver.is_some() || self.screenshot.is_some();
        self.tokens = tokens;
        let accent = self.setting_text("appearance.colors.accent", "#F2762E");
        apply_accent(&mut self.tokens, &accent);
        if !self.fonts_ready {
            let _ = forja_ui_kit::install_fonts(ctx);
            self.fonts_ready = true;
        }
        forja_ui_kit::apply(ctx, &self.tokens);
        ctx.set_pixels_per_point(native * scale);
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
    use super::*;
    use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue, Settings};
    use std::path::PathBuf;

    fn specs() -> Vec<SettingSpec> {
        vec![SettingSpec::new(
            "appearance.theme.mode",
            SettingKind::Choice {
                options: vec!["system".into(), "dark".into()],
            },
            SettingValue::Text("system".into()),
            "appearance.theme.mode.label",
            "appearance.theme.mode.help",
        )]
    }

    fn applies(gate: &mut LookGate, settings: &Settings, dark: bool, scale: f32) -> bool {
        gate.accepts(Look::new(
            settings.generation(),
            0,
            dark,
            scale,
            1.0,
        ))
    }

    #[test]
    fn theme_is_reapplied_only_when_an_input_changed() {
        let specs = specs();
        let (mut settings, _) = Settings::load(PathBuf::from("forja-test/settings.toml"), &specs);
        let mut gate = LookGate::default();
        assert!(applies(&mut gate, &settings, true, 1.0));
        assert!(
            !applies(&mut gate, &settings, true, 1.0),
            "two frames without a change must apply the theme once"
        );

        settings
            .set_memory(
                "appearance.theme.mode",
                SettingValue::Text("dark".into()),
                &specs,
            )
            .unwrap();
        assert!(
            applies(&mut gate, &settings, true, 1.0),
            "an appearance setting change must reapply the theme"
        );
        assert!(!applies(&mut gate, &settings, true, 1.0));

        assert!(
            applies(&mut gate, &settings, false, 1.0),
            "a system theme flip must reapply the theme"
        );
        assert!(
            applies(&mut gate, &settings, false, 1.25),
            "a screen scale change must reapply the theme"
        );
    }

    #[test]
    fn accent_changes_palette_and_selects_a_readable_foreground() {
        let mut tokens = forja_ui_kit::Tokens::brasa_dark();
        super::apply_accent(&mut tokens, "#0000FF");
        assert_eq!(tokens.color.accent, forja_ui_kit::Color::rgb(0, 0, 255));
        assert_eq!(tokens.color.on_accent, forja_ui_kit::Color::rgb(255, 255, 255));
    }

    #[test]
    fn motion_duration_is_wired_and_reduction_zeros_every_transition() {
        use forja_ui_kit::Motion;

        let spec = crate::hosts_motion::specs()
            .into_iter()
            .find(|spec| spec.key == "appearance.motion.duration_ms")
            .expect("motion duration is registered");
        let base = spec.default.as_f64().expect("a number");
        assert!(base > 0.0, "the design duration must actually be wired");
        let mut tokens = forja_ui_kit::Tokens::brasa_dark();
        tokens.motion_duration_ms = base as f32;
        assert!(tokens.duration(Motion::Base) > 0.0);
        tokens.reduce_motion = true;
        for motion in [Motion::Fast, Motion::Base, Motion::Slow] {
            assert_eq!(tokens.duration(motion), 0.0);
        }
    }
}
