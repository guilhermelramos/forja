//! Toast, badge de etapa e barra de progresso.

use eframe::egui::{self, Align2, Color32, CornerRadius, Frame, Margin, Response, Stroke, Ui};

use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, body_font, caption_font, paint_text};
use crate::tokens::Tokens;

/// Tom do aviso.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    /// Informação.
    Info,
    /// Sucesso.
    Ok,
    /// Atenção.
    Spark,
    /// Erro.
    Danger,
}

/// Faixa curta. O texto usa `text`; a cor fica na marca da esquerda.
pub fn toast(ui: &mut Ui, kind: ToastKind, text: &str) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    Frame::new()
        .fill(tokens.color.surface.to_color32())
        .stroke(Stroke::new(1.0, tokens.color.border.to_color32()))
        .corner_radius(radius_u8(metrics.radius_control))
        .shadow(elevation_shadow(2, tokens.mode))
        .inner_margin(Margin::symmetric(
            pad_i8(metrics.pad_x),
            pad_i8(metrics.pad_y),
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui
                    .allocate_exact_size(egui::vec2(4.0, metrics.body + 4.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(2), kind_color(&tokens, kind));
                ui.label(
                    egui::RichText::new(text)
                        .font(body_font(&metrics))
                        .color(tokens.color.text.to_color32()),
                );
            });
        })
        .response
}

/// Exibe uma mensagem de erro humana com detalhe técnico recolhível opcional.
pub fn error_banner(ui: &mut Ui, message: &str, detail: Option<&str>, details_label: &str) {
    let _ = error_notice(ui, message, detail, details_label, None);
}

/// Exibe uma mensagem de erro humana com detalhe técnico recolhível e botão
/// opcional de fechar (`dismiss_label`). Devolve `true` se o usuário fechou.
pub fn error_notice(
    ui: &mut Ui,
    message: &str,
    detail: Option<&str>,
    details_label: &str,
    dismiss_label: Option<&str>,
) -> bool {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let mut dismissed = false;
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(message)
                    .font(caption_font(&metrics))
                    .color(tokens.color.danger.to_color32()),
            );
            if let Some(dismiss) = dismiss_label {
                let button = crate::icon_button(ui, crate::CLOSE).on_hover_text(dismiss);
                button.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), dismiss)
                });
                if button.clicked() {
                    dismissed = true;
                }
            }
        });
        if let Some(detail) = detail.map(str::trim).filter(|text| !text.is_empty()) {
            egui::CollapsingHeader::new(
                egui::RichText::new(details_label)
                    .font(caption_font(&metrics))
                    .color(tokens.color.text_muted.to_color32()),
            )
            .id_salt((details_label, message))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(detail)
                        .monospace()
                        .font(caption_font(&metrics))
                        .color(tokens.color.text_muted.to_color32()),
                );
            });
        }
    });
    dismissed
}

/// Etapa de uma bancada. O nome vem de quem chama.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Rascunho.
    Draft,
    /// Forjando.
    Forging,
    /// Revisão.
    Review,
    /// Pronto.
    Ready,
}

/// Badge com marca colorida e texto `text`.
pub fn stage_badge(ui: &mut Ui, stage: Stage, label: &str) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let font = caption_font(&metrics);
    let text = tokens.color.text.to_color32();
    let text_size = paint::text_size(ui, label, font.clone(), text);
    let width = text_size.x + metrics.pad_x * 2.0 + 10.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, metrics.badge), egui::Sense::hover());
    let painter = paint::painter_for(ui);
    painter.rect_filled(
        rect,
        CornerRadius::same(radius_u8(metrics.radius_control)),
        tokens.color.surface_alt.to_color32(),
    );
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 6.0, rect.center().y - 4.0),
        egui::vec2(4.0, 8.0),
    );
    painter.rect_filled(bar, CornerRadius::same(2), stage_color(&tokens, stage));
    paint_text(
        &painter,
        rect.shrink2(egui::vec2(14.0, 0.0)),
        Align2::LEFT_CENTER,
        label,
        font,
        text,
    );
    response
}

/// Barra de 0 a 1. Valores fora do intervalo são cortados.
pub fn progress_bar(ui: &mut Ui, value: f32) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let width = ui.available_width().clamp(48.0, 280.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, metrics.progress), egui::Sense::hover());
    let radius = radius_u8(metrics.progress * 0.5);
    let painter = paint::painter_for(ui);
    painter.rect_filled(
        rect,
        CornerRadius::same(radius),
        tokens.color.surface_alt.to_color32(),
    );
    let fraction = value.clamp(0.0, 1.0);
    if fraction > 0.0 {
        let fill =
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * fraction, rect.height()));
        painter.rect_filled(
            fill,
            CornerRadius::same(radius),
            tokens.color.accent.to_color32(),
        );
    }
    response
}

fn kind_color(tokens: &Tokens, kind: ToastKind) -> Color32 {
    match kind {
        ToastKind::Info => tokens.color.info.to_color32(),
        ToastKind::Ok => tokens.color.ok.to_color32(),
        ToastKind::Spark => tokens.color.spark.to_color32(),
        ToastKind::Danger => tokens.color.danger.to_color32(),
    }
}

fn stage_color(tokens: &Tokens, stage: Stage) -> Color32 {
    match stage {
        Stage::Draft => tokens.color.text_muted.to_color32(),
        Stage::Forging => tokens.color.accent.to_color32(),
        Stage::Review => tokens.color.spark.to_color32(),
        Stage::Ready => tokens.color.ok.to_color32(),
    }
}
