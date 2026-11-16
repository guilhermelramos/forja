//! Cabeçalho de página e estado vazio. Os rótulos vêm de quem chama.

use eframe::egui::{self, Align, Align2, Layout, Response, RichText, Sense, Ui};

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
        // Pintura direta: um ícone PUA em ui.label() entra na árvore de acessibilidade como rótulo vazio/glifo.
        let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), Sense::hover());
        ui.painter().text(
            icon_rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            icon::font_id(28.0),
            muted,
        );
        ui.label(RichText::new(message).color(muted));
        if let Some(action) = action {
            ui.add_space(metrics.gap);
            response = Some(button(ui, ButtonKind::Primary, action));
        }
    });
    response
}

/// Read-only theme sample. Uses explicit tokens without changing the page theme.
pub fn theme_sample(ui: &mut Ui, tokens: &crate::Tokens, title: &str, body: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(210.0, 100.0), Sense::hover());
    let colors = tokens.color;
    ui.painter().rect(
        rect,
        8.0,
        colors.surface.to_color32(),
        egui::Stroke::new(1.0, colors.border.to_color32()),
        egui::StrokeKind::Inside,
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(4.0, 100.0)),
        2.0,
        colors.accent.to_color32(),
    );
    ui.put(
        egui::Rect::from_min_size(rect.min + egui::vec2(16.0, 12.0), egui::vec2(180.0, 30.0)),
        egui::Label::new(
            RichText::new(title)
                .strong()
                .color(colors.text.to_color32()),
        ),
    );
    ui.put(
        egui::Rect::from_min_size(rect.min + egui::vec2(16.0, 46.0), egui::vec2(180.0, 42.0)),
        egui::Label::new(RichText::new(body).color(colors.text_muted.to_color32())).wrap(),
    );
}

#[cfg(test)]
mod preview_tests {
    #[test]
    fn theme_preview_uses_explicit_colors_without_changing_the_page() {
        let ctx = eframe::egui::Context::default();
        let dark = crate::Tokens::brasa_dark();
        let light = crate::Tokens::brasa_light();
        crate::apply(&ctx, &dark);
        let output = ctx.run(Default::default(), |ctx| {
            eframe::egui::CentralPanel::default().show(ctx, |ui| {
                super::theme_sample(ui, &light, "Preview", "Sample")
            });
        });
        assert_eq!(crate::applied_tokens(&ctx).mode, dark.mode);
        assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,eframe::egui::Shape::Rect(rect) if rect.fill==light.color.surface.to_color32())));
    }
}
