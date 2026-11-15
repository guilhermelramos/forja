//! Componentes novos e o `select` que agora reporta mudança, sem janela.

use forja_ui_kit::egui::{self, Event, PointerButton, Pos2, RawInput, Rect};
use forja_ui_kit::{apply, context_menu, empty_state, install_fonts, page_heading, select, MenuItem, Tokens};
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

fn press(key: egui::Key) -> Vec<Event> {
    vec![
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        },
        Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        },
    ]
}

#[test]
fn select_changes_value_with_tab_space_arrows_and_enter() {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let mut selected = 0usize;
    let changed = Cell::new(false);
    let mut draw = |ui: &mut egui::Ui| {
        let response = select(ui, &mut selected, &OPTIONS);
        if response.changed() {
            changed.set(true);
        }
    };
    frame(&ctx, &tokens, Vec::new(), &mut draw);
    frame(&ctx, &tokens, press(egui::Key::Tab), &mut draw);
    frame(&ctx, &tokens, press(egui::Key::Space), &mut draw);
    frame(&ctx, &tokens, press(egui::Key::ArrowDown), &mut draw);
    frame(&ctx, &tokens, press(egui::Key::ArrowDown), &mut draw);
    frame(&ctx, &tokens, press(egui::Key::Enter), &mut draw);
    assert_eq!(selected, 2, "ArrowDown moves the highlight and Enter picks it");
    assert!(changed.get(), "the response reports the keyboard choice");
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

#[test]
fn context_menu_picks_item_in_long_scrollable_list() {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let labels: Vec<String> = (0..20).map(|i| format!("Recipe item {i}")).collect();
    let items: Vec<MenuItem<'_>> = labels
        .iter()
        .map(|l| MenuItem {
            label: l.as_str(),
            danger: false,
        })
        .collect();
    let mut picked = None;
    let open_init = frame(&ctx, &tokens, Vec::new(), |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Receitas", &items) {
            picked = Some(choice);
        }
    });
    let at_trigger = text_center(&open_init, "Receitas").expect("Receitas trigger rendered");
    frame(&ctx, &tokens, click(at_trigger), |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Receitas", &items) {
            picked = Some(choice);
        }
    });
    let open = frame(&ctx, &tokens, Vec::new(), |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Receitas", &items) {
            picked = Some(choice);
        }
    });
    let at_item = text_center(&open, "Recipe item 14").expect("item 14 is visible in 720px window");
    // Simula clique em dois quadros como no test_driver (press num quadro, release no seguinte).
    frame(&ctx, &tokens, vec![
        Event::PointerMoved(at_item),
        Event::PointerButton {
            pos: at_item,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    ], |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Receitas", &items) {
            picked = Some(choice);
        }
    });
    frame(&ctx, &tokens, vec![
        Event::PointerButton {
            pos: at_item,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ], |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Receitas", &items) {
            picked = Some(choice);
        }
    });
    assert_eq!(picked, Some(14));
}

#[test]
fn context_menu_picks_visible_item_when_scrolling() {
    let tokens = Tokens::brasa_dark();
    let ctx = context();
    let labels: Vec<String> = (0..30).map(|i| format!("Long menu entry {i}")).collect();
    let items: Vec<MenuItem<'_>> = labels
        .iter()
        .map(|l| MenuItem {
            label: l.as_str(),
            danger: false,
        })
        .collect();
    let mut picked = None;
    let init = frame(&ctx, &tokens, Vec::new(), |ui| {
        ui.add_space(40.0);
        let _ = context_menu(ui, "Menu", &items);
    });
    let trigger = text_center(&init, "Menu").expect("trigger");
    frame(&ctx, &tokens, click(trigger), |ui| {
        ui.add_space(40.0);
        let _ = context_menu(ui, "Menu", &items);
    });
    let open = frame(&ctx, &tokens, Vec::new(), |ui| {
        ui.add_space(40.0);
        let _ = context_menu(ui, "Menu", &items);
    });
    let item = text_center(&open, "Long menu entry 4").expect("entry 4 is visible");
    frame(&ctx, &tokens, click(item), |ui| {
        ui.add_space(40.0);
        if let Some(choice) = context_menu(ui, "Menu", &items) {
            picked = Some(choice);
        }
    });
    assert_eq!(picked, Some(4));
}
