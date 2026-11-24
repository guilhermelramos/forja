//! Módulo `trust` da Forja.
//!
//! Um único registro, por hash SHA-256 exato, para tudo que executa código
//! vindo de um projeto ou de uma extensão. A decisão vale só para o conteúdo
//! mostrado; qualquer byte novo volta a perguntar.

mod commands;
mod rpc;
mod state;
mod views;

use forja_contracts::names;
use forja_contracts::TrustDecided;
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use state::TrustState;
use std::sync::{Arc, Mutex};
use views::{ApprovalView, SettingsView};

/// Ponteiro compartilhado para o estado do módulo.
pub(crate) type Shared = Arc<Core>;

/// Estado global do módulo.
pub struct Core {
    /// Registro protegido por mutex.
    pub state: Mutex<TrustState>,
}

impl Core {
    fn new() -> Self {
        Self {
            state: Mutex::new(TrustState::default()),
        }
    }
}

#[forja_kernel::module]
static TRUST: fn() -> Box<dyn Module> = || {
    let core = Arc::new(Core::new());
    if std::env::var_os("FORJA_TRUST_PREVIEW").is_some() {
        views::demo_request(&core);
    }
    Box::new(TrustModule { core })
};

/// Módulo trust.
pub struct TrustModule {
    core: Shared,
}

impl TrustModule {
    /// Creates the module with an empty registry. Used by tests and the factory.
    pub fn new() -> Self {
        Self {
            core: Arc::new(Core::new()),
        }
    }
}

impl Default for TrustModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for TrustModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("trust", "trust.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));
        commands::register(r, &self.core);
        rpc::register(r, &self.core);
        r.contract_event::<TrustDecided>(names::TRUST_DECIDED);

        for key in [
            "trust.approval.title",
            "trust.approval.hash",
            "trust.approval.once",
            "trust.approval.always",
            "trust.approval.reject",
            "trust.settings.title",
            "trust.settings.pending",
            "trust.settings.empty",
            "trust.settings.approved",
            "trust.settings.rejected",
            "trust.settings.revoke",
            "trust.command.grant",
            "trust.settings.session-only",
            "trust.kind.setup",
            "trust.kind.teardown",
            "trust.kind.recipe",
            "trust.kind.command",
            "trust.kind.extension",
            "trust.kind.helper-profile",
        ] {
            r.uses_i18n(key);
        }

        let approval_core = Arc::clone(&self.core);
        r.view(
            Slot::Modal,
            "trust.approval",
            "trust.approval.title",
            10,
            move || Box::new(ApprovalView::new(Arc::clone(&approval_core))),
        );
        let settings_core = Arc::clone(&self.core);
        r.view(
            Slot::SettingsSection,
            "trust.settings",
            "trust.settings.title",
            10,
            move || Box::new(SettingsView::new(Arc::clone(&settings_core))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::{json, Map};
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-trust-module-{label}-{}-{}",
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
            vec![Box::new(TrustModule { core: Arc::new(Core::new()) })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("trust module registers")
    }

    #[test]
    fn contract_and_rpc_approval_flow() {
        let dir = temp_dir("rpc");
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let check = runtime
            .call_method(
                "trust.check",
                Map::from_iter([
                    ("kind".to_owned(), json!("recipe")),
                    ("source_path".to_owned(), json!("C:/p/.forja/recipes/a.toml")),
                    ("content_hash".to_owned(), json!("abc")),
                ]),
            )
            .expect("check");
        assert_eq!(check["approved"], false);
        let request = runtime
            .call_method(
                "trust.request",
                Map::from_iter([
                    ("kind".to_owned(), json!("recipe")),
                    ("source_path".to_owned(), json!("C:/p/.forja/recipes/a.toml")),
                    ("content".to_owned(), json!("cargo test")),
                    ("summary".to_owned(), json!("Tests")),
                ]),
            )
            .expect("request");
        assert!(request["request_id"].as_str().is_some());
        let list = runtime
            .call_method("trust.list", Map::new())
            .expect("list");
        assert_eq!(list["entries"].as_array().map(Vec::len), Some(0));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn approval_is_hosted_with_the_status_bar_hidden() {
        use forja_kernel::ui_contract;
        use forja_kernel::Slot;

        let dir = temp_dir("hidden-chrome");
        let mut runtime = runtime(&dir);
        // Nenhuma view de StatusBar é instanciada: a aprovação vive na camada
        // modal e ainda assim pede a janela quando há um pedido pendente.
        assert!(runtime.registry().views_in(Slot::StatusBar).iter().all(|view| view.id != "trust.approval"));
        let contribution = runtime
            .registry()
            .views_in(Slot::Modal)
            .into_iter()
            .find(|view| view.id == "trust.approval")
            .expect("approval is a modal view");
        let mut view = ui_contract::downcast(contribution.instantiate()).expect("typed view");
        runtime
            .call_method(
                "trust.request",
                Map::from_iter([
                    ("kind".to_owned(), json!("recipe")),
                    ("source_path".to_owned(), json!("C:/p/.forja/recipes/a.toml")),
                    ("content".to_owned(), json!("cargo test")),
                    ("summary".to_owned(), json!("Tests")),
                ]),
            )
            .expect("request");
        {
            let mut cx = runtime.ctx("trust");
            view.poll(&mut cx);
        }
        assert!(runtime.modal_is_open("trust.approval"));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
