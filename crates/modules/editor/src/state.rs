//! Editor tabs, find state and save/compare plumbing.

use crate::model::{Eol, Encoding, FindState};
use forja_contracts::ViewerDocument;
use forja_kernel::ui_contract::egui;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

/// A definition shown by `editor.peek.definition`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Peek {
    pub word: String,
    pub line: usize,
    pub text: String,
}

/// Candidates from `editor.completion`, plus the word range they replace.
#[derive(Clone, Debug)]
pub struct Completion {
    pub start: usize,
    pub end: usize,
    pub candidates: Vec<String>,
}

/// Signature shown by `editor.parameter_hint`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterHint {
    pub line: usize,
    pub text: String,
}

/// The inline rename prompt opened by `editor.rename`.
#[derive(Clone, Debug)]
pub struct RenameState {
    pub word: String,
    pub text: String,
}

/// A font read requested by the view and delivered by a task.
#[derive(Debug)]
pub struct FontLoad {
    pub choice: String,
    pub bytes: Option<Vec<u8>>,
}

/// One open file. Preview tabs (`pinned == false`) are reused by the next
/// single-click open; editing or a double click pins them.
#[derive(Clone)]
pub struct Tab {
    pub path: String,
    pub title: String,
    pub content: String,
    pub saved: String,
    pub eol: Eol,
    pub encoding: Encoding,
    pub dirty: bool,
    pub pinned: bool,
    pub preview: Option<ViewerDocument>,
    pub preview_mode: bool,
    pub external_changed: bool,
    pub last_edit: Option<Instant>,
    pub last_mtime: Option<SystemTime>,
    pub wrap: bool,
    pub notice: Option<String>,
    /// Snapshots before each edit, oldest first (`editor.undo`).
    pub undo: Vec<String>,
    /// Snapshots taken by `editor.undo` so `editor.redo` can replay them.
    pub redo: Vec<String>,
}

/// The editor keeps at most this many snapshots per tab.
pub const UNDO_LIMIT: usize = 200;

impl Tab {
    pub fn new(path: String, content: String, eol: Eol, encoding: Encoding, pinned: bool) -> Self {
        let title = Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&path)
            .to_owned();
        let mtime = std::fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok();
        Self {
            path,
            title,
            saved: content.clone(),
            content,
            eol,
            encoding,
            dirty: false,
            pinned,
            preview: None,
            preview_mode: false,
            external_changed: false,
            last_edit: None,
            last_mtime: mtime,
            wrap: true,
            notice: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = self.content != self.saved;
        self.last_edit = Some(Instant::now());
    }

    /// Records the buffer as it was before an edit. Consecutive identical
    /// snapshots collapse, and the oldest one falls off past [`UNDO_LIMIT`].
    pub fn record_undo(&mut self, previous: String) {
        if self.undo.last() == Some(&previous) {
            return;
        }
        self.undo.push(previous);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Restores the previous snapshot; returns false when nothing is left.
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.content, previous);
        self.redo.push(current);
        self.pinned = true;
        self.mark_dirty();
        true
    }

    /// Replays a snapshot taken by [`Self::undo`].
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.content, next);
        self.undo.push(current);
        self.pinned = true;
        self.mark_dirty();
        true
    }

    pub fn reload(&mut self, content: String, eol: Eol, encoding: Encoding) {
        self.content = content.clone();
        self.saved = content;
        self.eol = eol;
        self.encoding = encoding;
        self.dirty = false;
        self.external_changed = false;
        self.last_mtime = std::fs::metadata(&self.path)
            .and_then(|meta| meta.modified())
            .ok();
    }
}

