//! Página Ajudantes e seção de Ajustes.

use crate::commands::ensure_current;
use crate::hooks::{self, InstallState};
use crate::launch::ProfileOverride;
use crate::profile::Profile;
use crate::status::Run;
use crate::Shared;
use forja_contracts::HelperStatus;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Aba (por enquanto) com ajudantes ativos e perfis.
pub struct HelperPage {
    core: Shared,
    settings: HelperSettings,
}

impl HelperPage {
    /// Cria a página ligada ao estado.
    pub fn new(core: Shared) -> Self {
        Self {
            settings: HelperSettings::new(Arc::clone(&core)),
            core,
        }
    }

    fn seed_preview(&self) {
        if std::env::var_os("FORJA_HELPER_PREVIEW").is_none() {
            return;
        }
        let Ok(mut state) = self.core.state.lock() else {
            return;
        };
        if state.preview_seeded {
            return;
        }
        state.preview_seeded = true;
        let now = chrono::Utc::now();
        for (id, version) in [
            ("claude-code", "2.1.322"),
            ("codex", "0.167.1"),
            ("opencode", "2.2.6"),
        ] {
            state.detection.insert(
                id.to_owned(),
                crate::detect::Detection {
                    executable: Some(std::path::PathBuf::from(format!("preview-{id}"))),
                    version: Some(version.to_owned()),
                    checked_at: now.to_rfc3339(),
                    error: None,
                },
            );
        }
        for (id, profile, status, minutes) in [
            ("preview-1", "claude-code", HelperStatus::NeedsYou, 4),
            ("preview-2", "codex", HelperStatus::Working, 12),
            ("preview-3", "opencode", HelperStatus::Finished, 31),
        ] {
            state.runs.insert(
                id.to_owned(),
                Run {
                    terminal_id: id.to_owned(),
                    profile_id: profile.to_owned(),
                    bench_path: "C:/forja/pedido-123".to_owned(),
                    started_at: (now - chrono::Duration::minutes(minutes)).to_rfc3339(),
                    hook_token: String::new(),
                    status,
                    last_event: Some("preview".to_owned()),
                },
            );
        }
    }
}

impl View for HelperPage {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ensure_current(cx, &self.core);
        self.seed_preview();
        let (mut runs, profiles) = match self.core.state.lock() {
            Ok(state) => (
                state.runs.values().cloned().collect::<Vec<_>>(),
                state.profiles.clone(),
            ),
            Err(_) => return,
        };
        runs.sort_by_key(|run| rank(run.status));
        let active = format!("{}: {}", cx.t("helper.view.active"), runs.len());
        forja_ui_kit::page_heading(ui, &cx.t("helper.view.title"), Some(&active), |ui| {
            if !runs.is_empty()
                && forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cx.t("helper.view.detect")).clicked()
            {
                let _ = cx.run_command("helper.detect", Args::new());
            }
        });
        if runs.is_empty()
            && forja_ui_kit::empty_state(
                ui,
                forja_ui_kit::HELPERS,
                &cx.t("helper.view.empty"),
                Some(&cx.t("helper.view.detect")),
            )
            .is_some_and(|response| response.clicked())
        {
            let _ = cx.run_command("helper.detect", Args::new());
        }
        let mut focus = None;
        for run in &runs {
            let name = profile_name(&profiles, &run.profile_id);
            let status = status_label(cx, run.status);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(status)
                        .color(status_color(run.status)),
                );
                ui.label(egui::RichText::new(name).strong());
                ui.label(
                    egui::RichText::new(
                        std::path::Path::new(&run.bench_path)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| run.bench_path.clone()),
                    )
                    .weak(),
                );
                ui.label(
                    egui::RichText::new(format_duration(&run.started_at)).weak(),
                );
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cx.t("helper.view.focus"))
                    .clicked()
                {
                    focus = Some(run.terminal_id.clone());
                }
            });
        }
        if let Some(terminal_id) = focus {
            let mut args = Args::new();
            args.insert("terminal_id", json!(terminal_id));
            let _ = cx.run_command("helper.focus", args);
        }
        ui.separator();
        ui.label(egui::RichText::new(cx.t("helper.view.profiles")).strong());
        let mut launch = None;
        egui::ScrollArea::vertical()
            .max_height(420.0)
            .show(ui, |ui| {
                for profile in &profiles {
                    ui.horizontal(|ui| {
                        let installed = self
                            .core
                            .state
                            .lock()
                            .ok()
                            .and_then(|state| state.detection.get(&profile.id).cloned())
                            .is_some_and(|detection| detection.executable.is_some());
                        ui.label(&profile.name);
                        ui.label(
                            egui::RichText::new(if installed {
                                cx.t("helper.view.installed")
                            } else {
                                cx.t("helper.view.not-installed")
                            })
                            .weak(),
                        );
                        if ui
                            .add_enabled(
                                installed,
                                egui::Button::new(cx.t("helper.view.launch")),
                            )
                            .clicked()
                        {
                            launch = Some(profile.id.clone());
                        }
                    });
                }
            });
        if let Some(profile_id) = launch {
            let mut args = Args::new();
            args.insert("profile_id", json!(profile_id));
            if let Err(error) = cx.run_command("helper.launch", args) {
                ui.label(egui::RichText::new(error.to_string()).color(egui::Color32::LIGHT_RED));
            }
        }
        if std::env::var_os("FORJA_HELPER_SETTINGS_PREVIEW").is_some() {
            egui::Window::new(cx.t("helper.settings.title"))
                .id(egui::Id::new("forja-helper-settings-preview"))
                .default_width(620.0)
                .max_height(680.0)
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.settings.draw(ui, cx);
                    });
                });
        }
    }
}

