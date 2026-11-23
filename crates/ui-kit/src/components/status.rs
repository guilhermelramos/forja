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

/// Quebra `text` dentro da largura disponível do `Ui`, inclusive no meio de
/// sequências contínuas sem espaços (caminhos, URLs, entradas longas).
pub fn wrapped_label(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) -> Response {
    let max_width = ui.available_width().max(1.0);
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width,
        max_rows: usize::MAX,
        break_anywhere: true,
        ..Default::default()
    };
    let galley = ui.painter().layout_job(job);
    ui.add(egui::Label::new(galley))
}

/// Exibe `text` em uma linha com reticências quando ultrapassar a largura
/// disponível, anexando o texto completo em dica ao passar o cursor.
pub fn truncated_label(ui: &mut Ui, text: &str, font: egui::FontId, color: Color32) -> Response {
    let max_width = ui.available_width().max(1.0);
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    let truncated = galley.elided;
    let response = ui.add(egui::Label::new(galley));
    if truncated {
        crate::tooltip(response, text)
    } else {
        response
    }
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
            let max_w = ui.available_width().max(1.0);
            ui.set_max_width(max_w);
            ui.horizontal_top(|ui| {
                let (rect, _) = ui
                    .allocate_exact_size(egui::vec2(4.0, metrics.body + 4.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, CornerRadius::same(2), kind_color(&tokens, kind));
                ui.vertical(|ui| {
                    let text_max = ui.available_width().max(1.0);
                    ui.set_max_width(text_max);
                    wrapped_label(
                        ui,
                        text,
                        body_font(&metrics),
                        tokens.color.text.to_color32(),
                    );
                });
            });
        })
        .response
}

/// Toast with a presence envelope: fades in and out and slides from the
/// anchored edge. Only visuals move, so hit areas and accessibility are stable.
pub fn toast_transition(
    ui: &mut Ui,
    kind: ToastKind,
    text: &str,
    presence: f32,
    slide_x: f32,
) -> Response {
    ui.set_opacity(presence.clamp(0.0, 1.0));
    if slide_x.abs() > 0.01 {
        ui.with_visual_transform(
            egui::emath::TSTransform::from_translation(egui::vec2(slide_x, 0.0)),
            |ui| toast(ui, kind, text),
        )
        .inner
    } else {
        toast(ui, kind, text)
    }
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
        let row_width = ui.available_width().max(1.0);
        ui.set_max_width(row_width);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
            if let Some(dismiss) = dismiss_label {
                let button = crate::icon_button(ui, crate::CLOSE, dismiss);
                button.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), dismiss)
                });
                if button.clicked() {
                    dismissed = true;
                }
            }
            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                let msg_max = ui.available_width().max(1.0);
                ui.set_max_width(msg_max);
                wrapped_label(
                    ui,
                    message,
                    caption_font(&metrics),
                    tokens.color.danger.to_color32(),
                );
            });
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
                let detail_max = ui.available_width().max(1.0);
                ui.set_max_width(detail_max);
                wrapped_label(
                    ui,
                    detail,
                    egui::FontId::monospace(metrics.caption),
                    tokens.color.text_muted.to_color32(),
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
    let fraction = crate::motion_value(ui.ctx(), response.id.with("progress"), value.clamp(0.0, 1.0), crate::Motion::Slow);
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

/// Textos e estado da orientação de ajudante ausente.
pub struct HelperMissingGuide<'a> {
    /// Título do aviso (nenhum ou perfil específico ausente).
    pub headline: &'a str,
    /// Orientação curta sobre como instalar ou escolher outro ajudante.
    pub body: &'a str,
    /// Comando opcional de instalação em destaque.
    pub install_command: Option<&'a str>,
    /// Rótulo do botão de copiar o comando (ou confirmação de cópia).
    pub copy_label: &'a str,
    /// Rótulo do botão para abrir a página Ajudantes.
    pub open_helpers_label: Option<&'a str>,
}

/// Desenha o bloco padrão de orientação quando falta um ajudante instalado.
/// Devolve `true` se o usuário clicou para abrir a página de Ajudantes.
pub fn helper_missing_guide(
    ui: &mut Ui,
    guide: &HelperMissingGuide<'_>,
    copied: &mut bool,
) -> bool {
    let mut open_helpers = false;
    ui.group(|ui| {
        ui.label(egui::RichText::new(guide.headline).strong());
        ui.label(egui::RichText::new(guide.body).weak());
        ui.horizontal_wrapped(|ui| {
            if let Some(command) = guide.install_command {
                ui.label(egui::RichText::new(command).monospace());
                if ui.small_button(guide.copy_label).clicked() {
                    ui.ctx().copy_text(command.to_owned());
                    *copied = true;
                }
            }
            if let Some(label) = guide.open_helpers_label {
                if crate::button(ui, crate::ButtonKind::Secondary, label).clicked() {
                    open_helpers = true;
                }
            }
        });
    });
    open_helpers
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

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{CentralPanel, Context, FontId, Pos2, RawInput, Rect};

    fn run_in_width(width: f32, mut draw: impl FnMut(&mut Ui)) -> Rect {
        let ctx = Context::default();
        crate::fonts::install_fonts(&ctx).expect("fontes do Windows");
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 600.0))),
            ..Default::default()
        };
        let mut used = Rect::NOTHING;
        let _ = ctx.run(input, |ctx| {
            CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
                ui.set_max_width(width);
                draw(ui);
                used = ui.min_rect();
            });
        });
        used
    }

    #[test]
    fn wrapped_and_truncated_labels_stay_within_width_for_unbreakable_text() {
        let long = "C:\\forja-missing-folder\\ghost\\999\\a-very-long-unbreakable-segment-0123456789-abcdefghijklmnopqrstuvwxyz";
        let font = FontId::proportional(13.0);

        let wrapped_rect = run_in_width(160.0, |ui| {
            let r = wrapped_label(ui, long, font.clone(), Color32::WHITE);
            assert!(
                r.rect.width() <= 160.5,
                "wrapped width {} > 160",
                r.rect.width()
            );
        });
        assert!(wrapped_rect.width() <= 160.5);

        let truncated_rect = run_in_width(160.0, |ui| {
            let r = truncated_label(ui, long, font.clone(), Color32::WHITE);
            assert!(
                r.rect.width() <= 160.5,
                "truncated width {} > 160",
                r.rect.width()
            );
            assert!(
                r.rect.height() <= 30.0,
                "truncated should stay single line: {}",
                r.rect.height()
            );
        });
        assert!(truncated_rect.width() <= 160.5);
    }

    #[test]
    fn toast_and_error_notice_wrap_within_panel_width() {
        let long = "Não foi possível completar a operação: https://example.com/a-very-long-unbreakable-path-0123456789-abcdefghijklmnopqrstuvwxyz";

        let toast_rect = run_in_width(280.0, |ui| {
            let resp = toast(ui, ToastKind::Danger, long);
            assert!(
                resp.rect.right() <= 281.0,
                "toast right {} > 280",
                resp.rect.right()
            );
        });
        assert!(toast_rect.width() <= 281.0);

        let notice_rect = run_in_width(280.0, |ui| {
            let dismissed = error_notice(ui, long, Some(long), "Detalhes", Some("Fechar"));
            assert!(!dismissed);
        });
        assert!(
            notice_rect.width() <= 281.0,
            "notice width {} > 280",
            notice_rect.width()
        );
    }
}