#[derive(Default)]
pub struct EditorState {
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub find: FindState,
    pub status: Option<String>,
    pub compare: Option<(String, String)>,
    pub pending_line: Option<usize>,
    /// Set by `editor.find.next`/`previous` so the view moves the caret to the
    /// current match on its next frame.
    pub pending_match: bool,
    /// Last line that had the caret, kept so commands can act on it without
    /// asking the view for the selection.
    pub cursor_line: usize,
    /// Caret position in characters, updated by the view every frame.
    pub cursor_char: usize,
    /// Selected character range `(start, end)` while the text area has focus.
    pub selection: Option<(usize, usize)>,
    /// Egui context captured by the view so commands can use the clipboard.
    pub ui_context: Option<egui::Context>,
    /// Font bytes read by a task and waiting to be installed.
    pub font_load: Option<FontLoad>,
    /// Set by `editor.select_all`; the view applies it on the next frame.
    pub pending_select_all: bool,
    pub peek: Option<Peek>,
    pub completion: Option<Completion>,
    pub parameter_hint: Option<ParameterHint>,
    pub rename: Option<RenameState>,
    /// Set by `editor.quick_fix`; the view draws the available fixes.
    pub quick_fix: bool,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<EditorState>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(EditorState::default())))
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut EditorState) -> R) -> Option<R> {
        self.0.lock().ok().map(|mut state| f(&mut state))
    }

    pub fn active_tab(&self) -> Option<Tab> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.tabs.get(state.active).cloned())
    }

    pub fn tabs(&self) -> Vec<Tab> {
        self.0
            .lock()
            .map(|state| state.tabs.clone())
            .unwrap_or_default()
    }

    pub fn active_index(&self) -> usize {
        self.0.lock().map(|state| state.active).unwrap_or(0)
    }

    pub fn set_active(&self, index: usize) {
        if let Ok(mut state) = self.0.lock() {
            if index < state.tabs.len() {
                state.active = index;
            }
        }
    }

    /// Opens a file. A non-pinned open reuses the last preview tab, matching a
    /// single click in the explorer; a pinned open (double click, command)
    /// keeps the tab.
    pub fn open(
        &self,
        path: String,
        content: String,
        eol: Eol,
        encoding: Encoding,
        pinned: bool,
    ) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(index) = state
                .tabs
                .iter()
                .position(|tab| tab.path.eq_ignore_ascii_case(&path))
            {
                if let Some(tab) = state.tabs.get_mut(index) {
                    tab.pinned |= pinned;
                }
                state.active = index;
                return;
            }
            if !pinned {
                if let Some(index) = state.tabs.iter().position(|tab| !tab.pinned) {
                    state.tabs.remove(index);
                    if state.active > index {
                        state.active -= 1;
                    }
                }
            }
            state
                .tabs
                .push(Tab::new(path, content, eol, encoding, pinned));
            state.active = state.tabs.len() - 1;
        }
    }

    pub fn close(&self, index: usize) {
        if let Ok(mut state) = self.0.lock() {
            if index < state.tabs.len() {
                state.tabs.remove(index);
                if state.active >= state.tabs.len() {
                    state.active = state.tabs.len().saturating_sub(1);
                }
            }
        }
    }

    /// Stores view edits. Editing a preview tab pins it.
    pub fn set_content(&self, path: &str, content: String) -> bool {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                if tab.content != content {
                    tab.content = content;
                    tab.pinned = true;
                    tab.mark_dirty();
                    return true;
                }
            }
        }
        false
    }

    /// Remembers the buffer before an edit so `editor.undo` can restore it.
    pub fn record_undo(&self, path: &str, previous: String) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.record_undo(previous);
            }
        }
    }

    pub fn undo(&self, path: &str) -> bool {
        self.0
            .lock()
            .map(|mut state| {
                state
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.path == path)
                    .is_some_and(|tab| tab.undo())
            })
            .unwrap_or(false)
    }

    pub fn redo(&self, path: &str) -> bool {
        self.0
            .lock()
            .map(|mut state| {
                state
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.path == path)
                    .is_some_and(|tab| tab.redo())
            })
            .unwrap_or(false)
    }

    /// Records the caret and selection, in characters.
    pub fn set_cursor(&self, cursor_char: usize, selection: Option<(usize, usize)>) {
        if let Ok(mut state) = self.0.lock() {
            state.cursor_char = cursor_char;
            state.selection = selection.filter(|(start, end)| start != end);
        }
    }

    pub fn cursor_char(&self) -> usize {
        self.0.lock().map(|state| state.cursor_char).unwrap_or(0)
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        self.0.lock().ok().and_then(|state| state.selection)
    }

    /// Keeps the egui context around so commands can copy text.
    pub fn set_ui_context(&self, context: egui::Context) {
        if let Ok(mut state) = self.0.lock() {
            state.ui_context = Some(context);
        }
    }

    pub fn ui_context(&self) -> Option<egui::Context> {
        self.0.lock().ok().and_then(|state| state.ui_context.clone())
    }

    pub fn store_font(&self, choice: String, bytes: Option<Vec<u8>>) {
        if let Ok(mut state) = self.0.lock() {
            state.font_load = Some(FontLoad { choice, bytes });
        }
    }

    pub fn take_font(&self) -> Option<FontLoad> {
        self.0.lock().ok().and_then(|mut state| state.font_load.take())
    }

    pub fn mark_saved(&self, path: &str, saved: String, mtime: Option<SystemTime>) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.saved = saved;
                tab.dirty = false;
                tab.last_mtime = mtime;
                tab.external_changed = false;
            }
        }
    }

    pub fn set_preview(&self, path: &str, preview: Option<ViewerDocument>) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.preview = preview;
                tab.preview_mode = true;
            }
        }
    }

    pub fn set_preview_mode(&self, path: &str, visible: bool) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.preview_mode = visible;
            }
        }
    }

    pub fn set_external(&self, path: &str, changed: bool) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.external_changed = changed;
            }
        }
    }

    pub fn set_wrap(&self, path: &str, wrap: bool) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(tab) = state.tabs.iter_mut().find(|tab| tab.path == path) {
                tab.wrap = wrap;
            }
        }
    }

    pub fn tab(&self, path: &str) -> Option<Tab> {
        self.0.lock().ok().and_then(|state| {
            state
                .tabs
                .iter()
                .find(|tab| tab.path == path)
                .cloned()
        })
    }

    pub fn status(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.status.clone())
    }

    pub fn set_status(&self, status: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.status = status;
        }
    }

    pub fn set_cursor_line(&self, line: usize) {
        if let Ok(mut state) = self.0.lock() {
            state.cursor_line = line.max(1);
        }
    }

    pub fn cursor_line(&self) -> usize {
        self.0
            .lock()
            .map(|state| state.cursor_line.max(1))
            .unwrap_or(1)
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared_with_tab() -> (Shared, String) {
        let shared = Shared::new();
        let path = "C:\\temp\\nota.txt".to_owned();
        shared.open(
            path.clone(),
            "um\n".to_owned(),
            Eol::Lf,
            Encoding::Utf8,
            true,
        );
        (shared, path)
    }

    #[test]
    fn undo_and_redo_walk_the_snapshots() {
        let (shared, path) = shared_with_tab();
        shared.record_undo(&path, "um\n".to_owned());
        shared.set_content(&path, "um\ndois\n".to_owned());
        shared.record_undo(&path, "um\ndois\n".to_owned());
        shared.set_content(&path, "um\ndois\ntres\n".to_owned());
        assert!(shared.undo(&path));
        assert_eq!(shared.tab(&path).unwrap().content, "um\ndois\n");
        assert!(shared.undo(&path));
        assert_eq!(shared.tab(&path).unwrap().content, "um\n");
        assert!(!shared.undo(&path));
        assert!(shared.redo(&path));
        assert_eq!(shared.tab(&path).unwrap().content, "um\ndois\n");
        assert!(shared.tab(&path).unwrap().dirty);
    }

    #[test]
    fn undo_limit_keeps_the_most_recent_snapshots() {
        let mut tab = Tab::new(
            "a.txt".to_owned(),
            String::new(),
            Eol::Lf,
            Encoding::Utf8,
            true,
        );
        for index in 0..UNDO_LIMIT + 10 {
            tab.record_undo(index.to_string());
        }
        assert_eq!(tab.undo.len(), UNDO_LIMIT);
        assert_eq!(tab.undo.first().unwrap(), &"10".to_owned());
    }
}

