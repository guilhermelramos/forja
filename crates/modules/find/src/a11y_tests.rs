//! Contrato de acessibilidade dos controles nomeados do lote 2 (Refs #207).

use forja_kernel::{BundleSource, I18n, Locale};
use forja_ui_kit::egui;

fn bundle(locale: Locale) -> I18n {
    I18n::build(
        &[BundleSource {
            module: "find".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        }],
        locale,
    )
    .0
}

#[test]
fn named_fields_expose_non_empty_names_and_tooltips() {
    for locale in [Locale::PtBr, Locale::En] {
        let i18n = bundle(locale);
        let labels: Vec<String> = ["find.anvil.hint", "find.picker.hint"]
            .iter()
            .map(|key| i18n.t(key))
            .collect();
        for label in &labels {
            assert!(!label.trim().is_empty(), "empty label in {locale:?}");
        }
        let ctx = egui::Context::default();
        forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        ctx.enable_accesskit();
        let mut first_center = None;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut value = String::new();
                for (index, label) in labels.iter().enumerate() {
                    let response = forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                    if index == 0 {
                        first_center = Some(response.rect.center());
                    }
                }
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
        for expected in &labels {
            assert!(names.contains(expected), "missing name {expected:?} in {locale:?}");
        }
        let mut hovered = None;
        for _ in 0..3 {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::PointerMoved(first_center.unwrap()));
            hovered = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut value = String::new();
                    for label in &labels {
                        forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                    }
                });
            }));
        }
        assert!(
            hovered.unwrap().shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.text() == labels[0]
            )),
            "missing tooltip {:?} in {locale:?}",
            labels[0]
        );
    }
}
