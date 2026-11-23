//! Paletas de terminal carregadas de `content/themes/*.toml`.
//!
//! O `forja-ui-kit` já lê o TOML; aqui escolhemos a paleta a partir das
//! configurações e resolvemos as origens de cor guardadas na grade (ANSI,
//! 256 cores, padrão e RGB verdadeiro).

use forja_kernel::ui_contract::egui::Color32;
use forja_ui_kit::{Color, TerminalTheme};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const BUILTIN: [&str; 5] = [
    include_str!("../../../../content/themes/brasa-escuro.toml"),
    include_str!("../../../../content/themes/brasa-claro.toml"),
    include_str!("../../../../content/themes/carvao.toml"),
    include_str!("../../../../content/themes/cinza-quente.toml"),
    include_str!("../../../../content/themes/aco.toml"),
];

/// Paleta pronta para desenhar.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    /// Fundo da grade.
    pub background: Color32,
    /// Texto padrão.
    pub foreground: Color32,
    /// Cursor.
    pub cursor: Color32,
    /// Fundo da seleção.
    pub selection: Color32,
    /// Texto sobre a seleção.
    pub selection_foreground: Color32,
    /// As 16 cores ANSI.
    pub ansi: [Color32; 16],
}

/// Biblioteca de temas embutidos e descobertos.
#[derive(Clone, Debug, Default)]
pub struct ThemeLibrary {
    themes: BTreeMap<String, TerminalTheme>,
}

impl ThemeLibrary {
    /// Começa com os cinco temas do produto e acrescenta os do usuário/projeto.
    pub fn load(project: Option<&Path>) -> Self {
        let mut library = Self::default();
        for text in BUILTIN {
            if let Ok(theme) = TerminalTheme::from_toml(text) {
                library.insert(theme);
            }
        }
        if let Some(user) = user_themes_dir() {
            library.read_dir(&user);
        }
        if let Some(content) = std::env::var_os("FORJA_CONTENT_DIR") {
            library.read_dir(&PathBuf::from(content).join("themes"));
        }
        if let Some(project) = project {
            library.read_dir(&project.join(".forja").join("themes"));
        }
        library
    }

    /// Usa a lista entregue pelo módulo `content` (contrato
    /// `content.themes.list`). Cai para os temas embutidos quando a lista está
    /// vazia.
    pub fn from_texts(texts: &[String]) -> Self {
        let mut library = Self::default();
        for text in BUILTIN {
            if let Ok(theme) = TerminalTheme::from_toml(text) {
                library.insert(theme);
            }
        }
        for text in texts {
            if let Ok(theme) = TerminalTheme::from_toml(text) {
                library.insert(theme);
            }
        }
        library
    }

    fn insert(&mut self, theme: TerminalTheme) {
        self.themes.insert(normalize(&theme.id), theme);
    }

    fn read_dir(&mut self, directory: &Path) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
            .collect();
        files.sort();
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else {
                continue;
            };
            if let Ok(theme) = TerminalTheme::from_toml(&text) {
                self.insert(theme);
            }
        }
    }

    /// Escolhe a paleta: acompanha o app ou fixa o tema pedido.
    pub fn palette(&self, requested: &str, follow_app: bool, app_dark: bool) -> Palette {
        let name = if follow_app {
            if app_dark { "brasa-escuro" } else { "brasa-claro" }
        } else {
            requested
        };
        let key = normalize(name);
        let theme = self
            .themes
            .get(&key)
            .or_else(|| self.themes.get("brasa-escuro"));
        match theme {
            Some(theme) => palette_from(theme),
            None => fallback_dark(),
        }
    }

}

fn normalize(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .replace('_', "-")
}

fn palette_from(theme: &TerminalTheme) -> Palette {
    let ansi = theme.ansi.entries().map(|(_, color)| to_color32(color));
    Palette {
        background: to_color32(theme.background),
        foreground: to_color32(theme.foreground),
        cursor: to_color32(theme.cursor),
        selection: to_color32(theme.selection),
        selection_foreground: to_color32(theme.selection_foreground),
        ansi,
    }
}

fn to_color32(color: Color) -> Color32 {
    color.to_color32()
}

fn fallback_dark() -> Palette {
    Palette {
        background: Color32::from_rgb(0x12, 0x10, 0x10), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        foreground: Color32::from_rgb(0xED, 0xE7, 0xE1), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        cursor: Color32::from_rgb(0xFF, 0xC1, 0x4D), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        selection: Color32::from_rgb(0x6A, 0x46, 0x34), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        selection_foreground: Color32::from_rgb(0xED, 0xE7, 0xE1), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        ansi: [
            Color32::from_rgb(0x8A, 0x7A, 0x70), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xE5, 0x53, 0x4B), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0x6C, 0xC4, 0x8A), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xFF, 0xC1, 0x4D), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0x6F, 0xA8, 0xDC), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xE7, 0xA0, 0xC0), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0x7D, 0xCF, 0xC8), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xED, 0xE7, 0xE1), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xA8, 0x98, 0x8C), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xFF, 0xA4, 0x9E), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xA4, 0xE4, 0xC2), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xFF, 0xD5, 0x8A), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xA9, 0xD0, 0xEE), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xF2, 0xC0, 0xD6), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xB4, 0xE8, 0xE2), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            Color32::from_rgb(0xFF, 0xF8, 0xF3), // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
        ],
    }
}

