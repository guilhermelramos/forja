//! Contrato de acessibilidade da seção Contas (Refs #207).

use crate::view::AccountsSettings;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Runtime, RuntimeOptions};

#[test]
fn add_field_has_an_accessible_name_and_a_tooltip() {
    let dir = std::env::temp_dir().join(format!("forja-accounts-a11y-{}", std::process::id()));
    let mut runtime = Runtime::new(
        vec![Box::new(crate::AccountsModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("an empty runtime builds cleanly");
    // Um perfil embutido basta: o campo de nova conta aparece para cada perfil.
    let mut view = AccountsSettings::new(crate::state::Shared::new());

    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let _ = forja_ui_kit::install_fonts(&ctx);
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut cx = runtime.ctx("accounts");
    let mut point = None;
    let mut output = None;
    for _ in 0..3 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 700.0),
            )),
            ..Default::default()
        };
        if let Some(point) = point {
            input.events.push(egui::Event::PointerMoved(point));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                forja_kernel::ui_contract::View::ui(&mut view, ui, &mut cx);
            });
        }));
        if point.is_none() {
            let tree = output.as_ref().unwrap().platform_output.accesskit_update.as_ref().unwrap();
            let node = tree
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.label() == Some("Nome"))
                .expect("the account name field has a non-empty accessible name");
            let bounds = node.bounds().unwrap();
            point = Some(egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            ));
        }
    }
    drop(cx);
    let output = output.unwrap();
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Text(text) if text.galley.text() == "Nome para identificar esta conta do ajudante"
    )), "hovering the field shows its accessible name as a tooltip");
    let _ = std::fs::remove_dir_all(&dir);
}
