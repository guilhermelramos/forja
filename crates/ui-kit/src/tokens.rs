//! Tokens Brasa. O preset sai de `design/brand.md`; um TOML de extensão só cobre o que muda.

use serde::Deserialize;

use crate::color::Color;

/// Falha ao ler ou gravar um tema.
#[derive(Debug)]
pub enum ThemeError {
    /// O TOML não fechou ou uma cor não é hexadecimal.
    Parse(String),
}

impl std::fmt::Display for ThemeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ThemeError {}

/// Aparência geral da superfície.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Fundo quente escuro.
    #[default]
    Dark,
    /// Fundo de papel.
    Light,
}

/// Ritmo dos controles. Muda altura/folga, nunca a escala de texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    /// Padrão: controle de 32 px, corpo de 14 px.
    #[default]
    Comfortable,
    /// Controle de 24 px; a escala de texto continua a mesma.
    Compact,
}

/// Paleta nomeada. Os campos de marca vêm do `brand.md`; `on_accent` e `on_danger` são o texto que passa AA sobre o preenchimento.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Palette {
    /// Fundo da janela.
    pub bg: Color,
    /// Painéis e cartões.
    pub surface: Color,
    /// Listas e abas em repouso.
    pub surface_alt: Color,
    /// Divisores e contorno.
    pub border: Color,
    /// Texto.
    pub text: Color,
    /// Texto secundário.
    pub text_muted: Color,
    /// Ação principal e anel de foco.
    pub accent: Color,
    /// Fundo de destaque, sem servir de texto.
    pub accent_soft: Color,
    /// Atenção.
    pub spark: Color,
    /// Sucesso.
    pub ok: Color,
    /// Erro e ação destrutiva.
    pub danger: Color,
    /// Links e informação.
    pub info: Color,
    /// Texto sobre `accent`.
    pub on_accent: Color,
    /// Texto sobre `danger`.
    pub on_danger: Color,
}

/// Medidas derivadas da densidade. Raios de 6 e 10 vêm da marca; o resto cai na grade de 4 px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Altura de botão, campo, seletor, aba e segmentado.
    pub control: f32,
    /// Folga horizontal interna.
    pub pad_x: f32,
    /// Folga vertical interna.
    pub pad_y: f32,
    /// Espaço entre controles.
    pub gap: f32,
    /// Corpo.
    pub body: f32,
    /// Título de página, peso 600.
    pub title: f32,
    /// Título de seção, peso 600.
    pub section: f32,
    /// Número de destaque.
    pub display: f32,
    /// Legenda e badge.
    pub caption: f32,
    /// Código.
    pub code: f32,
    /// Raio de controle e badge.
    pub radius_control: f32,
    /// Raio de cartão, modal e menu.
    pub radius_card: f32,
    /// Altura de linha de lista e tabela.
    pub row: f32,
    /// Altura da barra de progresso.
    pub progress: f32,
    /// Largura e altura do trilho do toggle.
    pub toggle: (f32, f32),
    /// Raio do botão do toggle.
    pub knob: f32,
    /// Recuo de cada nível da árvore.
    pub indent: f32,
    /// Altura do badge de etapa.
    pub badge: f32,
}

/// Tema aplicável ao egui e gravável em TOML.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Tokens {
    /// Nome visível do tema.
    pub name: String,
    /// Escuro ou claro.
    pub mode: Mode,
    /// Confortável ou compacta.
    pub density: Density,
    /// Quando verdadeiro, as animações duram 0 ms.
    pub reduce_motion: bool,
    /// Cores.
    pub color: Palette,
}

impl Default for Tokens {
    fn default() -> Self {
        Self::brasa_dark()
    }
}

impl Tokens {
    /// Preset escuro da marca.
    pub fn brasa_dark() -> Self {
        Self {
            name: "Brasa Escuro".to_owned(),
            mode: Mode::Dark,
            density: Density::Comfortable,
            reduce_motion: false,
            color: Palette {
                bg: Color::rgb(0x12, 0x10, 0x10),
                surface: Color::rgb(0x1B, 0x18, 0x16),
                surface_alt: Color::rgb(0x23, 0x1F, 0x1C),
                border: Color::rgb(0x2E, 0x29, 0x25),
                text: Color::rgb(0xED, 0xE7, 0xE1),
                text_muted: Color::rgb(0xA6, 0x9C, 0x93),
                accent: Color::rgb(0xF2, 0x76, 0x2E),
                accent_soft: Color::rgb(0x3A, 0x24, 0x16),
                spark: Color::rgb(0xFF, 0xC1, 0x4D),
                ok: Color::rgb(0x6C, 0xC4, 0x8A),
                danger: Color::rgb(0xE5, 0x53, 0x4B),
                info: Color::rgb(0x6F, 0xA8, 0xDC),
                on_accent: Color::rgb(0x1F, 0x1A, 0x17),
                on_danger: Color::rgb(0x1F, 0x1A, 0x17),
            },
        }
    }

