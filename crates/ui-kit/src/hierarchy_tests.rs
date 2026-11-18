use crate::{egui, Tokens};
fn draw(render: impl FnOnce(&mut egui::Ui)) -> egui::FullOutput {
    let ctx = egui::Context::default();
    let _ = crate::install_fonts(&ctx);
    crate::apply(&ctx, &Tokens::brasa_dark());
    ctx.enable_accesskit();
    let mut render = Some(render);
    ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render.take().unwrap()(ui);
        });
    })
}
#[test]
fn selected_filters_use_soft_fill_not_primary_accent() {
    let output = draw(|ui| {
        crate::segmented(ui, &mut 0, &["Open", "Closed"]);
    });
    let tokens = Tokens::brasa_dark();
    assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(r) if r.fill==tokens.color.accent_soft.to_color32())));
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(r) if r.fill==tokens.color.accent.to_color32())));
}
#[test]
fn ordinary_destructive_buttons_do_not_use_filled_red() {
    let output = draw(|ui| {
        crate::button(ui, crate::ButtonKind::Danger, "Revoke");
    });
    assert!(!output.shapes.iter().any(|shape| matches!(&shape.shape,egui::Shape::Rect(r) if r.fill==Tokens::brasa_dark().color.danger.to_color32())));
}
#[test]
fn confirmation_is_the_only_filled_red_kind() {
    let output = draw(|ui| {
        crate::button(ui, crate::ButtonKind::Danger, "Delete");
        crate::button(ui, crate::ButtonKind::DangerConfirm, "Confirm delete");
    });
    let count=output.shapes.iter().filter(|shape|matches!(&shape.shape,egui::Shape::Rect(r) if r.fill==Tokens::brasa_dark().color.danger.to_color32())).count();
    assert_eq!(count, 1);
}

#[test]
fn only_one_visible_primary_control_keeps_the_accent_fill() {
    let output = draw(|ui| {
        crate::button(ui, crate::ButtonKind::Primary, "Create");
        crate::button_with_icon(ui, crate::Icon::Send, "Send", crate::ButtonKind::Primary);
        crate::action_tile(ui, crate::Icon::Folder, "Open", 220.0, true);
    });
    let count=output.shapes.iter().filter(|shape| matches!(&shape.shape,egui::Shape::Rect(r) if r.fill==Tokens::brasa_dark().color.accent.to_color32())).count();
    assert_eq!(count, 1);
    let tree = output.platform_output.accesskit_update.unwrap();
    assert_eq!(
        tree.nodes
            .iter()
            .filter(|(_, node)| node.author_id() == Some("forja.button.primary"))
            .count(),
        1
    );
}
#[test]
fn the_front_modal_owns_the_primary_action_after_it_opens() {
    let ctx = egui::Context::default();
    let _ = crate::install_fonts(&ctx);
    crate::apply(&ctx, &Tokens::brasa_dark());
    ctx.enable_accesskit();
    let mut last = None;
    for _ in 0..2 {
        last = Some(ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::button(ui, crate::ButtonKind::Primary, "Create");
            });
            egui::Window::new("Dialog").show(ctx, |ui| {
                crate::register_keyboard_layer(ui, egui::Id::new("test-dialog"));
                crate::button(ui, crate::ButtonKind::Primary, "Apply");
            });
        }));
    }
    let tree = last.unwrap().platform_output.accesskit_update.unwrap();
    assert!(tree
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some("Apply")
            && node.author_id() == Some("forja.button.primary")));
}

#[test]
fn card_titles_use_the_section_role_not_the_page_role() {
    let output = draw(|ui| {
        crate::card(ui, Some("Section"), |_| {});
    });
    let font = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Section" => {
                Some(&text.galley.job.sections[0].format.font_id)
            }
            _ => None,
        })
        .expect("visible card heading");
    assert_eq!(*font, crate::TextRole::Section.font());
}

#[test]
fn density_changes_spacing_not_the_text_scale() {
    for density in [crate::Density::Comfortable, crate::Density::Compact] {
        let mut tokens = Tokens::brasa_dark();
        tokens.density = density;
        let m = tokens.metrics();
        assert_eq!((m.caption, m.body, m.title), (12.0, 14.0, 20.0));
    }
}
