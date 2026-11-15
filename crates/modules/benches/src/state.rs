//! Shared, in-memory state for the benches module.

use crate::model::{BenchFile, BenchRecord, ScriptApproval};
use forja_kernel::Ctx;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub enum Dialog {
    #[default]
    None,
    Create(CreateForm),
    Store(StoreForm),
    Approve(ApproveForm),
}

#[derive(Clone, Debug, Default)]
pub struct CreateForm {
    pub project_id: String,
    pub name: String,
    pub base: String,
    pub fetch: Option<bool>,
    pub error: Option<String>,
    /// O trabalho assíncrono começou; o diálogo só fecha quando ele termina.
    pub creating: bool,
}

#[derive(Clone, Debug)]
pub struct StoreForm {
    pub bench_id: String,
    pub name: String,
    pub branch: Option<String>,
    pub delete_branch: bool,
    pub force: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ApproveForm {
    pub project_id: String,
    pub setup: String,
    pub teardown: String,
    pub setup_hash: Option<String>,
    pub teardown_hash: Option<String>,
    pub error: Option<String>,
}

#[derive(Default)]
struct State {
    loaded: bool,
    file: BenchFile,
    error: Option<String>,
    notice: Option<String>,
    dialog: Dialog,
    previews: BTreeMap<String, Option<Result<Value, String>>>,
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
        match cx.store().load::<BenchFile>() {
            forja_kernel::LoadResult::Loaded(file) => state.file = file,
            forja_kernel::LoadResult::Quarantined(path) => {
                state.error = Some(format!(
                    "bench file was corrupted and moved to {}",
                    path.display()
                ));
            }
            forja_kernel::LoadResult::Missing => {}
        }
    }

    pub fn file(&self) -> BenchFile {
        self.0
            .lock()
            .map(|state| state.file.clone())
            .unwrap_or_default()
    }

    pub fn find(&self, bench_id: &str) -> Option<BenchRecord> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.file.find(bench_id).cloned())
    }

    pub fn current(&self) -> Option<BenchRecord> {
        self.0.lock().ok().and_then(|state| {
            state
                .file
                .current
                .as_deref()
                .and_then(|id| state.file.find(id).cloned())
        })
    }

    pub fn current_id(&self) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.file.current.clone())
    }

    pub fn approval(&self, project_id: &str) -> ScriptApproval {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.file.approval(project_id).cloned())
            .unwrap_or_else(|| ScriptApproval {
                project_id: project_id.to_owned(),
                ..ScriptApproval::default()
            })
    }

    pub fn save(&self, cx: &Ctx<'_>) -> Result<(), String> {
        let file = {
            let state = self.0.lock().map_err(|_| "bench state is poisoned")?;
            state.file.clone()
        };
        cx.store()
            .save(&file)
            .map_err(|error| format!("could not save benches: {error}"))
    }

    pub fn mutate<F>(&self, cx: &Ctx<'_>, change: F) -> Result<(), String>
    where
        F: FnOnce(&mut BenchFile),
    {
        {
            let mut state = self.0.lock().map_err(|_| "bench state is poisoned")?;
            change(&mut state.file);
        }
        self.save(cx)
    }

    pub fn dialog(&self) -> Dialog {
        self.0
            .lock()
            .map(|state| state.dialog.clone())
            .unwrap_or_default()
    }

    pub fn set_dialog(&self, dialog: Dialog) {
        if let Ok(mut state) = self.0.lock() {
            state.dialog = dialog;
        }
    }

    pub fn start_preview(&self, id: String) {
        if let Ok(mut state) = self.0.lock() {
            while state.previews.len() >= 32 {
                let Some(oldest) = state.previews.keys().next().cloned() else {
                    break;
                };
                state.previews.remove(&oldest);
            }
            state.previews.insert(id, None);
        }
    }

    pub fn finish_preview(&self, id: &str, result: Result<Value, String>) {
        if let Ok(mut state) = self.0.lock() {
            if let Some(slot) = state.previews.get_mut(id) {
                *slot = Some(result);
            }
        }
    }

    pub fn preview(&self, id: &str) -> Option<Option<Result<Value, String>>> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.previews.get(id).cloned())
    }

    pub fn cancel_preview(&self, id: &str) {
        if let Ok(mut state) = self.0.lock() {
            state.previews.remove(id);
        }
    }

    /// Closes the create dialog when the worktree is ready, or keeps it open
    /// with the full error and the typed values when creation fails.
    pub fn finish_create(&self, error: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            let Dialog::Create(form) = &mut state.dialog else {
                return;
            };
            form.creating = false;
            match error {
                Some(error) => form.error = Some(error),
                None => state.dialog = Dialog::None,
            }
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
            state.error = value;
        }
    }

    pub fn set_notice(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.notice = value;
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}
