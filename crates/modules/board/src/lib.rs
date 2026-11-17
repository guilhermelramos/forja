//! Módulo `board`: Quadro por etapas e a lista de bancadas da lateral.
//!
//! O Quadro é uma página do trilho (`Slot::Rail`).
//! O módulo lê tudo pelos comandos públicos de `bench` (`bench.list_cards`,
//! `bench.stage.list`, `bench.stage.set`), então não depende do crate de
//! bancadas e degrada quando ele não está presente.

mod state;
mod view;
mod view_board;
mod view_shared;
mod view_sidebar;
#[cfg(test)]
mod view_tests;

use forja_contracts::{
    names, BenchActivated, BenchCreated, BenchStored, ProjectActivated, ProjectAdded,
    ProjectRemoved, StageChanged,
};
use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::json;
use state::Shared;

#[forja_kernel::module]
static BOARD: fn() -> Box<dyn Module> = || {
    Box::new(BoardModule {
        state: Shared::new(),
    })
};

pub struct BoardModule {
    state: Shared,
}

impl Module for BoardModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("board", "board.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        register_settings(r);

        // A create, store, stage move or activation changes what the board and
        // the sidebar must show; refresh at the next frame instead of waiting
        // for the periodic timer.
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);
        r.contract_event::<BenchStored>(names::BENCH_STORED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<StageChanged>(names::STAGE_CHANGED);
        r.contract_event::<ProjectActivated>(names::PROJECT_ACTIVATED);
        r.contract_event::<ProjectAdded>(names::PROJECT_ADDED);
        r.contract_event::<ProjectRemoved>(names::PROJECT_REMOVED);
        let created_state = self.state.clone();
        r.subscribe::<BenchCreated, _>(move |_cx, _event| created_state.request_refresh());
        let stored_state = self.state.clone();
        r.subscribe::<BenchStored, _>(move |_cx, _event| stored_state.request_refresh());
        let activated_state = self.state.clone();
        r.subscribe::<BenchActivated, _>(move |_cx, _event| activated_state.request_refresh());
        let stage_state = self.state.clone();
        r.subscribe::<StageChanged, _>(move |_cx, _event| stage_state.request_refresh());
        let project_activated_state = self.state.clone();
        r.subscribe::<ProjectActivated, _>(move |_cx, _event| {
            project_activated_state.request_refresh()
        });
        let project_added_state = self.state.clone();
        r.subscribe::<ProjectAdded, _>(move |_cx, _event| project_added_state.request_refresh());
        let project_removed_state = self.state.clone();
        r.subscribe::<ProjectRemoved, _>(move |_cx, _event| {
            project_removed_state.request_refresh()
        });

        let refresh_state = self.state.clone();
        r.command("board.refresh", "board.command.refresh", move |_, _| {
            refresh_state.request_refresh();
            Ok(json!({ "pending": true }))
        });

        for key in [
            "board.name",
            "board.command.refresh",
            "board.page.title",
            "board.sidebar.title",
            "board.sidebar.empty",
            "board.error.benches",
            "board.show-archived",
            "board.pinned-first",
            "board.filter.hint",
            "board.count",
            "board.group.project",
            "board.group.stage",
            "board.group.none",
            "board.sort.recent",
            "board.sort.name",
            "board.sort.stage",
            "board.action.refresh",
            "board.action.create",
            "board.action.move",
            "board.action.move-to",
            "board.action.move-back",
            "board.action.open",
            "board.action.pin",
            "board.action.read",
            "board.action.sleep",
            "board.action.wake",
            "board.action.store",
            "board.action.approve",
            "board.column.wip",
            "board.drag.backward",
            "board.drag.confirm",
            "board.drag.cancel",
            "board.empty",
            "board.card.helper.needs-you",
            "board.card.helper.working",
            "board.card.helper.idle",
            "board.card.helper.finished",
            "board.card.helper.unknown",
            "board.forging-offer.body",
            "board.forging-offer.start",
            "board.forging-offer.dismiss",
            "board.forging-offer.missing-title",
            "board.forging-offer.missing-body",
            "board.forging-offer.copy",
            "board.forging-offer.copied",
            "board.forging-offer.open-helpers",
            "board.forging-offer.close",
            "board.forging-offer.error",
        ] {
            r.uses_i18n(key);
        }

        let rail_state = self.state.clone();
        r.view(
            Slot::Rail,
            "board.page",
            "board.page.title",
            20,
            move || Box::new(view::BoardRail::new(rail_state.clone())),
        );
        r.view_glyph("board.page", forja_ui_kit::BOARD.id());
        let sidebar_state = self.state.clone();
        r.view(
            Slot::Sidebar,
            "board.sidebar",
            "board.sidebar.title",
            20,
            move || Box::new(view::BoardSidebar::new(sidebar_state.clone())),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    let mut add = |key: &str, kind: SettingKind, default: SettingValue, order: i32| {
        r.setting(
            SettingSpec::new(
                key,
                kind,
                default,
                format!("setting-{}-label", key.replace('.', "-")),
                format!("setting-{}-help", key.replace('.', "-")),
            )
            .order(order),
        );
    };
    add(
        "board.cards.show_branch",
        SettingKind::Bool,
        SettingValue::Bool(true),
        10,
    );
    add(
        "board.cards.show_helper",
        SettingKind::Bool,
        SettingValue::Bool(true),
        20,
    );
    add(
        "board.cards.show_request",
        SettingKind::Bool,
        SettingValue::Bool(true),
        30,
    );
    add(
        "board.cards.compact",
        SettingKind::Bool,
        SettingValue::Bool(false),
        40,
    );
    add(
        "board.stage.offer_helper",
        SettingKind::Bool,
        SettingValue::Bool(true),
        45,
    );
    add(
        "board.refresh.interval_s",
        SettingKind::Integer {
            min: Some(0),
            max: Some(120),
        },
        SettingValue::Integer(15),
        50,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-board-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn contract_passes() {
        let dir = temp_dir();
        let runtime = Runtime::new(
            vec![Box::new(BoardModule {
                state: Shared::new(),
            })],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("board registers cleanly");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = std::fs::remove_dir_all(dir);
    }
}
