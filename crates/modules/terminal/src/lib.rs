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
    FileOpenRequested, HelperStatus, HelperStatusChanged, TerminalBell, TerminalCommandFinished, TerminalExited, TerminalTitleChanged,
};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use rpc::register_rpc;
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
            "terminal.view.recipe-needs-trust",
            "terminal.view.trust-missing",
            "terminal.view.link-title",
            "terminal.view.error-launch",
            "terminal.view.restart",
            "terminal.view.maximize",
            "terminal.view.equalize",
            "terminal.recipes.empty",
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-terminal-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(TerminalModule {
                core: Arc::new(Core::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("terminal module registers")
    }

    #[test]
    fn helper_status_events_update_close_guard() {
        let dir = temp_dir("helper-events");
        let core = Arc::new(Core::new());
        let mut runtime = Runtime::new(vec![Box::new(TerminalModule { core: Arc::clone(&core) })],
            RuntimeOptions { data_dir: Some(dir.clone()), user_dir: Some(dir.join("user")), locale: None, wake: None }).unwrap();
        let event = |status| HelperStatusChanged { bench_path: dir.display().to_string(),
            terminal_id: Some("17".into()), profile_id: "test".into(), status };
        runtime.emit(&event(HelperStatus::Working));
        assert!(core.with_hub(|hub| hub.helper_terminals.contains(&17)).unwrap());
        runtime.emit(&event(HelperStatus::Finished));
        assert!(!core.with_hub(|hub| hub.helper_terminals.contains(&17)).unwrap());
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contract_and_rpc_open_send_read() {
        let dir = temp_dir("rpc");
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let open = runtime
            .call_method(
                "terminal.open",
                Map::from_iter([
                    ("cwd".to_owned(), json!(dir.display().to_string())),
                    ("command".to_owned(), json!("cmd.exe")),
                    ("args".to_owned(), json!("/d")),
                    ("title".to_owned(), json!("RPC test")),
                ]),
            )
            .expect("open terminal");
        let terminal_id = open["terminal_id"]
            .as_str()
            .expect("terminal id")
            .to_owned();
        runtime
            .call_method(
                "terminal.send",
                Map::from_iter([
                    ("terminal_id".to_owned(), json!(terminal_id)),
                    ("text".to_owned(), json!("echo forja_rpc_marker")),
                    ("enter".to_owned(), json!(true)),
                ]),
            )
            .expect("send");
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut text = String::new();
        while Instant::now() < deadline {
            let read = runtime
                .call_method(
                    "terminal.read",
                    Map::from_iter([("terminal_id".to_owned(), json!(terminal_id))]),
                )
                .expect("read");
            text = read["text"].as_str().unwrap_or_default().to_owned();
            if text.contains("forja_rpc_marker") {
                break;
            }
            std::thread::sleep(Duration::from_millis(40));
        }
        assert!(text.contains("forja_rpc_marker"), "{text}");
        let list = runtime
            .call_method("terminal.list", Map::new())
            .expect("list");
        assert_eq!(list["terminals"].as_array().map(Vec::len), Some(1));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
