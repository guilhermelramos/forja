//! Shared browser tabs, navigation requests and history.

use crate::history::History;
use std::sync::{Arc, Mutex};

pub const MAX_TABS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Url,
    Html,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub kind: Kind,
    pub html: Option<String>,
    pub zoom: i32,
    pub loading: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub error: Option<String>,
}

impl Tab {
    pub fn new(id: u64, url: &str, kind: Kind, html: Option<String>) -> Self {
        Self {
            id,
            url: url.to_owned(),
            title: if url.is_empty() {
                "Nova página".to_owned()
            } else {
                url.to_owned()
            },
            kind,
            html,
            zoom: 0,
            loading: true,
            can_back: false,
            can_forward: false,
            error: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Navigate { url: String, html: Option<String> },
    Back,
    Forward,
    Reload,
    HardReload,
    Find { query: String, backwards: bool },
    Zoom,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub pending: Option<Pending>,
    pub history: History,
    pub history_open: bool,
    pub address_focus: bool,
    pub find_open: bool,
    pub find_query: String,
    pub notice: Option<String>,
    pub error: Option<String>,
    pub closed: Vec<(String, Kind, Option<String>)>,
    pub next_id: u64,
    pub downloads: Vec<String>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new(history: History) -> Self {
        Self(Arc::new(Mutex::new(State {
            history,
            next_id: 1,
            ..State::default()
        })))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> Option<R> {
        self.0.lock().ok().map(|mut state| f(&mut state))
    }

    pub fn tabs(&self) -> Vec<Tab> {
        self.0
            .lock()
            .map(|state| state.tabs.clone())
            .unwrap_or_default()
    }

    pub fn active(&self) -> Option<Tab> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.tabs.get(state.active).cloned())
    }

    pub fn set_active(&self, index: usize) {
        if let Ok(mut state) = self.0.lock() {
            if index < state.tabs.len() {
                state.active = index;
            }
        }
    }

    pub fn open(&self, url: &str, html: Option<String>, new_tab: bool) {
        if let Ok(mut state) = self.0.lock() {
            if new_tab || state.tabs.is_empty() {
                let id = state.next_id;
                state.next_id = state.next_id.wrapping_add(1);
                state.tabs.push(Tab::new(
                    id,
                    url,
                    if html.is_some() {
                        Kind::Html
                    } else {
                        Kind::Url
                    },
                    html.clone(),
                ));
                state.active = state.tabs.len() - 1;
                if state.tabs.len() > MAX_TABS {
                    let removed = state.tabs.remove(0);
                    state.active = state.active.saturating_sub(1);
                    state.closed.push((removed.url, removed.kind, removed.html));
                }
            } else {
                let active = state.active;
                if let Some(tab) = state.tabs.get_mut(active) {
                    tab.url = url.to_owned();
                    tab.html = html.clone();
                    tab.kind = if html.is_some() {
                        Kind::Html
                    } else {
                        Kind::Url
                    };
                    tab.loading = true;
                }
            }
            state.pending = Some(Pending::Navigate {
                url: url.to_owned(),
                html,
            });
            state.error = None;
        }
    }

    pub fn request(&self, pending: Pending) {
        if let Ok(mut state) = self.0.lock() {
            state.pending = Some(pending);
        }
    }

    pub fn take_pending(&self) -> Option<Pending> {
        self.0
            .lock()
            .ok()
            .and_then(|mut state| state.pending.take())
    }

    pub fn close(&self, index: usize) {
        if let Ok(mut state) = self.0.lock() {
            if index < state.tabs.len() {
                let removed = state.tabs.remove(index);
                state.closed.push((removed.url, removed.kind, removed.html));
                if state.closed.len() > 20 {
                    state.closed.remove(0);
                }
                if state.active >= state.tabs.len() {
                    state.active = state.tabs.len().saturating_sub(1);
                }
            }
        }
    }

    pub fn reopen(&self) {
        if let Ok(mut state) = self.0.lock() {
            if let Some((url, kind, html)) = state.closed.pop() {
                let id = state.next_id;
                state.next_id = state.next_id.wrapping_add(1);
                state.tabs.push(Tab::new(id, &url, kind, html));
                state.active = state.tabs.len() - 1;
                if state.tabs.len() > MAX_TABS {
                    state.tabs.remove(0);
                    state.active = state.active.saturating_sub(1);
                }
            }
        }
    }

    pub fn set_address_focus(&self, focus: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.address_focus = focus;
        }
    }

    pub fn take_address_focus(&self) -> bool {
        self.0
            .lock()
            .ok()
            .map(|mut state| std::mem::take(&mut state.address_focus))
            .unwrap_or(false)
    }

    pub fn set_find(&self, open: bool, query: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.find_open = open;
            if let Some(query) = query {
                state.find_query = query;
            }
        }
    }

    pub fn find_open(&self) -> bool {
        self.0.lock().map(|state| state.find_open).unwrap_or(false)
    }

    pub fn find_query(&self) -> String {
        self.0
            .lock()
            .map(|state| state.find_query.clone())
            .unwrap_or_default()
    }

    pub fn history_open(&self) -> bool {
        self.0
            .lock()
            .map(|state| state.history_open)
            .unwrap_or(false)
    }

    pub fn toggle_history(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.history_open = !state.history_open;
        }
    }

    pub fn set_history_open(&self, open: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.history_open = open;
        }
    }

    pub fn history(&self) -> History {
        self.0
            .lock()
            .map(|state| state.history.clone())
            .unwrap_or_default()
    }

    pub fn add_history(&self, url: &str, title: &str) {
        if let Ok(mut state) = self.0.lock() {
            state.history.add(url, title);
        }
    }

    pub fn clear_history(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.history.clear();
        }
    }

    pub fn set_notice(&self, notice: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.notice = notice;
        }
    }

    pub fn set_error(&self, error: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.error = error;
        }
    }

    pub fn update_tab(&self, id: u64, f: impl FnOnce(&mut Tab)) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.id == id) {
                f(tab);
            }
        }
    }

    pub fn add_downloads(&self, paths: Vec<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.downloads.extend(paths);
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new(History::default())
    }
}
