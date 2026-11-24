//! Contrato de acessibilidade dos controles nomeados do lote 2 (Refs #207).

use forja_kernel::{BundleSource, I18n, Locale};
use forja_ui_kit::egui;

fn bundle(locale: Locale) -> I18n {
    I18n::build(
        &[BundleSource {
            module: "search".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        }],
        locale,
    )
    .0
}

#[test]
fn named_controls_expose_non_empty_names_and_tooltips() {
    for locale in [Locale::PtBr, Locale::En] {
        let i18n = bundle(locale);
        let labels: Vec<String> = [
            "search.query.hint",
            "search.include.hint",
            "search.exclude.hint",
            "search.replace.hint",
        ]
        .iter()
        .map(|key| i18n.t(key))
        .collect();
        let hit = i18n.t_args(
            "search.result.open-line",
            &[("path", "src/main.rs"), ("line", "42")],
        );
        for label in labels.iter().chain(std::iter::once(&hit)) {
            assert!(!label.trim().is_empty(), "empty label in {locale:?}");
        }
        let ctx = egui::Context::default();
        forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        ctx.enable_accesskit();
        let mut hit_center = None;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut value = String::new();
                for label in &labels {
                    forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                }
                let (_, line) =
                    ui.allocate_exact_size(egui::vec2(40.0, 16.0), egui::Sense::click());
                line.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hit.clone())
                });
                let line = forja_ui_kit::tooltip(line, hit.clone());
                hit_center = Some(line.rect.center());
            });
        });
        let names: Vec<String> = output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("accesskit habilitado")
            .nodes
            .iter()
            .filter_map(|(_, node)| node.label().map(str::to_owned))
            .collect();
        for expected in labels.iter().chain(std::iter::once(&hit)) {
            assert!(names.contains(expected), "missing name {expected:?} in {locale:?}");
        }
        let mut hovered = None;
        for _ in 0..3 {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::PointerMoved(hit_center.unwrap()));
            hovered = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut value = String::new();
                    for label in &labels {
                        forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                    }
                    let (_, line) =
                        ui.allocate_exact_size(egui::vec2(40.0, 16.0), egui::Sense::click());
                    line.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hit.clone())
                    });
                    forja_ui_kit::tooltip(line, hit.clone());
                });
            }));
        }
        assert!(
            hovered.unwrap().shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.text() == hit
            )),
            "missing tooltip {hit:?} in {locale:?}"
        );
    }
}