/// Seção de Ajustes com perfis e hooks.
pub struct HelperSettings {
    core: Shared,
    notice: Option<String>,
}

impl HelperSettings {
    /// Cria a seção ligada ao estado.
    pub fn new(core: Shared) -> Self {
        Self {
            core,
            notice: None,
        }
    }


}

impl HelperSettings {
    fn draw(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ensure_current(cx, &self.core);
        let (profiles, file) = match self.core.state.lock() {
            Ok(state) => (state.profiles.clone(), state.file.clone()),
            Err(_) => return,
        };
        for profile in profiles.iter().filter(|profile| profile.hooks.supported) {
            let mut args = Args::new();
            args.insert("profile_id", json!(profile.id));
            let _ = cx.run_command("helper.hooks.status", args);
        }
        let (hook_results, hook_busy) = match self.core.state.lock() {
            Ok(state) => (state.hook_results.clone(), state.hook_busy),
            Err(_) => return,
        };
        ui.heading(cx.t("helper.settings.title"));
        if let Some(notice) = &self.notice {
            ui.label(egui::RichText::new(notice).color(egui::Color32::from_rgb(0xE5, 0x53, 0x4B)));
        }
        let mut changes: Vec<(String, ProfileOverride)> = Vec::new();
        let mut hook_action: Option<HookAction> = None;
        for profile in &profiles {
            let overrides = file
                .profiles
                .get(&profile.id)
                .cloned()
                .unwrap_or_default();
            let mut enabled = !overrides.disabled;
            let mut extra = overrides.extra_args.join(" ");
            let mut environment = overrides
                .environment
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            let consent = file
                .hook_consent
                .get(&hooks::consent_key(profile))
                .copied()
                .unwrap_or(false);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&profile.name).strong());
                if ui.checkbox(&mut enabled, cx.t("helper.settings.enabled")).changed() {
                    let mut next = overrides.clone();
                    next.disabled = !enabled;
                    changes.push((profile.id.clone(), next));
                }
            });
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    cx.t("helper.settings.command"),
                    profile.start.command
                ))
                .monospace(),
            );
            ui.horizontal(|ui| {
                ui.label(cx.t("helper.settings.extra-args"));
                if ui
                    .add(egui::TextEdit::singleline(&mut extra).desired_width(260.0))
                    .lost_focus()
                {
                    let mut next = overrides.clone();
                    next.extra_args = split_arguments(&extra);
                    changes.push((profile.id.clone(), next));
                }
            });
            ui.horizontal(|ui| {
                ui.label(cx.t("helper.settings.environment"));
                if ui
                    .add(egui::TextEdit::singleline(&mut environment).desired_width(320.0))
                    .lost_focus()
                {
                    let mut next = overrides.clone();
                    next.environment = parse_environment(&environment);
                    changes.push((profile.id.clone(), next));
                }
            });
            if profile.hooks.supported {
                let state = hook_results.get(&hooks::consent_key(profile));
                let state_label = match state {
                    Some(Ok(status)) => hook_state_label(cx, status.state),
                    Some(Err(message)) => message.clone(),
                    None => cx.t("helper.settings.status.loading"),
                };
                if let Some(path) = hooks::config_path(profile) {
                    ui.horizontal(|ui| {
                        ui.label(cx.t("helper.settings.hook-target"));
                        ui.label(egui::RichText::new(path.display().to_string()).monospace());
                    });
                }
                ui.horizontal(|ui| {
                    ui.label(cx.t("helper.settings.hooks"));
                    ui.label(egui::RichText::new(state_label).weak());
                    let mut consent_now = consent;
                    if ui
                        .add_enabled(
                            profile.trusted && !hook_busy,
                            egui::Checkbox::new(&mut consent_now, cx.t("helper.settings.consent")),
                        )
                        .changed()
                    {
                        hook_action = Some(HookAction::Consent(profile.id.clone(), consent_now));
                    }
                    if ui
                        .add_enabled(consent && !hook_busy, egui::Button::new(cx.t("helper.settings.install")))
                        .clicked()
                    {
                        hook_action = Some(HookAction::Install(profile.id.clone()));
                    }
                    if ui
                        .add_enabled(consent && !hook_busy, egui::Button::new(cx.t("helper.settings.remove")))
                        .clicked()
                    {
                        hook_action = Some(HookAction::Remove(profile.id.clone()));
                    }

                });
            }
        }
        for (id, overrides) in changes {
            let _ = self.core.set_override(&id, overrides);
            self.core.save(cx);
        }
        if let Some(action) = hook_action {
            let result = match action {
                HookAction::Consent(id, granted) => {
                    let mut args = Args::new();
                    args.insert("profile_id", json!(id));
                    args.insert("granted", json!(granted));
                    cx.run_command("helper.hooks.consent", args)
                }
                HookAction::Install(id) => {
                    let mut args = Args::new();
                    args.insert("profile_id", json!(id));
                    cx.run_command("helper.hooks.install", args)
                }
                HookAction::Remove(id) => {
                    let mut args = Args::new();
                    args.insert("profile_id", json!(id));
                    cx.run_command("helper.hooks.remove", args)
                }
            };
            if let Err(error) = result {
                self.notice = Some(error.to_string());
            }

        }
    }
}

