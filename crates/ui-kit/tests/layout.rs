use forja_ui_kit::{
    apply, button, progress_bar, segmented, stage_badge, tabs, text_field, toggle, virtual_table,
    ButtonKind, Column, Stage, Tokens,
};

fn run(tokens: &Tokens, mut draw: impl FnMut(&mut forja_ui_kit::egui::Ui)) {
    let ctx = forja_ui_kit::egui::Context::default();
    let input = forja_ui_kit::egui::RawInput {
        screen_rect: Some(forja_ui_kit::egui::Rect::from_min_size(
            forja_ui_kit::egui::Pos2::ZERO,
            forja_ui_kit::egui::vec2(960.0, 720.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        apply(ctx, tokens);
        forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| draw(ui));
    });
}

#[test]
fn comfortable_controls_are_32_px() {
    let mut height = 0.0;
    run(&Tokens::brasa_dark(), |ui| {
        height = button(ui, ButtonKind::Primary, "Forjar").rect.height();
    });
    assert!((height - 32.0).abs() < 0.5, "altura {height}");
}

#[test]
fn compact_controls_are_24_px() {
    let mut tokens = Tokens::brasa_light();
    tokens.density = forja_ui_kit::Density::Compact;
    let mut button_h = 0.0;
    let mut badge_h = 0.0;
    let mut progress_h = 0.0;
    run(&tokens, |ui| {
        button_h = button(ui, ButtonKind::Secondary, "Guardar").rect.height();
        badge_h = stage_badge(ui, Stage::Ready, "Pronto").rect.height();
        progress_h = progress_bar(ui, 0.4).rect.height();
    });
    assert!((button_h - 24.0).abs() < 0.5, "botão {button_h}");
    assert!((badge_h - 16.0).abs() < 0.5, "badge {badge_h}");
    assert!((progress_h - 4.0).abs() < 0.5, "progresso {progress_h}");
}

#[test]
fn field_tabs_and_segment_share_the_control_height() {
    let mut field_h = 0.0;
    let mut tab_h = 0.0;
    let mut segment_h = 0.0;
    let mut name = String::from("bancada");
    let mut tab = 0;
    let mut segment = 1;
    run(&Tokens::brasa_dark(), |ui| {
        field_h = text_field(ui, &mut name, "nome").rect.height();
        tab_h = tabs(ui, &mut tab, &["Quadro", "Pedidos"]).rect.height();
        segment_h = segmented(ui, &mut segment, &["Terminal", "Editor"])
            .rect
            .height();
    });
    assert!((field_h - 32.0).abs() < 1.0, "campo {field_h}");
    assert!((tab_h - 32.0).abs() < 0.5, "abas {tab_h}");
    assert!((segment_h - 32.0).abs() < 0.5, "segmento {segment_h}");
}

#[test]
fn toggle_flips_on_click() {
    let ctx = forja_ui_kit::egui::Context::default();
    let mut on = false;
    let mut rect = forja_ui_kit::egui::Rect::NOTHING;
    let screen = forja_ui_kit::egui::RawInput {
        screen_rect: Some(forja_ui_kit::egui::Rect::from_min_size(
            forja_ui_kit::egui::Pos2::ZERO,
            forja_ui_kit::egui::vec2(480.0, 200.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(screen.clone(), |ctx| {
        apply(ctx, &Tokens::brasa_dark());
        forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| {
            rect = toggle(ui, &mut on, "Pronto").rect;
        });
    });
    let center = rect.center();
    let mut press = screen.clone();
    press
        .events
        .push(forja_ui_kit::egui::Event::PointerMoved(center));
    press.events.push(forja_ui_kit::egui::Event::PointerButton {
        pos: center,
        button: forja_ui_kit::egui::PointerButton::Primary,
        pressed: true,
        modifiers: forja_ui_kit::egui::Modifiers::default(),
    });
    let _ = ctx.run(press, |ctx| {
        apply(ctx, &Tokens::brasa_dark());
        forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| {
            toggle(ui, &mut on, "Pronto");
        });
    });
    let mut release = screen;
    release
        .events
        .push(forja_ui_kit::egui::Event::PointerMoved(center));
    release
        .events
        .push(forja_ui_kit::egui::Event::PointerButton {
            pos: center,
            button: forja_ui_kit::egui::PointerButton::Primary,
            pressed: false,
            modifiers: forja_ui_kit::egui::Modifiers::default(),
        });
    let _ = ctx.run(release, |ctx| {
        apply(ctx, &Tokens::brasa_dark());
        forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| {
            toggle(ui, &mut on, "Pronto");
        });
    });
    assert!(on, "o clique não inverteu o toggle");
}

#[test]
fn virtual_table_paints_only_the_visible_rows() {
    let mut painted = 0;
    run(&Tokens::brasa_dark(), |ui| {
        virtual_table(
            ui,
            &[
                Column {
                    title: "Peça",
                    width: 120.0,
                },
                Column {
                    title: "Etapa",
                    width: 80.0,
                },
            ],
            80,
            80.0,
            |_ui, _row, column| {
                if column == 0 {
                    painted += 1;
                }
            },
        );
    });
    assert!(painted > 0, "nenhuma linha pintada");
    assert!(
        painted < 80,
        "pintou {painted} de 80, a virtualização não cortou"
    );
}

#[test]
fn windows_fonts_shape_interface_and_code() {
    let ctx = forja_ui_kit::egui::Context::default();
    forja_ui_kit::install_fonts(&ctx).expect("fontes do Windows");
    let input = forja_ui_kit::egui::RawInput {
        screen_rect: Some(forja_ui_kit::egui::Rect::from_min_size(
            forja_ui_kit::egui::Pos2::ZERO,
            forja_ui_kit::egui::vec2(400.0, 120.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(input, |ctx| {
        apply(ctx, &Tokens::brasa_dark());
        forja_ui_kit::egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Forja");
            ui.label(
                forja_ui_kit::egui::RichText::new("fn forjar() {}")
                    .font(forja_ui_kit::egui::FontId::monospace(13.0)),
            );
        });
    });
}
