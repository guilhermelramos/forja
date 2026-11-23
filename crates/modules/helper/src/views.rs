//! Página Ajudantes e seção de Ajustes.

use crate::commands::{ensure_current, maybe_detect};
use crate::hooks::{self, InstallState};
use crate::launch::ProfileOverride;
use crate::profile::{self, Profile};
use crate::profile_cards::{
    draw_installed_profile, draw_missing_profile, run_status_color, run_status_label, ProfileAction,
};
use crate::status::Run;
use crate::Shared;
use forja_contracts::HelperStatus;
use forja_kernel::settings::SettingValue;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Aba (por enquanto) com ajudantes ativos e perfis.
pub struct HelperPage {
    core: Shared,
    settings: HelperSettings,
    show_missing: bool,
    search: String,
    launch_error: Option<String>,
    copied_profile: Option<String>,
}

impl HelperPage {
    /// Cria a página ligada ao estado.
    pub fn new(core: Shared) -> Self {
        Self {
            settings: HelperSettings::new(Arc::clone(&core)),
            core,
            show_missing: false,
            search: String::new(),
            launch_error: None,
            copied_profile: None,
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
        maybe_detect(cx, &self.core);
        self.seed_preview();
        let (mut runs, profiles, detection, installed_ids, detecting, has_verdict, last_benches) =
            match self.core.state.lock() {
                Ok(state) => {
                    let installed_ids: BTreeSet<String> = state
                        .profiles
                        .iter()
                        .filter(|profile| state.is_installed(&profile.id))
                        .map(|profile| profile.id.clone())
                        .collect();
                    (
                        state.runs.values().cloned().collect::<Vec<_>>(),
                        state.profiles.clone(),
                        state.detection.clone(),
                        installed_ids,
                        state.detecting,
                        state.has_detection_verdict(),
                        state.last_benches.clone(),
                    )
                }
                Err(_) => return,
            };
        runs.sort_by_key(|run| rank(run.status));
        let active_count = runs
            .iter()
            .filter(|run| !matches!(run.status, HelperStatus::Finished))
            .count();
        let active = format!("{}: {}", cx.t("helper.view.active"), active_count);
        let default_setting = cx
            .setting("helper.default.profile")
            .and_then(|value| value.as_str())
            .unwrap_or("auto")
            .to_owned();
        let is_installed = |id: &str| installed_ids.contains(id);
        let default_id = profile::pick_default(&profiles, is_installed, &default_setting)
            .map(|profile| profile.id.clone());
        let (installed, missing): (Vec<&Profile>, Vec<&Profile>) = profiles
            .iter()
            .partition(|profile| is_installed(&profile.id));

        let title = cx.t("helper.view.title");
        let refresh = cx.t("helper.page-state.refresh");
        let mut page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("helper.tab", &title)
                .subtitle(&active)
                .refresh(&refresh),
        );
        if page.response.refresh_clicked && !detecting {
            let _ = cx.run_command("helper.detect", Args::new());
        }
        page.filters(|ui| {
            ui.horizontal(|ui| {
                let search_label = cx.t("helper.view.search-hint");
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("helper.view.profiles")).strong());
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(search_label.clone())
                        .desired_width(220.0),
                )
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        search_label.clone(),
                    )
                });
                if !self.search.trim().is_empty()
                    && forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("helper.view.clear-filter"),
                    )
                    .clicked()
                {
                    self.search.clear();
                }
            });
        });
        page.body(|ui| {
            egui::ScrollArea::vertical()
                .id_salt("helper.page.body")
                .show(ui, |ui| {
                    if detecting {
                        forja_ui_kit::page_state(
                            ui,
                            forja_ui_kit::PageState::Loading,
                            &cx.t("helper.page-state.loading"),
                            None,
                        );
                    } else if has_verdict {
                        forja_ui_kit::text(ui,
                            egui::RichText::new(cx.t_args(
                                "helper.view.detect-summary",
                                &[
                                    ("installed", &installed.len().to_string()),
                                    ("missing", &missing.len().to_string()),
                                ],
                            ))
                            .weak(),
                        );
                    }
                    if let Some(error) = &self.launch_error {
                        let human = forja_kernel::human_error::humanize_ctx(cx, error);
                        let detail = human
                            .detail
                            .as_ref()
                            .map(|detail| format!("{}\n{}", human.summary, detail))
                            .unwrap_or(human.summary);
                        forja_ui_kit::page_state(
                            ui,
                            forja_ui_kit::PageState::Error,
                            &cx.t("helper.page-state.error"),
                            Some(&detail),
                        );
                    }
                    if runs.is_empty() {
                        forja_ui_kit::page_state(
                            ui,
                            forja_ui_kit::PageState::Empty,
                            &cx.t("helper.page-state.empty"),
                            Some(&cx.t("helper.view.empty")),
                        );
                    }
                    let mut focus = None;
                    for run in &runs {
                        let name = profile_name(&profiles, &run.profile_id);
                        let status = run_status_label(cx, run);
                        ui.horizontal(|ui| {
                            forja_ui_kit::text(ui, egui::RichText::new(status).color(run_status_color(run, &forja_ui_kit::applied_tokens(ui.ctx()).color)));
                            forja_ui_kit::text(ui, egui::RichText::new(name).strong());
                            forja_ui_kit::text(ui,
                                egui::RichText::new(
                                    std::path::Path::new(&run.bench_path)
                                        .file_name()
                                        .map(|name| name.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| run.bench_path.clone()),
                                )
                                .weak(),
                            );
                            forja_ui_kit::text(ui, egui::RichText::new(format_duration(&run.started_at)).weak());
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Ghost,
                                &cx.t("helper.view.focus"),
                            )
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
                    let raw_query = self.search.trim().to_owned();
                    let query = raw_query.to_ascii_lowercase();
                    let matches_query = |profile: &Profile| -> bool {
                        query.is_empty()
                            || profile.name.to_ascii_lowercase().contains(&query)
                            || profile.id.to_ascii_lowercase().contains(&query)
                    };
                    let installed_filtered: Vec<&Profile> = installed
                        .iter()
                        .copied()
                        .filter(|profile| matches_query(profile))
                        .collect();
                    let missing_filtered: Vec<&Profile> = missing
                        .iter()
                        .copied()
                        .filter(|profile| matches_query(profile))
                        .collect();
                    let mut action: Option<ProfileAction> = None;
                    let mut clear_filter = false;
                    egui::ScrollArea::vertical()
                        .max_height(460.0)
                        .show(ui, |ui| {
                            forja_ui_kit::text(ui,
                                egui::RichText::new(format!(
                                    "{} ({})",
                                    cx.t("helper.view.installed-section"),
                                    installed.len()
                                ))
                                .strong(),
                            );
                            if installed.is_empty() {
                                forja_ui_kit::text(ui,
                                    egui::RichText::new(cx.t("helper.view.installed-empty")).weak(),
                                );
                            } else if installed_filtered.is_empty() {
                                ui.horizontal(|ui| {
                                    forja_ui_kit::text(ui,
                                        egui::RichText::new(cx.t_args(
                                            "helper.view.filter-empty",
                                            &[("query", &raw_query)],
                                        ))
                                        .weak(),
                                    );
                                    if forja_ui_kit::button(
                                        ui,
                                        forja_ui_kit::ButtonKind::Ghost,
                                        &cx.t("helper.view.clear-filter"),
                                    )
                                    .clicked()
                                    {
                                        clear_filter = true;
                                    }
                                });
                            }
                            for profile in &installed_filtered {
                                if action.is_none() {
                                    action = draw_installed_profile(
                                        ui,
                                        cx,
                                        profile,
                                        detection.get(&profile.id),
                                        default_id.as_deref() == Some(profile.id.as_str()),
                                        last_benches.get(&profile.id).map(String::as_str),
                                    );
                                } else {
                                    let _ = draw_installed_profile(
                                        ui,
                                        cx,
                                        profile,
                                        detection.get(&profile.id),
                                        default_id.as_deref() == Some(profile.id.as_str()),
                                        last_benches.get(&profile.id).map(String::as_str),
                                    );
                                }
                            }
                            if !missing.is_empty() {
                                ui.add_space(forja_ui_kit::Spacing::SM);
                                let expanded = self.show_missing
                                    || (!query.is_empty() && !missing_filtered.is_empty());
                                let toggle_key = if expanded {
                                    "helper.view.hide-not-installed"
                                } else {
                                    "helper.view.show-not-installed"
                                };
                                let toggle_label =
                                    cx.t_args(toggle_key, &[("count", &missing.len().to_string())]);
                                if forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Ghost,
                                    &toggle_label,
                                )
                                .clicked()
                                {
                                    if expanded && !self.show_missing {
                                        clear_filter = true;
                                    } else {
                                        self.show_missing = !self.show_missing;
                                    }
                                }
                                if expanded && !missing_filtered.is_empty() {
                                    forja_ui_kit::text(ui,
                                        egui::RichText::new(
                                            cx.t("helper.view.not-installed-section"),
                                        )
                                        .strong(),
                                    );
                                    for profile in &missing_filtered {
                                        draw_missing_profile(
                                            ui,
                                            cx,
                                            profile,
                                            &mut self.copied_profile,
                                        );
                                    }
                                }
                            }
                        });
                    if clear_filter {
                        self.search.clear();
                    }
                    match action {
                        Some(ProfileAction::Launch(profile_id)) => {
                            self.launch_error = None;
                            let mut args = Args::new();
                            args.insert("profile_id", json!(profile_id));
                            if let Err(error) = cx.run_command("helper.launch", args) {
                                let message = match error {
                                    forja_kernel::KernelError::Command { message, .. } => message,
                                    other => other.to_string(),
                                };
                                self.launch_error = Some(message);
                            }
                        }
                        Some(ProfileAction::Chat(profile_id)) => {
                            self.launch_error = None;
                            let mut args = Args::new();
                            args.insert("profile_id", json!(profile_id));
                            if let Err(error) = cx.run_command("chat.open", args) {
                                let message = match error {
                                    forja_kernel::KernelError::Command { message, .. } => message,
                                    other => other.to_string(),
                                };
                                self.launch_error = Some(message);
                            } else if cx.registry().command("shell.tab.open").is_some() {
                                let mut tab = Args::new();
                                tab.insert("view_id", json!("chat.tab"));
                                let _ = cx.run_command("shell.tab.open", tab);
                            }
                        }
                        Some(ProfileAction::SetDefault(profile_id)) => {
                            let _ = cx.set_setting(
                                "helper.default.profile",
                                SettingValue::Text(profile_id),
                            );
                        }
                        None => {}
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
                });
        });
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
        Self { core, notice: None }
    }
}

