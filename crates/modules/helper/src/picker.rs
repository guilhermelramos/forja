//! `helper.launch.profile`: opens a chooser with the helper profiles and launches
//! the one picked. The keymap binds it to Ctrl+Alt+Shift+A.

use crate::commands::ensure_current;
use crate::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, Registrar};
use serde_json::json;

/// Id of the chooser modal view.
pub const PICKER_VIEW: &str = "helper.profile_picker";

pub(super) fn register(r: &mut Registrar<'_>) {
    r.command(
        "helper.launch.profile",
        "helper.command.launch-profile",
        move |cx, _args| {
            cx.open_modal(PICKER_VIEW);
            Ok(json!({ "opened": true }))
        },
    );
}

/// Chooser listing the helper profiles; picking one launches it and closes.
pub struct ProfilePicker {
    core: Shared,
}

impl ProfilePicker {
    pub fn new(core: Shared) -> Self {
        Self { core }
    }
}

impl View for ProfilePicker {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ensure_current(cx, &self.core);
        let profiles = match self.core.state.lock() {
            Ok(state) => state.profiles.clone(),
            Err(_) => return,
        };
        let mut chosen: Option<String> = None;
        let mut close = false;
        egui::Window::new(cx.t("helper.profile-picker.title"))
            .id(egui::Id::new(PICKER_VIEW))
            .collapsible(false)
            .resizable(false)
            .default_width(360.0)
            .show(ui.ctx(), |ui| {
                if profiles.is_empty() {
                    forja_ui_kit::text(ui, cx.t("helper.profile-picker.empty"));
                }
                for profile in &profiles {
                    if ui.button(&profile.name).clicked() {
                        chosen = Some(profile.id.clone());
                    }
                }
                ui.separator();
                if ui.button(cx.t("helper.profile-picker.cancel")).clicked() {
                    close = true;
                }
            });
        if let Some(profile_id) = chosen {
            let mut args = Args::new();
            args.insert("profile_id", json!(profile_id));
            if let Err(error) = cx.run_command("helper.launch", args) {
                eprintln!("{error}");
            }
            close = true;
        }
        if close {
            cx.close_modal(PICKER_VIEW);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::PICKER_VIEW;
    use forja_kernel::{Module, Runtime, RuntimeOptions};

    struct TempGuard(std::path::PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn launch_profile_command_opens_the_chooser_modal() {
        let dir = TempGuard(
            std::env::temp_dir().join(format!("forja-helper-picker-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(&dir.0);
        let modules: Vec<Box<dyn Module>> = vec![Box::new(crate::HelperModule {
            core: std::sync::Arc::new(crate::Core::new()),
        })];
        let mut runtime = Runtime::new(
            modules,
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("helper registers");
        assert!(!runtime.modal_is_open(PICKER_VIEW));
        runtime
            .run_command("helper.launch.profile", forja_kernel::Args::new())
            .expect("command runs");
        assert!(runtime.modal_is_open(PICKER_VIEW));
    }
}
