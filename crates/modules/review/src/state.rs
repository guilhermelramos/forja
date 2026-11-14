//! Shared state for the Review tab.

use crate::api::Target;
use crate::parse::{Check, PullRequest, Thread};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generated {
    pub kind: String,
    pub text: String,
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub target: Target,
    pub branch: String,
    pub base: String,
    pub pr: Option<PullRequest>,
    pub checks: Vec<Check>,
    pub threads: Vec<Thread>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Comparison {
    pub branch: String,
    pub head: String,
    pub base: Option<String>,
    pub base_commit: Option<String>,
    pub remote: bool,
    #[serde(default)]
    pub branches: Vec<String>,
    pub ahead: u32,
    #[serde(default)]
    pub commits: Vec<ComparisonCommit>,
    #[serde(default)]
    pub files: Vec<ComparisonFile>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ComparisonCommit {
    pub short: String,
    pub title: String,
    pub author: String,
    pub time: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ComparisonFile {
    pub path: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Default, Clone)]
pub struct State {
    pub bench: Option<String>,
    pub target: Option<Target>,
    pub pr: Option<PullRequest>,
    pub checks: Vec<Check>,
    pub threads: Vec<Thread>,
    pub logs: BTreeMap<String, String>,
    pub branch: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
    pub reviewers: String,
    pub labels: String,
    pub merge_method: usize,
    pub delete_branch: bool,
    pub auto_merge: bool,
    pub reply_drafts: BTreeMap<String, String>,
    pub status: Option<String>,
    pub notice: Option<String>,
    pub busy: bool,
    pub loading: bool,
    pub generated: Option<Generated>,
    pub comparison: Option<Comparison>,
    pub comparison_loading: bool,
    pub comparison_error: Option<String>,
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

    pub fn apply(&self, snapshot: Snapshot) {
        if let Ok(mut state) = self.0.lock() {
            state.target = Some(snapshot.target);
            state.branch = snapshot.branch.clone();
            if state.base.is_empty() {
                state.base = snapshot.base.clone();
            }
            state.pr = snapshot.pr.clone();
            state.checks = snapshot.checks;
            state.threads = snapshot.threads;
            if let Some(pr) = snapshot.pr {
                state.title = pr.title;
                state.body = pr.body;
                if !pr.target.is_empty() {
                    state.base = pr.target;
                }
            }
            state.loading = false;
            state.busy = false;
            state.status = None;
        }
    }

    pub fn set_bench(&self, bench: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            if state.bench != bench {
                state.target = None;
                state.pr = None;
                state.checks.clear();
                state.threads.clear();
                state.logs.clear();
                state.branch.clear();
                state.base.clear();
                state.comparison = None;
                state.comparison_loading = false;
                state.comparison_error = None;
                state.loading = false;
            }
            state.bench = bench;
        }
    }

    pub fn bench(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.bench.clone())
    }

    pub fn set_status(&self, status: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.status = status;
            state.busy = false;
            state.loading = false;
        }
    }

    pub fn set_notice(&self, notice: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.notice = notice;
            state.busy = false;
        }
    }

    pub fn set_loading(&self, loading: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.loading = loading;
            state.busy = loading;
            state.status = None;
        }
    }

    pub fn set_comparison_loading(&self, loading: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.comparison_loading = loading;
            state.comparison_error = None;
        }
    }

    pub fn set_comparison(&self, comparison: Comparison) {
        if let Ok(mut state) = self.0.lock() {
            state.branch = comparison.branch.clone();
            state.base = comparison.base.clone().unwrap_or_default();
            state.comparison = Some(comparison);
            state.comparison_loading = false;
            state.comparison_error = None;
        }
    }

    pub fn set_comparison_error(&self, error: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.comparison = None;
            state.comparison_error = error;
            state.comparison_loading = false;
        }
    }

    pub fn set_generated(&self, generated: Option<Generated>) {
        if let Ok(mut state) = self.0.lock() {
            state.generated = generated;
            state.busy = false;
        }
    }

    pub fn add_log(&self, id: String, log: String) {
        if let Ok(mut state) = self.0.lock() {
            state.logs.insert(id, log);
        }
    }

    pub fn set_threads(&self, threads: Vec<Thread>) {
        if let Ok(mut state) = self.0.lock() {
            state.threads = threads;
        }
    }

    pub fn set_pr(&self, pr: PullRequest) {
        if let Ok(mut state) = self.0.lock() {
            state.title = pr.title.clone();
            state.body = pr.body.clone();
            state.pr = Some(pr);
        }
    }

    pub fn mark_merged(&self) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(pr) = &mut state.pr {
                pr.state = "merged".to_owned();
            }
        }
    }

    pub fn reply_draft(&self, thread: &str) -> String {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.reply_drafts.get(thread).cloned())
            .unwrap_or_default()
    }

    pub fn set_reply_draft(&self, thread: &str, text: String) {
        if let Ok(mut state) = self.0.lock() {
            state.reply_drafts.insert(thread.to_owned(), text);
        }
    }

    pub fn clear_reply(&self, thread: &str) {
        if let Ok(mut state) = self.0.lock() {
            state.reply_drafts.remove(thread);
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
