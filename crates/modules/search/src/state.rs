//! Shared state of the search panel. Only the interface thread locks it.
//! Worker threads and tasks get copies of what they need and send results back
//! through channels or task callbacks. Never hold the lock while calling `Ctx`.

use crate::engine::FileHits;
use crate::query::SearchOptions;
use crate::replace::ReplacePlan;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub type Shared = Arc<Mutex<State>>;

/// Everything the user typed or toggled in the panel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Form {
    pub pattern: String,
    pub replacement: String,
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include: String,
    pub exclude: String,
}

impl Form {
    pub fn options(&self) -> SearchOptions {
        SearchOptions {
            pattern: self.pattern.clone(),
            regex: self.regex,
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            include: self.include.clone(),
            exclude: self.exclude.clone(),
        }
    }
}

#[derive(Default)]
pub struct State {
    pub root: Option<PathBuf>,
    pub bench_checked: Option<Instant>,
    pub form: Form,
    /// Bumped for each search. Messages from an older search are dropped.
    pub generation: u64,
    /// Set to stop the walk of the current search.
    pub cancel: Option<Arc<AtomicBool>>,
    pub running: bool,
    pub files: Vec<FileHits>,
    pub matches: usize,
    pub scanned: usize,
    pub truncated: bool,
    pub error: Option<String>,
    pub searched: bool,
    /// Replacement preview, and the form it was made from.
    pub plan: Option<ReplacePlan>,
    pub plan_form: Option<Form>,
    pub plan_error: Option<String>,
    pub planning: bool,
    /// The user pressed "Apply" once; the next press writes.
    pub confirm: bool,
    pub applying: bool,
    pub notice: Option<String>,
    pub focus_query: bool,
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
    fn form_builds_the_search_options() {
        let form = Form {
            pattern: "x".into(),
            regex: true,
            include: "*.rs".into(),
            ..Form::default()
        };
        let options = form.options();
        assert!(options.regex && !options.case_sensitive);
        assert_eq!(options.include, "*.rs");
    }
}
