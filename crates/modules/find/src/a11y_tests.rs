//! Contrato de acessibilidade desenhando as views reais do finder (Refs #207).

use crate::state::State;
use crate::view::{AnvilView, FindView};
use forja_kernel::ui_contract::View;
use forja_kernel::{Ctx, Runtime, RuntimeOptions};
use forja_ui_kit::egui;
use std::sync::{Arc, Mutex};

fn runtime() -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-find-a11y-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(crate::FindModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("find registers")
}

/// Desenha as duas views de produção (Bigorna e seletor aberto) com hover opcional.
fn render(cx: &mut Ctx<'_>, hover: Option<&str>) -> egui::FullOutput {
    let shared = Arc::new(Mutex::new(State::default()));
    shared.lock().expect("state").open = true;
    let mut anvil = AnvilView {
        shared: shared.clone(),
        ranked_for: None,
    };
    let mut picker = FindView { shared };
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let _ = forja_ui_kit::install_fonts(&ctx);
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut point = None;
    let mut output = None;
    for _ in 0..3 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1000.0, 700.0),
            )),
            ..Default::default()
        };
        if let Some(point) = point {
            input.events.push(egui::Event::PointerMoved(point));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                anvil.ui(ui, cx);
                picker.ui(ui, cx);
            });
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
fn production_fields_expose_names_and_tooltips() {
    let mut runtime = runtime();
    let mut cx = runtime.ctx("find");
    let labels = [cx.t("find.anvil.hint"), cx.t("find.picker.hint")];
    let output = render(&mut cx, None);
    let names: Vec<String> = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accesskit habilitado")
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    for label in &labels {
        assert!(names.contains(label), "missing name {label:?}");
        let hovered = render(&mut cx, Some(label));
        assert!(
            hovered.shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.text() == label
            )),
            "missing tooltip {label:?}"
        );
    }
}
