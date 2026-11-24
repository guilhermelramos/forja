//! Settings import/export and an in-app picker. All filesystem/process work uses Tasks.
use crate::app::ForjaApp;
use forja_kernel::{SettingSpec, SettingValue};
use forja_ui_kit::{egui, ButtonKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

type Mail<T> = Arc<Mutex<Option<Result<T, String>>>>;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Import,
    Export,
}
pub(crate) enum Completion {
    Imported(BTreeMap<String, SettingValue>),
    Exported,
}
struct Entry {
    path: PathBuf,
    directory: bool,
}
pub(crate) struct Picker {
    operation: Operation,
    directory: PathBuf,
    path: String,
    entries: Vec<Entry>,
    listing: Mail<Vec<Entry>>,
    requested: bool,
}
#[derive(Default)]
pub(crate) struct Transfer {
    mailbox: Mail<Completion>,
    pub picker: Option<Picker>,
    busy: bool,
    pub notice: Option<&'static str>,
}

pub(crate) fn decode(
    text: &str,
    specs: &[SettingSpec],
) -> Result<BTreeMap<String, SettingValue>, String> {
    fn flatten(
        prefix: &str,
        value: &toml::Value,
        out: &mut BTreeMap<String, SettingValue>,
    ) -> Result<(), String> {
        if let Some(table) = value.as_table() {
            for (key, value) in table {
                let key = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&key, value, out)?;
            }
        } else {
            let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
            let value = serde_json::from_value(value).map_err(|error| error.to_string())?;
            out.insert(prefix.to_owned(), value);
        }
        Ok(())
    }
    let table: toml::Value =
        toml::from_str::<toml::Value>(text).map_err(|error| error.to_string())?;
    let mut values = BTreeMap::new();
    flatten("", &table, &mut values)?;
    for (key, value) in &values {
        let spec = specs
            .iter()
            .find(|spec| spec.key == *key)
            .ok_or_else(|| format!("unknown setting {key}"))?;
        spec.validate_value(value)
            .map_err(|error| error.to_string())?;
    }
    Ok(values)
}

pub(crate) fn encode(values: &BTreeMap<String, SettingValue>) -> Result<String, String> {
    toml::to_string_pretty(values).map_err(|error| error.to_string())
}

impl ForjaApp {
    pub(crate) fn start_settings_transfer(&mut self, operation: Operation) {
        if self.settings_page.transfer.busy {
            return;
        }
        // Settings files have their own deterministic name inside the test picker folder.
        if self.driver.is_some() || self.screenshot.is_some() || cfg!(test) {
            if let Some(Some(folder)) = forja_kernel::test_pick::folder() {
                self.run_settings_transfer(operation, folder.join("settings.toml"));
            }
            return;
        }
        let directory = self
            .runtime
            .settings()
            .path()
            .parent()
            .unwrap_or(Path::new("."))
            .to_owned();
        self.settings_page.transfer.picker = Some(Picker {
            operation,
            path: directory.join("settings.toml").display().to_string(),
            directory,
            entries: Vec::new(),
            listing: Default::default(),
            requested: false,
        });
    }

    fn run_settings_transfer(&mut self, operation: Operation, path: PathBuf) {
        let values = self.runtime.settings().user_values().clone();
        let specs = self.runtime.registry().settings().to_vec();
        let mailbox = self.settings_page.transfer.mailbox.clone();
        self.settings_page.transfer.busy = true;
        self.settings_page.transfer.notice = None;
        self.runtime.tasks().spawn(
            "settings",
            move || match operation {
                Operation::Import => {
                    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
                    decode(&text, &specs).map(Completion::Imported)
                }
                Operation::Export => {
                    let text = encode(&values)?;
                    std::fs::write(path, text).map_err(|error| error.to_string())?;
                    Ok(Completion::Exported)
                }
            },
            move |_, result| {
                if let Ok(mut mail) = mailbox.lock() {
                    *mail = Some(result);
                }
            },
        );
    }

