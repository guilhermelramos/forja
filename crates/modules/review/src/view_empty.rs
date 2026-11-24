//! The single empty-state message of the Review summary, with a direct action
//! to accept the base the bench already declares.

use crate::state::State;
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

/// Base the bench declared, or the one already chosen in the form.
pub(crate) fn suggested_base(state: &State) -> String {
    if !state.base.trim().is_empty() {
        return state.base.trim().to_owned();
    }
    "main".to_owned()
}

/// Renders one message and one button. Returns true when the button was used.
pub(crate) fn render(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    state: &State,
    muted: egui::Color32,
) -> bool {
    let base = suggested_base(state);
    forja_ui_kit::text(
        ui,
        egui::RichText::new(cx.t_args(
            "review.summary.base-missing",
            &[("base", base.as_str())],
        ))
        .small()
        .color(muted),
    );
    forja_ui_kit::button(
        ui,
        forja_ui_kit::ButtonKind::Secondary,
        &cx.t_args("review.summary.use-base", &[("base", base.as_str())]),
    )
    .clicked()
}
