//! Contrato de acessibilidade desenhando a view real da busca (Refs #207).

use crate::engine::{ContextLine, FileHits, Hit};
use crate::state::State;
use crate::view::SearchView;
use forja_kernel::ui_contract::View;
use forja_kernel::{Ctx, Runtime, RuntimeOptions};
use forja_ui_kit::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn runtime() -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-search-a11y-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(crate::SearchModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("search registers")
}

fn view() -> SearchView {
    let shared = Arc::new(Mutex::new(State::default()));
    shared.lock().expect("state").files = vec![FileHits {
        rel: "src/main.rs".to_owned(),
        path: PathBuf::from("src/main.rs"),
        hits: vec![Hit {
            line: 42,
            lines: vec![ContextLine {
                number: 42,
                text: "fn main()".to_owned(),
                is_match: true,
            }],
        }],
    }];
    SearchView { shared }
}

fn render(cx: &mut Ctx<'_>, hover: Option<&str>) -> egui::FullOutput {
    let mut view = view();
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
fn production_fields_and_result_line_expose_names_and_tooltips() {
    let mut runtime = runtime();
    let mut cx = runtime.ctx("search");
    let hit = cx.t_args(
        "search.result.open-line",
        &[("path", "src/main.rs"), ("line", "42")],
    );
    let labels = [
        cx.t("search.query.hint"),
        cx.t("search.include.hint"),
        cx.t("search.exclude.hint"),
        cx.t("search.replace.hint"),
        hit,
    ];
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
    }
    for label in &labels {
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