fn user_themes_dir() -> Option<PathBuf> {
    std::env::var_os("FORJA_CONTENT_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join(".forja"))
        })
        .map(|root| root.join("themes"))
}

impl Palette {
    /// Resolve a cor de uma célula.
    pub fn resolve(&self, source: u16, true_color: Color32, background: bool) -> Color32 {
        match source {
            0..=15 => self.ansi[source as usize],
            16..=255 => ansi_256(self, source as usize),
            257 => true_color,
            _ if background => Color32::TRANSPARENT,
            _ => self.foreground,
        }
    }

    /// Aplica `terminal.color.opacity` ao fundo.
    pub fn with_opacity(mut self, opacity: f64) -> Self {
        let alpha = (opacity.clamp(0.6, 1.0) * 255.0).round() as u8;
        self.background = Color32::from_rgba_unmultiplied( // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            self.background.r(),
            self.background.g(),
            self.background.b(),
            alpha,
        );
        self
    }
}

fn ansi_256(palette: &Palette, index: usize) -> Color32 {
    if index < 16 {
        return palette.ansi[index];
    }
    if index < 232 {
        let value = index - 16;
        let level = |component: usize| -> u8 {
            match component {
                0 => 0,
                1 => 95,
                2 => 135,
                3 => 175,
                4 => 215,
                _ => 255,
            }
        };
        Color32::from_rgb( // ui-lint: allow terminal content palette and ANSI color resolution, not interface chrome
            level(value / 36),
            level(value / 6 % 6),
            level(value % 6),
        )
    } else {
        Color32::from_gray((8 + (index.min(255) - 232) * 10) as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_texts_override_a_builtin_theme() {
        let text = "id = \"brasa-escuro\"\nname = \"Changed\"\nbackground = \"#000000\"\nforeground = \"#FFFFFF\"\ncursor = \"#FFFFFF\"\nselection = \"#333333\"\nselection_foreground = \"#FFFFFF\"\n[ansi]\nblack = \"#000000\"\nred = \"#000000\"\ngreen = \"#000000\"\nyellow = \"#000000\"\nblue = \"#000000\"\nmagenta = \"#000000\"\ncyan = \"#000000\"\nwhite = \"#FFFFFF\"\nbright_black = \"#000000\"\nbright_red = \"#000000\"\nbright_green = \"#000000\"\nbright_yellow = \"#000000\"\nbright_blue = \"#000000\"\nbright_magenta = \"#000000\"\nbright_cyan = \"#000000\"\nbright_white = \"#FFFFFF\"\n";
        let library = ThemeLibrary::from_texts(&[text.to_owned()]);
        assert_eq!(
            library.palette("brasa-escuro", false, true).background,
            Color32::from_rgb(0, 0, 0)
        );
    }

    #[test]
    fn embedded_themes_are_complete() {
        let library = ThemeLibrary::load(None);
        let dark = library.palette("brasa_escuro", false, true);
        let light = library.palette("brasa_claro", false, false);
        assert_ne!(dark.background, light.background);
        assert_eq!(dark.foreground, Color32::from_rgb(0xED, 0xE7, 0xE1));
    }

    #[test]
    fn follow_app_overrides_the_requested_theme() {
        let library = ThemeLibrary::load(None);
        let light = library.palette("brasa_escuro", true, false);
        assert_eq!(light.background, Color32::from_rgb(0xFA, 0xF8, 0xF5));
    }

    #[test]
    fn color_sources_resolve_to_ansi_and_true_color() {
        let library = ThemeLibrary::load(None);
        let palette = library.palette("brasa_escuro", false, true);
        assert_eq!(
            palette.resolve(1, Color32::TRANSPARENT, false),
            palette.ansi[1]
        );
        assert_eq!(
            palette.resolve(257, Color32::from_rgb(1, 2, 3), false),
            Color32::from_rgb(1, 2, 3)
        );
        assert_eq!(
            palette.resolve(256, Color32::TRANSPARENT, true),
            Color32::TRANSPARENT
        );
    }

    #[test]
    fn opacity_only_changes_background_alpha() {
        let palette = ThemeLibrary::load(None)
            .palette("brasa_escuro", false, true)
            .with_opacity(0.6);
        assert_eq!(palette.background.a(), 153);
        assert_eq!(palette.foreground.a(), 255);
    }
}
