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
//! # Teclado em janelas
//!
//! Uma janela flutuante registra sua camada com [`register_keyboard_layer`]
//! a cada quadro e usa [`consume_layer_escape`] antes de desenhar para fechar
//! apenas quando ela estiver no topo e não houver menu aberto sobre ela.
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
    action_tile, button, card, context_menu, empty_state, error_banner, error_notice,
    helper_missing_guide, icon_menu, list, measure_tab_width, menu_panel, modal,
    modal_explicit_close, modal_preview, page_heading, panel_icon_tabs, plan_tab_fit, popup_menu,
    progress_bar, segmented, select, stage_badge, tab_strip, tabs, text_field, theme_sample, toast,
    toggle, tree, truncated_label, virtual_table, window_header, window_header_with_focus,
    wrapped_label, ButtonKind, Column, HelperMissingGuide, ListItem, MenuItem, PanelIconTab, Stage,
    TabFitPlan, TabItem, TabStripOutcome, ToastKind, TreeNode,
};
pub use eframe::egui;
pub use fonts::{install_fonts, FontError};
pub use icon::{
    font_id as icon_font, icon_button, ADD, ARCHIVE, BOARD, CHANGES, CHAT, CHEVRON_DOWN,
    CHEVRON_RIGHT, CLOSE, CODE, DIFF, EXTENSION, FOLDER, GLOBE, HELPERS, HISTORY, LAYERS, PORTS,
    PREVIEW, PROJECTS, REQUESTS, REVIEW, ROUTINES, SEARCH, SETTINGS, SKILLS, SPLIT_DOWN,
    SPLIT_RIGHT, TERMINAL, THEME,
};
pub use keyboard_layer::{
    consume_layer_escape, keyboard_layer_active, register_keyboard_layer, release_keyboard_focus,
};
pub use terminal::{AnsiPalette, TerminalTheme};
pub use tokens::{Density, Metrics, Mode, Palette, ThemeError, Tokens, GRID};

/// Verdadeiro enquanto a janela principal está visível e em foco.
///
/// Timers visuais e spinners usam este sinal para não agendar quadros quando a
/// janela está minimizada ou sem foco; eventos de I/O (como saída do ConPTY)
/// continuam acordando a interface direto pelo contexto.
pub fn window_active(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.viewport().focused.unwrap_or(true) && !input.viewport().minimized.unwrap_or(false)
    })
}

/// Agenda um repaint após `delay` apenas quando a janela está ativa (focada e
/// não minimizada).
pub fn request_repaint_after_active(ctx: &egui::Context, delay: std::time::Duration) {
    if window_active(ctx) {
        ctx.request_repaint_after(delay);
    }
}

/// Desenha o spinner quando a janela está ativa; quando minimizada ou sem foco,
/// apenas reserva o espaço sem pedir quadros a 60 fps.
pub fn spinner_if_active(ui: &mut egui::Ui) {
    if window_active(ui.ctx()) {
        ui.spinner();
    } else {
        let size = ui.spacing().interact_size.y;
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    }
}
