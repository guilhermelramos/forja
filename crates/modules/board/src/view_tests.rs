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

#[test]
fn refresh_filters_cards_by_active_project() {
    use forja_kernel::{Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
    use serde_json::json;
    use std::sync::{Arc, Mutex};

    struct StubProjects {
        active_id: Arc<Mutex<String>>,
    }

    impl Module for StubProjects {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("project", "stub.project.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("stub-project-name = Projects\n", "stub-project-name = Projetos\n");
            let active = Arc::clone(&self.active_id);
            r.command("project.current", "stub.project.name", move |_cx, _| {
                let id = active.lock().unwrap().clone();
                Ok(json!({ "id": id, "name": id, "path": "C:/p" }))
            });
            r.command("project.list", "stub.project.name", |_cx, _| {
                Ok(json!({
                    "projects": [
                        { "id": "p1", "name": "Alfa", "path": "C:/alfa" },
                        { "id": "p2", "name": "Beta", "path": "C:/beta" }
                    ]
                }))
            });
        }
    }

    struct StubBenches;

    impl Module for StubBenches {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "stub.bench.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("stub-bench-name = Benches\n", "stub-bench-name = Bancadas\n");
            r.command("bench.list_cards", "stub.bench.name", |_cx, args| {
                let filter = args.str("project_id").map(str::to_owned);
                let all = vec![
                    card("b1", "draft", "p1", false, false, 1),
                    card("b2", "forging", "p2", false, false, 2),
                ];
                let filtered: Vec<_> = all
                    .into_iter()
                    .filter(|c| filter.as_deref().is_none_or(|id| c.bench.project_id == id))
                    .collect();
                Ok(json!({ "benches": filtered }))
            });
            r.command("bench.stage.list", "stub.bench.name", |_cx, _| {
                Ok(json!({ "stages": [] }))
            });
        }
    }

    let root = std::env::temp_dir().join(format!("forja-board-filter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let active_id = Arc::new(Mutex::new("p1".to_owned()));
    let board_state = crate::state::Shared::new();
    let mut runtime = Runtime::new(
        vec![
            Box::new(crate::BoardModule {
                state: board_state.clone(),
            }),
            Box::new(StubProjects {
                active_id: Arc::clone(&active_id),
            }),
            Box::new(StubBenches),
        ],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("board + stub register cleanly");

    {
        let mut cx = runtime.ctx("board");
        crate::view::refresh(&mut cx, &board_state);
    }
    let cards = board_state.cards();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].bench.project_id, "p1");
    assert_eq!(board_state.active_project().as_deref(), Some("p1"));

    *active_id.lock().unwrap() = "p2".to_owned();
    {
        let mut cx = runtime.ctx("board");
        crate::view::refresh(&mut cx, &board_state);
    }
    let cards = board_state.cards();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].bench.project_id, "p2");
    assert_eq!(board_state.active_project().as_deref(), Some("p2"));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn moving_to_forging_offers_default_helper_or_missing_guide_and_respects_setting() {
    use forja_kernel::{
        Module, ModuleManifest, Registrar, Runtime, RuntimeOptions, SettingValue,
    };
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };

    struct StubBench;
    impl Module for StubBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "stub.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("stub-name = Stub", "stub-name = Stub");
            r.command("bench.stage.set", "stub.name", |_cx, args| {
                Ok(json!({ "stage": args.str("stage").unwrap_or("forging") }))
            });
        }
    }

    struct StubHelper {
        helpers_installed: Arc<AtomicBool>,
        launched: Arc<Mutex<Vec<(String, String)>>>,
    }
    impl Module for StubHelper {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("helper", "stub.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("stub-name = Stub", "stub-name = Stub");
            let installed = Arc::clone(&self.helpers_installed);
            r.command("helper.list", "stub.name", move |_cx, _args| {
                if installed.load(Ordering::Relaxed) {
                    Ok(json!({
                        "detecting": false,
                        "profiles": [
                            { "id": "codex", "name": "Codex", "installed": true, "default": false, "install_hint": "npm i -g @openai/codex" },
                            { "id": "claude-code", "name": "Claude Code", "installed": true, "default": true, "install_hint": "npm i -g @anthropic-ai/claude-code" }
                        ]
                    }))
                } else {
                    Ok(json!({
                        "detecting": false,
                        "profiles": [
                            { "id": "claude-code", "name": "Claude Code", "installed": false, "default": true, "install_hint": "npm i -g @anthropic-ai/claude-code" }
                        ]
                    }))
                }
            });
            let launched = Arc::clone(&self.launched);
            r.command("helper.launch", "stub.name", move |_cx, args| {
                let profile_id = args.str("profile_id").unwrap_or("").to_owned();
                let bench_path = args.str("bench_path").unwrap_or("").to_owned();
                launched.lock().unwrap().push((profile_id, bench_path));
                Ok(json!({ "terminal_id": "term-1" }))
            });
        }
    }

    struct StubShell {
        opened_view: Arc<Mutex<Option<String>>>,
    }
    impl Module for StubShell {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("shell", "stub.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("stub-name = Stub", "stub-name = Stub");
            let opened = Arc::clone(&self.opened_view);
            r.command("shell.rail.open", "stub.name", move |_cx, args| {
                *opened.lock().unwrap() = args.str("view_id").map(str::to_owned);
                Ok(json!({ "opened": true }))
            });
        }
    }

    let root = std::env::temp_dir().join(format!("forja-board-offer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let installed = Arc::new(AtomicBool::new(true));
    let launched = Arc::new(Mutex::new(Vec::new()));
    let opened_view = Arc::new(Mutex::new(None));
    let board_state = crate::state::Shared::new();
    let mut runtime = Runtime::new(
        vec![
            Box::new(crate::BoardModule {
                state: board_state.clone(),
            }),
            Box::new(StubBench),
            Box::new(StubHelper {
                helpers_installed: Arc::clone(&installed),
                launched: Arc::clone(&launched),
            }),
            Box::new(StubShell {
                opened_view: Arc::clone(&opened_view),
            }),
        ],
        RuntimeOptions {
            data_dir: Some(root.join("data")),
            user_dir: Some(root.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("board + stub register");

    let draft = card("b1", "draft", "p1", false, false, 1);
    let stages = vec![
        StageColumn {
            id: "draft".into(),
            label: "Rascunho".into(),
            order: 0,
            parent: "draft".into(),
            dot: "muted".into(),
        },
        StageColumn {
            id: "forging".into(),
            label: "Forjando".into(),
            order: 1,
            parent: "forging".into(),
            dot: "accent".into(),
        },
    ];

    {
        let mut cx = runtime.ctx("board");
        move_card(&mut cx, &board_state, &draft, "forging", &stages);
    }
    assert!(launched.lock().unwrap().is_empty(), "must offer, not force-start");
    let offer = board_state.helper_offer().expect("offer should open");
    assert_eq!(
        offer.default_profile.as_ref().map(|p| p.id.as_str()),
        Some("claude-code")
    );
    assert!(offer.install_hint.is_none());

    {
        let mut cx = runtime.ctx("board");
        launch_offered_helper(&mut cx, &board_state, &offer);
    }
    assert_eq!(
        launched.lock().unwrap().as_slice(),
        &[("claude-code".to_owned(), "C:/bancadas/b1".to_owned())]
    );
    assert!(board_state.helper_offer().is_none());

    installed.store(false, Ordering::Relaxed);
    {
        let mut cx = runtime.ctx("board");
        move_card(&mut cx, &board_state, &draft, "forging", &stages);
    }
    let missing = board_state.helper_offer().expect("missing-helper guide should open");
    assert!(missing.default_profile.is_none());
    assert_eq!(
        missing.install_hint.as_deref(),
        Some("npm i -g @anthropic-ai/claude-code")
    );
    {
        let mut cx = runtime.ctx("board");
        open_helpers_tab(&mut cx, &board_state);
    }
    assert_eq!(opened_view.lock().unwrap().as_deref(), Some("helper.tab"));
    assert!(board_state.helper_offer().is_none());

    runtime
        .set_setting("board.stage.offer_helper", SettingValue::Bool(false))
        .unwrap();
    installed.store(true, Ordering::Relaxed);
    {
        let mut cx = runtime.ctx("board");
        move_card(&mut cx, &board_state, &draft, "forging", &stages);
    }
    assert!(
        board_state.helper_offer().is_none(),
        "disabled setting must suppress offer"
    );

    let _ = std::fs::remove_dir_all(&root);
}
