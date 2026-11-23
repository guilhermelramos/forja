use super::*;

fn render(width: f32, wide: bool) -> (egui::FullOutput, PageMarker) {
    let ctx = egui::Context::default();
    let _ = crate::install_fonts(&ctx);
    crate::apply(&ctx, &crate::Tokens::brasa_dark());
    ctx.enable_accesskit();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Default::default(),
                egui::vec2(width, 800.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let mut spec = PageSpec::new("test.page", "Page title")
                        .primary("Create")
                        .refresh("Refresh")
                        .footer();
                    if wide {
                        spec = spec.wide();
                    }
                    let mut page = page(ui, spec);
                    page.filters(|ui| {
                        ui.label("Search, segments, selectors");
                    });
                    page.body(|ui| {
                        ui.label("Body");
                    });
                    page.footer(Some("3 items"), |ui| {
                        crate::button(ui, crate::ButtonKind::Ghost, "Next");
                    });
                });
        },
    );
    (
        output,
        page_marker(&ctx, "test.page").expect("page contract marker"),
    )
}

#[test]
fn page_zones_leave_the_footer_visible_and_limit_reading_width() {
    for width in [800.0, 1400.0] {
        let (_, marker) = render(width, false);
        assert!(marker.body.width() <= 960.0);
        assert!(marker.header.height() >= 56.0);
        assert!(marker.filters.height() >= 40.0);
        assert!(marker.header.bottom() <= marker.filters.top());
        assert!(marker.filters.bottom() <= marker.body.top());
        let footer = marker.footer.unwrap();
        assert!(marker.body.bottom() <= footer.top());
        assert!(footer.bottom() <= 800.0);
        assert!(footer.height() >= 32.0);
    }
    assert!(render(1400.0, true).1.body.width() > 960.0);
    assert_eq!(
        render(1400.0, false).1.refresh.unwrap().right(),
        render(1400.0, true).1.refresh.unwrap().right(),
        "refresh must stay at the same slot edge for reading and wide pages"
    );
}

#[test]
fn page_exposes_its_title_and_named_header_actions() {
    let (output, marker) = render(1000.0, false);
    assert_eq!(marker.title, "Page title");
    let update = output.platform_output.accesskit_update.unwrap();
    for label in ["Page title", "Create", "Refresh", "3 items", "Next"] {
        assert!(
            update
                .nodes
                .iter()
                .any(|(_, node)| node.label().or(node.value()) == Some(label)),
            "missing {label}"
        );
    }
    assert_eq!(marker.refresh.unwrap().right(), marker.bounds.right());
    let next = update
        .nodes
        .iter()
        .find_map(|(_, node)| (node.label() == Some("Next")).then(|| node.bounds().unwrap()))
        .unwrap();
    let footer = marker.footer.unwrap();
    assert!(
        next.y0 >= f64::from(footer.top()) && next.y1 <= f64::from(footer.bottom()),
        "pagination must share the footer row with the count: {next:?}"
    );
}

#[test]
fn wrapped_subtitles_expand_the_header_before_the_filter_zone() {
    let ctx = egui::Context::default();
    let _ = crate::install_fonts(&ctx);
    let subtitle =
        "A long explanatory subtitle with enough words to wrap across several lines. ".repeat(5);
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Default::default(),
                egui::vec2(480.0, 800.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let mut frame = page(
                        ui,
                        PageSpec::new("wrapped.page", "Page")
                            .subtitle(&subtitle)
                            .refresh("Refresh"),
                    );
                    frame.filters(|ui| {
                        ui.label("Filters");
                    });
                    frame.body(|_| {});
                });
        },
    );
    let marker = page_marker(&ctx, "wrapped.page").unwrap();
    assert!(
        marker.header.height() > 56.0,
        "subtitle must not overlap the filters"
    );
    assert!(marker.filters.top() >= marker.header.bottom() + 8.0);
}

#[test]
fn page_body_exposes_its_viewport_to_accessibility() {
    let (output, marker) = render(1000.0, false);
    let update = output.platform_output.accesskit_update.unwrap();
    let nodes: std::collections::HashMap<_, _> =
        update.nodes.iter().map(|(id, node)| (*id, node)).collect();
    assert!(
        update.nodes.iter().any(|(_, node)| {
            if node.role() != egui::accesskit::Role::Pane
                || !node
                    .bounds()
                    .is_some_and(|bounds| (bounds.y1 - f64::from(marker.body.bottom())).abs() < 0.1)
            {
                return false;
            }
            let mut pending = node.children().to_vec();
            while let Some(id) = pending.pop() {
                if let Some(child) = nodes.get(&id) {
                    if child.label().or(child.value()) == Some("Body") {
                        return true;
                    }
                    pending.extend_from_slice(child.children());
                }
            }
            false
        }),
        "the body Pane must actually parent the content, not be an unrelated marker"
    );
}

#[test]
fn shared_empty_loading_and_error_states_expose_the_supplied_copy() {
    for (state, message) in [
        (PageState::Empty, "No items yet"),
        (PageState::Loading, "Loading"),
        (PageState::Error, "Could not load"),
    ] {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                page_state(ui, state, message, Some("Detail"));
            });
        });
        let update = output.platform_output.accesskit_update.unwrap();
        assert!(
            update
                .nodes
                .iter()
                .any(|(_, node)| node.label().or(node.value()) == Some(message)),
            "state {state:?}: {:?}",
            update
                .nodes
                .iter()
                .map(|(_, node)| node.label())
                .collect::<Vec<_>>()
        );
        assert!(update
            .nodes
            .iter()
            .any(|(_, node)| node.label().or(node.value()) == Some("Detail")));
    }
}
