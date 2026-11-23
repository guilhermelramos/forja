//! Every registered rail view must render the ui-kit page frame, not just a similar heading.
#[test]
fn every_registered_rail_view_uses_the_page_component() {
    let dir = std::env::temp_dir().join(format!("forja-pages-contract-{}", std::process::id()));
    let mut runtime = crate::app::build_runtime(
        Some("pt-BR".into()),
        Some(dir.clone()),
        Some(dir.join("user")),
        true,
    )
    .unwrap();
    let mut views: Vec<_> = runtime
        .registry()
        .views_in(forja_kernel::Slot::Rail)
        .into_iter()
        .map(|view| {
            (
                view.id.clone(),
                view.module.clone(),
                runtime.i18n().t(&view.title_key),
                forja_kernel::ui_contract::downcast(view.instantiate()).unwrap(),
            )
        })
        .collect();
    assert!(views.len() >= 9);
    for (id, module, title, view) in &mut views {
        for size in [
            forja_ui_kit::egui::vec2(1280.0, 720.0),
            forja_ui_kit::egui::vec2(1920.0, 1080.0),
        ] {
            let ctx = forja_ui_kit::egui::Context::default();
            let _ = forja_ui_kit::install_fonts(&ctx);
            forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
            let _ = ctx.run(
                forja_ui_kit::egui::RawInput {
                    screen_rect: Some(forja_ui_kit::egui::Rect::from_min_size(
                        Default::default(),
                        size,
                    )),
                    ..Default::default()
                },
                |ctx| {
                    forja_ui_kit::egui::CentralPanel::default()
                        .frame(forja_ui_kit::egui::Frame::NONE)
                        .show(ctx, |ui| {
                            view.ui(ui, &mut runtime.ctx(module));
                        });
                },
            );
            let marker = forja_ui_kit::page_marker(&ctx, id)
                .unwrap_or_else(|| panic!("{id} bypasses ui_kit::page"));
            assert_eq!(&marker.title, title, "wrong page heading for {id}");
            assert!(marker.header.height() >= 56.0, "{id}");
            assert!(marker.filters.height() >= 40.0, "{id}");
            assert!(marker.body.bottom() <= size.y, "{id}");
            assert_eq!(
                marker.refresh.unwrap().right(),
                marker.bounds.right(),
                "refresh moved in {id}"
            );
        }
    }
    drop(views);
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}
