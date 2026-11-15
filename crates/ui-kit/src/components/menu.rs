//! Menu de contexto.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, Frame, Margin, Popup, PopupKind, Response, Sense, Stroke,
    Ui,
};

use super::button::{ButtonKind, button};
use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, body_font, paint_text};

/// Uma entrada de menu. O perigo leva uma marca colorida; o texto continua `text`.
pub struct MenuItem<'a> {
    /// Rótulo.
    pub label: &'a str,
    /// Quando verdadeiro, a marca à esquerda usa `danger`.
    pub danger: bool,
}

const MENU_HEIGHT_FRACTION: f32 = 0.6;
const MENU_MIN_HEIGHT: f32 = 160.0;
const MENU_MAX_HEIGHT: f32 = 640.0;

/// Botão que abre o menu. Devolve o índice escolhido neste quadro.
pub fn context_menu(ui: &mut Ui, label: &str, items: &[MenuItem<'_>]) -> Option<usize> {
    let response = button(ui, ButtonKind::Secondary, label);
    open_menu(&response, items)
}

/// Ícone que abre o mesmo menu. O tooltip é o nome da ação.
pub fn icon_menu(ui: &mut Ui, glyph: &str, tooltip: &str, items: &[MenuItem<'_>]) -> Option<usize> {
    let response = crate::icon::icon_button(ui, glyph).on_hover_text(tooltip.to_owned());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
    });
    open_menu(&response, items)
}

fn open_menu(response: &Response, items: &[MenuItem<'_>]) -> Option<usize> {
    let mut chosen = None;
    let screen = response.ctx.content_rect();
    let max_width = (screen.width() - 24.0).clamp(160.0, 420.0);
    let below = (screen.bottom() - response.rect.bottom() - 12.0).max(0.0);
    let above = (response.rect.top() - screen.top() - 12.0).max(0.0);
    let available_height = below
        .max(above)
        .clamp(MENU_MIN_HEIGHT, (screen.height() - 24.0).max(MENU_MIN_HEIGHT));
    let max_height = menu_height_limit(screen.height()).min(available_height);
    let tokens = crate::applied_tokens(&response.ctx);
    let row_height = tokens.metrics().row;
    let spacing_y = 2.0_f32;
    let content_height = items.len() as f32 * (row_height + spacing_y) + 8.0;
    Popup::from_response(response)
        .kind(PopupKind::Menu)
        .layout(egui::Layout::top_down(egui::Align::Min))
        .gap(4.0)
        .open_memory(response.clicked().then_some(egui::SetOpenCommand::Toggle))
        .show(|ui| {
            let width = menu_width(ui, items, max_width);
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = spacing_y;
            if content_height <= max_height {
                chosen = menu_list(ui, items, width);
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("forja-ui-kit.context-menu")
                    .max_height(max_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        chosen = menu_list(ui, items, width);
                    });
            }
        });
    chosen
}

fn menu_height_limit(viewport_height: f32) -> f32 {
    (viewport_height * MENU_HEIGHT_FRACTION)
        .clamp(MENU_MIN_HEIGHT, MENU_MAX_HEIGHT)
        .min(viewport_height.max(0.0))
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
            let max_width = ui.available_width().clamp(160.0, 420.0);
            let width = menu_width(ui, items, max_width);
            chosen = menu_list(ui, items, width);
        });
    chosen
}

fn menu_width(ui: &Ui, items: &[MenuItem<'_>], max_width: f32) -> f32 {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let font = body_font(&metrics);
    let color = tokens.color.text.to_color32();
    let widest = items
        .iter()
        .map(|item| paint::text_size(ui, item.label, font.clone(), color).x + 28.0)
        .fold(160.0_f32, f32::max);
    widest.min(max_width.max(160.0))
}

fn menu_list(ui: &mut Ui, items: &[MenuItem<'_>], width: f32) -> Option<usize> {
    let mut chosen = None;
    for (index, item) in items.iter().enumerate() {
        if menu_row(ui, item, width).clicked() {
            chosen = Some(index);
        }
    }
    chosen
}

fn menu_row(ui: &mut Ui, item: &MenuItem<'_>, width: f32) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let row_width = ui.available_width().clamp(120.0, width);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(row_width, metrics.row), Sense::click());
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
    let text_rect = rect.shrink2(egui::vec2(12.0, 0.0));
    let text_painter = ui.painter().with_clip_rect(text_rect.intersect(ui.clip_rect()));
    paint_text(
        &text_painter,
        text_rect,
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

#[cfg(test)]
mod tests {
    use super::menu_height_limit;

    #[test]
    fn context_menu_height_tracks_small_viewports_and_caps_large_ones() {
        assert!((menu_height_limit(720.0) - 432.0).abs() < 0.001);
        assert_eq!(menu_height_limit(1080.0), 640.0);
        assert_eq!(menu_height_limit(200.0), 160.0);
        assert_eq!(menu_height_limit(100.0), 100.0);
    }
}
