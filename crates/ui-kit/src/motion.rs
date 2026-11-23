//! Bounded Brasa transitions. Only egui's animation clock requests frames.
use crate::egui::{self, Context, Id, Response};

/// Three speeds from the product motion specification (#202).
#[derive(Clone, Copy, Debug)]
pub enum Motion {
    /// Hover and press: 90 ms at the default base.
    Fast,
    /// Selection, panels and dialogs: 150 ms.
    Base,
    /// Toasts and numeric values: 240 ms.
    Slow,
}

/// Smooth ease-out shared by every transition.
pub fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

#[derive(Clone, Copy)]
struct Segment {
    from: f32,
    target: f32,
    value: f32,
}

/// Animate a value with a bounded ease-out, preserving continuity on retarget.
/// First observation is immediate, so loading a page never moves its layout.
/// The animation clock stops requesting frames once its target is reached.
pub fn motion_value(ctx: &Context, id: Id, target: f32, motion: Motion) -> f32 {
    let duration = crate::applied_tokens(ctx).duration(motion);
    let state_id = id.with("motion.segment");
    let clock_id = id.with("motion.clock");
    let previous = ctx.data(|data| data.get_temp::<Segment>(state_id));
    let Some(mut segment) = previous else {
        ctx.data_mut(|data| data.insert_temp(state_id, Segment { from: target, target, value: target }));
        return target;
    };
    if segment.target != target {
        segment.from = segment.value;
        segment.target = target;
        ctx.animate_value_with_time(clock_id, 0.0, 0.0);
    }
    segment.value = if duration == 0.0 || segment.from == target {
        target
    } else {
        let t = ctx.animate_value_with_time(clock_id, 1.0, duration);
        egui::lerp(segment.from..=target, ease_out(t))
    };
    ctx.data_mut(|data| data.insert_temp(state_id, segment));
    segment.value
}

/// Bool transition, with the same curve and accessibility policy as values.
pub fn motion_bool(ctx: &Context, id: Id, on: bool, motion: Motion) -> f32 {
    ease_out(ctx.animate_bool_with_time(id, on, crate::applied_tokens(ctx).duration(motion)))
}

/// Presence envelope for a short-lived surface such as a toast.
///
/// The first observation starts at zero so entry animates; lowering `present`
/// animates the exit. Frames are requested only while the value is in flight.
pub fn presence(ctx: &Context, id: Id, present: bool) -> f32 {
    let seen_id = id.with("presence.seen");
    let seen = ctx.data(|data| data.get_temp::<bool>(seen_id));
    if seen.is_none() {
        ctx.animate_bool_with_time(id, false, 0.0);
        ctx.data_mut(|data| data.insert_temp(seen_id, true));
    }
    motion_bool(ctx, id, present, Motion::Slow)
}

/// Tooltip policy shared by all Brasa components.
pub fn tooltips_enabled(ctx: &Context) -> bool {
    crate::applied_tokens(ctx).tooltips_enabled
}

/// Add a tooltip without ever changing the control's accessible name.
pub fn tooltip(response: Response, label: impl Into<egui::WidgetText>) -> Response {
    if tooltips_enabled(&response.ctx) {
        response.on_hover_text(label)
    } else {
        response
    }
}

/// Entry clock resets after a component was absent, including conditional views.
pub fn entry_motion(ctx: &Context, id: Id, motion: Motion) -> f32 {
    let frame = ctx.cumulative_frame_nr();
    let last = ctx.data_mut(|data| {
        let last = data.get_temp::<u64>(id.with("mounted"));
        data.insert_temp(id.with("mounted"), frame);
        last
    });
    if last.is_none_or(|last| frame > last + 1) {
        ctx.animate_bool_with_time(id, false, 0.0);
    }
    motion_bool(ctx, id, true, motion)
}

/// Scale painted overlay contents around their center during entry only.
/// Hit areas remain unchanged; deterministic and reduced modes are identity.
pub fn scale_layer(ctx: &Context, layer: egui::LayerId, center: egui::Pos2, scale: f32) {
    if scale < 1.0 {
        ctx.transform_layer_shapes(layer, egui::emath::TSTransform {
            scaling: scale,
            translation: center.to_vec2() * (1.0 - scale),
        });
    }
}

/// Entry transition for overlays; callers invoke it with `false` while hidden.
/// Returns veil opacity and the card's 98–100% scale fraction.
pub fn overlay_motion(ctx: &Context, id: Id, open: bool) -> (f32, f32) {
    let t = motion_bool(ctx, id, open, Motion::Base);
    (t, egui::lerp(0.98..=1.0, t))
}

/// Paint a single sliding selection mark. Use one shared id for the whole strip.
pub fn selection_mark(ui: &egui::Ui, id: Id, rect: egui::Rect, vertical: bool) {
    let mut mark = rect;
    if vertical {
        mark.min.y = motion_value(ui.ctx(), id.with("top"), rect.min.y, Motion::Base);
        mark.max.y = motion_value(ui.ctx(), id.with("bottom"), rect.max.y, Motion::Base);
    } else {
        mark.min.x = motion_value(ui.ctx(), id.with("left"), rect.min.x, Motion::Base);
        mark.max.x = motion_value(ui.ctx(), id.with("right"), rect.max.x, Motion::Base);
    }
    ui.painter().rect_filled(mark, 1.0, crate::applied_tokens(ui.ctx()).color.accent.to_color32());
}

/// Continuous phase is requested only by mounted busy-state controls and only
/// in an active window with motion enabled. Static in reduced/capture mode.
pub fn activity_phase(ctx: &Context) -> Option<f32> {
    if crate::applied_tokens(ctx).duration(Motion::Base) == 0.0 || !crate::window_active(ctx) {
        return None;
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(32));
    Some(ctx.input(|i| i.time) as f32 * std::f32::consts::TAU)
}

/// Attention dot; render only while the helper needs input.
pub fn attention_dot(ui: &mut egui::Ui, color: egui::Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
    let opacity = activity_phase(ui.ctx()).map_or(1.0, |phase| 0.65 + 0.35 * phase.sin());
    ui.painter().circle_filled(rect.center(), 3.0, color.gamma_multiply(opacity));
    response
}
