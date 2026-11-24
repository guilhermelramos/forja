//! Contrato de acessibilidade desenhando a view real do navegador (Refs #207).

use crate::state::Shared;
use crate::view::BrowserView;
use forja_kernel::ui_contract::View;
use forja_kernel::{Ctx, Runtime, RuntimeOptions};
use forja_ui_kit::egui;

fn runtime() -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-browser-a11y-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(crate::BrowserModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("browser registers")
}

/// Desenha a toolbar de produção numa janela estreita; `hover` recebe o nome a apontar.
fn render(cx: &mut Ctx<'_>, hover: Option<&str>) -> egui::FullOutput {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let _ = forja_ui_kit::install_fonts(&ctx);
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut view = BrowserView::new(Shared::new(crate::history::History::default()));
    let mut point = None;
    let mut output = None;
    for _ in 0..3 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        if let Some(point) = point {
            input.events.push(egui::Event::PointerMoved(point));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| view.ui(ui, cx));
        }));
        if point.is_none() {
            if let Some(target) = hover {
                let tree = output
                    .as_ref()
                    .unwrap()
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap();
                if let Some(node) = tree
                    .nodes
                    .iter()
                    .map(|(_, node)| node)
                    .find(|node| node.label() == Some(target))
                {
                    let bounds = node.bounds().unwrap();
                    point = Some(egui::pos2(
                        ((bounds.x0 + bounds.x1) * 0.5) as f32,
                        ((bounds.y0 + bounds.y1) * 0.5) as f32,
                    ));
                }
            }
        }
    }
    output.unwrap()
}

#[test]
fn production_address_field_keeps_420px_and_exposes_name_and_tooltip() {
    let mut runtime = runtime();
    let mut cx = runtime.ctx("browser");
    let output = render(&mut cx, None);
    let tree = output.platform_output.accesskit_update.as_ref().unwrap();
    let node = tree
        .nodes
        .iter()
        .map(|(_, node)| node)
        .find(|node| node.label() == Some("Endereço"))
        .expect("address field has a non-empty accessible name");
    let bounds = node.bounds().unwrap();
    let width = bounds.x1 - bounds.x0;
    assert!((380.0..=460.0).contains(&width), "address width {width} in a narrow window");

    let hovered = render(&mut cx, Some("Endereço"));
    assert!(
        hovered.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Endereço"
        )),
        "hovering the production field shows its tooltip"
    );
}
