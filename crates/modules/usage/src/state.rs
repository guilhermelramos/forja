//! Shared usage summary, incremental cache and budget notice marks.

use crate::model::UsageSummary;
use crate::scan::{Cache, ScanStats};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    pub summary: UsageSummary,
    pub stats: ScanStats,
    pub pricing_errors: Vec<String>,
    pub error: Option<String>,
    pub busy: bool,
    pub last_scan: Option<Instant>,
    pub scanned_at_ms: Option<i64>,
    pub cache: Option<Cache>,
    budget_period: String,
    budget_notified: BTreeSet<String>,
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

    pub fn pricing_errors(&self) -> Vec<String> {
        self.0
            .lock()
            .map(|state| state.pricing_errors.clone())
            .unwrap_or_default()
    }

    pub fn scanned_at_ms(&self) -> Option<i64> {
        self.0.lock().ok().and_then(|state| state.scanned_at_ms)
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

    pub fn finish_scan(
        &self,
        cache: Cache,
        summary: UsageSummary,
        stats: ScanStats,
        pricing_errors: Vec<String>,
    ) {
        if let Ok(mut state) = self.0.lock() {
            state.cache = Some(cache);
            state.summary = summary;
            state.stats = stats;
            state.pricing_errors = pricing_errors;
            state.error = None;
            state.busy = false;
            state.last_scan = Some(Instant::now());
            state.scanned_at_ms = Some(crate::parse::now_ms());
        }
    }

    pub fn seed(&self, summary: UsageSummary) {
        if let Ok(mut state) = self.0.lock() {
            if state.summary.total.is_empty() {
                state.summary = summary;
                state.last_scan = Some(Instant::now());
                state.scanned_at_ms = Some(crate::parse::now_ms());
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

    /// Clears the per-threshold marks when the day or month turns over.
    pub fn sync_budget_period(&self, period: &str) {
        if let Ok(mut state) = self.0.lock() {
            if state.budget_period != period {
                state.budget_period = period.to_owned();
                state.budget_notified.clear();
            }
        }
    }

    /// Returns true the first time a threshold is marked in this period.
    pub fn mark_budget_notified(&self, key: &str) -> bool {
        self.0
            .lock()
            .map(|mut state| state.budget_notified.insert(key.to_owned()))
            .unwrap_or(false)
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