impl HelperSettings {
    fn draw(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ensure_current(cx, &self.core);
        maybe_detect(cx, &self.core);
        let (profiles, file, installed) = match self.core.state.lock() {
            Ok(state) => (
                state.profiles.clone(),
                state.file.clone(),
                state
                    .profiles
                    .iter()
                    .filter(|profile| state.is_installed(&profile.id))
                    .map(|profile| profile.id.clone())
                    .collect::<BTreeSet<_>>(),
            ),
            Err(_) => return,
        };
        for profile in profiles
            .iter()
            .filter(|profile| profile.hooks.supported && installed.contains(&profile.id))
        {
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
            let human = forja_kernel::human_error::humanize_ctx(cx, notice);
            forja_ui_kit::error_banner(
                ui,
                &human.summary,
                human.detail.as_deref(),
                &cx.t("kernel.error.details"),
            );
        }
        let mut changes: Vec<(String, ProfileOverride)> = Vec::new();
        let mut hook_action: Option<HookAction> = None;
        for profile in &profiles {
            let overrides = file.profiles.get(&profile.id).cloned().unwrap_or_default();
            let is_installed = installed.contains(&profile.id);
            let mut enabled = is_installed && !overrides.disabled;
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
                forja_ui_kit::text(ui, egui::RichText::new(&profile.name).strong());
                forja_ui_kit::text(ui, cx.t(if is_installed {
                    "helper.settings.cli-installed"
                } else {
                    "helper.settings.cli-missing"
                }));
                let response = ui
                    .add_enabled(
                        is_installed,
                        egui::Checkbox::new(&mut enabled, cx.t("helper.settings.enabled")),
                    )
                    .on_hover_text(cx.t("helper.settings.enabled-help"));
                if response.changed() {
                    let mut next = overrides.clone();
                    next.disabled = !enabled;
                    changes.push((profile.id.clone(), next));
                }
            });
            forja_ui_kit::text(ui, egui::RichText::new(format!(
                "{}: {}",
                cx.t("helper.settings.command"),
                profile.start.command
            )));
            ui.horizontal(|ui| {
                forja_ui_kit::text(ui, cx.t("helper.settings.extra-args"));
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
                forja_ui_kit::text(ui, cx.t("helper.settings.environment"));
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
                let state_label = if !is_installed {
                    cx.t("helper.settings.cli-missing")
                } else {
                    match state {
                        Some(Ok(status)) => hook_state_label(cx, status.state),
                        Some(Err(message)) => message.clone(),
                        None => cx.t("helper.settings.status.loading"),
                    }
                };
                if let Some(path) = hooks::config_path(profile) {
                    ui.horizontal(|ui| {
                        forja_ui_kit::text(ui, cx.t("helper.settings.hook-target"));
                        forja_ui_kit::text(ui, egui::RichText::new(path.display().to_string()).monospace());
                    });
                }
                ui.horizontal(|ui| {
                    forja_ui_kit::text(ui, cx.t("helper.settings.hooks"));
                    forja_ui_kit::text(ui, egui::RichText::new(state_label).weak());
                    let mut consent_now = consent;
                    if ui
                        .add_enabled(
                            is_installed && profile.trusted && !hook_busy,
                            egui::Checkbox::new(&mut consent_now, cx.t("helper.settings.consent")),
                        )
                        .changed()
                    {
                        hook_action = Some(HookAction::Consent(profile.id.clone(), consent_now));
                    }
                    if ui
                        .add_enabled(
                            is_installed && consent && !hook_busy,
                            egui::Button::new(cx.t("helper.settings.install")),
                        )
                        .clicked()
                    {
                        hook_action = Some(HookAction::Install(profile.id.clone()));
                    }
                    if ui
                        .add_enabled(
                            is_installed && consent && !hook_busy,
                            egui::Button::new(cx.t("helper.settings.remove")),
                        )
                        .clicked()
                    {
                        hook_action = Some(HookAction::Remove(profile.id.clone()));
                    }
                });
            }
        }
        for (id, overrides) in changes {
            let _ = self.core.set_override(&id, overrides);
            crate::commands::save_file(cx, &self.core);
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
