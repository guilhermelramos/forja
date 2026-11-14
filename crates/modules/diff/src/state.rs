//! Shared state for the diff tab.

use crate::model::{Changes, FileDiff, Note};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Split,
    Unified,
    Combined,
}

impl Mode {
    pub fn from_setting(value: &str) -> Self {
        match value {
            "unified" => Self::Unified,
            "combined" => Self::Combined,
            _ => Self::Split,
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Split => "diff.mode.split",
            Self::Unified => "diff.mode.unified",
            Self::Combined => "diff.mode.combined",
        }
    }
}

#[derive(Default)]
pub struct DiffState {
    pub bench: Option<String>,
    pub changes: Option<Changes>,
    pub selected: Option<String>,
    pub staged: bool,
    pub diff: Option<FileDiff>,
    pub notes: Vec<Note>,
    pub note_line: Option<u32>,
    pub note_draft: String,
    pub status: Option<String>,
    pub notice: Option<String>,
    pub filter: String,
    pub version: u64,
    pub stale: bool,
    pub comparison: Option<String>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<DiffState>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(DiffState::default())))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut DiffState) -> R) -> Option<R> {
        self.0.lock().ok().map(|mut state| f(&mut state))
    }

    pub fn bench(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.bench.clone())
    }

    pub fn set_bench(&self, bench: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            if state.bench != bench {
                state.changes = None;
                state.diff = None;
                state.selected = None;
                state.comparison = None;
            }
            state.bench = bench;
        }
    }

    pub fn set_changes(&self, changes: Changes) {
        if let Ok(mut state) = self.0.lock() {
            state.changes = Some(changes);
        }
    }

    pub fn set_comparison(&self, comparison: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.comparison = comparison;
            state.changes = None;
            state.diff = None;
            state.selected = None;
            state.stale = true;
        }
    }

    pub fn comparison(&self) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.comparison.clone())
    }

    pub fn set_diff(&self, diff: FileDiff) {
        if let Ok(mut state) = self.0.lock() {
            state.selected = Some(diff.path.clone());
            state.staged = diff.staged;
            state.diff = Some(diff);
            state.version = state.version.wrapping_add(1);
        }
    }

    pub fn version(&self) -> u64 {
        self.0.lock().map(|state| state.version).unwrap_or(0)
    }

    pub fn set_stale(&self, stale: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.stale = stale;
        }
    }

    pub fn stale(&self) -> bool {
        self.0.lock().map(|state| state.stale).unwrap_or(false)
    }

    pub fn add_note(&self, path: &str, line: u32, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        if let Ok(mut state) = self.0.lock() {
            state.notes.push(Note::new(path, line, text));
            state.note_line = None;
            state.note_draft.clear();
        }
    }

    pub fn notes(&self) -> Vec<Note> {
        self.0
            .lock()
            .map(|state| state.notes.clone())
            .unwrap_or_default()
    }

    pub fn notes_for(&self, path: &str) -> Vec<Note> {
        self.0
            .lock()
            .map(|state| {
                state
                    .notes
                    .iter()
                    .filter(|note| note.path == path)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn clear_notes(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.notes.clear();
        }
    }

    pub fn status(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.status.clone())
    }

    pub fn set_status(&self, status: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.status = status;
        }
    }

    pub fn notice(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.notice.clone())
    }

    pub fn set_notice(&self, notice: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.notice = notice;
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
