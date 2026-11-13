//! Botões primário, secundário, fantasma e perigo.

use eframe::egui::{self, Align2, Color32, CornerRadius, Response, Stroke, Ui};

use crate::paint::{self, body_font, click_zone, hover_fill, paint_focus, paint_text, painter_for};
use crate::tokens::Tokens;

/// Qual botão desenhar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// Preenchimento `accent`, texto `on_accent`.
    Primary,
    /// Superfície com contorno.
    Secondary,
    /// Sem preenchimento até o hover.
    Ghost,
    /// Preenchimento `danger`, texto `on_danger`.
    Danger,
}

/// Desenha um botão. O rótulo vem de quem chama (o app usa i18n).
pub fn button(ui: &mut Ui, kind: ButtonKind, label: &str) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let (fill, text, stroke) = colors(kind, &tokens);
    let font = body_font(&metrics);
    let text_size = paint::text_size(ui, label, font.clone(), text);
    let width = (text_size.x + metrics.pad_x * 2.0).max(metrics.control);
    let (rect, response) = click_zone(ui, egui::vec2(width, metrics.control));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));

    let fill = if kind == ButtonKind::Ghost {
        if response.hovered() || response.is_pointer_button_down_on() {
            tokens.color.surface_alt.to_color32()
        } else {
            Color32::TRANSPARENT
        }
    } else {
        hover_fill(fill, &response, &tokens)
    };

    let painter = painter_for(ui);
    painter.rect(
        rect,
        CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
        fill,
        stroke,
        egui::StrokeKind::Inside,
    );
    paint_text(&painter, rect, Align2::CENTER_CENTER, label, font, text);
    paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );
    response
}

fn colors(kind: ButtonKind, tokens: &Tokens) -> (Color32, Color32, Stroke) {
    let color = tokens.color;
    let text = color.text.to_color32();
    let border = Stroke::new(1.0, color.border.to_color32());
    match kind {
        ButtonKind::Primary => (
            color.accent.to_color32(),
            color.on_accent.to_color32(),
            Stroke::NONE,
        ),
        ButtonKind::Secondary => (color.surface.to_color32(), text, border),
        ButtonKind::Ghost => (Color32::TRANSPARENT, text, Stroke::NONE),
        ButtonKind::Danger => (
            color.danger.to_color32(),
            color.on_danger.to_color32(),
            Stroke::NONE,
        ),
    }
}
