//! Contrato de acessibilidade desenhando a view real do editor (Refs #207).

use crate::model::{Encoding, Eol};
use crate::state::{RenameState, Tab};
use crate::view::EditorView;
use forja_kernel::ui_contract::View;
use forja_kernel::{Ctx, Runtime, RuntimeOptions};
use forja_ui_kit::egui;

fn runtime() -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-editor-a11y-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(crate::EditorModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("editor registers")
}

fn view_with_fields_open() -> EditorView {
    let state = crate::state::Shared::new();
    state.with(|state| {
        state.tabs = vec![Tab::new(
            "src/main.rs".to_owned(),
            "fn main() {}\n".to_owned(),
            Eol::Lf,
            Encoding::Utf8,
            false,
        )];
        state.active = 0;
        state.find.open = true;
        state.find.replace_open = true;
        state.rename = Some(RenameState {
            word: "main".to_owned(),
            text: "main2".to_owned(),
        });
    });
    let mut view = EditorView::new(state);
    view.goto_open = true;
    view
}

fn render(cx: &mut Ctx<'_>, view: &mut EditorView) -> egui::FullOutput {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let _ = forja_ui_kit::install_fonts(&ctx);
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    let mut output = None;
    for _ in 0..2 {
        output = Some(ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| view.ui(ui, cx));
            },
        ));
    }
    output.unwrap()
}

#[test]
fn production_fields_render_without_panic_and_have_names() {
    let mut runtime = runtime();
    let mut cx = runtime.ctx("editor");
    let mut view = view_with_fields_open();
    let output = render(&mut cx, &mut view);
    let names: Vec<String> = output
        .platform_output
        .accesskit_update
        .as_ref()
        .expect("accesskit habilitado")
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    for expected in ["Linha", "Buscar", "Substituir por", "Novo nome"] {
        assert!(names.iter().any(|name| name == expected), "missing {expected:?}");
    }
}
