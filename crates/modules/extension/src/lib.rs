//! Módulo de extensões Lua da Forja.
//!
//! Cada extensão é uma pasta com `extension.toml` e `init.lua`. A aprovação é
//! por hash da pasta inteira, o Lua roda numa thread própria sem `os`, `io`,
//! `require` ou `debug`, e a interface recebe árvores de painel prontas.

mod approval;
mod commands;
mod manifest;
mod runtime;
mod state;
mod view;

pub use manifest::{Layer, Manifest};

use forja_contracts::names;
use forja_contracts::{
    BenchActivated, BenchCreated, GitStatusChanged, HelperStatusChanged, NotifyRequested,
    TrustDecided,
};
use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use state::ExtensionCore;
use std::sync::Arc;
use view::{ExtensionAnvil, ExtensionPage, ExtensionPanels};

/// Ponteiro compartilhado do módulo.
pub(crate) type Shared = Arc<ExtensionCore>;

#[forja_kernel::module]
static EXTENSION: fn() -> Box<dyn Module> = || {
    Box::new(ExtensionModule {
        core: Arc::new(ExtensionCore::new()),
    })
};

/// Módulo extension.
pub struct ExtensionModule {
    core: Shared,
}

impl ExtensionModule {
    /// Creates the module with an empty state. Used by tests and the factory.
    pub fn new() -> Self {
        Self {
            core: Arc::new(ExtensionCore::new()),
        }
    }
}

