//! Módulo `terminal` da Forja.
//!
//! Entrega a aba de bancada com ConPTY, painéis e abas, emulação VT, seleção,
//! busca, links, temas, receitas, RPC/CLI e os eventos do contrato. O estado
//! vive em [`core::Core`]; a view desenha na thread da interface e nunca
//! bloqueia em disco, processo ou rede.

mod commands;
mod core;
mod links;
mod panes;
mod pty;
mod project;
mod recipe_trust;
mod recipes;
mod requests;
mod rpc;
mod search;
mod selection;
mod settings;
mod settings_registry;
mod shells;
mod terminal;
mod theme;
mod view;
mod vt;

use commands::register_commands;
use core::Core;
use forja_contracts::names;
use forja_contracts::{
    FileOpenRequested, HelperStatus, HelperStatusChanged, TerminalBell, TerminalCommandFinished,
    TerminalExited, TerminalTitleChanged, TrustDecided, TrustKind,
};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use rpc::register_rpc;
use settings::TermSettings;
use settings_registry::register_settings;
use std::sync::Arc;
use view::{RecipeView, TerminalView};

/// Ponteiro compartilhado para o estado do módulo.
pub(crate) type Shared = Arc<Core>;

#[forja_kernel::module]
static TERMINAL: fn() -> Box<dyn Module> = || {
    Box::new(TerminalModule {
        core: Arc::new(Core::new()),
    })
};

/// Módulo terminal.
pub struct TerminalModule {
    core: Shared,
}

impl Module for TerminalModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("terminal", "terminal.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        register_commands(r, &self.core);
        register_rpc(r, &self.core);
        register_events(r, &self.core);

        for key in [
            "terminal.view.search-hint",
            "terminal.view.previous",
            "terminal.view.next",
            "terminal.view.match",
            "terminal.view.matches",
            "terminal.view.close",
            "terminal.view.open",
            "terminal.view.copy",
            "terminal.view.paste",
            "terminal.view.dismiss",
            "terminal.view.recipes",
            "terminal.view.shell",
            "terminal.view.new-tab",
            "terminal.view.split-right",
            "terminal.view.split-down",
            "terminal.view.close-pane",
            "terminal.view.click-to-type",
            "terminal.view.empty-title",
            "terminal.view.empty-hint",
            "terminal.view.open-terminal",
            "terminal.view.confirm-close-title",
            "terminal.view.recipe-confirm-title",
            "terminal.view.recipe-confirm-body",
            "terminal.view.recipe-confirm-accept",
            "terminal.view.confirm-close-body",
            "terminal.view.confirm-close-accept",
            "terminal.view.cancel",
            "terminal.view.paste-title",
            "terminal.view.paste-body",
            "terminal.view.paste-accept",
            "terminal.view.notice-dismiss",
            "terminal.view.recipe-scope",
            "terminal.view.wsl",
            "terminal.view.wsl-default",
            "terminal.view.git-bash",
            "terminal.view.powershell",
            "terminal.view.pwsh",
            "terminal.view.cmd",
            "terminal.view.select-all",
            "terminal.view.copy-output",
            "terminal.view.clear",
            "terminal.view.search",
            "terminal.view.running",
            "terminal.view.exited",
            "terminal.view.recipe-empty",
            "terminal.view.recipe-scope-global",
            "terminal.view.recipe-scope-project",
            "terminal.view.recipe-picker-filter",
            "terminal.view.recipe-picker-project",
            "terminal.view.recipe-picker-others",
            "terminal.view.recipe-picker-empty",
            "terminal.view.recipe-picker-command",
            "terminal.view.recipe-picker-run",
            "terminal.view.recipe-picker-hint",
            "terminal.view.recipe-picker-untyped",
            "terminal.view.recipe-kind-go",
            "terminal.view.recipe-kind-node",
            "terminal.view.recipe-kind-python",
            "terminal.view.recipe-kind-rust",
            "terminal.view.recipe-directory",
            "terminal.view.recipe-needs-trust",
            "terminal.view.recipe-stale",
            "terminal.view.recipe-failed",
            "terminal.view.trust-missing",
            "terminal.view.link-title",
            "terminal.view.error-launch",
            "terminal.view.restart",
            "terminal.view.maximize",
            "terminal.view.equalize",
            "terminal.view.max-panes",
            "terminal.recipes.empty",
            "terminal.view.failed",
            "terminal.view.recipe-declined",
            "terminal.view.error-not-found-title",
            "terminal.view.error-not-found-body",
            "terminal.view.error-launch-body",
            "terminal.view.error-retry",
            "terminal.view.error-settings",
            "terminal.view.error-details",
        ] {
            r.uses_i18n(key);
        }

        let tab_core = Arc::clone(&self.core);
        r.view(
            Slot::BenchTab,
            "terminal.tab",
            "terminal.tab.title",
            10,
            move || Box::new(TerminalView::new(Arc::clone(&tab_core))),
        );
        let anvil_core = Arc::clone(&self.core);
        r.view(
            Slot::AnvilSource,
            "terminal.recipes",
            "terminal.recipes.title",
            10,
            move || Box::new(RecipeView::new(Arc::clone(&anvil_core))),
        );
    }
}

fn register_events(r: &mut Registrar<'_>, core: &Shared) {
    r.contract_event::<HelperStatusChanged>(names::HELPER_STATUS_CHANGED);
    let helper_core = Arc::clone(core);
    r.subscribe(move |_, event: &HelperStatusChanged| {
        if let Some(id) = event.terminal_id.as_deref().and_then(|id| id.parse().ok()) {
            let _ = helper_core.with_hub(|hub| {
                if event.status == HelperStatus::Finished {
                    hub.helper_terminals.remove(&id);
                } else {
                    hub.helper_terminals.insert(id);
                }
            });
        }
    });
    r.contract_event::<TerminalExited>(names::TERMINAL_EXITED);
    r.contract_event::<TerminalBell>(names::TERMINAL_BELL);
    r.contract_event::<TerminalCommandFinished>(names::TERMINAL_COMMAND_FINISHED);
    r.contract_event::<TerminalTitleChanged>(names::TERMINAL_TITLE_CHANGED);
    r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
    r.contract_event::<TrustDecided>(names::TRUST_DECIDED);
    let trust_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TrustDecided| {
        if event.kind != TrustKind::Recipe {
            return;
        }
        let settings = TermSettings::from_ctx(cx);
        let locale = cx.i18n().locale().id().to_owned();
        let pending = trust_core
            .with_hub(|hub| {
                hub.notice = None;
                let matches = hub.pending_trust_recipe.as_ref().is_some_and(|recipe| {
                    recipe.source_path == event.source_path
                        && recipe.content_hash == event.content_hash
                });
                if !matches {
                    return None;
                }
                let recipe = hub.pending_trust_recipe.take()?;
                if event.approved {
                    if settings.recipe_confirm {
                        hub.pending_recipe = Some(recipe);
                    } else if let Err(error) = hub.run_recipe(&recipe, &settings) {
                        hub.notice = Some(error);
                    }
                    None
                } else {
                    Some(recipe.title(&locale).to_owned())
                }
            })
            .ok()
            .flatten();
        if let Some(title) = pending {
            let declined = cx.t_args("terminal.view.recipe-declined", &[("recipe", &title)]);
            let _ = trust_core.with_hub(|hub| hub.notice = Some(declined));
        }
    });
}

#[cfg(test)]
mod tests;
