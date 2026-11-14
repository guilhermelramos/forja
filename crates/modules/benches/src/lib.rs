//! MÃ³dulo `bench` (pasta `benches`): worktrees git, etapas e automaÃ§Ã£o.
//!
//! Uma bancada Ã© um worktree. A principal Ã© o checkout original do projeto.
//! O mÃ³dulo Ã© dono de `bench.*` (comandos, RPC, eventos e settings), do
//! `forja.toml` e da aprovaÃ§Ã£o por hash dos scripts de preparo e desmonte.
//! O Quadro mora no mÃ³dulo `board`; a lateral de projetos no mÃ³dulo `project`.

mod commands;
mod config;
mod create;
mod git;
mod model;
mod refresh;
mod rpc;
mod scripts;
mod service;
mod settings;
mod stages;
mod state;
mod store;
mod tabs;
#[cfg(test)]
mod tests;
mod view;
mod view_dialogs;

use forja_contracts::names;
use forja_contracts::{BenchActivated, BenchCreated, BenchSleepChanged, BenchStored, StageChanged};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use state::Shared;

#[forja_kernel::module]
static BENCHES: fn() -> Box<dyn Module> = || {
    Box::new(BenchModule {
        state: Shared::new(),
    })
};

pub struct BenchModule {
    state: Shared,
}

impl Module for BenchModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("bench", "bench.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        settings::settings(r);

        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);
        r.contract_event::<BenchStored>(names::BENCH_STORED);
        r.contract_event::<BenchSleepChanged>(names::BENCH_SLEEP_CHANGED);
        r.contract_event::<StageChanged>(names::STAGE_CHANGED);

        commands::register_commands(r, &self.state);
        rpc::register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let dialog_state = self.state.clone();
        // Só desenha janelas de diálogo: é camada modal, não aba do painel direito.
        r.view(
            Slot::Modal,
            "bench.dialogs",
            "bench.dialogs.title",
            30,
            move || Box::new(view_dialogs::DialogsView::new(dialog_state.clone())),
        );
        let detail_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "bench.detail",
            "bench.tab.title",
            40,
            move || Box::new(view::DetailView::new(detail_state.clone())),
        );
    }
}

pub(crate) fn kernel_error(message: String) -> forja_kernel::KernelError {
    forja_kernel::KernelError::Message(message)
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "bench.name",
        "bench.command.current",
        "bench.command.list",
        "bench.command.list_cards",
        "bench.command.activate",
        "bench.command.create",
        "bench.command.store",
        "bench.command.stage_set",
        "bench.command.stage_list",
        "bench.command.sleep",
        "bench.command.wake",
        "bench.command.pin",
        "bench.command.read",
        "bench.command.refresh",
        "bench.command.script_approve",
        "bench.command.create_dialog",
        "bench.command.store_dialog",
        "bench.command.approve_dialog",
        "bench.command.next",
        "bench.command.previous",
        "bench.command.switch",
        "bench.ask.bench",
        "bench.ask.project",
        "bench.ask.name",
        "bench.ask.stage",
        "bench.tab.title",
        "bench.dialogs.title",
        "bench.stage.draft",
        "bench.stage.forging",
        "bench.stage.review",
        "bench.stage.ready",
        "bench.stage.reason.manual",
        "bench.stage.error.unknown",
        "bench.stage.error.backward",
        "bench.stage.error.forbidden",
        "bench.stage.error.commits",
        "bench.stage.error.ready",
        "bench.error.main-cannot-store",
        "bench.notify.create-failed.title",
        "bench.notify.setup-ok.title",
        "bench.notify.setup-ok.body",
        "bench.notify.setup-failed.title",
        "bench.notify.setup-failed.body",
        "bench.notify.setup-skipped.title",
        "bench.notify.setup-policy.body",
        "bench.notify.setup-unapproved.body",
        "bench.notify.setup-timeout",
        "bench.notify.teardown-failed.title",
        "bench.notify.teardown-failed.body",
        "bench.notify.teardown-unapproved.body",
        "bench.notify.stored.title",
        "bench.notify.stored.body",
        "bench.notify.store-failed.title",
        "bench.notify.fetch-failed.body",
        "bench.detail.title",
        "bench.detail.empty",
        "bench.detail.empty-hint",
        "bench.detail.project",
        "bench.detail.branch",
        "bench.detail.path",
        "bench.detail.base",
        "bench.detail.request",
        "bench.detail.notice",
        "bench.detail.actions",
        "bench.detail.archived",
        "bench.detail.sleeping",
        "bench.dialog.create.title",
        "bench.dialog.create.project",
        "bench.dialog.create.name",
        "bench.dialog.create.name-hint",
        "bench.dialog.create.base",
        "bench.dialog.create.base-hint",
        "bench.dialog.create.fetch",
        "bench.dialog.create.submit",
        "bench.dialog.create.creating",
        "bench.error.name-required",
        "bench.error.no-commits",
        "bench.error.create",
        "bench.dialog.create.add-folder",
        "bench.dialog.store.title",
        "bench.dialog.store.body",
        "bench.dialog.store.delete-branch",
        "bench.dialog.store.force",
        "bench.dialog.store.submit",
        "bench.dialog.approve.title",
        "bench.dialog.approve.setup",
        "bench.dialog.approve.teardown",
        "bench.dialog.approve.hash",
        "bench.dialog.approve.none",
        "bench.dialog.approve.submit",
        "bench.dialog.cancel",
        "bench.notice.terminal-missing",
        "bench.notice.no-projects",
        "bench.action.create",
        "bench.action.store",
        "bench.action.sleep",
        "bench.action.wake",
        "bench.action.refresh",
        "bench.action.pin",
        "bench.action.read",
        "bench.action.approve",
        "bench.method.current.description",
        "bench.method.current.result",
        "bench.method.list.description",
        "bench.method.list.result",
        "bench.method.list_cards.description",
        "bench.method.list_cards.result",
        "bench.method.activate.description",
        "bench.method.activate.result",
        "bench.method.create.description",
        "bench.method.create.result",
        "bench.method.store.description",
        "bench.method.store.result",
        "bench.method.stage_set.description",
        "bench.method.stage_set.result",
        "bench.method.stage_list.description",
        "bench.method.stage_list.result",
        "bench.method.sleep.description",
        "bench.method.sleep.result",
        "bench.method.wake.description",
        "bench.method.wake.result",
        "bench.method.pin.description",
        "bench.method.pin.result",
        "bench.method.read.description",
        "bench.method.read.result",
        "bench.method.refresh.description",
        "bench.method.refresh.result",
        "bench.method.script_approve.description",
        "bench.method.script_approve.result",
    ]
}
