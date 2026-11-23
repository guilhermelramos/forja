//! Cartões de perfil instalados, não instalados e rótulos de status.

use crate::detect::{self, Detection};
use crate::profile::Profile;
use crate::status::{self, Run};
use forja_contracts::HelperStatus;
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

pub(crate) enum ProfileAction {
    Launch(String),
    Chat(String),
    SetDefault(String),
}

pub(crate) fn draw_installed_profile(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    profile: &Profile,
    detection: Option<&Detection>,
    is_default: bool,
    last_bench: Option<&str>,
) -> Option<ProfileAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        forja_ui_kit::text(ui, egui::RichText::new(&profile.name).strong());
        if let Some(version) = detection.and_then(|item| item.version.as_deref()) {
            forja_ui_kit::text(ui, egui::RichText::new(version).weak());
        } else {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("helper.view.installed")).weak());
        }
        if is_default {
            forja_ui_kit::tag(ui, &cx.t("helper.view.default-badge"));
        }
        if let Some(bench) = last_bench {
            let name = std::path::Path::new(bench)
                .file_name()
                .map(|part| part.to_string_lossy().into_owned())
                .unwrap_or_else(|| bench.to_owned());
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t_args("helper.view.last-bench", &[("bench", &name)]))
                    .weak(),
            );
        }
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Primary,
            &cx.t("helper.view.launch"),
        )
        .clicked()
        {
            action = Some(ProfileAction::Launch(profile.id.clone()));
        }
        if cx.registry().command("chat.open").is_some()
            && forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("helper.view.chat-action"),
            )
            .clicked()
        {
            action = Some(ProfileAction::Chat(profile.id.clone()));
        }
        if !is_default
            && forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("helper.view.set-default"),
            )
            .clicked()
        {
            action = Some(ProfileAction::SetDefault(profile.id.clone()));
        }
    });
    action
}

pub(crate) fn draw_missing_profile(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    profile: &Profile,
    copied_profile: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        forja_ui_kit::text(ui, &profile.name);
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("helper.view.not-installed")).weak());
        if let Some(command) = detect::install_hint(&profile.id) {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("helper.view.how-to-install")).weak());
            forja_ui_kit::text(ui, egui::RichText::new(command).monospace());
            let copied = copied_profile.as_deref() == Some(profile.id.as_str());
            let label = if copied {
                cx.t("helper.view.copied")
            } else {
                cx.t("helper.view.copy-command")
            };
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &label).clicked() {
                ui.ctx().copy_text(command.to_owned());
                *copied_profile = Some(profile.id.clone());
            }
        }
        if !profile.site.is_empty()
            && forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("helper.view.open-docs"),
            )
            .clicked()
        {
            ui.ctx().open_url(egui::OpenUrl::new_tab(&profile.site));
        }
    });
}

pub(crate) fn run_status_label(cx: &Ctx<'_>, run: &Run) -> String {
    if matches!(run.status, HelperStatus::Finished) && status::is_failed(run) {
        return cx.t("helper.view.failed");
    }
    cx.t(match run.status {
        HelperStatus::NeedsYou => "helper.view.needs-you",
        HelperStatus::Working => "helper.view.working",
        HelperStatus::Idle => "helper.view.idle",
        HelperStatus::Finished => "helper.view.finished",
        HelperStatus::Unknown => "helper.view.unknown",
    })
}

pub(crate) fn run_status_color(run: &Run, color: &forja_ui_kit::Palette) -> egui::Color32 {
    if matches!(run.status, HelperStatus::Finished) && status::is_failed(run) {
        return color.danger.to_color32();
    }
    match run.status {
        HelperStatus::NeedsYou => color.spark.to_color32(),
        HelperStatus::Working => color.info.to_color32(),
        HelperStatus::Idle => color.ok.to_color32(),
        HelperStatus::Finished => color.text_muted.to_color32(),
        HelperStatus::Unknown => color.text_muted.to_color32(),
    }
}