impl View for HelperSettings {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.draw(ui, cx);
    }
}

enum HookAction {
    Consent(String, bool),
    Install(String),
    Remove(String),
}

fn rank(status: HelperStatus) -> u8 {
    match status {
        HelperStatus::NeedsYou => 0,
        HelperStatus::Working => 1,
        HelperStatus::Idle => 2,
        HelperStatus::Finished => 3,
        HelperStatus::Unknown => 4,
    }
}

fn status_label(cx: &Ctx<'_>, status: HelperStatus) -> String {
    cx.t(match status {
        HelperStatus::NeedsYou => "helper.view.needs-you",
        HelperStatus::Working => "helper.view.working",
        HelperStatus::Idle => "helper.view.idle",
        HelperStatus::Finished => "helper.view.finished",
        HelperStatus::Unknown => "helper.view.unknown",
    })
}

fn status_color(status: HelperStatus) -> egui::Color32 {
    match status {
        HelperStatus::NeedsYou => egui::Color32::from_rgb(0xFF, 0xC1, 0x4D),
        HelperStatus::Working => egui::Color32::from_rgb(0x6F, 0xA8, 0xDC),
        HelperStatus::Idle => egui::Color32::from_rgb(0x6C, 0xC4, 0x8A),
        HelperStatus::Finished => egui::Color32::from_rgb(0xA6, 0x9C, 0x93),
        HelperStatus::Unknown => egui::Color32::GRAY,
    }
}

fn hook_state_label(cx: &Ctx<'_>, state: InstallState) -> String {
    cx.t(match state {
        InstallState::Installed => "helper.settings.status.installed",
        InstallState::Partial => "helper.settings.status.partial",
        InstallState::NotInstalled => "helper.settings.status.not-installed",
        InstallState::Unsupported => "helper.settings.status.unsupported",
        InstallState::Error => "helper.settings.status.error",
    })
}

fn profile_name(profiles: &[Profile], id: &str) -> String {
    profiles
        .iter()
        .find(|profile| profile.id == id)
        .map_or_else(|| id.to_owned(), |profile| profile.name.clone())
}

fn format_duration(started_at: &str) -> String {
    let Ok(started) = chrono::DateTime::parse_from_rfc3339(started_at) else {
        return String::new();
    };
    let elapsed = chrono::Utc::now().signed_duration_since(started.with_timezone(&chrono::Utc));
    let minutes = elapsed.num_minutes().max(0);
    format!("{}m {:02}s", minutes, elapsed.num_seconds().max(0) % 60)
}

fn split_arguments(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for ch in text.chars() {
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            quote = Some(ch);
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

fn parse_environment(text: &str) -> BTreeMap<String, String> {
    text.split(';')
        .filter_map(|item| item.split_once('='))
        .filter(|(key, _)| !key.trim().is_empty())
        .map(|(key, value)| (key.trim().to_owned(), value.trim().to_owned()))
        .collect()
}


