//! Frame budget: `app.performance.max_fps` and `app.performance.unfocused_fps`.
//!
//! egui keeps the *smallest* delay anyone asked for, so a pass cannot cancel an
//! immediate repaint requested by a widget. What this module does is pace the
//! frames the shell asks for itself: the wake callbacks that run on background
//! threads (tasks and RPC) wait for the budget instead of asking for a frame
//! right away. That is where most of the frames come from while terminal output
//! or a scan streams in.
//!
//! Nothing here ever sleeps on the interface thread: the next frame is deferred
//! with [`egui::Context::request_repaint_after`], which the event loop turns into
//! a wait. An idle app still asks for no frame at all.

use crate::app::ForjaApp;
use eframe::egui;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Highest limit accepted from the settings; above this the display rules.
pub(crate) const MAX_FPS: f64 = 240.0;

/// Time between frames for the current window state. `None` means no limit.
///
/// `max_fps` applies while the window is focused and `unfocused_fps` while it is
/// not; a value below 1 (including 0) removes the limit.
pub(crate) fn interval(max_fps: f64, unfocused_fps: f64, focused: bool) -> Option<Duration> {
    let limit = if focused { max_fps } else { unfocused_fps };
    match limit {
        limit if limit.is_nan() || limit < 1.0 => None,
        limit => Some(Duration::from_secs_f64(1.0 / limit.min(MAX_FPS))),
    }
}

/// Moment when the next frame is allowed, shared with background wakers.
#[derive(Clone)]
pub(crate) struct FrameGate {
    base: Instant,
    next_micros: Arc<AtomicU64>,
}

impl FrameGate {
    /// Wakes the UI no earlier than the frame budget allows.
    pub(crate) fn request_repaint(&self, ctx: &egui::Context) {
        let remaining = self.remaining();
        if remaining.is_zero() {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(remaining);
        }
    }

    fn remaining(&self) -> Duration {
        let next = self.next_micros.load(Ordering::Relaxed);
        if next == 0 {
            return Duration::ZERO;
        }
        let now = self.base.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        Duration::from_micros(next.saturating_sub(now))
    }

    fn allow_at(&self, when: Instant) {
        let micros = when.saturating_duration_since(self.base).as_micros();
        self.next_micros
            .store(micros.min(u128::from(u64::MAX)) as u64, Ordering::Relaxed);
    }
}

/// Frame counter, frame time and budget for the window loop.
pub(crate) struct FramePacer {
    gate: FrameGate,
    started: Option<Instant>,
    frame_time: Duration,
    frames: u64,
}

impl FramePacer {
    pub(crate) fn new() -> Self {
        let gate = FrameGate {
            base: Instant::now(),
            next_micros: Arc::new(AtomicU64::new(0)),
        };
        Self {
            gate,
            started: None,
            frame_time: Duration::ZERO,
            frames: 0,
        }
    }

    /// Handle for the wake callbacks that run outside the interface thread.
    pub(crate) fn gate(&self) -> FrameGate {
        self.gate.clone()
    }

    /// Counts one pass and remembers when it started.
    pub(crate) fn begin(&mut self) {
        self.started = Some(Instant::now());
        self.frames += 1;
    }

    /// Records how long the pass took and defers the next frame the shell asks
    /// for to the end of the budget.
    pub(crate) fn end(&mut self, budget: Option<Duration>) {
        let now = Instant::now();
        self.frame_time = self
            .started
            .map(|started| now.saturating_duration_since(started))
            .unwrap_or_default();
        match budget {
            Some(budget) => self.gate.allow_at(now + budget),
            None => self.gate.allow_at(now),
        }
    }

    /// Frames drawn since the app started.
    pub(crate) fn frames(&self) -> u64 {
        self.frames
    }

    /// How long the last pass took.
    pub(crate) fn frame_time(&self) -> Duration {
        self.frame_time
    }
}

impl ForjaApp {
    /// Frame budget for the current window state (`app.performance.*`).
    pub(crate) fn frame_budget(&self, focused: bool) -> Option<Duration> {
        interval(
            self.setting_number("app.performance.max_fps", 60.0),
            self.setting_number("app.performance.unfocused_fps", 10.0),
            focused,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focused_limit_sets_the_interval() {
        let sixty = interval(60.0, 10.0, true).expect("60 fps has an interval");
        assert!((sixty.as_secs_f64() - 1.0 / 60.0).abs() < 1e-6);
    }

    #[test]
    fn unfocused_limit_applies_only_without_focus() {
        let focused = interval(120.0, 10.0, true).expect("limit");
        let unfocused = interval(120.0, 10.0, false).expect("limit");
        assert!((focused.as_secs_f64() - 1.0 / 120.0).abs() < 1e-6);
        assert!((unfocused.as_secs_f64() - 1.0 / 10.0).abs() < 1e-6);
    }

    #[test]
    fn zero_means_no_limit_and_huge_values_are_clamped() {
        assert!(interval(0.0, 10.0, true).is_none());
        assert!(interval(60.0, 0.0, false).is_none());
        let fast = interval(10_000.0, 10.0, true).expect("limit");
        assert!((fast.as_secs_f64() - 1.0 / MAX_FPS).abs() < 1e-9);
    }

    #[test]
    fn the_gate_defers_a_wake_until_the_budget_is_over() {
        let pacer = FramePacer::new();
        let gate = pacer.gate();
        gate.allow_at(Instant::now() + Duration::from_millis(200));
        let remaining = gate.remaining();
        assert!(remaining > Duration::from_millis(100));
        assert!(remaining <= Duration::from_millis(200));
    }
}
