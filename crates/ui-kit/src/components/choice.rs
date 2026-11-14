//! Controle segmentado e abas.

use eframe::egui::{self, Align2, Color32, CornerRadius, Response, Stroke, StrokeKind, Ui};

use crate::paint::{self, body_font, paint_text, painter_for};

/// Uma opção ativa por vez, no mesmo trilho.
pub fn segmented(ui: &mut Ui, selected: &mut usize, labels: &[&str]) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    if labels.is_empty() {
        return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
    }
    let font = body_font(&metrics);
    let widest = labels
        .iter()
        .map(|label| paint::text_size(ui, label, font.clone(), tokens.color.text.to_color32()).x)
        .fold(0.0_f32, f32::max);
    let segment = (widest + metrics.pad_x * 2.0).max(metrics.control * 2.0);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(segment * labels.len() as f32, metrics.control),
        egui::Sense::hover(),
    );
    let painter = painter_for(ui);
    painter.rect(
        rect,
        CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
        tokens.color.surface_alt.to_color32(),
        Stroke::NONE,
        StrokeKind::Inside,
    );
    for (index, label) in labels.iter().enumerate() {
        let piece = segment_rect(rect, segment, index);
        let piece_response = ui.interact(piece, response.id.with(index), egui::Sense::click());
        if piece_response.clicked() {
            *selected = index;
        }
        let is_active = index == *selected;
        piece_response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), is_active, *label)
        });
        let active = index == *selected;
        if active {
            painter.rect_filled(
                piece.shrink(2.0),
                CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
                tokens.color.accent.to_color32(),
            );
        } else if piece_response.hovered() {
            painter.rect_filled(
                piece.shrink(2.0),
                CornerRadius::same(4),
                hover_surface(&tokens),
            );
        }
        let text = if active {
            tokens.color.on_accent.to_color32()
        } else {
            tokens.color.text.to_color32()
        };
        paint_text(
            &painter,
            piece,
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            text,
        );
        paint::paint_focus(
            ui,
            &piece_response,
            piece,
            metrics.radius_control,
            tokens.color.accent.to_color32(),
        );
    }
    response
}

/// Abas com sublinhado `accent` na ativa. O texto ativo usa `text`, não o laranja.
pub fn tabs(ui: &mut Ui, selected: &mut usize, labels: &[&str]) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    if labels.is_empty() {
        return ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());
    }
    let font = body_font(&metrics);
    let mut widths = Vec::with_capacity(labels.len());
    for label in labels {
        let text = paint::text_size(ui, label, font.clone(), tokens.color.text.to_color32()).x;
        widths.push((text + metrics.pad_x * 2.0).max(metrics.control));
    }
    let total = widths.iter().sum::<f32>();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(total, metrics.control), egui::Sense::hover());
    let painter = painter_for(ui);
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, tokens.color.border.to_color32()),
    );
    let mut x = rect.left();
    for (index, label) in labels.iter().enumerate() {
        let piece = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(widths[index], rect.height()),
        );
        x += widths[index];
        let piece_response = ui.interact(
            piece,
            response.id.with(("tab", index)),
            egui::Sense::click(),
        );
        let activated_by_key = piece_response.has_focus()
            && ui.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
            });
        if piece_response.clicked() || activated_by_key {
            *selected = index;
            piece_response.request_focus();
        }
        let active = index == *selected;
        piece_response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), active, *label)
        });
        let text = if active || piece_response.hovered() {
            tokens.color.text.to_color32()
        } else {
            tokens.color.text_muted.to_color32()
        };
        paint_text(
            &painter,
            piece,
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            text,
        );
        if active {
            painter.hline(
                piece.x_range(),
                piece.bottom(),
                Stroke::new(2.0, tokens.color.accent.to_color32()),
            );
        }
        paint::paint_focus(
            ui,
            &piece_response,
            piece,
            2.0,
            tokens.color.accent.to_color32(),
        );
    }
    response
}

fn segment_rect(rect: egui::Rect, width: f32, index: usize) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.left() + width * index as f32, rect.top()),
        egui::vec2(width, rect.height()),
    )
}

fn hover_surface(tokens: &crate::tokens::Tokens) -> Color32 {
    if tokens.mode == crate::tokens::Mode::Dark {
        crate::apply::mix_color32(tokens.color.surface_alt.to_color32(), Color32::WHITE, 0.06)
    } else {
        tokens.color.surface.to_color32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_input(key: egui::Key) -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        raw
    }

    #[test]
    fn tabs_enter_and_space_activate_focused_tab() {
        let ctx = egui::Context::default();
        let mut selected = 0usize;
        let labels = ["Terminal", "Editor", "Navegador"];
        let mut tab_ids = Vec::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let response = tabs(ui, &mut selected, &labels);
                for index in 0..labels.len() {
                    tab_ids.push(response.id.with(("tab", index)));
                }
            });
        });

        // Focus second tab ("Editor") and press Enter -> activates index 1.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.memory_mut(|memory| memory.request_focus(tab_ids[1]));
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = tabs(ui, &mut selected, &labels);
            });
        });
        let _ = ctx.run(key_input(egui::Key::Enter), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = tabs(ui, &mut selected, &labels);
            });
        });
        assert_eq!(selected, 1);

        // Move focus to third tab via Tab, then press Space -> activates index 2.
        let _ = ctx.run(key_input(egui::Key::Tab), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = tabs(ui, &mut selected, &labels);
            });
        });
        let _ = ctx.run(key_input(egui::Key::Space), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let _ = tabs(ui, &mut selected, &labels);
            });
        });
        assert_eq!(selected, 2);
    }
}
