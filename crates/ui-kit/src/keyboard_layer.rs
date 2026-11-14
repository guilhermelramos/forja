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

/// Consumes Escape only for the top keyboard layer, leaving an open popup to
/// handle it first. Call before drawing the window, then close it when true.
pub fn consume_layer_escape(ctx: &egui::Context, id: Id) -> bool {
    keyboard_layer_active(ctx, id)
        && !egui::Popup::is_any_open(ctx)
        && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

/// Releases keyboard focus held by `id` (or its active focus lock).
pub fn release_keyboard_focus(ctx: &egui::Context, id: Id) {
    ctx.memory_mut(|memory| {
        if memory.has_focus(id) {
            memory.surrender_focus(id);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw(ctx: &egui::Context, id: Id) {
        egui::Area::new(id).show(ctx, |ui| register_keyboard_layer(ui, id));
    }

    fn escape_input() -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        raw
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

    #[test]
    fn escape_waits_for_popups_and_closes_only_the_front_layer() {
        let ctx = egui::Context::default();
        let back = Id::new("back");
        let front = Id::new("front");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            egui::Popup::open_id(ctx, Id::new("menu"));
            assert!(!consume_layer_escape(ctx, back));
            assert!(!consume_layer_escape(ctx, front));
            assert!(ctx.input(|input| input.key_pressed(egui::Key::Escape)));
            egui::Popup::close_all(ctx);
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            assert!(!consume_layer_escape(ctx, back));
            assert!(consume_layer_escape(ctx, front));
            assert!(!ctx.input(|input| input.key_pressed(egui::Key::Escape)));
        });
    }

    #[test]
    fn release_keyboard_focus_clears_focus_lock() {
        let ctx = egui::Context::default();
        let field = Id::new("field");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            ctx.memory_mut(|memory| memory.request_focus(field));
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(ctx.memory(|memory| memory.has_focus(field)));
            release_keyboard_focus(ctx, field);
            assert!(!ctx.memory(|memory| memory.has_focus(field)));
        });
    }
}
