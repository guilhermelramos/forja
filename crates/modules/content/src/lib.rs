//! Módulo `content` da Forja.
//!
//! Conteúdo como arquivo: comandos, temas, modelos e receitas das camadas
//! embutida, usuária e de projeto. A leitura é validada, o `trust` decide o que
//! executa e `notify` recarrega depois de 300 ms sem mudanças.

mod commands;
pub mod layouts;
mod model;
mod state;
mod view;

use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use state::ContentCore;
use std::sync::Arc;
use view::{ContentAnvilView, ContentSettingsView};

/// Ponteiro compartilhado do módulo.
pub(crate) type Shared = Arc<ContentCore>;

#[forja_kernel::module]
static CONTENT: fn() -> Box<dyn Module> = || {
    Box::new(ContentModule {
        core: Arc::new(ContentCore::new()),
    })
};

/// Módulo content.
pub struct ContentModule {
    core: Shared,
}

impl Module for ContentModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("content", "content.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.setting(SettingSpec::new(
            "content.reload.watch",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "content.setting.reload-watch.label",
            "content.setting.reload-watch.help",
        ));
        r.setting(SettingSpec::new(
            "content.trust.review_on_change",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "content.setting.trust-review.label",
            "content.setting.trust-review.help",
        ));
        commands::register(r, &self.core);
        layouts::register(r, &self.core.layouts);
        let tick_core = Arc::clone(&self.core);
        r.tick(move |cx| layouts::tick(&tick_core.layouts, cx));
        for key in [
            "content.settings.title",
            "content.settings.reload",
            "content.settings.diagnostics",
            "content.settings.no-diagnostics",
            "content.settings.commands",
            "content.settings.themes",
            "content.settings.templates",
            "content.settings.recipes",
            "content.settings.layouts",
        ] {
            r.uses_i18n(key);
        }
        let anvil_core = Arc::clone(&self.core);
        r.view(
            Slot::AnvilSource,
            "content.commands",
            "content.anvil.title",
            30,
            move || Box::new(ContentAnvilView::new(Arc::clone(&anvil_core))),
        );
        let settings_core = Arc::clone(&self.core);
        r.view(
            Slot::SettingsSection,
            "content.settings",
            "content.settings.title",
            40,
            move || Box::new(ContentSettingsView::new(Arc::clone(&settings_core))),
        );
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_layouts;
