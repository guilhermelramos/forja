//! Shared state of the finder. Only the interface thread locks it; worker threads
//! get copies of what they need. Never hold the lock while calling `Ctx`.

use crate::history::History;
use crate::index::Index;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub type Shared = Arc<Mutex<State>>;

/// One result list (the picker or the Anvil source) and the query that produced it.
#[derive(Clone, Debug, Default)]
pub struct Results {
    pub query: String,
    /// Bumped for each ranking request. A late answer with an older number is dropped.
    pub seq: u64,
    pub items: Vec<String>,
    pub pending: bool,
    pub selected: usize,
}

/// Which result list a request refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Picker,
    Anvil,
}

impl Which {
    pub fn slot(self, state: &mut State) -> &mut Results {
        match self {
            Self::Picker => &mut state.picker,
            Self::Anvil => &mut state.anvil,
        }
    }
}

#[derive(Default)]
pub struct State {
    pub root: Option<PathBuf>,
    pub bench_checked: Option<Instant>,
    pub index: Option<Arc<Index>>,
    pub indexing: bool,
    pub indexed_at: Option<Instant>,
    pub history: History,
    pub history_loaded: bool,
    /// The floating picker is open.
    pub open: bool,
    /// Ask the picker's input to take focus on its next frame.
    pub focus_query: bool,
    pub picker: Results,
    pub anvil: Results,
    pub notice: Option<String>,
}

/// Locks the state, recovering from a poisoned mutex (the data is still usable).
pub fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
