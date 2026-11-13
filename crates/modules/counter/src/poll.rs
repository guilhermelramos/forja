//! `counter.poll.interval_s`: asks the requests page to refresh the counters
//! every interval, while at least one counter is installed. The view is a modal
//! slot that never opens; its `poll` runs each frame, so it works with the window
//! closed.

use crate::sources;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use std::time::{Duration, Instant};

/// Id of the polling view.
pub const POLLER_VIEW: &str = "counter.poller";
/// Interval used when the setting is missing.
const DEFAULT_INTERVAL_S: i64 = 300;

#[derive(Default)]
pub struct Poller {
    last: Option<Instant>,
}

impl View for Poller {
    fn ui(&mut self, _ui: &mut egui::Ui, _cx: &mut Ctx<'_>) {}

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        let interval = cx
            .setting("counter.poll.interval_s")
            .and_then(SettingValue::as_i64)
            .unwrap_or(DEFAULT_INTERVAL_S);
        if interval <= 0 {
            return;
        }
        let due = self
            .last
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(interval as u64));
        if !due {
            return;
        }
        self.last = Some(Instant::now());
        let installed = sources(cx)
            .map(|value| value["counters"].as_array().is_some_and(|items| !items.is_empty()))
            .unwrap_or(false);
        if installed {
            let _ = cx.run_command("request.refresh", Args::new());
        }
    }
}
