//! Toggle. O estado também está na posição do botão, não só na cor.

use eframe::egui::{self, Align2, CornerRadius, Response, Stroke, StrokeKind, Ui};

use crate::paint::{self, body_font, paint_text, painter_for};

/// Trilho com rótulo. Clique ou Enter/Espaço invertem `on`.
pub fn toggle(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let (track_w, track_h) = metrics.toggle;
    let font = body_font(&metrics);
    let text_color = tokens.color.text.to_color32();
    let text_size = paint::text_size(ui, label, font.clone(), text_color);
    let gap = metrics.gap;
    let height = track_h.max(text_size.y);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(track_w + gap + text_size.x, height),
        egui::Sense::click(),
    );
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }

    let track = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - track_h * 0.5),
        egui::vec2(track_w, track_h),
    );
    let travel = ui
        .ctx()
        .animate_bool_with_time(response.id, *on, tokens.motion_secs());
    let painter = painter_for(ui);
    let track_fill = if *on {
        tokens.color.accent.to_color32()
    } else {
        tokens.color.surface_alt.to_color32()
    };
    let track_stroke = if *on {
        Stroke::NONE
    } else {
        Stroke::new(1.0, tokens.color.border.to_color32())
    };
    painter.rect(
        track,
        CornerRadius::same(crate::apply::radius_u8(track_h * 0.5)),
        track_fill,
        track_stroke,
        StrokeKind::Inside,
    );
    let x = egui::lerp(
        (track.left() + track_h * 0.5)..=(track.right() - track_h * 0.5),
        travel,
    );
    let knob = if *on {
        tokens.color.on_accent.to_color32()
    } else {
        tokens.color.text.to_color32()
    };
    painter.circle_filled(egui::pos2(x, track.center().y), metrics.knob, knob);

    let label_rect = egui::Rect::from_min_max(
        egui::pos2(track.right() + gap, rect.top()),
        rect.right_bottom(),
    );
    paint_text(
        &painter,
        label_rect,
        Align2::LEFT_CENTER,
        label,
        font,
        text_color,
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
