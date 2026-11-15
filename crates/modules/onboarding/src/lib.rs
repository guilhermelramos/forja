//! Primeiro uso da Forja. Pode ser reaberto pelos Ajustes.

mod steps;
mod view;

use forja_kernel::human_error::HumanError;
use forja_kernel::ui_contract::View;
use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};
use view::{SettingsView, WizardView};

#[forja_kernel::module]
static ONBOARDING: fn() -> Box<dyn Module> =
    || Box::new(OnboardingModule { state: Shared::new() });

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Progress {
    pub finished: bool,
    pub language: bool,
    pub theme: bool,
    pub folder: bool,
    pub notices: bool,
}

struct Gate {
    progress: Progress,
    loaded: bool,
    force: bool,
    dismissed: bool,
    step: usize,
    picking: bool,
    validating_folder: bool,
    folder_draft: Option<String>,
    folder_error: Option<HumanError>,
    helpers: Option<String>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<Gate>>);

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Gate {
            progress: Progress::default(),
            loaded: false,
            force: false,
            dismissed: false,
            step: 0,
            picking: false,
            validating_folder: false,
            folder_draft: None,
            folder_error: None,
            helpers: None,
        })))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Gate> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn loaded(&self) -> bool {
        self.lock().loaded
    }

    pub fn mark_loaded(&self) {
        self.lock().loaded = true;
    }

    pub fn set_progress(&self, progress: Progress) {
        self.lock().progress = progress;
    }

    pub fn progress(&self) -> Progress {
        self.lock().progress.clone()
    }

    pub fn reopen(&self) {
        let mut gate = self.lock();
        gate.force = true;
        gate.dismissed = false;
        // `FORJA_ONBOARDING_STEP=<índice>` abre direto num passo (capturas de tela).
        gate.step = std::env::var("FORJA_ONBOARDING_STEP")
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        gate.validating_folder = false;
        gate.folder_draft = None;
        gate.folder_error = None;
        gate.helpers = None;
    }

    pub fn dismiss(&self) {
        let mut gate = self.lock();
        gate.force = false;
        gate.dismissed = true;
    }

    pub fn finish(&self) {
        let mut gate = self.lock();
        gate.progress.finished = true;
        gate.force = false;
    }

    pub fn should_show(&self) -> bool {
        let gate = self.lock();
        if !gate.loaded {
            return false;
        }
        if gate.force {
            return true;
        }
        if gate.dismissed || gate.progress.finished {
            return false;
        }
        std::env::var_os("FORJA_SCREENSHOT").is_none()
    }

    pub fn step(&self) -> usize {
        self.lock().step
    }

    pub fn set_step(&self, step: usize) {
        self.lock().step = step;
    }

    pub fn picking(&self) -> bool {
        self.lock().picking
    }

    pub fn set_picking(&self, picking: bool) {
        self.lock().picking = picking;
    }

    pub fn validating_folder(&self) -> bool {
        self.lock().validating_folder
    }

    pub fn set_validating_folder(&self, busy: bool) {
        self.lock().validating_folder = busy;
    }

    pub fn folder_draft(&self) -> Option<String> {
        self.lock().folder_draft.clone()
    }

    pub fn set_folder_draft(&self, draft: Option<String>) {
        self.lock().folder_draft = draft;
    }

    pub fn folder_error(&self) -> Option<HumanError> {
        self.lock().folder_error.clone()
    }

    pub fn set_folder_error(&self, error: Option<HumanError>) {
        self.lock().folder_error = error;
    }

    pub fn helpers_text(&self) -> Option<String> {
        self.lock().helpers.clone()
    }

    pub fn set_helpers(&self, text: String) {
        self.lock().helpers = Some(text);
    }
}

pub struct OnboardingModule {
    state: Shared,
}

impl Module for OnboardingModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("onboarding", "onboarding.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));
        r.setting(
            SettingSpec::new(
                "onboarding.projects.dir",
                SettingKind::Path,
                SettingValue::Text(String::new()),
                "setting-onboarding-projects-dir-label",
                "setting-onboarding-projects-dir-help",
            )
            .order(10),
        );

        let open_state = self.state.clone();
        r.command("onboarding.open", "onboarding.command.open", move |_cx, _args| {
            open_state.reopen();
            Ok(json!({ "open": true }))
        });

        let wizard = self.state.clone();
        r.view(
            Slot::Modal,
            "onboarding.wizard",
            "onboarding.title",
            20,
            move || Box::new(WizardView { state: wizard.clone() }) as Box<dyn View>,
        );
        let settings = self.state.clone();
        r.view(
            Slot::SettingsSection,
            "onboarding.settings",
            "onboarding.settings.title",
            30,
            move || Box::new(SettingsView { state: settings.clone() }) as Box<dyn View>,
        );

        for key in [
            "onboarding.settings.hint",
            "onboarding.settings.reopen",
            "onboarding.step.language",
            "onboarding.step.theme",
            "onboarding.step.folder",
            "onboarding.step.helpers",
            "onboarding.step.import",
            "onboarding.import.lead",
            "onboarding.import.run",
            "onboarding.import.skip",
            "onboarding.import.done",
            "onboarding.import.none",
            "onboarding.step.notices",
            "onboarding.step.checklist",
            "onboarding.language.auto",
            "onboarding.language.pt",
            "onboarding.language.en",
            "onboarding.theme.system",
            "onboarding.theme.dark",
            "onboarding.theme.light",
            "onboarding.folder.hint",
            "onboarding.folder.placeholder",
            "onboarding.folder.pick",
            "onboarding.folder.validating",
            "onboarding.error.dismiss",
            "onboarding.helpers.lead",
            "onboarding.helpers.detecting",
            "onboarding.helpers.found",
            "onboarding.helpers.none",
            "onboarding.helpers.unavailable",
            "onboarding.notices.lead",
            "onboarding.notices.toast",
            "onboarding.notices.windows",
            "onboarding.check.language",
            "onboarding.check.theme",
            "onboarding.check.folder",
            "onboarding.check.notices",
            "onboarding.back",
            "onboarding.next",
            "onboarding.finish",
            "onboarding.close",
            "onboarding.done.title",
            "onboarding.done.body",
        ] {
            r.uses_i18n(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-onboarding-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn contract_and_reopen() {
        let dir = temp_dir();
        let state = Shared::new();
        let mut runtime = Runtime::new(
            vec![Box::new(OnboardingModule { state: state.clone() })],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("onboarding registers");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let value = runtime
            .run_command("onboarding.open", Args::new())
            .unwrap();
        assert_eq!(value["open"], true);
        state.mark_loaded();
        assert!(state.should_show());
        state.finish();
        state.dismiss();
        assert!(!state.should_show());
        let _ = std::fs::remove_dir_all(dir);
    }
}
