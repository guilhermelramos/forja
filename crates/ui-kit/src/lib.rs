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
//! [`install_fonts`] lê fontes de texto em `%WINDIR%/Fonts`, com fallback.
//! [`Icon`] e [`icon`] usam traços autorais embutidos, sem fonte de ícones.
//! [`icon_button`] exige nome acessível/dica e [`button_with_icon`] combina texto e ícone.
//!
//! # Páginas
//!
//! [`page`] cria as cinco zonas de [`PageSpec`]: cabeçalho, filtros, corpo,
//! estados via [`page_state`] e rodapé opcional. [`PageWidth`] distingue leitura
//! e formulários (até 960 px) de quadros/tabelas. [`page_marker`] permite testar
//! as views reais sem acoplar o kernel ao design system.
//!
//! # Peças de composição (item 9)
//!
//! [`toolbar`] agrupa ações, [`toolbar_separator`] separa grupos e
//! [`toolbar_more`] reutiliza o menu com nome obrigatório. [`list_row`] reúne
//! ícone, título, detalhe, [`tag`] e ações no hover/foco. [`section_heading`]
//! e [`form_row`] organizam formulários; [`setting_row`] mantém o contrato
//! especializado dos Ajustes. [`text`] usa o corpo Brasa, preservando spans
//! explícitos; [`Spacing`] nomeia os passos da grade. [`spinner`] respeita foco
//! e movimento reduzido, [`skeleton`] é estável, [`number_card`] usa Display
//! e [`splitter`] aceita arrasto e setas, com nome acessível obrigatório.
//!
//! # Terminal
//!
//! [`TerminalTheme`] lê os arquivos de `content/themes/`.

#![allow(float_literal_f32_fallback)]

mod apply;
mod color;
#[cfg(test)]
mod component_contract_tests;
mod components;
mod fonts;
mod hierarchy;
#[cfg(test)]
mod hierarchy_tests;
mod icon;
#[rustfmt::skip]
mod icons_gen;
mod keyboard_layer;
mod motion;
mod paint;
mod terminal;
mod tokens;
mod typography;
pub use typography::{TextRole, Typography};

pub use apply::{applied_tokens, apply};
pub use color::{contrast_ratio, luminance, Color};
pub use components::{
    action_tile, button, card, context_menu, empty_state, error_banner, error_notice,
    helper_missing_guide, icon_menu, list, measure_tab_width, menu_panel, modal,
    modal_explicit_close, modal_preview, page, page_heading, page_marker, page_state,
    panel_icon_tabs, plan_tab_fit, popup_menu, popup_menu_contents, progress_bar, secret_field,
    segmented, select, select_labeled, stage_badge, tab_strip, tabs, text_area, text_field,
    text_field_labeled, theme_sample, toast, toast_transition, toggle, toggle_labeled, tree,
    truncated_label, virtual_table,
    window_header, window_header_with_focus, wrapped_label, ButtonKind, Column, HelperMissingGuide,
    ListItem, MenuItem, Page, PageIconAction, PageMarker, PageResponse, PageSpec, PageState,
    PageWidth, PanelIconTab, Stage, TabFitPlan, TabItem, TabStripOutcome, ToastKind, TreeNode,
};
pub use components::{
    form_row, list_row, number_card, section_heading, skeleton, spinner, splitter, tag, text,
    toolbar, toolbar_more, toolbar_separator, FormRow, ListRow, Spacing,
};
pub use components::{
    highlighted_text, setting_row, settings_navigation, settings_viewport, SettingRow,
};
pub use eframe::egui;
pub use fonts::{install_fonts, FontError};
pub use icon::{
    button_with_icon, icon, icon_button, paint_icon, Icon, ADD, ARCHIVE, BOARD, CHANGES, CHAT,
    CHEVRON_DOWN, CHEVRON_RIGHT, CLOSE, CODE, DIFF, EXTENSION, FOLDER, GLOBE, HELPERS, HISTORY,
    LAYERS, PORTS, PREVIEW, PROJECTS, REQUESTS, REVIEW, ROUTINES, SEARCH, SETTINGS, SKILLS,
    SPLIT_DOWN, SPLIT_RIGHT, TERMINAL, THEME,
};
pub use keyboard_layer::{
    consume_layer_escape, keyboard_layer_active, register_keyboard_layer, release_keyboard_focus,
};
pub use terminal::{AnsiPalette, TerminalTheme};
pub use tokens::{Density, Metrics, Mode, Palette, ThemeError, Tokens, GRID};
pub use motion::{
    activity_phase, attention_dot, ease_out, entry_motion, motion_bool, motion_value,
    overlay_motion, presence, scale_layer, selection_mark, tooltip, tooltips_enabled, Motion,
};

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

/// Compatibility entry point for [`spinner`]: active windows may animate;
/// inactive windows or reduced motion use a static ring without frame timers.
pub fn spinner_if_active(ui: &mut egui::Ui) {
    spinner(ui);
}
