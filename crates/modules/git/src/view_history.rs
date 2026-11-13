//! Large history tab: graph, commit details and blame.

use crate::state::Shared;
use crate::view_changes::{c32, ensure_context, ensure_fetch, messages, refresh_if_needed};
use crate::view_history_panel::{load_log, render_blame_panel, render_history};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;

pub struct HistoryView {
    state: Shared,
    checked: bool,
    blame_input: String,
}

impl HistoryView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            checked: false,
            blame_input: String::new(),
        }
    }
}

impl View for HistoryView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.checked {
            self.checked = true;
            ensure_context(cx, &self.state);
        }
        refresh_if_needed(cx, &self.state);
        ensure_fetch(cx, &self.state);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        ui.horizontal(|ui| {
            ui.heading(cx.t("git.history.title"));
            ui.label(
                egui::RichText::new(
                    self.state
                        .path()
                        .unwrap_or_else(|| cx.t("git.changes.empty")),
                )
                .small()
                .color(c32(tokens.color.text_muted)),
            );
        });
        ui.add_space(4.0);
        let mut tab = self.state.selected_tab();
        if tab == 0 {
            tab = 1;
        }
        let labels = [
            cx.t("git.changes.tab.history"),
            cx.t("git.changes.tab.blame"),
        ];
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut selected = if tab == 2 { 1 } else { 0 };
        let before = selected;
        forja_ui_kit::tabs(ui, &mut selected, &label_refs);
        if selected != before {
            self.state
                .set_selected_tab(if selected == 1 { 2 } else { 1 });
        }
        ui.separator();
        match selected {
            1 => render_blame_panel(ui, cx, &self.state, &mut self.blame_input),
            _ => {
                load_log(cx, &self.state);
                render_history(ui, cx, &self.state, false);
            }
        }
        messages(ui, cx, &self.state, &tokens);
    }
}
