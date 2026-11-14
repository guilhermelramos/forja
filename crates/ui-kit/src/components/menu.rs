//! Menu de contexto.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Margin, Popup, PopupKind, Response, Sense, Stroke,
    Ui,
};

use super::button::{button, ButtonKind};
use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, body_font, paint_text};

/// Uma entrada de menu. O perigo leva uma marca colorida; o texto continua `text`.
pub struct MenuItem<'a> {
    /// Rótulo.
    pub label: &'a str,
    /// Quando verdadeiro, a marca à esquerda usa `danger`.
    pub danger: bool,
}

/// Botão que abre o menu. Devolve o índice escolhido neste quadro.
pub fn context_menu(ui: &mut Ui, label: &str, items: &[MenuItem<'_>]) -> Option<usize> {
    let response = button(ui, ButtonKind::Secondary, label);
    open_menu(&response, items)
}

/// Ícone que abre o mesmo menu. O tooltip é o nome da ação.
pub fn icon_menu(ui: &mut Ui, glyph: &str, tooltip: &str, items: &[MenuItem<'_>]) -> Option<usize> {
    let response = crate::icon::icon_button(ui, glyph).on_hover_text(tooltip.to_owned());
    open_menu(&response, items)
}

fn open_menu(response: &Response, items: &[MenuItem<'_>]) -> Option<usize> {
    let mut chosen = None;
    Popup::from_response(response)
        .kind(PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .gap(4.0)
        .open_memory(response.clicked().then_some(egui::SetOpenCommand::Toggle))
        .show(|ui| {
            chosen = menu_list(ui, items);
        });
    chosen
}

/// O mesmo miolo do menu, sempre visível. Serve para a galeria e para encaixar o popup.
pub fn menu_panel(ui: &mut Ui, items: &[MenuItem<'_>]) -> Option<usize> {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let mut chosen = None;
    Frame::new()
        .fill(tokens.color.surface.to_color32())
        .stroke(Stroke::new(1.0, tokens.color.border.to_color32()))
        .corner_radius(radius_u8(metrics.radius_card))
        .shadow(elevation_shadow(2, tokens.mode))
        .inner_margin(Margin::same(pad_i8(4.0)))
        .show(ui, |ui| {
            chosen = menu_list(ui, items);
        });
    chosen
}

fn menu_list(ui: &mut Ui, items: &[MenuItem<'_>]) -> Option<usize> {
    let mut chosen = None;
    for (index, item) in items.iter().enumerate() {
        if menu_row(ui, item).clicked() {
            chosen = Some(index);
        }
    }
    chosen
}

fn menu_row(ui: &mut Ui, item: &MenuItem<'_>) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let width = ui.available_width().max(160.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, metrics.row), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), item.label)
    });
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(radius_u8(metrics.radius_control)),
            tokens.color.surface_alt.to_color32(),
        );
    }
    let mark = if item.danger {
        tokens.color.danger.to_color32()
    } else {
        Color32::TRANSPARENT
    };
    let bar = egui::Rect::from_min_size(
        rect.left_center() - egui::vec2(0.0, 6.0),
        egui::vec2(3.0, 12.0),
    );
    ui.painter().rect_filled(bar, CornerRadius::same(2), mark);
    paint_text(
        ui.painter(),
        rect.shrink2(egui::vec2(12.0, 0.0)),
        Align2::LEFT_CENTER,
        item.label,
        body_font(&metrics),
        tokens.color.text.to_color32(),
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );
    response
}
