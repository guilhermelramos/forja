//! Button hierarchy shared by text, icon/text and action-tile controls.
use crate::{egui, ButtonKind};
/// The first visible primary in a pass owns the accent. Auxiliary panels must
/// request Secondary; page headers are drawn before their own body actions.
pub(crate) fn resolve(ui: &egui::Ui, response: &egui::Response, kind: ButtonKind) -> ButtonKind {
    if kind != ButtonKind::Primary || !ui.is_rect_visible(response.rect) {
        return kind;
    }
    if ui.ctx().memory(|memory| {
        memory
            .top_modal_layer()
            .is_some_and(|layer| layer != ui.layer_id())
    }) {
        return ButtonKind::Secondary;
    }
    let key = egui::Id::new("forja.primary.owner");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx()
        .data_mut(|data| match data.get_temp::<(u64, egui::Id)>(key) {
            Some((previous, owner)) if previous == pass && owner != response.id => {
                ButtonKind::Secondary
            }
            _ => {
                data.insert_temp(key, (pass, response.id));
                kind
            }
        })
}
/// Style metadata uses author_id, not a spoken label/description. The snapshot
/// contract reads the actual resolved appearance, not the requested enum.
pub(crate) fn mark(ui: &egui::Ui, response: &egui::Response, kind: ButtonKind) {
    let id = match kind {
        ButtonKind::Primary => "forja.button.primary",
        ButtonKind::Secondary => "forja.button.secondary",
        ButtonKind::Ghost => "forja.button.ghost",
        ButtonKind::Danger => "forja.button.destructive",
        ButtonKind::DangerConfirm => "forja.button.danger-confirm",
    };
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_author_id(id);
    });
}