    pub(crate) fn poll_settings_transfer(&mut self) {
        let result = self
            .settings_page
            .transfer
            .mailbox
            .lock()
            .ok()
            .and_then(|mut mail| mail.take());
        let Some(result) = result else {
            return;
        };
        self.settings_page.transfer.busy = false;
        match result {
            Ok(Completion::Imported(values)) => {
                for (key, value) in values {
                    self.apply_setting(&key, value);
                }
                self.setting_drafts.clear();
                self.settings_page.transfer.notice = Some("shell.settings.imported");
            }
            Ok(Completion::Exported) => {
                self.settings_page.transfer.notice = Some("shell.settings.exported")
            }
            Err(error) => {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Warn,
                    format!("settings transfer: {error}"),
                );
                self.settings_page.transfer.notice = Some("shell.settings.transfer-error");
            }
        }
    }

    pub(crate) fn settings_transfer_windows(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.settings_page.transfer.picker.take() else {
            return;
        };
        if !picker.requested {
            picker.requested = true;
            let directory = picker.directory.clone();
            let listing = picker.listing.clone();
            self.runtime.tasks().spawn(
                "settings",
                move || {
                    let mut entries = Vec::new();
                    for item in std::fs::read_dir(directory).map_err(|error| error.to_string())? {
                        let item = item.map_err(|error| error.to_string())?;
                        let directory = item
                            .file_type()
                            .map_err(|error| error.to_string())?
                            .is_dir();
                        if directory || item.path().extension().is_some_and(|ext| ext == "toml") {
                            entries.push(Entry {
                                path: item.path(),
                                directory,
                            });
                        }
                    }
                    entries.sort_by(|a, b| {
                        b.directory
                            .cmp(&a.directory)
                            .then_with(|| a.path.cmp(&b.path))
                    });
                    Ok(entries)
                },
                move |_, result| {
                    if let Ok(mut mail) = listing.lock() {
                        *mail = Some(result);
                    }
                },
            );
        }
        if let Some(result) = picker.listing.lock().ok().and_then(|mut mail| mail.take()) {
            match result {
                Ok(entries) => picker.entries = entries,
                Err(_) => {
                    self.settings_page.transfer.notice = Some("shell.settings.transfer-error")
                }
            }
        }
        let mut open = true;
        let mut chosen = None;
        let title = self.text(if picker.operation == Operation::Import {
            "shell.settings.import"
        } else {
            "shell.settings.export"
        });
        forja_ui_kit::modal_explicit_close(ctx, &mut open, &title, |ui| {
            ui.set_max_width(500.0);
            ui.label(picker.directory.display().to_string());
            if forja_ui_kit::button(ui, ButtonKind::Ghost, &self.text("shell.settings.parent"))
                .clicked()
            {
                if let Some(parent) = picker.directory.parent() {
                    picker.directory = parent.to_owned();
                    picker.requested = false;
                }
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    for entry in &picker.entries {
                        let label = entry.path.file_name().unwrap_or_default().to_string_lossy();
                        if forja_ui_kit::button(ui, ButtonKind::Ghost, &label).clicked() {
                            if entry.directory {
                                picker.directory = entry.path.clone();
                                picker.requested = false;
                            } else {
                                picker.path = entry.path.display().to_string();
                            }
                        }
                    }
                });
            let path_label = self.text("shell.settings.file-path");
            ui.label(&path_label);
            forja_ui_kit::text_field_labeled(ui, &mut picker.path, &path_label, "");
            if forja_ui_kit::button(
                ui,
                ButtonKind::Primary,
                &self.text("shell.settings.choose-file"),
            )
            .clicked()
            {
                chosen = Some(PathBuf::from(&picker.path));
            }
        });
        if let Some(path) = chosen {
            self.run_settings_transfer(picker.operation, path);
        } else if open {
            self.settings_page.transfer.picker = Some(picker);
        }
    }

    pub(crate) fn open_settings_file(&self) {
        if self.driver.is_some() || self.screenshot.is_some() || cfg!(test) {
            return;
        }
        let path = self.runtime.settings().path().to_owned();
        self.runtime.tasks().spawn(
            "settings",
            move || {
                #[cfg(windows)]
                {
                    forja_kernel::process::command("notepad.exe")
                        .arg(path)
                        .spawn()
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
                #[cfg(not(windows))]
                {
                    forja_kernel::process::command("xdg-open")
                        .arg(path)
                        .spawn()
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            },
            |_, _| {},
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn specs() -> Vec<SettingSpec> {
        vec![SettingSpec::new(
            "app.enabled",
            forja_kernel::SettingKind::Bool,
            SettingValue::Bool(true),
            "label",
            "help",
        )]
    }
    #[test]
    fn settings_transfer_roundtrips_flat_keys_and_accepts_nested_files() {
        let values = BTreeMap::from([("app.enabled".to_owned(), SettingValue::Bool(false))]);
        assert_eq!(decode(&encode(&values).unwrap(), &specs()).unwrap(), values);
        assert_eq!(decode("[app]\nenabled = false", &specs()).unwrap(), values);
    }
    #[test]
    fn import_rejects_unknown_and_invalid_values_before_applying_any() {
        assert!(decode("unknown = true", &specs()).is_err());
        assert!(decode("\"app.enabled\" = 'wrong'", &specs()).is_err());
        assert!(decode("[", &specs()).is_err());
    }
}