    /// Preset claro da marca.
    pub fn brasa_light() -> Self {
        Self {
            name: "Brasa Claro".to_owned(),
            mode: Mode::Light,
            density: Density::Comfortable,
            reduce_motion: false,
            color: Palette {
                bg: Color::rgb(0xFA, 0xF8, 0xF5),
                surface: Color::rgb(0xFF, 0xFF, 0xFF),
                surface_alt: Color::rgb(0xF2, 0xEE, 0xE9),
                border: Color::rgb(0xE3, 0xDD, 0xD6),
                text: Color::rgb(0x1F, 0x1A, 0x17),
                text_muted: Color::rgb(0x6B, 0x62, 0x5B),
                accent: Color::rgb(0xD9, 0x60, 0x1B),
                accent_soft: Color::rgb(0xFC, 0xE6, 0xD7),
                spark: Color::rgb(0xB7, 0x86, 0x0B),
                ok: Color::rgb(0x2E, 0x8A, 0x4E),
                danger: Color::rgb(0xC2, 0x3A, 0x32),
                info: Color::rgb(0x2F, 0x6F, 0xA8),
                on_accent: Color::rgb(0x1F, 0x1A, 0x17),
                on_danger: Color::rgb(0xFF, 0xFF, 0xFF),
            },
        }
    }

    /// Carvão: tinta fria sobre a base do preset escuro (`design/ui.md` §4).
    pub fn carvao() -> Self {
        Self::dark_from(
            "Carvão",
            Color::rgb(0x10, 0x14, 0x16),
            Color::rgb(0xE4, 0xE8, 0xEA),
        )
    }

    /// Cinza Quente: pedra aquecida, fundo quente e texto claro.
    pub fn cinza_quente() -> Self {
        Self::dark_from(
            "Cinza Quente",
            Color::rgb(0x2C, 0x28, 0x25),
            Color::rgb(0xF7, 0xF1, 0xEB),
        )
    }

    /// Aço: fundo azulado, texto frio.
    pub fn aco() -> Self {
        Self::dark_from(
            "Aço",
            Color::rgb(0x15, 0x20, 0x28),
            Color::rgb(0xE7, 0xF0, 0xF4),
        )
    }

    /// Derives a dark theme from its background and text colours. Surfaces lift from
    /// `bg` toward `text` (4%, 8%, 16% for the border), the muted text moves from
    /// `text` toward `bg`, and the accent family and status colours come from the
    /// Brasa dark preset. The contrast tests check the result.
    fn dark_from(name: &str, bg: Color, text: Color) -> Self {
        let brasa = Self::brasa_dark();
        let palette = Palette {
            bg,
            surface: mix(bg, text, 0.04),
            surface_alt: mix(bg, text, 0.08),
            border: mix(bg, text, 0.16),
            text,
            text_muted: mix(text, bg, 0.35),
            accent: brasa.color.accent,
            accent_soft: mix(bg, brasa.color.accent, 0.22),
            spark: brasa.color.spark,
            ok: brasa.color.ok,
            danger: brasa.color.danger,
            info: brasa.color.info,
            on_accent: brasa.color.on_accent,
            on_danger: brasa.color.on_danger,
        };
        Self {
            name: name.to_owned(),
            mode: Mode::Dark,
            density: Density::Comfortable,
            reduce_motion: false,
            color: palette,
        }
    }

    /// Medidas do ritmo atual.
    pub fn metrics(&self) -> Metrics {
        match self.density {
            Density::Comfortable => Metrics {
                control: 32.0,
                pad_x: 12.0,
                pad_y: 8.0,
                gap: 8.0,
                body: crate::Typography::BRASA.body,
                title: crate::Typography::BRASA.page,
                section: crate::Typography::BRASA.section,
                display: crate::Typography::BRASA.display,
                caption: crate::Typography::BRASA.caption,
                code: crate::Typography::BRASA.body,
                radius_control: 6.0,
                radius_card: 10.0,
                row: 32.0,
                progress: 8.0,
                toggle: (40.0, 24.0),
                knob: 8.0,
                indent: 16.0,
                badge: 20.0,
            },
            Density::Compact => Metrics {
                control: 24.0,
                pad_x: 8.0,
                pad_y: 4.0,
                gap: 4.0,
                body: crate::Typography::BRASA.body,
                title: crate::Typography::BRASA.page,
                section: crate::Typography::BRASA.section,
                display: crate::Typography::BRASA.display,
                caption: crate::Typography::BRASA.caption,
                code: crate::Typography::BRASA.body,
                radius_control: 6.0,
                radius_card: 10.0,
                row: 24.0,
                progress: 4.0,
                toggle: (32.0, 20.0),
                knob: 6.0,
                indent: 12.0,
                badge: 16.0,
            },
        }
    }

