//! Keyboard-accessible confirmation for request-driven bench creation.

use crate::commands;
use crate::state::Shared;
use forja_contracts::RequestItem;
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;
use forja_ui_kit::{button, ButtonKind};
use serde::Deserialize;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PREVIEW_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
pub(crate) struct ForgePreview {
    state: Option<ForgePreviewState>,
}

#[derive(Clone)]
enum ForgePreviewState {
    Loading {
        id: String,
        item: RequestItem,
    },
    Ready {
        id: String,
        item: RequestItem,
        preview: ForgePreviewData,
    },
    Failed {
        id: Option<String>,
        summary: String,
        detail: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct ForgePreviewData {
    name: String,
    branch: String,
    base: String,
    path: String,
    #[serde(default)]
    path_exists: bool,
    #[serde(default)]
    path_registered: bool,
    setup: String,
    setup_state: String,
    #[serde(default)]
    setup_warnings: Vec<String>,
    #[serde(default)]
    fetch_before: bool,
    #[serde(default)]
    helper: bool,
}

impl ForgePreview {
    pub(crate) fn begin(&mut self, cx: &mut Ctx<'_>, shared: &Shared, item: &RequestItem) {
        let id = format!(
            "request-preview-{}",
            NEXT_PREVIEW_ID.fetch_add(1, Ordering::Relaxed)
        );
        match commands::start_forge_preview(cx, shared, &id, &item.counter, &item.key) {
            Ok(()) => {
                self.state = Some(ForgePreviewState::Loading {
                    id,
                    item: item.clone(),
                });
            }
            Err(error) => {
                let human = forja_kernel::human_error::humanize_kernel_error(cx.i18n(), &error);
                self.state = Some(ForgePreviewState::Failed {
                    id: None,
                    summary: human.summary,
                    detail: human.detail,
                });
            }
        }
    }

    pub(crate) fn poll(&mut self, cx: &mut Ctx<'_>) {
        let Some(ForgePreviewState::Loading { id, item }) = self.state.clone() else {
            return;
        };
        match commands::forge_preview_status(cx, &id) {
            Ok(value) => match value["state"].as_str() {
                Some("pending") => cx.request_repaint(),
                Some("ready") => {
                    match serde_json::from_value::<ForgePreviewData>(value["preview"].clone()) {
                        Ok(preview) => {
                            self.state = Some(ForgePreviewState::Ready { id, item, preview });
                        }
                        Err(error) => {
                            let human =
                                forja_kernel::human_error::humanize_ctx(cx, &error.to_string());
                            self.state = Some(ForgePreviewState::Failed {
                                id: Some(id),
                                summary: human.summary,
                                detail: human.detail,
                            });
                        }
                    }
                }
                Some("error") => {
                    let raw = value["error"].as_str().unwrap_or_default();
                    let summary = if raw == "no-commits" {
                        cx.t("request.forge.preview.error.no-commits")
                    } else {
                        forja_kernel::human_error::humanize_ctx(cx, raw).summary
                    };
                    self.state = Some(ForgePreviewState::Failed {
                        id: Some(id),
                        summary,
                        detail: (!raw.is_empty()).then(|| raw.to_owned()),
                    });
                }
                _ => {
                    self.state = Some(ForgePreviewState::Failed {
                        id: Some(id),
                        summary: cx.t("request.forge.preview.error.missing"),
                        detail: None,
                    });
                }
            },
            Err(error) => {
                let human = forja_kernel::human_error::humanize_kernel_error(cx.i18n(), &error);
                self.state = Some(ForgePreviewState::Failed {
                    id: Some(id),
                    summary: human.summary,
                    detail: human.detail,
                });
            }
        }
    }

    pub(crate) fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, shared: &Shared) {
        self.poll(cx);
        let Some(preview_state) = self.state.clone() else {
            return;
        };
        let layer = egui::Id::new("forja.request.forge-preview.keyboard");
        if forja_ui_kit::consume_layer_escape(ui.ctx(), layer) {
            self.cancel(cx);
            return;
        }

        let mut confirm = false;
        let mut cancel = false;
        let mut can_confirm = false;
        let mut cancel_focused = false;
        egui::Window::new(cx.t("request.forge.preview.title"))
            .id(egui::Id::new("forja.request.forge-preview"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .default_width(560.0)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                forja_ui_kit::register_keyboard_layer(ui, layer);
                ui.set_min_width(540.0);
                ui.heading(cx.t("request.forge.preview.title"));
                ui.add_space(8.0);
                match &preview_state {
                    ForgePreviewState::Loading { .. } => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(cx.t("request.forge.preview.loading"));
                        });
                    }
                    ForgePreviewState::Ready { preview, .. } => {
                        ui.label(preview_row(cx, "request.forge.preview.name", &preview.name));
                        ui.label(preview_row(
                            cx,
                            "request.forge.preview.branch",
                            &preview.branch,
                        ));
                        ui.label(preview_row(cx, "request.forge.preview.base", &preview.base));
                        ui.label(preview_row(cx, "request.forge.preview.path", &preview.path));
                        if preview.fetch_before {
                            ui.label(
                                egui::RichText::new(cx.t("request.forge.preview.fetch")).weak(),
                            );
                        }
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(cx.t("request.forge.preview.setup")).strong());
                        if preview.setup.is_empty() {
                            ui.label(
                                egui::RichText::new(cx.t("request.forge.preview.setup.none"))
                                    .weak(),
                            );
                        } else {
                            egui::ScrollArea::vertical()
                                .max_height(96.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(&preview.setup).monospace());
                                });
                            let setup_status = match preview.setup_state.as_str() {
                                "approved" => "request.forge.preview.setup.approved",
                                "disabled" => "request.forge.preview.setup.disabled",
                                _ => "request.forge.preview.setup.needs-approval",
                            };
                            ui.label(egui::RichText::new(cx.t(setup_status)).weak());
                        }
                        if !preview.setup_warnings.is_empty() {
                            ui.label(
                                egui::RichText::new(preview.setup_warnings.join("\n"))
                                    .color(ui.visuals().warn_fg_color),
                            );
                        }
                        ui.add_space(6.0);
                        let helper_status = if preview.helper {
                            "request.forge.preview.helper.yes"
                        } else {
                            "request.forge.preview.helper.no"
                        };
                        ui.label(preview_row(
                            cx,
                            "request.forge.preview.helper",
                            &cx.t(helper_status),
                        ));
                        if preview.path_exists || preview.path_registered {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                cx.t(if preview.path_exists {
                                    "request.forge.preview.path-exists"
                                } else {
                                    "request.forge.preview.path-registered"
                                }),
                            );
                        } else {
                            can_confirm = true;
                        }
                    }
                    ForgePreviewState::Failed {
                        summary, detail, ..
                    } => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            cx.t("request.forge.preview.error"),
                        );
                        ui.label(summary);
                        if let Some(detail) = detail {
                            ui.label(egui::RichText::new(detail).monospace().weak());
                        }
                    }
                }
                ui.add_space(12.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(can_confirm, |ui| {
                        if button(
                            ui,
                            ButtonKind::Primary,
                            &cx.t("request.forge.preview.confirm"),
                        )
                        .clicked()
                        {
                            confirm = true;
                        }
                    });
                    let cancel_button =
                        button(ui, ButtonKind::Ghost, &cx.t("request.forge.preview.cancel"));
                    cancel_focused = cancel_button.has_focus();
                    if cancel_button.clicked() {
                        cancel = true;
                    }
                });
            });

        let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
        if enter_pressed {
            if cancel_focused {
                cancel = true;
            } else if can_confirm {
                confirm = true;
            }
        }
        if cancel {
            self.cancel(cx);
        } else if confirm {
            if let ForgePreviewState::Ready { item, .. } = preview_state {
                let _ = commands::forge(cx, shared, &item.counter, &item.key);
            }
            self.cancel(cx);
        }
    }

    fn cancel(&mut self, cx: &mut Ctx<'_>) {
        if let Some(id) = self.state.as_ref().and_then(preview_id) {
            commands::cancel_forge_preview(cx, id);
        }
        self.state = None;
    }
}

fn preview_id(state: &ForgePreviewState) -> Option<&str> {
    match state {
        ForgePreviewState::Loading { id, .. } | ForgePreviewState::Ready { id, .. } => Some(id),
        ForgePreviewState::Failed { id, .. } => id.as_deref(),
    }
}

fn preview_row(cx: &Ctx<'_>, key: &str, value: &str) -> String {
    cx.t_args(key, &[("value", value)])
}
