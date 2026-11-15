//! Board preferences and the cached view of the benches module.

use forja_contracts::BenchCard;
use forja_kernel::Ctx;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(default)]
    pub filter: String,
    #[serde(default)]
    pub show_archived: bool,
    #[serde(default = "default_true")]
    pub pinned_first: bool,
}

fn default_group() -> String {
    "project".to_owned()
}
fn default_sort() -> String {
    "recent".to_owned()
}
fn default_true() -> bool {
    true
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            group: default_group(),
            sort: default_sort(),
            filter: String::new(),
            show_archived: false,
            pinned_first: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardFile {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub prefs: Prefs,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageColumn {
    pub id: String,
    pub label: String,
    pub order: i32,
    pub parent: String,
    pub dot: String,
}

#[derive(Clone, Debug)]
pub struct PendingMove {
    pub bench_id: String,
    pub bench_name: String,
    pub to: String,
    pub to_label: String,
}

#[derive(Default)]
struct State {
    loaded: bool,
    prefs: Prefs,
    active_project: Option<String>,
    cards: Vec<BenchCard>,
    stages: Vec<StageColumn>,
    names: BTreeMap<String, String>,
    /// Helper status per bench id, as snake_case `HelperStatus` values.
    helpers: BTreeMap<String, String>,
    error: Option<String>,
    dragging: Option<String>,
    pending: Option<PendingMove>,
    last_refresh: Option<Instant>,
    refresh_requested: bool,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    pub fn ensure_loaded(&self, cx: &Ctx<'_>) {
        let mut state = match self.0.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if state.loaded {
            return;
        }
        state.loaded = true;
        match cx.store().load::<BoardFile>() {
            forja_kernel::LoadResult::Loaded(file) => state.prefs = file.prefs,
            forja_kernel::LoadResult::Quarantined(path) => {
                state.error = Some(format!(
                    "board file was corrupted and moved to {}",
                    path.display()
                ));
            }
            forja_kernel::LoadResult::Missing => {}
        }
    }

    pub fn prefs(&self) -> Prefs {
        self.0
            .lock()
            .map(|state| state.prefs.clone())
            .unwrap_or_default()
    }

    pub fn cards(&self) -> Vec<BenchCard> {
        self.0
            .lock()
            .map(|state| state.cards.clone())
            .unwrap_or_default()
    }

    pub fn stages(&self) -> Vec<StageColumn> {
        self.0
            .lock()
            .map(|state| state.stages.clone())
            .unwrap_or_default()
    }

    pub fn names(&self) -> BTreeMap<String, String> {
        self.0
            .lock()
            .map(|state| state.names.clone())
            .unwrap_or_default()
    }

    /// Helper status of a bench, when a helper run is known for it.
    pub fn helper_status(&self, bench_id: &str) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.helpers.get(bench_id).cloned())
    }

    pub fn error(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.error.clone())
    }

    pub fn set_error(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.error = value;
        }
    }

    pub fn active_project(&self) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.active_project.clone())
    }

    pub fn set_active_project(&self, project_id: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.active_project = project_id;
        }
    }

    pub fn set_data(
        &self,
        cards: Vec<BenchCard>,
        stages: Vec<StageColumn>,
        names: BTreeMap<String, String>,
        helpers: BTreeMap<String, String>,
    ) {
        if let Ok(mut state) = self.0.lock() {
            state.cards = cards;
            state.stages = stages;
            state.names = names;
            state.helpers = helpers;
            state.error = None;
            state.last_refresh = Some(Instant::now());
            state.refresh_requested = false;
        }
    }

    pub fn needs_refresh(&self, interval_secs: i64) -> bool {
        let Ok(state) = self.0.lock() else {
            return false;
        };
        if state.refresh_requested || state.last_refresh.is_none() {
            return true;
        }
        if interval_secs <= 0 {
            return false;
        }
        state
            .last_refresh
            .map(|last| last.elapsed() >= Duration::from_secs(interval_secs as u64))
            .unwrap_or(true)
    }

    pub fn request_refresh(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.refresh_requested = true;
        }
    }

    pub fn update_prefs(
        &self,
        cx: &Ctx<'_>,
        change: impl FnOnce(&mut Prefs),
    ) -> Result<(), String> {
        let snapshot = {
            let mut state = self.0.lock().map_err(|_| "board state is poisoned")?;
            change(&mut state.prefs);
            state.prefs.clone()
        };
        cx.store()
            .save(&BoardFile {
                schema_version: SCHEMA_VERSION,
                prefs: snapshot,
            })
            .map_err(|error| format!("could not save board preferences: {error}"))
    }

    pub fn dragging(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.dragging.clone())
    }

    pub fn set_dragging(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.dragging = value;
        }
    }

    pub fn pending(&self) -> Option<PendingMove> {
        self.0.lock().ok().and_then(|state| state.pending.clone())
    }

    pub fn set_pending(&self, value: Option<PendingMove>) {
        if let Ok(mut state) = self.0.lock() {
            state.pending = value;
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
