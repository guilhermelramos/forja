use crate::{egui, *};

fn draw(tokens: Tokens, render: impl FnOnce(&mut egui::Ui)) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let _ = install_fonts(&ctx);
    apply(&ctx, &tokens);
    ctx.enable_accesskit();
    let mut render = Some(render);
    ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| render.take().unwrap()(ui));
    })
}

#[test]
#[should_panic(expected = "icon buttons require a label")]
fn icon_action_cannot_have_an_empty_accessible_name() {
    draw(Tokens::brasa_dark(), |ui| {
        icon_button(ui, Icon::Add, " ");
    });
}

#[test]
fn icon_action_exposes_its_required_name_and_disabled_state() {
    let output = draw(Tokens::brasa_dark(), |ui| {
        ui.add_enabled_ui(false, |ui| {
            let response = icon_button(ui, Icon::Add, "Add entry");
            assert!(!response.clicked());
        });
    });
    let tree = output.platform_output.accesskit_update.unwrap();
    assert!(tree
        .nodes
        .iter()
        .any(|(_, n)| n.label() == Some("Add entry") && n.is_disabled()));
}

#[test]
fn new_pieces_share_theme_and_semantic_text_roles() {
    for mut tokens in [Tokens::brasa_dark(), Tokens::brasa_light()] {
        tokens.reduce_motion = true;
        let output = draw(tokens.clone(), |ui| {
            toolbar(ui, |ui| {
                button_with_icon(ui, Icon::Add, "Add", ButtonKind::Secondary);
                toolbar_separator(ui);
                toolbar_more(ui, "More actions", &[]);
            });
            section_heading(ui, "Section");
            form_row(
                ui,
                FormRow {
                    label: "Name",
                    help: Some("Help"),
                    error: Some("Error"),
                },
                |ui| {
                    text(ui, "Value");
                },
            );
            list_row(
                ui,
                ListRow {
                    icon: Some(Icon::File),
                    title: "Entry",
                    detail: Some("Detail"),
                    tag: Some("Ready"),
                    selected: true,
                },
                |ui| {
                    icon_button(ui, Icon::Close, "Remove");
                },
            );
            tag(ui, "Ready");
            spinner(ui);
            skeleton(ui, egui::vec2(120.0, 16.0));
            number_card(ui, "42", "Count", Some("+2"));
            let mut ratio = 0.5;
            splitter(ui, "Resize panels", &mut ratio, 500.0);
            assert_eq!(ratio, 0.5);
        });
        for (label, role) in [("Section", TextRole::Section), ("42", TextRole::Display)] {
            assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label && t.galley.job.sections[0].format.font_id == role.font())));
        }
        assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.fill == tokens.color.accent_soft.to_color32())));
    }
}

#[test]
fn list_actions_stay_visible_when_the_pointer_is_over_the_action() {
    let ctx = egui::Context::default();
    apply(&ctx, &Tokens::brasa_dark());
    let mut action_rect = egui::Rect::NOTHING;
    let mut output = None;
    for frame in 0..3 {
        let mut input = egui::RawInput::default();
        if frame > 0 {
            input
                .events
                .push(egui::Event::PointerMoved(action_rect.center()));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                list_row(
                    ui,
                    ListRow {
                        icon: None,
                        title: "Entry",
                        detail: None,
                        tag: None,
                        selected: false,
                    },
                    |ui| {
                        action_rect = icon_button(ui, Icon::Close, "Remove").rect;
                    },
                );
            });
        }));
    }
    assert!(output
        .unwrap()
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Circle(c) if c.fill.a() > 0)));
}

#[test]
fn splitter_clamps_even_without_interaction_and_disabled_rows_do_not_click() {
    draw(Tokens::brasa_dark(), |ui| {
        let mut ratio = 2.0;
        splitter(ui, "Resize", &mut ratio, 500.0);
        assert_eq!(ratio, 0.9);
        ui.add_enabled_ui(false, |ui| {
            let row = list_row(
                ui,
                ListRow {
                    icon: None,
                    title: "Entry",
                    detail: None,
                    tag: None,
                    selected: false,
                },
                |_| {},
            );
            assert!(!row.clicked());
        });
    });
}

fn labels(output: &egui::FullOutput) -> Vec<String> {
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    tree.nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect()
}

#[test]
fn named_inputs_expose_a_non_empty_accessible_name() {
    for name in ["Rótulo do campo", "Chave secreta", "Área grande", "Idioma", "Ajuste"] {
        let output = draw(Tokens::brasa_dark(), |ui| {
            let mut value = String::new();
            let mut index = 0usize;
            let mut on = false;
            text_field_labeled(ui, &mut value, name, "dica");
            secret_field(ui, &mut value, name, "dica");
            text_area(ui, &mut value, name, "dica", 3);
            select_labeled(ui, &mut index, name, &["A", "B"]);
            toggle_labeled(ui, &mut on, name);
        });
        assert!(
            labels(&output).iter().filter(|label| *label == name).count() >= 2,
            "expected the control and its tooltip to carry {name:?}"
        );
    }
}

#[test]
fn legacy_text_field_uses_the_hint_as_accessible_name() {
    let output = draw(Tokens::brasa_dark(), |ui| {
        let mut value = String::new();
        text_field(ui, &mut value, "Buscar arquivos");
    });
    assert!(labels(&output)
        .iter()
        .any(|label| label == "Buscar arquivos"));
}

#[test]
fn tab_close_button_has_a_tooltip() {
    let ctx = egui::Context::default();
    let _ = install_fonts(&ctx);
    apply(&ctx, &Tokens::brasa_dark());
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    ctx.enable_accesskit();
    let mut close_center = None;
    let mut output = None;
    for _ in 0..3 {
        let mut input = egui::RawInput::default();
        if let Some(point) = close_center {
            input.events.push(egui::Event::PointerMoved(point));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                tab_strip(
                    ui,
                    "tabs",
                    &[TabItem {
                        icon: Icon::Terminal,
                        label: "Terminal",
                        close_label: Some("Fechar aba"),
                        tooltip: None,
                    }],
                    0,
                    false,
                    |count| format!("+{count}"),
                    "Adicionar",
                    &[],
                    false,
                );
            });
        }));
        if close_center.is_none() {
            let tree = output
                .as_ref()
                .unwrap()
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap();
            let node = tree
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.label() == Some("Fechar aba"))
                .expect("close button has an accessible name");
            let bounds = node.bounds().unwrap();
            close_center = Some(egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            ));
        }
    }
    assert!(output
        .unwrap()
        .shapes
        .iter()
        .any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Fechar aba")));
}
