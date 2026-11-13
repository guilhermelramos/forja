//! Shared usage summary and incremental cache.

use crate::model::UsageSummary;
use crate::scan::{Cache, ScanStats};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    pub summary: UsageSummary,
    pub stats: ScanStats,
    pub error: Option<String>,
    pub busy: bool,
    pub last_scan: Option<Instant>,
    pub cache: Option<Cache>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> Option<R> {
        self.0.lock().ok().map(|mut state| f(&mut state))
    }

    pub fn summary(&self) -> UsageSummary {
        self.0
            .lock()
            .map(|state| state.summary.clone())
            .unwrap_or_default()
    }

    pub fn error(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.error.clone())
    }

    pub fn begin_scan(&self) -> Option<Option<Cache>> {
        self.0.lock().ok().and_then(|mut state| {
            if state.busy {
                return None;
            }
            state.busy = true;
            Some(state.cache.take())
        })
    }

    pub fn finish_scan(&self, cache: Cache, summary: UsageSummary, stats: ScanStats) {
        if let Ok(mut state) = self.0.lock() {
            state.cache = Some(cache);
            state.summary = summary;
            state.stats = stats;
            state.error = None;
            state.busy = false;
            state.last_scan = Some(Instant::now());
        }
    }

    pub fn seed(&self, summary: UsageSummary) {
        if let Ok(mut state) = self.0.lock() {
            if state.summary.total.is_empty() {
                state.summary = summary;
                state.last_scan = Some(Instant::now());
            }
        }
    }

    pub fn due(&self, interval_s: u64) -> bool {
        if interval_s == 0 {
            return false;
        }
        self.0
            .lock()
            .map(|state| {
                state
                    .last_scan
                    .is_none_or(|last| last.elapsed() >= Duration::from_secs(interval_s))
            })
            .unwrap_or(false)
    }

    pub fn expire(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.last_scan = None;
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
