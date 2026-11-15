//! Seletor de ajudantes, sincronização de instalação e avisos de chat.

use crate::model::SessionStatus;
use crate::profile::ChatProfile;
use crate::state::ChatState;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn sync_helpers(cx: &mut Ctx<'_>, state: &mut ChatState, bench: &Path) {
    let mut installed = BTreeSet::new();
    let mut hints = BTreeMap::new();
    let mut listed = false;
    if let Ok(value) = cx.run_command("helper.list", Args::new()) {
        listed = true;
        if let Some(items) = value.get("profiles").and_then(Value::as_array) {
            for item in items {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                if item.get("installed").and_then(Value::as_bool) == Some(true) {
                    installed.insert(id.to_owned());
                }
                if let Some(hint) = item.get("install_hint").and_then(Value::as_str) {
                    hints.insert(id.to_owned(), hint.to_owned());
                }
            }
        }
    }
    state.installed_helpers = installed;
    state.install_hints = hints;
    state.detection_known = listed;

    let preferred = cx
        .setting("helper.default.profile")
        .and_then(|value| value.as_str())
        .unwrap_or("auto")
        .to_owned();
    let permission = cx
        .setting("helper.default.permissions")
        .and_then(|value| value.as_str())
        .unwrap_or("ask")
        .to_owned();

    if state.active.is_none() {
        let picked = state
            .pick_installed_default(&preferred)
            .or_else(|| {
                state
                    .profiles
                    .iter()
                    .find(|profile| profile.supports_stream())
                    .map(|profile| profile.id.clone())
            })
            .unwrap_or_else(|| "claude-code".to_owned());
        let _ = state.open(bench, &picked, None, permission);
        return;
    }

    if !state.user_picked_profile {
        let should_switch = state.active.as_ref().is_some_and(|session| {
            session.messages.is_empty()
                && !matches!(session.status, SessionStatus::Working)
                && !state.is_installed(&session.profile.id)
        });
        if should_switch {
            if let Some(target) = state.pick_installed_default(&preferred) {
                if state
                    .active
                    .as_ref()
                    .is_some_and(|session| session.profile.id != target)
                {
                    let _ = state.open(bench, &target, None, permission);
                }
            }
        }
    }
}

pub(crate) fn draw_helper_switcher(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    current_id: &str,
    current_name: &str,
    profiles: &[ChatProfile],
    installed: &BTreeSet<String>,
) -> Option<String> {
    let mut stream_profiles: Vec<&ChatProfile> = profiles
        .iter()
        .filter(|profile| profile.supports_stream())
        .collect();
    stream_profiles.sort_by_key(|profile| !installed.contains(&profile.id));

    let current_installed = installed.contains(current_id);
    let selected_text = if current_installed {
        current_name.to_owned()
    } else {
        format!("{current_name} · {}", cx.t("chat.view.not-installed"))
    };
    let mut chosen: Option<String> = None;
    let combo = egui::ComboBox::from_id_salt("chat-helper-switcher")
        .selected_text(selected_text)
        .width(190.0)
        .show_ui(ui, |ui| {
            for profile in &stream_profiles {
                let is_installed = installed.contains(&profile.id);
                let label = if is_installed {
                    profile.name.clone()
                } else {
                    format!("{} · {}", profile.name, cx.t("chat.view.not-installed"))
                };
                if ui.selectable_label(profile.id == current_id, label).clicked() {
                    chosen = Some(profile.id.clone());
                }
            }
        });
    combo.response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ComboBox,
            true,
            cx.t("chat.view.helper-picker"),
        )
    });
    chosen
}

pub(crate) fn draw_uninstalled_banner(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    _profile_id: &str,
    profile_name: &str,
    hint: Option<&str>,
    any_installed: bool,
    copied_hint: &mut bool,
) {
    let headline = if any_installed {
        format!("{profile_name} · {}", cx.t("chat.view.not-installed"))
    } else {
        cx.t("chat.view.none-installed")
    };
    let body = cx.t("chat.view.install-guide");
    let copy_label = if *copied_hint {
        cx.t("chat.view.copied")
    } else {
        cx.t("chat.view.copy-install")
    };
    let open_helpers_label = cx
        .registry()
        .command("shell.rail.open")
        .is_some()
        .then(|| cx.t("chat.view.open-helpers"));
    let guide = forja_ui_kit::HelperMissingGuide {
        headline: &headline,
        body: &body,
        install_command: hint,
        copy_label: &copy_label,
        open_helpers_label: open_helpers_label.as_deref(),
    };
    if forja_ui_kit::helper_missing_guide(ui, &guide, copied_hint) {
        let mut args = Args::new();
        args.insert("view_id", json!("helper.tab"));
        let _ = cx.run_command("shell.rail.open", args);
    }
}

pub(crate) fn humanize_error(
    cx: &Ctx<'_>,
    helper_name: &str,
    raw: &str,
) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.starts_with("executable-not-found:")
        || lower.contains("os error 2")
        || lower.contains("not found")
    {
        return forja_kernel::HumanError::with_detail(
            cx.t_args("chat.view.error-not-found", &[("helper", helper_name)]),
            trimmed,
        );
    }
    if lower.contains("is not installed") || lower.contains("não está instalado") {
        return forja_kernel::HumanError::with_detail(cx.t("chat.view.send-no-helper"), trimmed);
    }
    if trimmed.starts_with("launch-failed:") {
        return forja_kernel::HumanError::with_detail(
            cx.t_args("chat.view.error-launch", &[("helper", helper_name)]),
            trimmed,
        );
    }
    forja_kernel::human_error::humanize_ctx(cx, trimmed)
}
