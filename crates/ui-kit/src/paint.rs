//! Pintura compartilhada: foco, hover e opacidade de desabilitado.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Response, Sense, Stroke,
    StrokeKind, Ui, Vec2,
};

use crate::apply::{hover_tint, mix_color32, radius_u8};
use crate::tokens::{Metrics, Tokens};

pub(crate) fn tokens(ui: &Ui) -> Tokens {
    crate::applied_tokens(ui.ctx())
}

pub(crate) fn body_font(metrics: &Metrics) -> FontId {
    FontId::new(metrics.body, FontFamily::Proportional)
}

pub(crate) fn caption_font(metrics: &Metrics) -> FontId {
    FontId::new(metrics.caption, FontFamily::Proportional)
}

pub(crate) fn title_font(metrics: &Metrics) -> FontId {
    FontId::new(metrics.title, crate::apply::semibold_family())
}

pub(crate) fn painter_for(ui: &Ui) -> Painter {
    let mut painter = ui.painter().clone();
    if !ui.is_enabled() {
        painter.set_opacity(0.4);
    }
    painter
}

pub(crate) fn hover_fill(fill: Color32, response: &Response, tokens: &Tokens) -> Color32 {
    if !response.sense.interactive() {
        return fill;
    }
    let amount = if response.is_pointer_button_down_on() {
        0.14
    } else if response.hovered() {
        0.08
    } else {
        0.0
    };
    mix_color32(fill, hover_tint(tokens.mode), amount)
}

pub(crate) fn focus_ring(ui: &Ui, rect: egui::Rect, radius: f32, color: Color32) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        CornerRadius::same(radius_u8(radius + 2.0)),
        Stroke::new(2.0, color),
        StrokeKind::Outside,
    );
}

pub(crate) fn paint_focus(
    ui: &Ui,
    response: &Response,
    rect: egui::Rect,
    radius: f32,
    color: Color32,
) {
    if ui.is_enabled() && response.has_focus() {
        focus_ring(ui, rect, radius, color);
    }
}

pub(crate) fn text_size(ui: &Ui, text: &str, font: FontId, color: Color32) -> Vec2 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, color)
        .size()
}

pub(crate) fn paint_text(
    painter: &Painter,
    rect: egui::Rect,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) {
    let pos = if align == Align2::RIGHT_CENTER {
        rect.right_center()
    } else if align == Align2::CENTER_CENTER {
        rect.center()
    } else {
        rect.left_center()
    };
    painter.text(pos, align, text, font, color);
}

pub(crate) fn chevron(painter: &Painter, tip: egui::Pos2, color: Color32) {
    let points = vec![
        tip + egui::vec2(-4.0, -2.0),
        tip + egui::vec2(0.0, 2.0),
        tip + egui::vec2(4.0, -2.0),
    ];
    painter.add(egui::Shape::line(points, Stroke::new(1.5, color)));
}

pub(crate) fn click_zone(ui: &mut Ui, size: Vec2) -> (egui::Rect, Response) {
    ui.allocate_exact_size(size, Sense::click())
}
