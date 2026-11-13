//! Shared state for the viewer tab.

use forja_contracts::ViewerDocument;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    path: Option<String>,
    document: Option<ViewerDocument>,
    page: usize,
    zoom: f32,
    notice: Option<String>,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    pub fn path(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.path.clone())
    }

    pub fn document(&self) -> Option<ViewerDocument> {
        self.0.lock().ok().and_then(|state| state.document.clone())
    }

    pub fn set_request(&self, path: String) {
        if let Ok(mut state) = self.0.lock() {
            if state.path.as_deref() != Some(path.as_str()) {
                state.path = Some(path);
                state.document = None;
                state.page = 0;
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut state) = self.0.lock() {
            state.path = None;
            state.document = None;
            state.page = 0;
        }
    }

    pub fn set_document(&self, document: ViewerDocument) {
        if let Ok(mut state) = self.0.lock() {
            state.path = Some(document.path.clone());
            state.document = Some(document);
            state.page = 0;
        }
    }

    pub fn page(&self) -> usize {
        self.0.lock().map(|state| state.page).unwrap_or(0)
    }

    pub fn set_page(&self, page: usize) {
        if let Ok(mut state) = self.0.lock() {
            state.page = page;
        }
    }

    pub fn zoom(&self) -> f32 {
        self.0
            .lock()
            .map(|state| if state.zoom <= 0.0 { 1.0 } else { state.zoom })
            .unwrap_or(1.0)
    }

    pub fn add_zoom(&self, delta: f32) {
        if let Ok(mut state) = self.0.lock() {
            state.zoom = (state.zoom.max(1.0) + delta).clamp(0.25, 4.0);
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
