//! Keyboard ownership for floating Forja windows.

use eframe::egui::{self, Id, LayerId, Ui};

/// Restricts keyboard focus to this window and layers above it.
pub fn register_keyboard_layer(ui: &Ui, id: Id) {
    let layer = ui.layer_id();
    ui.ctx().data_mut(|data| data.insert_temp(id, layer));
    ui.memory_mut(|memory| memory.set_modal_layer(layer));
}

/// Whether this window owned the top keyboard layer in the previous frame.
pub fn keyboard_layer_active(ctx: &egui::Context, id: Id) -> bool {
    let layer: Option<LayerId> = ctx.data(|data| data.get_temp(id));
    layer.is_some_and(|layer| ctx.memory(|memory| memory.top_modal_layer() == Some(layer)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(ctx: &egui::Context, id: Id) {
        egui::Area::new(id).show(ctx, |ui| register_keyboard_layer(ui, id));
    }

    #[test]
    fn only_the_front_window_owns_the_keyboard() {
        let ctx = egui::Context::default();
        let back = Id::new("back");
        let front = Id::new("front");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(!keyboard_layer_active(ctx, back));
            assert!(keyboard_layer_active(ctx, front));
            draw(ctx, back);
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(keyboard_layer_active(ctx, back));
            assert!(!keyboard_layer_active(ctx, front));
        });
    }
}
