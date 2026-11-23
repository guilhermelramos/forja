//! Storage folder controls in Settings and their restart/copy workflow.
use crate::app::ForjaApp;
use crate::overlays::humanize_setting_error_for;
use forja_kernel::{SettingSpec, SettingValue};
use forja_ui_kit::{button, egui, text_field, ButtonKind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) enum PendingStorageChange {
    Prompt {
        key: String,
        previous: String,
        source_dir: PathBuf,
        target_dir: PathBuf,
    },
    Copying {
        target: String,
        result: Arc<Mutex<Option<Result<usize, String>>>>,
    },
}

impl ForjaApp {
    pub(crate) fn storage_path_setting(
        &mut self,
        ui: &mut egui::Ui,
        spec: &SettingSpec,
        current: &SettingValue,
        label: &str,
    ) {
        let mut value = self
            .setting_drafts
            .get(&spec.key)
            .cloned()
            .unwrap_or_else(|| current.as_str().unwrap_or("").to_owned());
        let choose_label = self.text("shell.storage.choose");
        let apply_label = self.text("shell.storage.apply");
        let can_change = self.pending_storage_change.is_none();
        ui.add_enabled_ui(can_change, |ui| {
            ui.vertical(|ui| {
                let response = text_field(ui, &mut value, "");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), label)
                });
                if response.changed() {
                    self.setting_drafts.insert(spec.key.clone(), value.clone());
                }
                let mut submit =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                ui.horizontal_wrapped(|ui| {
                    if button(ui, ButtonKind::Secondary, &choose_label).clicked() {
                        let picked = match forja_kernel::test_pick::folder() {
                            Some(choice) => choice,
                            None if !cfg!(test)
                                && self.driver.is_none()
                                && self.screenshot.is_none() =>
                            {
                                rfd::FileDialog::new().pick_folder()
                            }
                            None => None,
                        };
                        if let Some(folder) = picked {
                            value = folder.display().to_string();
                            submit = true;
                        }
                    }
                    submit |= button(ui, ButtonKind::Secondary, &apply_label).clicked();
                });
                if submit {
                    self.apply_storage_setting(&spec.key, &value);
                }
            });
        });
    }

    fn apply_storage_setting(&mut self, key: &str, raw_value: &str) {
        let trimmed = raw_value.trim();
        let previous = self
            .runtime
            .settings()
            .get(key)
            .and_then(SettingValue::as_str)
            .unwrap_or("")
            .to_owned();
        let source_dir = if key == "app.storage.data_dir" {
            self.runtime.data_dir().to_path_buf()
        } else {
            self.runtime.user_dir().to_path_buf()
        };
        match self
            .runtime
            .set_setting(key, SettingValue::Text(trimmed.to_owned()))
        {
            Ok(()) => {
                self.clear_status();
                self.settings_page.errors.remove(key);
                self.setting_drafts
                    .insert(key.to_owned(), trimmed.to_owned());
                let target_dir = if trimmed.is_empty() {
                    if key == "app.storage.data_dir" {
                        forja_kernel::paths::base_default_data_dir()
                    } else {
                        forja_kernel::paths::base_default_user_dir()
                    }
                } else {
                    forja_kernel::bootstrap::expand_path(trimmed)
                        .unwrap_or_else(|| PathBuf::from(trimmed))
                };
                if !forja_kernel::bootstrap::same_path(&source_dir, &target_dir) {
                    self.storage_notice = Some(self.text("shell.storage.restart-required"));
                    self.pending_storage_change = Some(PendingStorageChange::Prompt {
                        key: key.to_owned(),
                        previous,
                        source_dir,
                        target_dir,
                    });
                }
            }
            Err(error) => {
                let raw = error.to_string();
                let summary = humanize_setting_error_for(
                    self.runtime.i18n(),
                    self.runtime.registry(),
                    key,
                    &raw,
                );
                self.settings_page.errors.insert(key.to_owned(), summary);
            }
        }
    }

    pub(crate) fn storage_copy_modal(&mut self, ctx: &egui::Context) {
        let copying_done = match &self.pending_storage_change {
            Some(PendingStorageChange::Copying { target, result }) => {
                let outcome = result.lock().ok().and_then(|mut slot| slot.take());
                outcome.map(|res| (target.clone(), res))
            }
            _ => None,
        };
        if let Some((target, outcome)) = copying_done {
            self.pending_storage_change = None;
            match outcome {
                Ok(files) => {
                    let files_str = files.to_string();
                    self.storage_notice = Some(
                        self.runtime
                            .i18n()
                            .t_args("shell.storage.copy-done", &[("files", &files_str)]),
                    );
                    self.clear_status();
                }
                Err(detail) => {
                    let summary = self.runtime.i18n().t_args(
                        "shell.storage.copy-failed",
                        &[("target", &target), ("detail", &detail)],
                    );
                    self.status = Some(forja_kernel::HumanError::with_detail(summary, detail));
                    self.status_details_open = false;
                }
            }
            return;
        }

        let Some(PendingStorageChange::Prompt {
            key,
            previous,
            source_dir,
            target_dir,
        }) = self.pending_storage_change.as_ref()
        else {
            return;
        };
        let key = key.clone();
        let previous = previous.clone();
        let source_dir = source_dir.clone();
        let target_dir = target_dir.clone();
        let target_display = target_dir.display().to_string();
        let source_display = source_dir.display().to_string();

        let title = self.text("shell.storage.copy-title");
        let body = self.runtime.i18n().t_args(
            "shell.storage.copy-body",
            &[("target", &target_display), ("source", &source_display)],
        );
        let confirm_label = self.text("shell.storage.copy-confirm");
        let empty_label = self.text("shell.storage.copy-empty");
        let cancel_label = self.text("shell.storage.copy-cancel");

        enum Choice {
            None,
            Copy,
            Empty,
            Cancel,
        }

        let mut open = true;
        let mut choice = Choice::None;
        forja_ui_kit::modal_explicit_close(ctx, &mut open, &title, |ui| {
            ui.set_min_width(400.0);
            ui.set_max_width(480.0);
            let font = ui
                .style()
                .text_styles
                .get(&egui::TextStyle::Body)
                .cloned()
                .unwrap_or_else(|| egui::FontId::proportional(14.0));
            let color = ui.visuals().text_color();
            forja_ui_kit::wrapped_label(ui, &body, font, color);
            forja_ui_kit::toolbar(ui, |ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &confirm_label)
                    .clicked()
                {
                    choice = Choice::Copy;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &empty_label)
                    .clicked()
                {
                    choice = Choice::Empty;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cancel_label)
                    .clicked()
                {
                    choice = Choice::Cancel;
                }
            });
        });

        if !open && matches!(choice, Choice::None) {
            choice = Choice::Cancel;
        }
        match choice {
            Choice::None => {}
            Choice::Copy => {
                let result: Arc<Mutex<Option<Result<usize, String>>>> = Arc::new(Mutex::new(None));
                let result_bg = Arc::clone(&result);
                let target_log = target_display.clone();
                self.runtime.tasks().spawn(
                    "shell",
                    move || {
                        forja_kernel::bootstrap::copy_dir_contents(&source_dir, &target_dir)
                            .map(|report| report.files_copied)
                            .map_err(|error| error.to_string())
                    },
                    move |_cx, outcome| {
                        match &outcome {
                            Ok(files) => forja_kernel::logging::log(
                                forja_kernel::logging::LogLevel::Info,
                                format!(
                                    "forja: copied {files} file(s) to new storage folder `{target_log}`"
                                ),
                            ),
                            Err(detail) => forja_kernel::logging::log(
                                forja_kernel::logging::LogLevel::Warn,
                                format!(
                                    "forja: failed copying data to `{target_log}`: {detail}"
                                ),
                            ),
                        }
                        if let Ok(mut slot) = result_bg.lock() {
                            *slot = Some(outcome);
                        }
                    },
                );
                self.pending_storage_change = Some(PendingStorageChange::Copying {
                    target: target_display,
                    result,
                });
            }
            Choice::Empty => {
                self.pending_storage_change = None;
                self.storage_notice = Some(self.text("shell.storage.restart-required"));
            }
            Choice::Cancel => {
                let _ = self
                    .runtime
                    .set_setting(&key, SettingValue::Text(previous.clone()));
                self.setting_drafts.insert(key, previous);
                self.storage_notice = None;
                self.pending_storage_change = None;
            }
        }
    }
}
