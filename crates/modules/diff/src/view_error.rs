//! Humanized error banner for the diff tab.
//!
//! Keeps the technical Git error out of the view body and the line count of
//! `view.rs` at its pre-fix size.

use forja_kernel::ui_contract::egui;
use forja_kernel::{Ctx, HumanError};

/// Humanizes a raw error coming from the Git commands before it reaches the screen.
pub(super) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> HumanError {
    forja_kernel::human_error::humanize_ctx(cx, raw.trim())
}

/// Draws the human phrase in the body; the raw text stays in collapsed details.
pub(super) fn banner(ui: &mut egui::Ui, cx: &Ctx<'_>, raw: &str) {
    let human = humanize_error(cx, raw);
    let details_label = cx.t("kernel.error.details");
    forja_ui_kit::error_banner(ui, &human.summary, human.detail.as_deref(), &details_label);
}

#[cfg(test)]
mod tests;
