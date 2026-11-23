//! Leitura das 24 configurações do grupo Terminal do catálogo.
//!
//! As chaves, padrões e faixas vêm de `design/settings.toml`; nenhum nome novo
//! é inventado. A view e os comandos usam este retrato para aplicar o efeito
//! real de cada opção.

use forja_kernel::{Ctx, SettingValue};

/// Retrato imutável das configurações do grupo Terminal.
#[derive(Clone, Debug, PartialEq)]
pub struct TermSettings {
    /// `terminal.shell.default`.
    pub shell_default: String,
    /// `terminal.shell.path`.
    pub shell_path: String,
    /// `terminal.shell.start_dir`.
    pub start_dir: String,
    /// `terminal.shell.env`.
    pub environment: Vec<String>,
    /// `terminal.scroll.history_lines`.
    pub history_lines: i64,
    /// `terminal.scroll.wheel_lines`.
    pub wheel_lines: i64,
    /// `terminal.scroll.follow_output`.
    pub follow_output: bool,
    /// `terminal.cursor.style`.
    pub cursor_style: String,
    /// `terminal.font.family`.
    pub font_family: String,
    /// `terminal.font.size`.
    pub font_size: f64,
    /// `terminal.theme.name`.
    pub theme_name: String,
    /// `terminal.theme.follow_app`.
    pub theme_follow_app: bool,
    /// `terminal.color.opacity`.
    pub opacity: f64,
    /// `terminal.bell.mode`.
    pub bell_mode: String,
    /// `terminal.copy.on_select`.
    pub copy_on_select: bool,
    /// `terminal.recipes.press_enter`: Enter after a recipe is pasted.
    pub recipe_press_enter: bool,
    /// `terminal.recipes.confirm`: ask before running a recipe.
    pub recipe_confirm: bool,
    /// `terminal.recipes.show_scope`: `all`, `global` or `project`.
    pub recipe_show_scope: String,
    /// `terminal.recipes.shell`: `inherit`, `pwsh`, `powershell` or `cmd`.
    pub recipe_shell: String,
    /// `terminal.copy.trim`.
    pub copy_trim: bool,
    /// `terminal.paste.warn_multiline`.
    pub paste_warn_multiline: bool,
    /// `terminal.paste.bracketed`.
    pub paste_bracketed: bool,
    /// `terminal.links.confirm`.
    pub links_confirm: bool,
    /// `terminal.links.open_with`.
    pub links_open_with: String,
    /// `terminal.shell.marks`.
    pub shell_marks: bool,
    /// `terminal.close.confirm_running`.
    pub close_confirm_running: bool,
    /// Whether closing an active helper needs confirmation.
    pub helper_close_confirm_running: bool,
    /// `terminal.images.enabled`.
    pub images_enabled: bool,
    /// `terminal.osc52.enabled`.
    pub osc52_enabled: bool,
}

impl Default for TermSettings {
    fn default() -> Self {
        Self {
            shell_default: "auto".to_owned(),
            shell_path: String::new(),
            start_dir: "bench".to_owned(),
            environment: Vec::new(),
            history_lines: 10_000,
            wheel_lines: 3,
            follow_output: true,
            cursor_style: "block".to_owned(),
            font_family: "inherit".to_owned(),
            font_size: 13.0,
            theme_name: "brasa_escuro".to_owned(),
            theme_follow_app: true,
            opacity: 1.0,
            bell_mode: "visual".to_owned(),
            copy_on_select: false,
            recipe_press_enter: true,
            recipe_confirm: false,
            recipe_show_scope: "all".to_owned(),
            recipe_shell: "inherit".to_owned(),
            copy_trim: true,
            paste_warn_multiline: true,
            paste_bracketed: true,
            links_confirm: true,
            links_open_with: "system".to_owned(),
            shell_marks: true,
            close_confirm_running: true,
            helper_close_confirm_running: true,
            images_enabled: true,
            osc52_enabled: false,
        }
    }
}

