//! Shared state for the Changes panel and the History view.

use crate::engine::StatusReport;
use crate::log::{BlameLine, LogLine};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Default)]
struct State {
    path: Option<String>,
    report: Option<StatusReport>,
    commits: Vec<LogLine>,
    log_for: Option<String>,
    blame: Vec<BlameLine>,
    blame_for: Option<(String, String)>,
    blame_file: String,
    selected_tab: usize,
    selected_commit: Option<String>,
    commit_detail: Option<(String, Result<String, String>)>,
    commit_detail_loading: Option<String>,
    commit_message: String,
    busy: bool,
    refreshing: bool,
    last_fetch: Option<Instant>,
    fetching: bool,
    last_refresh: Option<Instant>,
    error: Option<String>,
    notice: Option<String>,
    discard: Option<Vec<String>>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        let selected_tab = match std::env::var("FORJA_GIT_PANEL")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "history" | "historico" => 1,
            "blame" => 2,
            _ => 0,
        };
        let state = State {
            selected_tab,
            ..State::default()
        };
        Self(Arc::new(Mutex::new(state)))
    }

    pub fn path(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.path.clone())
    }

    pub fn set_path(&self, path: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            if state.path != path {
                state.path = path;
                state.report = None;
                state.commits.clear();
                state.log_for = None;
                state.selected_commit = None;
                state.commit_detail = None;
                state.commit_detail_loading = None;
                state.blame.clear();
                state.blame_for = None;
                state.last_refresh = None;
            }
        }
    }

    pub fn report(&self) -> Option<StatusReport> {
        self.0.lock().ok().and_then(|state| state.report.clone())
    }

    pub fn set_report(&self, report: StatusReport) {
        if let Ok(mut state) = self.0.lock() {
            state.report = Some(report);
            state.last_refresh = Some(Instant::now());
            state.refreshing = false;
        }
    }

    pub fn begin_refresh(&self, interval_secs: i64) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if state.refreshing {
            return false;
        }
        if let Some(last) = state.last_refresh {
            if interval_secs > 0 && last.elapsed().as_secs() < interval_secs as u64 {
                return false;
            }
        }
        state.refreshing = true;
        true
    }

    pub fn finish_refresh(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.refreshing = false;
            state.last_refresh = Some(Instant::now());
        }
    }

    pub fn begin_fetch(&self, interval_min: i64) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if state.fetching || interval_min <= 0 {
            return false;
        }
        if let Some(last) = state.last_fetch {
            if last.elapsed().as_secs() < (interval_min as u64) * 60 {
                return false;
            }
        }
        state.fetching = true;
        true
    }

    pub fn finish_fetch(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.fetching = false;
            state.last_fetch = Some(Instant::now());
        }
    }

    pub fn commits(&self) -> Vec<LogLine> {
        self.0
            .lock()
            .map(|state| state.commits.clone())
            .unwrap_or_default()
    }

    pub fn set_commits(&self, path: &str, commits: Vec<LogLine>) {
        if let Ok(mut state) = self.0.lock() {
            state.commits = commits;
            state.log_for = Some(path.to_owned());
        }
    }

    pub fn log_for(&self, path: &str) -> bool {
        self.0
            .lock()
            .map(|state| state.log_for.as_deref() == Some(path))
            .unwrap_or(false)
    }

    pub fn blame(&self) -> Vec<BlameLine> {
        self.0
            .lock()
            .map(|state| state.blame.clone())
            .unwrap_or_default()
    }

    pub fn set_blame(&self, path: &str, file: &str, blame: Vec<BlameLine>) {
        if let Ok(mut state) = self.0.lock() {
            state.blame = blame;
            state.blame_for = Some((path.to_owned(), file.to_owned()));
            state.blame_file = file.to_owned();
        }
    }

    pub fn blame_for(&self, path: &str, file: &str) -> bool {
        self.0
            .lock()
            .map(|state| {
                state
                    .blame_for
                    .as_ref()
                    .map(|(current_path, current_file)| {
                        current_path == path && current_file == file
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    pub fn blame_file(&self) -> String {
        self.0
            .lock()
            .map(|state| state.blame_file.clone())
            .unwrap_or_default()
    }

    pub fn set_blame_file(&self, file: String) {
        if let Ok(mut state) = self.0.lock() {
            state.blame_file = file;
        }
    }

    pub fn selected_tab(&self) -> usize {
        self.0.lock().map(|state| state.selected_tab).unwrap_or(0)
    }

    pub fn set_selected_tab(&self, tab: usize) {
        if let Ok(mut state) = self.0.lock() {
            state.selected_tab = tab;
        }
    }

    pub fn selected_commit(&self) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.selected_commit.clone())
    }

    pub fn set_selected_commit(&self, hash: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.selected_commit = hash;
        }
    }

    pub fn commit_detail(&self, hash: &str) -> Option<Result<String, String>> {
        self.0.lock().ok().and_then(|state| {
            state
                .commit_detail
                .as_ref()
                .filter(|(cached, _)| cached == hash)
                .map(|(_, detail)| detail.clone())
        })
    }

    pub fn commit_detail_loading(&self, hash: &str) -> bool {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.commit_detail_loading.clone())
            .as_deref()
            == Some(hash)
    }

    /// Reserves the load for `hash`; false when it is already loaded or loading.
    pub fn begin_commit_detail(&self, hash: &str) -> bool {
        let Ok(mut state) = self.0.lock() else {
            return false;
        };
        if state
            .commit_detail
            .as_ref()
            .is_some_and(|(cached, _)| cached == hash)
            || state.commit_detail_loading.as_deref() == Some(hash)
        {
            return false;
        }
        state.commit_detail_loading = Some(hash.to_owned());
        true
    }

    pub fn finish_commit_detail(&self, hash: &str, result: Result<String, String>) {
        if let Ok(mut state) = self.0.lock() {
            if state.commit_detail_loading.as_deref() == Some(hash) {
                state.commit_detail_loading = None;
            }
            state.commit_detail = Some((hash.to_owned(), result));
        }
    }

    pub fn commit_message(&self) -> String {
        self.0
            .lock()
            .map(|state| state.commit_message.clone())
            .unwrap_or_default()
    }

    pub fn set_commit_message(&self, message: String) {
        if let Ok(mut state) = self.0.lock() {
            state.commit_message = message;
        }
    }

    pub fn busy(&self) -> bool {
        self.0.lock().map(|state| state.busy).unwrap_or(false)
    }

    pub fn set_busy(&self, busy: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.busy = busy;
        }
    }

    pub fn error(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.error.clone())
    }

    pub fn notice(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.notice.clone())
    }

    pub fn set_error(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            let has_value = value.is_some();
            state.error = value;
            if has_value {
                state.busy = false;
            }
        }
    }

    pub fn set_notice(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            let has_value = value.is_some();
            state.notice = value;
            if has_value {
                state.busy = false;
            }
        }
    }

    pub fn discard(&self) -> Option<Vec<String>> {
        self.0.lock().ok().and_then(|state| state.discard.clone())
    }

    pub fn set_discard(&self, value: Option<Vec<String>>) {
        if let Ok(mut state) = self.0.lock() {
            state.discard = value;
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
