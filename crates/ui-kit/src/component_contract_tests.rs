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
