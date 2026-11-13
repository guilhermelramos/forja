//! Cartão e lista.

use eframe::egui::{self, Align2, Color32, Frame, Margin, Stroke, Ui};

use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, body_font, caption_font, paint_text, title_font};

/// Uma linha de lista.
pub struct ListItem<'a> {
    /// Texto principal.
    pub label: &'a str,
    /// Texto curto à direita.
    pub detail: Option<&'a str>,
}

/// Cartão com título opcional. Raio 10, elevação 1.
pub fn card<R>(
    ui: &mut Ui,
    title: Option<&str>,
    add: impl FnOnce(&mut Ui) -> R,
) -> egui::InnerResponse<R> {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    Frame::new()
        .fill(tokens.color.surface.to_color32())
        .stroke(Stroke::new(1.0, tokens.color.border.to_color32()))
        .corner_radius(radius_u8(metrics.radius_card))
        .shadow(elevation_shadow(1, tokens.mode))
        .inner_margin(Margin::same(pad_i8(metrics.gap * 2.0)))
        .show(ui, |ui| {
            if let Some(title) = title {
                ui.label(
                    egui::RichText::new(title)
                        .font(title_font(&metrics))
                        .color(tokens.color.text.to_color32()),
                );
                ui.add_space(metrics.gap);
            }
            add(ui)
        })
}

/// Lista de linhas. `selected` acompanha o índice clicado.
pub fn list(
    ui: &mut Ui,
    items: &[ListItem<'_>],
    selected: &mut Option<usize>,
) -> egui::InnerResponse<()> {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    ui.vertical(|ui| {
        for (index, item) in items.iter().enumerate() {
            let width = ui.available_width();
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(width, metrics.row), egui::Sense::click());
            if response.clicked() {
                *selected = Some(index);
            }
            let fill = if *selected == Some(index) {
                tokens.color.accent_soft.to_color32()
            } else if response.hovered() {
                tokens.color.surface_alt.to_color32()
            } else {
                Color32::TRANSPARENT
            };
            if fill != Color32::TRANSPARENT {
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(radius_u8(metrics.radius_control)),
                    fill,
                );
            }
            let text_rect = rect.shrink2(egui::vec2(metrics.pad_x, 0.0));
            paint_text(
                ui.painter(),
                text_rect,
                Align2::LEFT_CENTER,
                item.label,
                body_font(&metrics),
                tokens.color.text.to_color32(),
            );
            if let Some(detail) = item.detail {
                paint_text(
                    ui.painter(),
                    text_rect,
                    Align2::RIGHT_CENTER,
                    detail,
                    caption_font(&metrics),
                    tokens.color.text_muted.to_color32(),
                );
            }
            paint::paint_focus(
                ui,
                &response,
                rect,
                metrics.radius_control,
                tokens.color.accent.to_color32(),
            );
        }
    })
}
