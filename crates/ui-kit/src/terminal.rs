//! Tema de terminal. O arquivo mora em `content/themes/` ou numa extensão.

use std::path::Path;

use serde::Deserialize;

use crate::color::Color;
use crate::tokens::ThemeError;

/// Uma paleta de 16 cores ANSI mais fundo, frente, cursor e seleção.
#[derive(Clone, Debug, PartialEq, serde::Serialize, Deserialize)]
pub struct TerminalTheme {
    /// Id estável, em inglês.
    pub id: String,
    /// Nome visível.
    pub name: String,
    /// Fundo do terminal.
    pub background: Color,
    /// Texto padrão.
    pub foreground: Color,
    /// Cursor.
    pub cursor: Color,
    /// Fundo da seleção.
    pub selection: Color,
    /// Texto sobre a seleção.
    pub selection_foreground: Color,
    /// As 16 cores ANSI.
    pub ansi: AnsiPalette,
}

/// Cores ANSI nomeadas, na ordem clássica 0–15.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, Deserialize)]
pub struct AnsiPalette {
    /// 0.
    pub black: Color,
    /// 1.
    pub red: Color,
    /// 2.
    pub green: Color,
    /// 3.
    pub yellow: Color,
    /// 4.
    pub blue: Color,
    /// 5.
    pub magenta: Color,
    /// 6.
    pub cyan: Color,
    /// 7.
    pub white: Color,
    /// 8.
    pub bright_black: Color,
    /// 9.
    pub bright_red: Color,
    /// 10.
    pub bright_green: Color,
    /// 11.
    pub bright_yellow: Color,
    /// 12.
    pub bright_blue: Color,
    /// 13.
    pub bright_magenta: Color,
    /// 14.
    pub bright_cyan: Color,
    /// 15.
    pub bright_white: Color,
}

impl TerminalTheme {
    /// Lê um arquivo de tema.
    pub fn from_toml(text: &str) -> Result<Self, ThemeError> {
        toml::from_str(text).map_err(|err| ThemeError::Parse(err.to_string()))
    }

    /// Lê todos os `*.toml` de uma pasta, em ordem de nome.
    pub fn load_dir(path: &Path) -> Result<Vec<Self>, ThemeError> {
        let mut files = std::fs::read_dir(path)
            .map_err(|err| ThemeError::Parse(format!("{}: {err}", path.display())))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| ThemeError::Parse(err.to_string()))?;
        files.sort_by_key(|entry| entry.file_name());
        let mut themes = Vec::new();
        for entry in files {
            let file_path = entry.path();
            if file_path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let text = std::fs::read_to_string(&file_path)
                .map_err(|err| ThemeError::Parse(format!("{}: {err}", file_path.display())))?;
            themes.push(Self::from_toml(&text)?);
        }
        Ok(themes)
    }
}

impl AnsiPalette {
    /// As 16 cores, com o nome do campo.
    pub fn entries(self) -> [(&'static str, Color); 16] {
        [
            ("black", self.black),
            ("red", self.red),
            ("green", self.green),
            ("yellow", self.yellow),
            ("blue", self.blue),
            ("magenta", self.magenta),
            ("cyan", self.cyan),
            ("white", self.white),
            ("bright_black", self.bright_black),
            ("bright_red", self.bright_red),
            ("bright_green", self.bright_green),
            ("bright_yellow", self.bright_yellow),
            ("bright_blue", self.bright_blue),
            ("bright_magenta", self.bright_magenta),
            ("bright_cyan", self.bright_cyan),
            ("bright_white", self.bright_white),
        ]
    }
}
