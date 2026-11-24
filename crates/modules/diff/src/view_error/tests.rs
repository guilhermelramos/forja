//! Tests for the humanized diff error banner: the wrapper and the drawn screen.

use super::humanize_error;
use crate::state::Shared;
use crate::view::DiffView;
use forja_kernel::ui_contract::View;
use forja_kernel::{Runtime, RuntimeOptions};
use std::path::Path;

const RAW_GIT_ERROR: &str = "git rev-parse --verify --end-of-options C:\\ghost\\999^{commit} failed: fatal: Needed a single revision";
const HUMAN_PHRASE: &str = "A branch base ou referência informada não existe neste repositório.";

fn test_runtime(root: &Path) -> Runtime {
    Runtime::new(
        vec![],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("runtime")
}

#[test]
fn git_rev_parse_error_becomes_human_phrase() {
    let root = std::env::temp_dir().join(format!("forja-diff-human-{}", std::process::id()));
    let mut runtime = test_runtime(&root);
    let cx = runtime.ctx("diff");
    let human = humanize_error(&cx, RAW_GIT_ERROR);
    assert_eq!(human.summary, HUMAN_PHRASE);
    assert!(!human.summary.contains("rev-parse"));
    assert!(!human.summary.to_ascii_lowercase().contains("failed"));
    assert!(human.detail.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

/// Collects every text galley egui emitted, so the assertion sees the same
/// body a person sees and not just the string passed to the banner helper.
fn collect_text(shape: &forja_kernel::ui_contract::egui::Shape, out: &mut String) {
    use forja_kernel::ui_contract::egui::Shape;
    match shape {
        Shape::Text(text) => out.push_str(text.galley.text()),
        Shape::Vec(shapes) => {
            for shape in shapes {
                collect_text(shape, out);
            }
        }
        _ => {}
    }
}

fn draw_diff_with_error(raw: &str) -> String {
    let root = std::env::temp_dir().join(format!("forja-diff-draw-{}", std::process::id()));
    let mut runtime = test_runtime(&root);
    let shared = Shared::new();
    shared.set_status(Some(raw.to_owned()));
    let mut view = DiffView::new(shared);
    let ctx = forja_kernel::ui_contract::egui::Context::default();
    forja_ui_kit::install_fonts(&ctx).expect("fontes do Windows");
    let input = forja_kernel::ui_contract::egui::RawInput {
        screen_rect: Some(forja_kernel::ui_contract::egui::Rect::from_min_size(
            forja_kernel::ui_contract::egui::Pos2::ZERO,
            forja_kernel::ui_contract::egui::vec2(1280.0, 720.0),
        )),
        ..Default::default()
    };
    let output = ctx.run(input, |ctx| {
        forja_kernel::ui_contract::egui::CentralPanel::default().show(ctx, |ui| {
            let mut cx = runtime.ctx("diff");
            view.ui(ui, &mut cx);
        });
    });
    let mut text = String::new();
    for clipped in &output.shapes {
        collect_text(&clipped.shape, &mut text);
    }
    let _ = std::fs::remove_dir_all(&root);
    text
}

#[test]
fn diff_screen_shows_human_phrase_and_not_the_raw_git_error() {
    let body = draw_diff_with_error(RAW_GIT_ERROR);
    assert!(body.contains(HUMAN_PHRASE), "frase humana ausente: {body}");
    assert!(body.contains("Detalhes técnicos"), "detalhes ausentes: {body}");
    assert!(!body.contains("rev-parse"), "texto cru na tela: {body}");
    assert!(!body.to_ascii_lowercase().contains("fatal:"), "texto cru na tela: {body}");
    assert!(!body.contains("C:\\ghost"), "caminho cru na tela: {body}");
}
