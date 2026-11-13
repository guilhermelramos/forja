//! Shared port snapshot and scan pacing.

use crate::model::Snapshot;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct State {
    pub snapshot: Snapshot,
    pub busy: bool,
    pub last_scan: Option<Instant>,
    pub pending_kill: Option<u32>,
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

    pub fn snapshot(&self) -> Snapshot {
        self.0
            .lock()
            .map(|state| state.snapshot.clone())
            .unwrap_or_default()
    }

    pub fn busy(&self) -> bool {
        self.0.lock().map(|state| state.busy).unwrap_or(false)
    }

    pub fn begin_scan(&self) -> bool {
        self.0
            .lock()
            .map(|mut state| {
                if state.busy {
                    return false;
                }
                state.busy = true;
                true
            })
            .unwrap_or(false)
    }

    pub fn finish_scan(&self, snapshot: Snapshot) {
        if let Ok(mut state) = self.0.lock() {
            state.snapshot = snapshot;
            state.busy = false;
            state.last_scan = Some(Instant::now());
        }
    }

    pub fn seed(&self, snapshot: Snapshot) {
        if let Ok(mut state) = self.0.lock() {
            if state.snapshot.ports.is_empty() {
                state.snapshot = snapshot;
                state.last_scan = Some(Instant::now());
            }
        }
    }

    pub fn due(&self, interval_s: u64) -> bool {
        self.0
            .lock()
            .map(|state| match state.last_scan {
                None => true,
                Some(_) if interval_s == 0 => false,
                Some(last) => last.elapsed() >= Duration::from_secs(interval_s),
            })
            .unwrap_or(false)
    }

    pub fn pending_kill(&self) -> Option<u32> {
        self.0.lock().ok().and_then(|state| state.pending_kill)
    }

    pub fn set_pending_kill(&self, pid: Option<u32>) {
        if let Ok(mut state) = self.0.lock() {
            state.pending_kill = pid;
        }
    }

    pub fn set_error(&self, message: String) {
        if let Ok(mut state) = self.0.lock() {
            state.snapshot.error = Some(message);
        }
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
