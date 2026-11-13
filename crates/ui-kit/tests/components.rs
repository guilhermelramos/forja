//! Componentes novos e o `select` que agora reporta mudança, sem janela.

use forja_ui_kit::egui::{self, Event, PointerButton, Pos2, RawInput, Rect};
use forja_ui_kit::{apply, empty_state, install_fonts, page_heading, select, Tokens};
use std::cell::Cell;

const WIDTH: f32 = 960.0;

fn frame(
    ctx: &egui::Context,
    tokens: &Tokens,
    events: Vec<Event>,
    mut draw: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(WIDTH, 720.0))),
        events,
        ..Default::default()
    };
    ctx.run(input, |ctx| {
        apply(ctx, tokens);
        egui::CentralPanel::default().show(ctx, |ui| draw(ui));
    })
}

fn click(at: Pos2) -> Vec<Event> {
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

const OPTIONS: [&str; 3] = ["Rascunho", "Forjando", "Pronto"];

/// Contexto com as fontes do sistema instaladas (o glifo de ícone precisa da família `forja-icons`).
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    install_fonts(&ctx).expect("Windows fonts are installed on the build machine");
    ctx
}

/// Abre o seletor com um clique no gatilho e depois clica na opção `index`. Devolve se a resposta
/// do último quadro marcou mudança, e o índice escolhido.
fn choose(index: Option<usize>) -> (bool, usize) {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let mut selected = 0usize;
    let trigger: Cell<Option<Rect>> = Cell::new(None);
    let changed = Cell::new(false);
    let draw = |ui: &mut egui::Ui, selected: &mut usize| {
        let response = select(ui, selected, &OPTIONS);
        trigger.set(Some(response.rect));
        changed.set(response.changed());
    };
    frame(&ctx, &tokens, Vec::new(), |ui| draw(ui, &mut selected));
    let gate = trigger.get().expect("select laid out");
    // Abre o popup. Com ele aberto, cada opção aparece como texto desenhado: clicamos no centro do
    // texto da opção pedida, que é onde a linha do popup está.
    frame(&ctx, &tokens, click(gate.center()), |ui| draw(ui, &mut selected));
    // O popup aparece no quadro seguinte ao clique: um quadro sem eventos o desenha.
    let open = frame(&ctx, &tokens, Vec::new(), |ui| draw(ui, &mut selected));
    if let Some(index) = index {
        let at = text_center(&open, OPTIONS[index]).expect("the open popup draws the option text");
        frame(&ctx, &tokens, click(at), |ui| draw(ui, &mut selected));
    }
    (changed.get(), selected)
}

/// Centro, na tela, do texto `wanted` desenhado no quadro.
fn text_center(output: &egui::FullOutput, wanted: &str) -> Option<Pos2> {
    output.shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Text(text) if text.galley.text() == wanted => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        _ => None,
    })
}

#[test]
fn select_reports_a_change_when_an_option_is_picked() {
    let (changed, selected) = choose(Some(2));
    assert_eq!(selected, 2, "the option under the click is chosen");
    assert!(changed, "the response of the frame that picked the option is marked changed");
}

#[test]
fn select_reports_no_change_when_it_only_opens() {
    let (changed, selected) = choose(None);
    assert_eq!(selected, 0);
    assert!(!changed);
}

#[test]
fn page_heading_returns_what_the_actions_return_and_places_them_on_the_right() {
    let tokens = Tokens::brasa_dark();
    let ctx = egui::Context::default();
    let action_x: std::cell::Cell<Option<f32>> = std::cell::Cell::new(None);
    let mut returned = 0;
    frame(&ctx, &tokens, Vec::new(), |ui| {
        returned = page_heading(ui, "Quadro", Some("Bancadas por estágio"), |ui| {
            let response = forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, "Nova bancada");
            action_x.set(Some(response.rect.center().x));
            7
        });
    });
    assert_eq!(returned, 7, "the closure result comes back");
    let x = action_x.get().expect("action laid out");
    assert!(x > WIDTH / 2.0, "actions sit on the right half, at x = {x}");
}

#[test]
fn empty_state_without_an_action_has_no_response() {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let mut response_is_some = true;
    frame(&ctx, &tokens, Vec::new(), |ui| {
        response_is_some = empty_state(ui, forja_ui_kit::BOARD, "Nada aqui.", None).is_some();
    });
    assert!(!response_is_some);
}

#[test]
fn empty_state_action_reports_its_click() {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let rect: Cell<Option<Rect>> = Cell::new(None);
    let clicked = Cell::new(false);
    let draw = |ui: &mut egui::Ui| {
        if let Some(response) = empty_state(ui, forja_ui_kit::HELPERS, "Nenhum ajudante.", Some("Detectar")) {
            rect.set(Some(response.rect));
            if response.clicked() {
                clicked.set(true);
            }
        }
    };
    frame(&ctx, &tokens, Vec::new(), draw);
    let at = rect.get().expect("action laid out").center();
    frame(&ctx, &tokens, click(at), draw);
    assert!(clicked.get(), "clicking the action of an empty state counts");
}
