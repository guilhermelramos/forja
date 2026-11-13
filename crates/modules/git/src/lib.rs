//! MÃ³dulo `git`: painel MudanÃ§as, histÃ³rico com grafo, blame e sincronizaÃ§Ã£o.
//!
//! O diff textual fica para o mÃ³dulo `diff` da onda C2: aqui `git.diff`
//! devolve os hunks em JSON. OperaÃ§Ãµes de escrita e rede sÃ£o despachadas por
//! `Ctx::spawn`; as leituras curtas (status, diff, log, blame) rodam no
//! handler para alimentar a interface e o CLI.

mod commands;
mod diff;
mod engine;
mod helpers;
mod log;
mod rpc;
mod settings;
mod state;
#[cfg(test)]
mod tests;
mod view_changes;
mod view_history;
mod view_history_panel;

use forja_contracts::names;
use forja_contracts::{BenchActivated, BenchStored, GitStatusChanged};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use helpers::refresh_async;
use state::Shared;

#[forja_kernel::module]
static GIT: fn() -> Box<dyn Module> = || {
    Box::new(GitModule {
        state: Shared::new(),
    })
};

pub struct GitModule {
    state: Shared,
}

impl Module for GitModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("git", "git.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        settings::settings(r);

        r.contract_event::<GitStatusChanged>(names::GIT_STATUS_CHANGED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<BenchStored>(names::BENCH_STORED);

        let event_state = self.state.clone();
        r.subscribe::<BenchActivated, _>(move |cx, event| {
            event_state.set_path(Some(event.0.path.clone()));
            refresh_async(cx, &event_state);
        });
        let stored_state = self.state.clone();
        r.subscribe::<BenchStored, _>(move |_cx, event| {
            if stored_state.path().as_deref() == Some(event.path.as_str()) {
                stored_state.set_path(None);
            }
        });

        commands::register_commands(r, &self.state);
        rpc::register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let changes_state = self.state.clone();
        r.view(
            Slot::RightPanel,
            "git.changes",
            "git.changes.title",
            20,
            move || Box::new(view_changes::ChangesView::new(changes_state.clone())),
        );
        let history_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "git.history",
            "git.history.title",
            60,
            move || Box::new(view_history::HistoryView::new(history_state.clone())),
        );
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "git.name",
        "git.command.status",
        "git.command.changes",
        "git.command.refresh",
        "git.command.diff",
        "git.command.stage",
        "git.command.stage_all",
        "git.command.unstage",
        "git.command.unstage_all",
        "git.command.discard",
        "git.command.commit",
        "git.command.push",
        "git.command.pull",
        "git.command.fetch",
        "git.command.sync",
        "git.command.publish",
        "git.command.rebase",
        "git.command.abort",
        "git.command.conflicts",
        "git.command.log",
        "git.command.blame",
        "git.changes.title",
        "git.history.title",
        "git.changes.empty",
        "git.changes.empty-hint",
        "git.changes.branch",
        "git.changes.ahead",
        "git.changes.behind",
        "git.changes.staged",
        "git.changes.unstaged",
        "git.changes.untracked",
        "git.changes.conflicts",
        "git.changes.commit-message",
        "git.changes.commit",
        "git.changes.commit-all",
        "git.changes.generate",
        "git.changes.stage-all",
        "git.changes.unstage-all",
        "git.changes.discard",
        "git.changes.discard-title",
        "git.changes.discard-body",
        "git.changes.discard-confirm",
        "git.changes.cancel",
        "git.changes.refresh",
        "git.changes.fetch",
        "git.changes.pull",
        "git.changes.push",
        "git.changes.sync",
        "git.changes.publish",
        "git.changes.rebase",
        "git.changes.abort",
        "git.changes.force-with-lease",
        "git.notice.ok",
        "git.notice.fetch-pull",
        "git.error.protected",
        "git.error.no-file",
        "git.error.not-repo",
        "git.error.nothing-to-commit",
        "git.error.conflict",
        "git.error.push-rejected",
        "git.error.no-upstream",
        "git.error.auth",
        "git.error.exec",
        "git.error.generic",
        "git.error.dismiss",
        "git.changes.detached",
        "git.changes.error",
        "git.changes.notice",
        "git.changes.busy",
        "git.changes.tab.changes",
        "git.changes.tab.history",
        "git.changes.tab.blame",
        "git.changes.blame-file",
        "git.changes.blame-run",
        "git.changes.blame-empty",
        "git.history.empty",
        "git.history.empty-hint",
        "git.history.load",
        "git.history.author",
        "git.history.time",
        "git.history.selected",
        "git.history.blame",
        "git.history.close",
        "git.method.status.description",
        "git.method.status.result",
        "git.method.changes.description",
        "git.method.changes.result",
        "git.method.diff.description",
        "git.method.diff.result",
        "git.method.stage.description",
        "git.method.stage.result",
        "git.method.unstage.description",
        "git.method.unstage.result",
        "git.method.discard.description",
        "git.method.discard.result",
        "git.method.commit.description",
        "git.method.commit.result",
        "git.method.push.description",
        "git.method.push.result",
        "git.method.pull.description",
        "git.method.pull.result",
        "git.method.fetch.description",
        "git.method.fetch.result",
        "git.method.sync.description",
        "git.method.sync.result",
        "git.method.publish.description",
        "git.method.publish.result",
        "git.method.rebase.description",
        "git.method.rebase.result",
        "git.method.abort.description",
        "git.method.abort.result",
        "git.method.conflicts.description",
        "git.method.conflicts.result",
        "git.method.log.description",
        "git.method.log.result",
        "git.method.blame.description",
        "git.method.blame.result",
    ]
}
