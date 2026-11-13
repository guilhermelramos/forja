//! Estados dos componentes, sem janela: clique em controle desabilitado não conta, e o
//! desabilitado pinta com 40% de opacidade (ui.md, 1.6).

use forja_ui_kit::egui::{self, Color32, Event, PointerButton, Pos2, RawInput, Rect};
use forja_ui_kit::{apply, button, ButtonKind, Tokens};
use std::cell::Cell;

/// Roda um quadro com `draw` e os eventos dados. Devolve a saída (formas pintadas) do quadro.
fn frame(
    ctx: &egui::Context,
    tokens: &Tokens,
    events: Vec<Event>,
    mut draw: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(960.0, 720.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| {
        apply(ctx, tokens);
        egui::CentralPanel::default().show(ctx, |ui| draw(ui));
    })
}

/// Clique completo (mover, apertar e soltar) no centro de `at`.
fn click_at(at: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(at),
        Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ]
}

/// Clica num botão do quadro anterior e diz se o clique foi registrado no quadro seguinte.
fn clicked_when(enabled: bool) -> bool {
    let tokens = Tokens::brasa_dark();
    let ctx = egui::Context::default();
    let rect: Cell<Option<Rect>> = Cell::new(None);
    let clicked = Cell::new(false);
    let draw = |ui: &mut egui::Ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let response = button(ui, ButtonKind::Primary, "Enviar");
            if response.clicked() {
                clicked.set(true);
            }
            rect.set(Some(response.rect));
        });
    };
    // Primeiro quadro: só para saber onde o botão cai.
    frame(&ctx, &tokens, Vec::new(), draw);
    let at = rect.get().expect("button laid out").center();
    // Segundo quadro: o clique de verdade.
    frame(&ctx, &tokens, click_at(at), draw);
    clicked.get()
}

#[test]
fn an_enabled_button_reports_its_click() {
    assert!(clicked_when(true));
}

#[test]
fn a_disabled_button_ignores_the_click() {
    assert!(!clicked_when(false));
}

/// Maior alfa entre os retângulos pintados com `color`. As cores do egui são pré-multiplicadas:
/// um preenchimento `color` com alfa `a` guarda cada canal como `channel * a / 255`.
fn filled_alpha(output: &egui::FullOutput, color: Color32) -> Option<u8> {
    output
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Rect(rect) => Some(rect.fill),
            _ => None,
        })
        .filter(|fill| {
            let a = fill.a() as f32 / 255.0;
            a > 0.0
                && [
                    (fill.r(), color.r()),
                    (fill.g(), color.g()),
                    (fill.b(), color.b()),
                ]
                .iter()
                .all(|(stored, channel)| (*stored as f32 - *channel as f32 * a).abs() <= 2.0)
        })
        .map(|fill| fill.a())
        .max()
}

#[test]
fn a_disabled_primary_button_is_painted_at_forty_percent() {
    let tokens = Tokens::brasa_dark();
    let accent = tokens.color.accent.to_color32();
    let ctx = egui::Context::default();

    let enabled = frame(&ctx, &tokens, Vec::new(), |ui| {
        button(ui, ButtonKind::Primary, "Enviar");
    });
    let disabled = frame(&ctx, &tokens, Vec::new(), |ui| {
        ui.add_enabled_ui(false, |ui| {
            button(ui, ButtonKind::Primary, "Enviar");
        });
    });

    let full = filled_alpha(&enabled, accent).expect("enabled fill is painted");
    let dim = filled_alpha(&disabled, accent).expect("disabled fill is painted");
    assert_eq!(full, 255, "the accent fill is opaque when enabled");
    let ratio = dim as f32 / full as f32;
    assert!(
        (ratio - 0.4).abs() < 0.05,
        "disabled fill alpha {dim} is {ratio:.2} of the enabled one, expected 0.40"
    );
}
