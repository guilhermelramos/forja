//! Persistência da escolha de arranjo no store da bancada.

use crate::app::shell_store::BenchLayout;
use serde_json::json;

#[test]
fn legacy_bench_state_without_a_layout_choice_still_loads() {
    let old = json!({
        "active_tab": "editor.tab",
        "open_tabs": ["editor.tab", "terminal.tab"],
        "sidebar_open": true,
        "right_panel_open": false,
    });
    let layout = BenchLayout::from_json(&old, true);
    assert_eq!(layout.layout, None);
    assert_eq!(layout.to_json()["active_tab"], "editor.tab");
    let with_choice = json!({
        "layout": { "id": "foco", "layer": "user", "hash": "abc" }
    });
    let layout = BenchLayout::from_json(&with_choice, true);
    assert_eq!(layout.layout.as_ref().unwrap().id, "foco");
    assert_eq!(layout.to_json()["layout"]["layer"], "user");
}
