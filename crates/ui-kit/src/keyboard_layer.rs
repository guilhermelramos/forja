//! Keyboard ownership for floating Forja windows.
//!
//! The top layer follows *activation order*, not drawing order: a dialog that
//! opens above the Anvil must own Escape even when the shell draws the Anvil
//! later in the frame. [`begin_keyboard_frame`] rotates the per-frame lists;
//! the shell calls it once before any panel is drawn.

use eframe::egui::{self, Id, LayerId, Ui};

fn stack_id() -> Id {
    Id::new("forja.ui.keyboard.stack")
}
fn frame_ids() -> Id {
    Id::new("forja.ui.keyboard.frame_ids")
}
fn prev_ids() -> Id {
    Id::new("forja.ui.keyboard.prev_ids")
}
fn begun_id() -> Id {
    Id::new("forja.ui.keyboard.begun")
}

/// Starts a frame of keyboard-layer bookkeeping. Call once, before any window
/// registers its layer. Layers that did not draw in the previous frame leave
/// the stack, so the layer below becomes active again as soon as one closes.
pub fn begin_keyboard_frame(ctx: &egui::Context) {
    let begun = ctx.data(|data| data.get_temp::<bool>(begun_id()).unwrap_or(false));
    if begun {
        ctx.data_mut(|data| {
            let frame: Vec<Id> = data.get_temp(frame_ids()).unwrap_or_default();
            let mut stack: Vec<Id> = data.get_temp(stack_id()).unwrap_or_default();
            stack.retain(|id| frame.contains(id));
            data.insert_temp(prev_ids(), frame);
            data.insert_temp(frame_ids(), Vec::<Id>::new());
            data.insert_temp(stack_id(), stack);
        });
    }
    ctx.data_mut(|data| data.insert_temp(begun_id(), true));
}

fn top_id(ctx: &egui::Context) -> Option<Id> {
    ctx.data(|data| data.get_temp::<Vec<Id>>(stack_id()).unwrap_or_default())
        .last()
        .copied()
}

/// Restricts keyboard focus to this window and layers above it.
///
/// A layer becomes the top one when it first appears; layers that were already
/// open keep their relative order. egui's modal layer is still claimed by each
/// window as before, but Escape ownership follows this stack, so a window drawn
/// earlier cannot steal Escape from a newer one.
pub fn register_keyboard_layer(ui: &Ui, id: Id) {
    let ctx = ui.ctx();
    let layer: LayerId = ui.layer_id();
    let previous: Vec<Id> = ctx.data(|data| data.get_temp(prev_ids()).unwrap_or_default());
    ctx.data_mut(|data| {
        let mut frame: Vec<Id> = data.get_temp(frame_ids()).unwrap_or_default();
        if !frame.contains(&id) {
            frame.push(id);
        }
        let mut stack: Vec<Id> = data.get_temp(stack_id()).unwrap_or_default();
        if !previous.contains(&id) {
            stack.retain(|entry| *entry != id);
            stack.push(id);
        }
        data.insert_temp(frame_ids(), frame.clone());
        data.insert_temp(stack_id(), stack);
        data.insert_temp(id.with("layer"), layer);
    });
    ctx.memory_mut(|memory| memory.set_modal_layer(layer));
}

/// Whether this window owned the top keyboard layer in the previous frame.
pub fn keyboard_layer_active(ctx: &egui::Context, id: Id) -> bool {
    let known = ctx.data(|data| data.get_temp::<LayerId>(id.with("layer")).is_some());
    known && top_id(ctx) == Some(id)
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
            begin_keyboard_frame(ctx);
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            assert!(!keyboard_layer_active(ctx, back));
            assert!(keyboard_layer_active(ctx, front));
            draw(ctx, back);
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
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
            begin_keyboard_frame(ctx);
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            begin_keyboard_frame(ctx);
            egui::Popup::open_id(ctx, Id::new("menu"));
            assert!(!consume_layer_escape(ctx, back));
            assert!(!consume_layer_escape(ctx, front));
            assert!(ctx.input(|input| input.key_pressed(egui::Key::Escape)));
            egui::Popup::close_all(ctx);
            draw(ctx, back);
            draw(ctx, front);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            begin_keyboard_frame(ctx);
            assert!(!consume_layer_escape(ctx, back));
            assert!(consume_layer_escape(ctx, front));
            assert!(!ctx.input(|input| input.key_pressed(egui::Key::Escape)));
        });
    }

    #[test]
    fn a_layer_opened_later_stays_on_top_even_when_drawn_earlier() {
        let ctx = egui::Context::default();
        let shell_dialog = Id::new("shell.dialog");
        let anvil = Id::new("anvil");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            draw(ctx, shell_dialog);
        });
        // The Anvil opens above the dialog, but the shell still draws the
        // dialog first. Escape must belong to the Anvil.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            draw(ctx, shell_dialog);
            draw(ctx, anvil);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            begin_keyboard_frame(ctx);
            assert!(!consume_layer_escape(ctx, shell_dialog));
            assert!(consume_layer_escape(ctx, anvil));
        });
        // The Anvil is gone: the dialog is active again on the next frame.
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            draw(ctx, shell_dialog);
            assert!(keyboard_layer_active(ctx, shell_dialog));
            assert!(!keyboard_layer_active(ctx, anvil));
        });
    }

    #[test]
    fn a_dialog_opened_above_the_anvil_closes_first() {
        let ctx = egui::Context::default();
        let anvil = Id::new("anvil");
        let dialog = Id::new("dialog");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            draw(ctx, anvil);
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            draw(ctx, dialog);
            draw(ctx, anvil);
        });
        let _ = ctx.run(escape_input(), |ctx| {
            begin_keyboard_frame(ctx);
            assert!(consume_layer_escape(ctx, dialog));
            assert!(!consume_layer_escape(ctx, anvil));
            assert!(!ctx.input(|input| input.key_pressed(egui::Key::Escape)));
        });
    }

    #[test]
    fn release_keyboard_focus_clears_focus_lock() {
        let ctx = egui::Context::default();
        let field = Id::new("field");
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            ctx.memory_mut(|memory| memory.request_focus(field));
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            begin_keyboard_frame(ctx);
            assert!(ctx.memory(|memory| memory.has_focus(field)));
            release_keyboard_focus(ctx, field);
            assert!(!ctx.memory(|memory| memory.has_focus(field)));
        });
    }
}