impl Default for ExtensionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ExtensionModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("extension", "extension.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.setting(SettingSpec::new(
            "extension.mode.enabled",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "extension.setting.enabled.label",
            "extension.setting.enabled.help",
        ));
        r.setting(SettingSpec::new(
            "extension.sandbox.mode",
            SettingKind::Choice {
                options: vec!["strict".to_owned(), "ask".to_owned()],
            },
            SettingValue::Text("strict".to_owned()),
            "extension.setting.sandbox.label",
            "extension.setting.sandbox.help",
        ));
        r.setting(SettingSpec::new(
            "extension.lua.memory_mb",
            SettingKind::Integer {
                min: Some(8),
                max: Some(512),
            },
            SettingValue::Integer(64),
            "extension.setting.memory.label",
            "extension.setting.memory.help",
        ));
        r.setting(SettingSpec::new(
            "extension.lua.timeout_ms",
            SettingKind::Integer {
                min: Some(100),
                max: Some(10_000),
            },
            SettingValue::Integer(1000),
            "extension.setting.timeout.label",
            "extension.setting.timeout.help",
        ));
        r.setting(SettingSpec::new(
            "extension.fs.allow_outside_project",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "extension.setting.fs-outside.label",
            "extension.setting.fs-outside.help",
        ));
        r.setting(SettingSpec::new(
            "extension.network.allow",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "extension.setting.network.label",
            "extension.setting.network.help",
        ));
        r.setting(SettingSpec::new(
            "extension.process.allow_spawn",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "extension.setting.process.label",
            "extension.setting.process.help",
        ));
        r.setting(SettingSpec::new(
            "extension.log.level",
            SettingKind::Choice {
                options: vec![
                    "error".to_owned(),
                    "warn".to_owned(),
                    "info".to_owned(),
                    "debug".to_owned(),
                ],
            },
            SettingValue::Text("warn".to_owned()),
            "extension.setting.log.label",
            "extension.setting.log.help",
        ));
        r.setting(SettingSpec::new(
            "extension.dev.watch",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "extension.setting.dev.label",
            "extension.setting.dev.help",
        ));
        commands::register(r, &self.core);
        r.contract_event::<TrustDecided>(names::TRUST_DECIDED);
        r.contract_event::<NotifyRequested>(names::NOTIFY_REQUESTED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);
        r.contract_event::<HelperStatusChanged>(names::HELPER_STATUS_CHANGED);
        r.contract_event::<GitStatusChanged>(names::GIT_STATUS_CHANGED);
        let decided = Arc::clone(&self.core);
        r.subscribe(move |cx, event: &TrustDecided| {
            state::on_decided(&decided, cx, event);
        });
        let bench_core = Arc::clone(&self.core);
        r.subscribe(move |_cx, event: &BenchActivated| {
            state::broadcast_event(
                &bench_core,
                "bench.selected",
                serde_json::json!({
                    "id": event.0.bench_id,
                    "name": event.0.name,
                    "branch": event.0.branch,
                }),
            );
        });
        let created_core = Arc::clone(&self.core);
        r.subscribe(move |_cx, event: &BenchCreated| {
            state::broadcast_event(
                &created_core,
                "bench.created",
                serde_json::json!({
                    "id": event.0.bench_id,
                    "name": event.0.name,
                    "branch": event.0.branch,
                }),
            );
        });
        let helper_core = Arc::clone(&self.core);
        r.subscribe(move |_cx, event: &HelperStatusChanged| {
            let helper = serde_json::to_value(event.status).unwrap_or(serde_json::Value::Null);
            state::broadcast_event(
                &helper_core,
                "helper.status",
                serde_json::json!({
                    "id": event.terminal_id,
                    "bench": event.bench_path,
                    "profile": event.profile_id,
                    "state": helper,
                }),
            );
        });
        let git_core = Arc::clone(&self.core);
        r.subscribe(move |_cx, event: &GitStatusChanged| {
            state::broadcast_event(
                &git_core,
                "git.changed",
                serde_json::json!({
                    "bench": event.bench_path,
                    "branch": event.summary.branch,
                }),
            );
        });

        for key in [
            "extension.page.title",
            "extension.page.count",
            "extension.page.rescan",
            "extension.page.search",
            "extension.page.empty",
            "extension.page.empty-hint",
            "extension.state.pending",
            "extension.state.active",
            "extension.state.changed",
            "extension.state.invalid",
            "extension.state.suspended",
            "extension.state.conflict",
            "extension.state.loading",
            "extension.badge.dev",
            "extension.hash",
            "extension.capabilities",
            "extension.capability.commands",
            "extension.capability.ui.panel",
            "extension.capability.settings",
            "extension.capability.events",
            "extension.capability.terminal.send",
            "extension.capability.git.read",
            "extension.capability.fs.read",
            "extension.capability.fs.write",
            "extension.capability.http",
            "extension.capability.ui.open_url",
            "extension.commands",
            "extension.settings",
            "extension.errors",
            "extension.action.approve",
            "extension.action.reload",
            "extension.action.revoke",
            "extension.action.show-logs",
            "extension.action.hide-logs",
            "extension.panels.empty",
        ] {
            r.uses_i18n(key);
        }

        let page_core = Arc::clone(&self.core);
        r.view(Slot::Rail, "extension.page", "extension.page.title", 90, move || {
            Box::new(ExtensionPage::new(Arc::clone(&page_core)))
        });
        r.view_glyph("extension.page", forja_ui_kit::EXTENSION);
        for (slot, id, order) in [
            (Slot::RightPanel, "extension.panels.right", 90),
            (Slot::BenchTab, "extension.panels.bench", 90),
            (Slot::Sidebar, "extension.panels.side", 90),
        ] {
            let core = Arc::clone(&self.core);
            let slot_id = slot.id().to_owned();
            r.view(slot, id, "extension.panels.title", order, move || {
                Box::new(ExtensionPanels::new(Arc::clone(&core), &slot_id))
            });
        }
        let anvil_core = Arc::clone(&self.core);
        r.view(
            Slot::AnvilSource,
            "extension.commands",
            "extension.commands",
            20,
            move || Box::new(ExtensionAnvil::new(Arc::clone(&anvil_core))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-extension-module-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(ExtensionModule {
                core: Arc::new(ExtensionCore::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("extension module registers")
    }

    #[test]
    fn contract_lists_the_extension_page_and_commands() {
        let dir = temp_dir("contract");
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let list = runtime
            .call_method("extension.list", serde_json::Map::new())
            .expect("list");
        assert!(list["extensions"].is_array());
        assert!(runtime.registry().command("extension.run").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
