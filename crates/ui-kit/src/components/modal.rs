//! Modal e o corpo que a galeria mostra sem o véu.

use eframe::egui::{self, Color32, Frame, Id, Margin, Modal, Ui};

use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, title_font};

/// Diálogo centrado. Fecha no véu ou no Escape.
pub fn modal(ctx: &egui::Context, open: &mut bool, title: &str, add_body: impl FnOnce(&mut Ui)) {
    if !*open {
        return;
    }
    let tokens = crate::applied_tokens(ctx);
    let metrics = tokens.metrics();
    let backdrop = if tokens.mode == crate::tokens::Mode::Dark {
        Color32::from_black_alpha(160)
    } else {
        Color32::from_black_alpha(90)
    };
    let response = Modal::new(Id::new(("forja_modal", title)))
        .backdrop_color(backdrop)
        .frame(dialog_frame(&tokens, &metrics))
        .show(ctx, |ui| {
            dialog_body(ui, &tokens, &metrics, title, add_body)
        });
    if response.should_close() {
        *open = false;
    }
}

/// O cartão do modal, sem véu, para a galeria mostrar o componente junto dos outros.
pub fn modal_preview(
    ui: &mut Ui,
    title: &str,
    add_body: impl FnOnce(&mut Ui),
) -> egui::InnerResponse<()> {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    dialog_frame(&tokens, &metrics).show(ui, |ui| {
        dialog_body(ui, &tokens, &metrics, title, add_body);
    })
}

fn dialog_frame(tokens: &crate::tokens::Tokens, metrics: &crate::tokens::Metrics) -> Frame {
    Frame::new()
        .fill(tokens.color.surface.to_color32())
        .stroke(egui::Stroke::new(1.0, tokens.color.border.to_color32()))
        .corner_radius(radius_u8(metrics.radius_card))
        .shadow(elevation_shadow(3, tokens.mode))
        .inner_margin(Margin::same(pad_i8(metrics.gap * 2.0)))
}

fn dialog_body(
    ui: &mut Ui,
    tokens: &crate::tokens::Tokens,
    metrics: &crate::tokens::Metrics,
    title: &str,
    add_body: impl FnOnce(&mut Ui),
) {
    ui.set_min_width(280.0);
    ui.label(
        egui::RichText::new(title)
            .font(title_font(metrics))
            .color(tokens.color.text.to_color32()),
    );
    ui.add_space(metrics.gap);
    add_body(ui);
}
