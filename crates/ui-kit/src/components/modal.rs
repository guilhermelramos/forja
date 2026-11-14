//! Modal e o corpo que a galeria mostra sem o véu.

use eframe::egui::{self, Color32, Frame, Id, Margin, Modal, Ui};

use crate::apply::{elevation_shadow, pad_i8, radius_u8};
use crate::paint::{self, title_font};

/// Diálogo centrado. Fecha no véu ou no Escape.
pub fn modal(ctx: &egui::Context, open: &mut bool, title: &str, add_body: impl FnOnce(&mut Ui)) {
    modal_impl(ctx, open, title, true, add_body);
}

/// Diálogo centrado que só fecha por ação explícita: Escape, um botão de
/// fechar ou `Ui::close`. O clique no véu não fecha, então um clique perdido
/// não descarta o diálogo nem vaza para o controle atrás dele.
pub fn modal_explicit_close(
    ctx: &egui::Context,
    open: &mut bool,
    title: &str,
    add_body: impl FnOnce(&mut Ui),
) {
    modal_impl(ctx, open, title, false, add_body);
}

fn modal_impl(
    ctx: &egui::Context,
    open: &mut bool,
    title: &str,
    close_on_backdrop: bool,
    add_body: impl FnOnce(&mut Ui),
) {
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
    let layer = Id::new(("forja_modal.keyboard", title));
    let response = Modal::new(Id::new(("forja_modal", title)))
        .backdrop_color(backdrop)
        .frame(dialog_frame(&tokens, &metrics))
        .show(ctx, |ui| {
            crate::keyboard_layer::register_keyboard_layer(ui, layer);
            dialog_body(ui, &tokens, &metrics, title, add_body)
        });
    let close = if close_on_backdrop {
        response.should_close()
    } else {
        response.response.should_close()
            || crate::keyboard_layer::consume_layer_escape(ctx, layer)
    };
    if close {
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

/// Cabeçalho de janela com botão de fechar localizado (substitui o `.open(&mut open)` padrão do egui).
pub fn window_header(ui: &mut Ui, title: &str, close_label: &str) -> bool {
    window_header_with_focus(ui, title, close_label, false)
}

/// Window header that can place initial keyboard focus on its close button.
pub fn window_header_with_focus(ui: &mut Ui, title: &str, close_label: &str, focus_close: bool) -> bool {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let mut close = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .font(title_font(&metrics))
                .color(tokens.color.text.to_color32()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let response = crate::icon::icon_button(ui, crate::icon::CLOSE)
                .on_hover_text(close_label);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), close_label)
            });
            if focus_close {
                response.request_focus();
            }
            if response.clicked() {
                close = true;
            }
        });
    });
    ui.separator();
    close
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::fonts::install_fonts(&ctx).expect("fontes do Windows");
        ctx
    }

    fn frame_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        }
    }

    fn backdrop_click(pressed: bool) -> egui::RawInput {
        let mut raw = frame_input();
        let pos = egui::pos2(2.0, 2.0);
        raw.events.push(egui::Event::PointerMoved(pos));
        raw.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        raw
    }

    fn click_backdrop(ctx: &egui::Context, mut draw: impl FnMut(&egui::Context)) {
        for _ in 0..2 {
            let _ = ctx.run(frame_input(), &mut draw);
        }
        let _ = ctx.run(backdrop_click(true), &mut draw);
        let _ = ctx.run(backdrop_click(false), &mut draw);
    }

    #[test]
    fn explicit_close_modal_ignores_the_backdrop() {
        let ctx = test_ctx();
        let mut open = true;
        click_backdrop(&ctx, |ctx| {
            modal_explicit_close(ctx, &mut open, "Guia", |_ui| {});
        });
        assert!(open, "the backdrop closed an explicit-close modal");
    }

    #[test]
    fn explicit_close_modal_blocks_background_clicks() {
        let ctx = test_ctx();
        let mut open = true;
        let mut background_clicks = 0;
        click_backdrop(&ctx, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if ui.allocate_rect(ctx.content_rect(), egui::Sense::click()).clicked() {
                    background_clicks += 1;
                }
            });
            modal_explicit_close(ctx, &mut open, "Guia", |_ui| {});
        });
        assert!(open);
        assert_eq!(background_clicks, 0);
    }

    #[test]
    fn focused_window_header_closes_with_enter() {
        let ctx = test_ctx();
        let mut closed = false;
        for frame in 0..3 {
            let mut raw = frame_input();
            if frame == 2 {
                raw.events.push(egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(raw, |ctx| {
                egui::Window::new("Window").show(ctx, |ui| {
                    closed |= window_header_with_focus(ui, "Window", "Close", frame == 1);
                });
            });
        }
        assert!(closed);
    }

    #[test]
    fn default_modal_still_closes_on_the_backdrop() {
        let ctx = test_ctx();
        let mut open = true;
        click_backdrop(&ctx, |ctx| {
            modal(ctx, &mut open, "Diálogo", |_ui| {});
        });
        assert!(!open, "the backdrop did not close a default modal");
    }
}
