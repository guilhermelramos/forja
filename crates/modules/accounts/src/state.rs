//! Persisted account list and active selection.

use crate::model::Data;
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::{Ctx, LoadResult};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct State {
    pub data: Data,
    pub loaded: bool,
    pub load_failed: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
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
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

pub fn ensure_loaded(cx: &mut Ctx<'_>, shared: &Shared) {
    if shared.with(|state| state.loaded).unwrap_or(true) {
        return;
    }
    let result = cx.store().load::<Data>();
    if let Ok(mut state) = shared.0.lock() {
        state.loaded = true;
        match result {
            LoadResult::Loaded(data) => state.data = data,
            LoadResult::Missing => state.data = Data::default(),
            LoadResult::Quarantined(_) => {
                state.load_failed = true;
                state.error = Some("accounts file was corrupted and moved aside".to_owned());
            }
        }
    }
}

pub fn persist(cx: &mut Ctx<'_>, shared: &Shared) {
    let snapshot = match shared.0.lock() {
        Ok(state) => {
            if state.load_failed {
                return;
            }
            state.data.clone()
        }
        Err(_) => return,
    };
    let path = cx.store().path();
    cx.spawn(
        move || {
            if let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) {
                let _ = write_atomic_bytes(&path, &bytes);
            }
        },
        |_cx, _| {},
    );
}

/// Deterministic accounts for screenshots.
pub fn seed_preview(cx: &Ctx<'_>, shared: &Shared) {
    let root = crate::actions::root_from(cx);
    let mut data = Data::default();
    for (profile, id, label) in [
        ("claude-code", "ana", "Ana"),
        ("claude-code", "cliente", "Cliente"),
        ("codex", "pessoal", "Pessoal"),
    ] {
        let dir = root.join(profile).join(id);
        let _ = std::fs::create_dir_all(&dir);
        data.accounts.push(crate::model::Account {
            profile_id: profile.into(),
            id: id.into(),
            label: label.into(),
            dir: dir.display().to_string(),
        });
        data.active
            .entry(profile.to_owned())
            .or_insert_with(|| id.to_owned());
    }
    if let Ok(mut state) = shared.0.lock() {
        if state.data.accounts.is_empty() {
            state.data = data;
            state.loaded = true;
        }
    }
}