impl TermSettings {
    /// Lê o retrato das configurações do runtime.
    pub fn from_ctx(cx: &Ctx<'_>) -> Self {
        let defaults = Self::default();
        let text = |key: &str, fallback: &str| {
            cx.setting(key)
                .and_then(SettingValue::as_str)
                .unwrap_or(fallback)
                .to_owned()
        };
        let boolean = |key: &str, fallback: bool| {
            cx.setting(key).and_then(SettingValue::as_bool).unwrap_or(fallback)
        };
        let integer = |key: &str, fallback: i64| {
            cx.setting(key).and_then(SettingValue::as_i64).unwrap_or(fallback)
        };
        let number = |key: &str, fallback: f64| {
            cx.setting(key).and_then(SettingValue::as_f64).unwrap_or(fallback)
        };
        Self {
            shell_default: text("terminal.shell.default", &defaults.shell_default),
            shell_path: text("terminal.shell.path", &defaults.shell_path),
            start_dir: text("terminal.shell.start_dir", &defaults.start_dir),
            environment: cx
                .setting("terminal.shell.env")
                .and_then(SettingValue::as_list)
                .map(<[String]>::to_vec)
                .unwrap_or_default(),
            history_lines: integer("terminal.scroll.history_lines", defaults.history_lines),
            wheel_lines: integer("terminal.scroll.wheel_lines", defaults.wheel_lines),
            follow_output: boolean("terminal.scroll.follow_output", defaults.follow_output),
            cursor_style: text("terminal.cursor.style", &defaults.cursor_style),
            font_family: text("terminal.font.family", &defaults.font_family),
            font_size: number("terminal.font.size", defaults.font_size),
            theme_name: text("terminal.theme.name", &defaults.theme_name),
            theme_follow_app: boolean("terminal.theme.follow_app", defaults.theme_follow_app),
            opacity: number("terminal.color.opacity", defaults.opacity),
            bell_mode: text("terminal.bell.mode", &defaults.bell_mode),
            copy_on_select: boolean("terminal.copy.on_select", defaults.copy_on_select),
            recipe_press_enter: boolean("terminal.recipes.press_enter", defaults.recipe_press_enter),
            recipe_confirm: boolean("terminal.recipes.confirm", defaults.recipe_confirm),
            recipe_show_scope: text("terminal.recipes.show_scope", &defaults.recipe_show_scope),
            recipe_shell: text("terminal.recipes.shell", &defaults.recipe_shell),
            copy_trim: boolean("terminal.copy.trim", defaults.copy_trim),
            paste_warn_multiline: boolean(
                "terminal.paste.warn_multiline",
                defaults.paste_warn_multiline,
            ),
            paste_bracketed: boolean("terminal.paste.bracketed", defaults.paste_bracketed),
            links_confirm: boolean("terminal.links.confirm", defaults.links_confirm),
            links_open_with: text("terminal.links.open_with", &defaults.links_open_with),
            shell_marks: boolean("terminal.shell.marks", defaults.shell_marks),
            close_confirm_running: boolean(
                "terminal.close.confirm_running",
                defaults.close_confirm_running,
            ),
            helper_close_confirm_running: boolean("helper.close.confirm_running", true),
            images_enabled: boolean("terminal.images.enabled", defaults.images_enabled),
            osc52_enabled: boolean("terminal.osc52.enabled", defaults.osc52_enabled),
        }
    }

    /// Teto de histórico validado.
    pub fn history_limit(&self) -> usize {
        self.history_lines.clamp(1_000, 200_000) as usize
    }

    /// Linhas por entalhe da roda.
    pub fn wheel_notch(&self) -> f32 {
        self.wheel_lines.clamp(1, 10) as f32
    }

    /// Tamanho de fonte validado.
    pub fn font_size(&self) -> f32 {
        self.font_size.clamp(8.0, 32.0) as f32
    }

    /// Opacidade do fundo validada.
    pub fn background_opacity(&self) -> f64 {
        self.opacity.clamp(0.6, 1.0)
    }

    /// Program that runs recipes in a new tab, or `None` to use the focused tab.
    pub fn recipe_program(&self) -> Option<&'static str> {
        match self.recipe_shell.as_str() {
            "pwsh" => Some("pwsh"),
            "powershell" => Some("powershell"),
            "cmd" => Some("cmd"),
            _ => None,
        }
    }

    /// Whether a recipe of this scope appears in the picker.
    pub fn recipe_listed(&self, scope: &str) -> bool {
        match self.recipe_show_scope.as_str() {
            "global" => scope == "global",
            "project" => scope == "project",
            _ => true,
        }
    }

    /// Aviso visual de campainha.
    pub fn bell_visual(&self) -> bool {
        matches!(self.bell_mode.as_str(), "visual" | "both")
    }

    /// Aviso sonoro de campainha.
    pub fn bell_audible(&self) -> bool {
        matches!(self.bell_mode.as_str(), "audible" | "both")
    }
}