    /// Duração das transições, em segundos. `0` quando o tema pede menos movimento.
    pub fn motion_secs(&self) -> f32 {
        if self.reduce_motion {
            0.0
        } else {
            0.12
        }
    }

    /// Lê um tema completo ou parcial. O que faltar vem do preset do `mode`.
    pub fn from_toml(text: &str) -> Result<Self, ThemeError> {
        let file: ThemeFile =
            toml::from_str(text).map_err(|err| ThemeError::Parse(err.to_string()))?;
        Ok(file.overlay())
    }

    /// Grava o tema inteiro.
    pub fn to_toml(&self) -> Result<String, ThemeError> {
        toml::to_string_pretty(self).map_err(|err| ThemeError::Parse(err.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    mode: Option<Mode>,
    #[serde(default)]
    density: Option<Density>,
    #[serde(default)]
    reduce_motion: Option<bool>,
    #[serde(default)]
    color: ColorPatch,
}

#[derive(Debug, Default, Deserialize)]
struct ColorPatch {
    bg: Option<Color>,
    surface: Option<Color>,
    surface_alt: Option<Color>,
    border: Option<Color>,
    text: Option<Color>,
    text_muted: Option<Color>,
    accent: Option<Color>,
    accent_soft: Option<Color>,
    spark: Option<Color>,
    ok: Option<Color>,
    danger: Option<Color>,
    info: Option<Color>,
    on_accent: Option<Color>,
    on_danger: Option<Color>,
}

impl ThemeFile {
    fn overlay(self) -> Tokens {
        let mut tokens = match self.mode.unwrap_or(Mode::Dark) {
            Mode::Dark => Tokens::brasa_dark(),
            Mode::Light => Tokens::brasa_light(),
        };
        if let Some(name) = self.name {
            tokens.name = name;
        }
        if let Some(density) = self.density {
            tokens.density = density;
        }
        if let Some(reduce_motion) = self.reduce_motion {
            tokens.reduce_motion = reduce_motion;
        }
        let patch = self.color;
        let color = &mut tokens.color;
        set(&mut color.bg, patch.bg);
        set(&mut color.surface, patch.surface);
        set(&mut color.surface_alt, patch.surface_alt);
        set(&mut color.border, patch.border);
        set(&mut color.text, patch.text);
        set(&mut color.text_muted, patch.text_muted);
        set(&mut color.accent, patch.accent);
        set(&mut color.accent_soft, patch.accent_soft);
        set(&mut color.spark, patch.spark);
        set(&mut color.ok, patch.ok);
        set(&mut color.danger, patch.danger);
        set(&mut color.info, patch.info);
        set(&mut color.on_accent, patch.on_accent);
        set(&mut color.on_danger, patch.on_danger);
        tokens
    }
}

fn set(slot: &mut Color, value: Option<Color>) {
    if let Some(value) = value {
        *slot = value;
    }
}

/// Passo da grade, em px.
pub const GRID: f32 = 4.0;

/// Linear blend of two colours: `t = 0` gives `from`, `t = 1` gives `to`.
fn mix(from: Color, to: Color, t: f32) -> Color {
    let channel = |a: u8, b: u8| -> u8 {
        (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color::rgb(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
    )
}

#[cfg(test)]
mod named_dark_tests {
    use super::*;

    #[test]
    fn named_dark_themes_keep_their_background_and_text() {
        let carvao = Tokens::carvao();
        assert_eq!(carvao.color.bg, Color::rgb(0x10, 0x14, 0x16));
        assert_eq!(carvao.color.text, Color::rgb(0xE4, 0xE8, 0xEA));
        assert_eq!(carvao.mode, Mode::Dark);
        assert_eq!(Tokens::aco().name, "Aço");
    }

    #[test]
    fn mix_moves_between_the_two_colours() {
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        assert_eq!(mix(black, white, 0.0), black);
        assert_eq!(mix(black, white, 1.0), white);
        assert_eq!(mix(black, white, 0.5), Color::rgb(128, 128, 128));
    }
}
