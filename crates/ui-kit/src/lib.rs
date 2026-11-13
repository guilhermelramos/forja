//! Design system **Brasa** da Forja.
//!
//! Tokens de cor, ritmo e os controles que a oficina desenha com egui.
//! O app passa os rótulos (i18n); este crate não embute texto de interface.
//!
//! # Tema
//!
//! [`Tokens::brasa_dark`] e [`Tokens::brasa_light`] seguem `design/brand.md`.
//! [`Tokens::from_toml`] aceita um tema de extensão, completo ou só com o que muda.
//! [`apply`] grava o tema no contexto e ajusta o estilo do egui.
//!
//! # Fontes
//!
//! [`install_fonts`] lê Segoe UI Variable, Segoe UI, Cascadia Code e os ícones
//! Segoe Fluent Icons (com Segoe MDL2 Assets atrás) em `C:\Windows\Fonts`.
//! Nada é redistribuído.
//!
//! # Terminal
//!
//! [`TerminalTheme`] lê os arquivos de `content/themes/`.

#![allow(float_literal_f32_fallback)]

mod apply;
mod color;
mod components;
mod fonts;
mod icon;
mod keyboard_layer;
mod paint;
mod terminal;
mod tokens;

pub use apply::{applied_tokens, apply};
pub use color::{contrast_ratio, luminance, Color};
pub use components::{
    button, card, context_menu, empty_state, icon_menu, list, menu_panel, modal, modal_preview,
    page_heading, progress_bar, segmented, select, stage_badge, tabs, text_field, toast, toggle,
    tree, virtual_table, ButtonKind, Column, ListItem, MenuItem, Stage, ToastKind, TreeNode,
};
pub use eframe::egui;
pub use fonts::{install_fonts, FontError};
pub use icon::{
    font_id as icon_font, icon_button, ARCHIVE, BOARD, CLOSE, EXTENSION, HELPERS, REQUESTS,
    ROUTINES, SEARCH, SETTINGS, SKILLS, SPLIT_DOWN, SPLIT_RIGHT, TERMINAL, THEME,
};
pub use keyboard_layer::{keyboard_layer_active, register_keyboard_layer};
pub use terminal::{AnsiPalette, TerminalTheme};
pub use tokens::{Density, Metrics, Mode, Palette, ThemeError, Tokens, GRID};
