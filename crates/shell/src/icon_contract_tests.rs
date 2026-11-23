//! Contract for the original icons assigned to built-in navigation destinations.
#[test]
fn every_builtin_destination_has_a_distinct_original_icon() {
    let dir = std::env::temp_dir().join(format!("forja-icons-contract-{}", std::process::id()));
    let runtime = crate::app::build_runtime(
        Some("pt-BR".into()),
        Some(dir.clone()),
        Some(dir.join("user")),
        true,
    )
    .unwrap();
    let mut owners = std::collections::HashMap::from([
        (forja_ui_kit::Icon::Workbench, "shell.bench".to_owned()),
        (forja_ui_kit::Icon::Settings, "shell.settings".to_owned()),
    ]);
    for slot in [
        forja_kernel::Slot::Rail,
        forja_kernel::Slot::BenchTab,
        forja_kernel::Slot::Sidebar,
        forja_kernel::Slot::RightPanel,
    ] {
        for view in runtime.registry().views_in(slot) {
            if view.id == "project.home" {
                continue;
            }
            let icon = view
                .icon_glyph
                .as_deref()
                .and_then(forja_ui_kit::Icon::from_id)
                .unwrap_or_else(|| crate::app::tab_bar::tab_icon(&view.id));
            assert_ne!(
                icon,
                forja_ui_kit::Icon::Info,
                "unmapped destination {}",
                view.id
            );
            let previous = owners.insert(icon, view.id.clone());
            assert!(
                previous.is_none(),
                "{} and {:?} share {:?}",
                view.id,
                previous,
                icon
            );
        }
    }
    drop(runtime);
    let _ = std::fs::remove_dir_all(dir);
}
