//! Cabeçalho de página e estado vazio. Os rótulos vêm de quem chama.

use eframe::egui::{Align, Layout, Response, RichText, Ui};

use super::button::{button, ButtonKind};
use crate::icon;
use crate::paint;

/// Cabeçalho de uma página: título, subtítulo opcional à esquerda e as ações à direita, na mesma
/// linha. Devolve o que `add_actions` devolver.
pub fn page_heading<R>(
    ui: &mut Ui,
    title: &str,
    subtitle: Option<&str>,
    add_actions: impl FnOnce(&mut Ui) -> R,
) -> R {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let mut result = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(title)
                    .size(metrics.title)
                    .strong()
                    .color(tokens.color.text.to_color32()),
            );
            if let Some(subtitle) = subtitle {
                ui.label(
                    RichText::new(subtitle)
                        .size(metrics.caption)
                        .color(tokens.color.text_muted.to_color32()),
                );
            }
        });
        result = Some(
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| add_actions(ui))
                .inner,
        );
    });
    result.expect("the header closure always runs")
}

/// Estado vazio: um ícone grande, uma frase curta e, se houver, a ação principal. Devolve a
/// resposta da ação (para `.clicked()`), ou `None` quando não há ação.
pub fn empty_state(ui: &mut Ui, glyph: &str, message: &str, action: Option<&str>) -> Option<Response> {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let muted = tokens.color.text_muted.to_color32();
    let mut response = None;
    ui.vertical_centered(|ui| {
        ui.add_space(metrics.gap * 2.0);
        ui.label(RichText::new(glyph).font(icon::font_id(28.0)).color(muted));
        ui.label(RichText::new(message).color(muted));
        if let Some(action) = action {
            ui.add_space(metrics.gap);
            response = Some(button(ui, ButtonKind::Primary, action));
        }
    });
    response
}
