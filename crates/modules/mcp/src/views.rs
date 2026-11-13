//! Modal de aprovação de uma chamada MCP de escrita.

use crate::state::Request;
use crate::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use forja_ui_kit::{button, ButtonKind};
use serde_json::json;

/// Janela modal com o pedido pendente.
pub struct ApprovalView {
    shared: Shared,
}

impl ApprovalView {
    /// Cria a view ligada ao estado.
    pub fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl View for ApprovalView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let Some(request) = self
            .shared
            .lock()
            .ok()
            .and_then(|state| state.front().cloned())
        else {
            cx.close_modal("mcp.approval");
            return;
        };
        let pending = self
            .shared
            .lock()
            .map(|state| state.pending())
            .unwrap_or(0);
        let mut decision = None;
        egui::Window::new(cx.t("mcp.approval.title"))
            .id(egui::Id::new("forja-mcp-approval"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("mcp.approval.client"),
                        request.client
                    ))
                    .strong(),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("mcp.approval.tool"),
                        request.tool
                    ))
                    .monospace(),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("mcp.approval.effect"),
                        request.effect
                    ))
                    .weak(),
                );
                if !request.summary.is_empty() {
                    ui.label(egui::RichText::new(&request.summary).weak());
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {pending}",
                        cx.t("mcp.approval.pending")
                    ))
                    .weak(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if button(ui, ButtonKind::Primary, &cx.t("mcp.approval.once")).clicked() {
                        decision = Some((true, false));
                    }
                    if button(ui, ButtonKind::Secondary, &cx.t("mcp.approval.until-close")).clicked()
                    {
                        decision = Some((true, true));
                    }
                    if button(ui, ButtonKind::Danger, &cx.t("mcp.approval.deny")).clicked() {
                        decision = Some((false, false));
                    }
                });
            });
        let Some((approved, session_only)) = decision else {
            return;
        };
        let next = self
            .shared
            .lock()
            .ok()
            .and_then(|mut state| {
                state.decide(&request.id, approved, session_only)?;
                state.front().map(|next| next.id.clone())
            });
        cx.close_modal("mcp.approval");
        if let Some(next) = next {
            cx.open_modal_with("mcp.approval", json!({ "request_id": next }));
        }
    }
}

/// Pedido exibido, para o teste de contrato da view.
#[allow(dead_code)]
fn _request_shape(request: &Request) -> (&str, &str) {
    (request.client.as_str(), request.tool.as_str())
}
