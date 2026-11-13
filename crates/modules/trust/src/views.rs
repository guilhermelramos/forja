//! Modal de aprovação e seção de Ajustes do trust.

use crate::state::sha256_hex;
use crate::Shared;
use forja_contracts::{TrustDecided, TrustKind};
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use serde_json::json;

/// Modal exibido quando há um pedido pendente.
pub struct ApprovalView {
    core: Shared,
}

impl ApprovalView {
    /// Cria a view ligada ao estado do módulo.
    pub fn new(core: Shared) -> Self {
        Self { core }
    }
}

impl View for ApprovalView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let pending = self
            .core
            .with_state(cx, |state| state.pending.front().cloned())
            .ok()
            .flatten();
        let Some(pending) = pending else {
            return;
        };
        let mut decision = None;
        egui::Window::new(cx.t("trust.approval.title"))
            .id(egui::Id::new("forja-trust-approval"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(
                    egui::RichText::new(kind_label(cx, pending.kind)).strong(),
                );
                ui.label(egui::RichText::new(&pending.source_path).monospace());
                if !pending.summary.is_empty() {
                    ui.label(&pending.summary);
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("trust.approval.hash"),
                        short_hash(&pending.content_hash)
                    ))
                    .weak(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Primary,
                        &cx.t("trust.approval.once"),
                    )
                    .clicked()
                    {
                        decision = Some((true, true));
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Secondary,
                        &cx.t("trust.approval.always"),
                    )
                    .clicked()
                    {
                        decision = Some((true, false));
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Danger,
                        &cx.t("trust.approval.reject"),
                    )
                    .clicked()
                    {
                        decision = Some((false, false));
                    }
                });
            });
        let Some((approved, session_only)) = decision else {
            return;
        };
        let approval = self
            .core
            .with_state(cx, |state| {
                state.decide(&pending.request_id, approved, session_only)
            })
            .ok()
            .flatten();
        if let Some(approval) = approval {
            if !approval.session_only {
                persist(cx, &self.core);
            }
            let _ = cx.emit(&TrustDecided {
                request_id: pending.request_id.clone(),
                kind: pending.kind,
                source_path: pending.source_path.clone(),
                content_hash: pending.content_hash.clone(),
                approved,
                decided_at: approval.decided_at,
            });
        }
    }
}

/// Lista de aprovações na seção de Ajustes.
pub struct SettingsView {
    core: Shared,
}

impl SettingsView {
    /// Cria a seção ligada ao estado do módulo.
    pub fn new(core: Shared) -> Self {
        Self { core }
    }
}

impl View for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let (entries, pending) = self
            .core
            .with_state(cx, |state| (state.list(), state.pending.len()))
            .unwrap_or_default();
        let pending_text = format!("{}: {pending}", cx.t("trust.settings.pending"));
        forja_ui_kit::page_heading(ui, &cx.t("trust.settings.title"), Some(&pending_text), |_| {});
        if entries.is_empty() {
            forja_ui_kit::empty_state(ui, forja_ui_kit::SETTINGS, &cx.t("trust.settings.empty"), None);
            return;
        }
        let mut revoke = None;
        for entry in &entries {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(if entry.approved {
                        cx.t("trust.settings.approved")
                    } else {
                        cx.t("trust.settings.rejected")
                    })
                    .color(if entry.approved {
                        egui::Color32::from_rgb(0x6C, 0xC4, 0x8A)
                    } else {
                        egui::Color32::from_rgb(0xE5, 0x53, 0x4B)
                    }),
                );
                ui.label(kind_label(cx, entry.kind));
                ui.label(
                    egui::RichText::new(&entry.source_path).monospace(),
                );
                ui.label(
                    egui::RichText::new(short_hash(&entry.content_hash)).weak(),
                );
                if entry.session_only {
                    ui.label(
                        egui::RichText::new(cx.t("trust.settings.session-only"))
                            .weak(),
                    );
                }
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("trust.settings.revoke"),
                )
                .clicked()
                {
                    revoke = Some((entry.kind, entry.source_path.clone()));
                }
            });
        }
        if let Some((kind, source)) = revoke {
            let revoked = self
                .core
                .with_state(cx, |state| state.revoke(kind, &source, None))
                .unwrap_or(false);
            if revoked {
                persist(cx, &self.core);
            }
        }
    }
}

fn kind_label(cx: &Ctx<'_>, kind: TrustKind) -> String {
    let key = match kind {
        TrustKind::Setup => "trust.kind.setup",
        TrustKind::Teardown => "trust.kind.teardown",
        TrustKind::Recipe => "trust.kind.recipe",
        TrustKind::Command => "trust.kind.command",
        TrustKind::Extension => "trust.kind.extension",
        TrustKind::HelperProfile => "trust.kind.helper-profile",
    };
    cx.t(key)
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

fn persist(cx: &mut Ctx<'_>, core: &Shared) {
    let (file, path) = match core.with_state(cx, |state| {
        (state.file(), state.store_path().map(std::path::Path::to_path_buf))
    }) {
        Ok(value) => value,
        Err(_) => return,
    };
    if let Some(path) = path {
        cx.spawn(
            move || {
                if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
                    let _ = write_atomic_bytes(&path, &bytes);
                }
            },
            |_, _| {},
        );
    }
}

/// Hash usado pelo teste de prévia do modal.
pub fn demo_hash() -> String {
    sha256_hex("forja-trust-demo")
}

/// Conteúdo de demonstração usado com `FORJA_TRUST_PREVIEW`.
pub fn demo_request(core: &Shared) {
    let _ = core.state.lock().map(|mut state| {
        state.pending.push_back(crate::state::PendingRequest {
            request_id: "trust-preview".to_owned(),
            kind: TrustKind::Recipe,
            source_path: "C:/forja/.forja/recipes/lint.toml".to_owned(),
            content_hash: demo_hash(),
            summary: "lint: cargo clippy --workspace".to_owned(),
        });
        json!({})
    });
}


