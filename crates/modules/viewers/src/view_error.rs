//! Humanized error banner for the read-only viewer tab.

use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

/// Draws the human phrase in the body; the raw text stays in collapsed details.
pub(super) fn banner(ui: &mut egui::Ui, cx: &Ctx<'_>, raw: &str) {
    let human = forja_kernel::human_error::humanize_ctx(cx, raw);
    let details_label = cx.t("kernel.error.details");
    forja_ui_kit::error_banner(ui, &human.summary, human.detail.as_deref(), &details_label);
}