/// Lê a chave antiga `terminal.gpu.mode` de um `settings.toml` quando
/// `terminal.images.enabled` ainda não foi gravada, para compatibilidade.
pub fn legacy_images_enabled_from_toml(text: &str) -> Option<bool> {
    let table: toml::Table = toml::from_str(text).ok()?;
    let terminal = table.get("terminal")?.as_table()?;
    if terminal
        .get("images")
        .and_then(toml::Value::as_table)
        .and_then(|images| images.get("enabled"))
        .and_then(toml::Value::as_bool)
        .is_some()
    {
        return None;
    }
    let mode = terminal
        .get("gpu")
        .and_then(toml::Value::as_table)
        .and_then(|gpu| gpu.get("mode"))
        .and_then(toml::Value::as_str)?;
    match mode {
        "off" => Some(false),
        "on" | "auto" => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_catalog() {
        let settings = TermSettings::default();
        assert_eq!(settings.shell_default, "auto");
        assert_eq!(settings.start_dir, "bench");
        assert_eq!(settings.history_limit(), 10_000);
        assert_eq!(settings.wheel_notch(), 3.0);
        assert!(settings.follow_output);
        assert_eq!(settings.cursor_style, "block");
        assert_eq!(settings.font_family, "inherit");
        assert_eq!(settings.font_size(), 13.0);
        assert_eq!(settings.theme_name, "brasa_escuro");
        assert!(settings.theme_follow_app);
        assert_eq!(settings.background_opacity(), 1.0);
        assert_eq!(settings.bell_mode, "visual");
        assert!(!settings.copy_on_select);
        assert!(settings.copy_trim);
        assert!(settings.paste_warn_multiline);
        assert!(settings.paste_bracketed);
        assert!(settings.links_confirm);
        assert_eq!(settings.links_open_with, "system");
        assert!(settings.shell_marks);
        assert!(settings.close_confirm_running);
        assert!(settings.images_enabled);
        assert!(!settings.osc52_enabled);
    }

    #[test]
    fn ranges_are_clamped() {
        let settings = TermSettings {
            history_lines: i64::MAX,
            wheel_lines: 99,
            font_size: 100.0,
            opacity: 0.0,
            ..TermSettings::default()
        };
        assert_eq!(settings.history_limit(), 200_000);
        assert_eq!(settings.wheel_notch(), 10.0);
        assert_eq!(settings.font_size(), 32.0);
        assert_eq!(settings.background_opacity(), 0.6);
    }
}

#[cfg(test)]
mod recipe_settings_tests {
    use super::*;

    #[test]
    fn scope_setting_filters_the_recipe_picker() {
        let mut settings = TermSettings::default();
        assert!(settings.recipe_listed("global") && settings.recipe_listed("project"));
        settings.recipe_show_scope = "global".to_owned();
        assert!(settings.recipe_listed("global"));
        assert!(!settings.recipe_listed("project"));
        settings.recipe_show_scope = "project".to_owned();
        assert!(!settings.recipe_listed("global"));
        assert!(settings.recipe_listed("project"));
    }

    #[test]
    fn shell_setting_picks_the_program_or_inherits() {
        let mut settings = TermSettings::default();
        assert_eq!(settings.recipe_program(), None);
        settings.recipe_shell = "pwsh".to_owned();
        assert_eq!(settings.recipe_program(), Some("pwsh"));
        settings.recipe_shell = "cmd".to_owned();
        assert_eq!(settings.recipe_program(), Some("cmd"));
    }

    #[test]
    fn recipe_defaults_match_the_catalog() {
        let settings = TermSettings::default();
        assert!(settings.recipe_press_enter);
        assert!(!settings.recipe_confirm);
        assert_eq!(settings.recipe_show_scope, "all");
        assert_eq!(settings.recipe_shell, "inherit");
    }

    #[test]
    fn legacy_gpu_mode_maps_to_images_enabled_until_new_key_exists() {
        assert_eq!(
            legacy_images_enabled_from_toml("terminal.gpu.mode = \"off\"\n"),
            Some(false)
        );
        assert_eq!(
            legacy_images_enabled_from_toml("[terminal.gpu]\nmode = \"off\"\n"),
            Some(false)
        );
        assert_eq!(
            legacy_images_enabled_from_toml("[terminal.gpu]\nmode = \"on\"\n"),
            Some(true)
        );
        assert_eq!(
            legacy_images_enabled_from_toml("[terminal.gpu]\nmode = \"auto\"\n"),
            Some(true)
        );
        assert_eq!(
            legacy_images_enabled_from_toml(
                "terminal.images.enabled = true\nterminal.gpu.mode = \"off\"\n"
            ),
            None
        );
        assert_eq!(legacy_images_enabled_from_toml(""), None);
    }
}
