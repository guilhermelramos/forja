use crate::state::{Prefs, StageColumn};
use crate::view_shared::*;
use forja_contracts::{BenchCard, BenchRef};
use std::collections::BTreeMap;

fn card(
    id: &str,
    stage: &str,
    project: &str,
    pinned: bool,
    archived: bool,
    activated_at: u64,
) -> BenchCard {
    BenchCard {
        bench: BenchRef {
            project_id: project.to_owned(),
            bench_id: id.to_owned(),
            name: id.to_owned(),
            path: format!("C:/bancadas/{id}"),
            branch: Some(format!("forja/{id}")),
        },
        is_main: false,
        stage: stage.to_owned(),
        archived,
        pinned,
        unread: false,
        sleeping: false,
        base_branch: Some("main".to_owned()),
        created_at: 0,
        activated_at,
        request: None,
    }
}

#[test]
fn filters_hide_archived_and_respect_pinned_order() {
    let cards = vec![
        card("antiga", "ready", "p1", false, true, 9),
        card("recente", "forging", "p1", false, false, 5),
        card("fixada", "draft", "p1", true, false, 1),
    ];
    let names = BTreeMap::new();
    let prefs = Prefs::default();
    let visible = visible_cards(&cards, &prefs, &names);
    assert_eq!(visible.len(), 2);
    assert_eq!(visible[0].bench.name, "fixada");
    let mut with_archived = prefs.clone();
    with_archived.show_archived = true;
    assert_eq!(visible_cards(&cards, &with_archived, &names).len(), 3);
}

#[test]
fn grouping_by_stage_keeps_column_order() {
    let cards = [
        card("a", "draft", "p1", false, false, 1),
        card("b", "forging", "p1", false, false, 2),
    ];
    let stages = vec![
        StageColumn {
            id: "draft".into(),
            label: "Rascunho".into(),
            order: 10,
            parent: "draft".into(),
            dot: "text_muted".into(),
        },
        StageColumn {
            id: "forging".into(),
            label: "Forjando".into(),
            order: 20,
            parent: "forging".into(),
            dot: "accent".into(),
        },
    ];
    let names = BTreeMap::new();
    let refs: Vec<&BenchCard> = cards.iter().collect();
    let prefs = Prefs {
        group: "stage".into(),
        ..Prefs::default()
    };
    let groups = grouped(&refs, &prefs, &stages, &names);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].0, "Rascunho");
    assert_eq!(groups[1].0, "Forjando");
}
