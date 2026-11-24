//! Contrato de acessibilidade dos controles nomeados do lote 2 (Refs #207).

use forja_kernel::{BundleSource, I18n, Locale};
use forja_ui_kit::egui;

fn bundle(locale: Locale) -> I18n {
    I18n::build(
        &[BundleSource {
            module: "terminal".to_owned(),
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
            "terminal.view.search-hint",
            "terminal.view.recipe-picker-filter",
        ]
        .iter()
        .map(|key| i18n.t(key))
        .collect();
        let resize = i18n.t("terminal.view.resize-panes");
        let surface = i18n.t_args("terminal.view.surface-label", &[("title", "bash")]);
        for label in labels
            .iter()
            .chain([&resize, &surface])
        {
            assert!(!label.trim().is_empty(), "empty label in {locale:?}");
        }
        let ctx = egui::Context::default();
        forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        ctx.enable_accesskit();
        let mut divider_center = None;
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut value = String::new();
                for label in &labels {
                    forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                }
                let (_, divider) =
                    ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::drag());
                divider.widget_info(|| egui::WidgetInfo::slider(true, 0.5, resize.clone()));
                let divider = forja_ui_kit::tooltip(divider, resize.clone());
                divider_center = Some(divider.rect.center());
                let (_, body) =
                    ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click_and_drag());
                body.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, surface.clone())
                });
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
        for expected in labels.iter().chain([&resize, &surface]) {
            assert!(names.contains(expected), "missing name {expected:?} in {locale:?}");
        }
        let mut hovered = None;
        for _ in 0..3 {
            let mut input = egui::RawInput::default();
            input.events.push(egui::Event::PointerMoved(divider_center.unwrap()));
            hovered = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut value = String::new();
                    for label in &labels {
                        forja_ui_kit::text_field_labeled(ui, &mut value, label, label);
                    }
                    let (_, divider) =
                        ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::drag());
                    divider.widget_info(|| egui::WidgetInfo::slider(true, 0.5, resize.clone()));
                    forja_ui_kit::tooltip(divider, resize.clone());
                });
            }));
        }
        assert!(
            hovered.unwrap().shapes.iter().any(|shape| matches!(
                &shape.shape,
                egui::Shape::Text(text) if text.galley.text() == resize
            )),
            "missing tooltip {resize:?} in {locale:?}"
        );
    }
}
