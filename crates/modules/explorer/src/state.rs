//! Shared state of the Files panel.
//!
//! The view and the commands both reach this state through [`Shared`]. Only the
//! interface thread locks it: worker threads receive copies of what they need,
//! and their results come back to the interface thread through kernel tasks.
//! Never hold the lock while calling `Ctx` (commands can lock it again).

use crate::ops::History;
use crate::tree::{IgnoreCache, TreeCache};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub type Shared = Arc<Mutex<State>>;

/// A small modal the panel shows for input that needs the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// Asks for a name for a new file or folder inside `dir`.
    NewEntry {
        dir: PathBuf,
        is_dir: bool,
        name: String,
        template: Option<String>,
    },
    /// Asks for a new name for `path`.
    Rename { path: PathBuf, name: String },
    /// Asks before `path` goes to the Recycle Bin.
    ConfirmTrash { path: PathBuf },
}

#[derive(Default)]
pub struct State {
    pub tree: TreeCache,
    pub ignores: IgnoreCache,
    pub expanded: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
    pub bench_name: Option<String>,
    pub history: History,
    /// True while one disk operation runs. Other operations wait for it.
    pub busy: bool,
    pub dialog: Option<Dialog>,
    /// One-line status shown under the toolbar.
    pub notice: Option<String>,
    /// Text the view copies to the clipboard on its next frame.
    pub pending_copy: Option<String>,
    /// Bumped when the watched root changes, so old watcher threads stop.
    pub watch_generation: u64,
    /// Keeps the OS watcher alive. Dropping it stops the events.
    pub watcher: Option<notify::RecommendedWatcher>,
    /// Last time the active bench was read, to avoid asking on every frame.
    pub bench_checked: Option<Instant>,
}

/// Locks the state, recovering from a poisoned mutex (the data is still usable).
pub fn lock(shared: &Shared) -> MutexGuard<'_, State> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_recovers_from_poison() {
        let shared: Shared = Arc::new(Mutex::new(State::default()));
        let clone = Arc::clone(&shared);
        let _ = std::thread::spawn(move || {
            let _guard = clone.lock().unwrap();
            panic!("poison the mutex");
        })
        .join();
        lock(&shared).notice = Some("still usable".into());
        assert_eq!(lock(&shared).notice.as_deref(), Some("still usable"));
    }
}
