//! Contrato de acessibilidade desenhando a view real do terminal (Refs #207).

use crate::core::Core;
use crate::panes::tree::PaneNode;
use crate::terminal::Terminal;
use crate::view::TerminalView;
use forja_kernel::ui_contract::View;
use forja_kernel::{Ctx, Runtime, RuntimeOptions};
use forja_ui_kit::egui;
use std::sync::Arc;

fn view_with_search_open() -> TerminalView {
    let core = Arc::new(Core::new());
    {
        let mut hub = core.hub.lock().expect("hub");
        let mut terminal = Terminal::test_dummy(0);
        terminal.open_search(String::new());
        hub.terminals.insert(0, terminal);
        hub.root = Some(PaneNode::leaf(0, 0));
        hub.focused = 0;
        hub.restore_pending = true;
    }
    TerminalView::new(core)
}

fn render(cx: &mut Ctx<'_>, view: &mut TerminalView, hover: Option<&str>) -> egui::FullOutput {
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
fn production_search_field_renders_without_panic_and_has_a_name() {
    let dir = std::env::temp_dir().join(format!("forja-terminal-a11y-{}", std::process::id()));
    let core = Arc::new(Core::new());
    let mut runtime = Runtime::new(
        vec![Box::new(crate::TerminalModule { core })],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("terminal registers");
    let mut cx = runtime.ctx("terminal");
    let output = render(&mut cx, &mut view_with_search_open(), None);
    let names: Vec<String> = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accesskit habilitado")
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    assert!(
        names.iter().any(|name| name == "Buscar no terminal"),
        "the production search field has a non-empty accessible name"
    );
    let hovered = render(&mut cx, &mut view_with_search_open(), Some("Buscar no terminal"));
    assert!(
        hovered.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Buscar no terminal"
        )),
        "hovering the production field shows its tooltip"
    );
}
