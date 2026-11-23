//! Settings section "Contas".

use crate::model::ProfileMeta;
use crate::profiles;
use crate::state::{ensure_loaded, seed_preview, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::collections::BTreeMap;

pub struct AccountsSettings {
    shared: Shared,
    profiles: Vec<ProfileMeta>,
    installed: BTreeMap<String, String>,
    loaded_meta: bool,
    adding: BTreeMap<String, String>,
    pending_remove: Option<(String, String)>,
}

impl AccountsSettings {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            profiles: Vec::new(),
            installed: BTreeMap::new(),
            loaded_meta: false,
            adding: BTreeMap::new(),
            pending_remove: None,
        }
    }

    fn load_meta(&mut self, cx: &mut Ctx<'_>) {
        if self.loaded_meta {
            return;
        }
        self.loaded_meta = true;
        self.profiles = profiles::list();
        if let Ok(value) = cx.run_command("helper.list", Args::new()) {
            if let Some(items) = value.get("profiles").and_then(serde_json::Value::as_array) {
                for item in items {
                    let Some(id) = item.get("id").and_then(serde_json::Value::as_str) else {
                        continue;
                    };
                    if let Some(executable) =
                        item.get("executable").and_then(serde_json::Value::as_str)
                    {
                        self.installed.insert(id.to_owned(), executable.to_owned());
                    }
                }
            }
        }
        if fixture() {
            for profile in &self.profiles {
                self.installed
                    .entry(profile.id.clone())
                    .or_insert_with(|| profile.id.clone());
            }
        }
    }
}

impl View for AccountsSettings {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_loaded(cx, &self.shared);
        if fixture() {
            seed_preview(cx, &self.shared);
        }
        self.load_meta(cx);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        ui.heading(cx.t("accounts.settings.title"));
        ui.label(
            egui::RichText::new(cx.t("accounts.settings.subtitle"))
                .small()
                .color(muted),
        );
        if self.profiles.is_empty() {
            ui.label(egui::RichText::new(cx.t("accounts.settings.empty")).color(muted));
            return;
        }
        let data = self
            .shared
            .with(|state| state.data.clone())
            .unwrap_or_default();
        let pending = self.pending_remove.clone();
        let profiles = self.profiles.clone();
        for meta in &profiles {
            let views = data.view(&meta.id);
            let installed = self.installed.get(&meta.id).cloned();
            let title = meta.name.clone();
            forja_ui_kit::card(ui, Some(&title), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} · {}", meta.env, meta.id))
                        .small()
                        .color(muted)
                        .monospace(),
                );
                if installed.is_none() {
                    ui.label(
                        egui::RichText::new(cx.t("accounts.not-installed"))
                            .small()
                            .color(c32(tokens.color.danger)),
                    );
                }
                for view in &views {
                    ui.horizontal(|ui| {
                        if ui.radio(view.active, &view.label).clicked() {
                            self.select(cx, &meta.id, &view.id);
                        }
                        if view.active {
                            ui.label(
                                egui::RichText::new(cx.t("accounts.active"))
                                    .small()
                                    .color(c32(tokens.color.ok)),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if pending == Some((meta.id.clone(), view.id.clone())) {
                                if forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Primary,
                                    &cx.t("accounts.confirm"),
                                )
                                .clicked()
                                {
                                    self.pending_remove = None;
                                    self.remove(cx, &meta.id, &view.id);
                                }
                                if forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Ghost,
                                    &cx.t("accounts.cancel"),
                                )
                                .clicked()
                                {
                                    self.pending_remove = None;
                                }
                            } else if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Ghost,
                                &cx.t("accounts.remove"),
                            )
                            .clicked()
                            {
                                self.pending_remove = Some((meta.id.clone(), view.id.clone()));
                            }
                        });
                    });
                    ui.label(
                        egui::RichText::new(&view.dir)
                            .small()
                            .color(muted)
                            .monospace(),
                    );
                }
                ui.horizontal(|ui| {
                    let mut label = self.adding.get(&meta.id).cloned().unwrap_or_default();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut label)
                            .hint_text(cx.t("accounts.add-hint"))
                            .desired_width(180.0),
                    );
                    if response.changed() {
                        self.adding.insert(meta.id.clone(), label.clone());
                    }
                    let enabled = installed.is_some();
                    ui.add_enabled_ui(enabled, |ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Secondary,
                            &cx.t("accounts.add"),
                        )
                        .clicked()
                        {
                            self.add(cx, &meta.id, &label);
                            self.adding.insert(meta.id.clone(), String::new());
                        }
                    });
                });
            });
            ui.add_space(4.0);
        }
        if let Some(error) = self.shared.with(|state| state.error.clone()).flatten() {
            ui.label(
                egui::RichText::new(error)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
        }
        if let Some(notice) = self.shared.with(|state| state.notice.clone()).flatten() {
            ui.label(
                egui::RichText::new(notice)
                    .small()
                    .color(c32(tokens.color.ok)),
            );
        }
    }
}

impl AccountsSettings {
    fn add(&mut self, cx: &mut Ctx<'_>, profile_id: &str, label: &str) {
        let mut args = Args::new();
        args.insert("profile_id", json!(profile_id));
        args.insert("label", json!(label));
        match cx.run_command("accounts.add", args) {
            Ok(value) => {
                let terminal = value
                    .get("terminal_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                let notice = if terminal.is_some() {
                    cx.t("accounts.notice-added-login")
                } else {
                    cx.t("accounts.notice-added")
                };
                self.shared.with(|state| state.notice = Some(notice));
            }
            Err(error) => self.fail(cx, error.to_string()),
        }
    }

    fn select(&mut self, cx: &mut Ctx<'_>, profile_id: &str, id: &str) {
        let mut args = Args::new();
        args.insert("profile_id", json!(profile_id));
        args.insert("id", json!(id));
        match cx.run_command("accounts.select", args) {
            Ok(_) => {
                self.shared
                    .with(|state| state.notice = Some(cx.t("accounts.notice-selected")));
            }
            Err(error) => self.fail(cx, error.to_string()),
        }
    }

    fn remove(&mut self, cx: &mut Ctx<'_>, profile_id: &str, id: &str) {
        let mut args = Args::new();
        args.insert("profile_id", json!(profile_id));
        args.insert("id", json!(id));
        args.insert("confirm", json!(true));
        match cx.run_command("accounts.remove", args) {
            Ok(_) => {
                self.shared
                    .with(|state| state.notice = Some(cx.t("accounts.notice-removed")));
            }
            Err(error) => self.fail(cx, error.to_string()),
        }
    }

    fn fail(&self, cx: &Ctx<'_>, message: String) {
        let _ = cx;
        self.shared.with(|state| state.error = Some(message));
    }
}

fn fixture() -> bool {
    std::env::var("FORJA_ACCOUNTS_FIXTURE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}
