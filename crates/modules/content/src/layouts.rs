//! Catálogo de arranjos, recarga e escrita atômica.
//!
//! Leitura, escrita, remoção e observador acontecem em `Tasks` do kernel. A
//! thread da interface só publica o resultado se a geração e a raiz do projeto
//! continuam sendo as mesmas do pedido.

mod catalog;
mod watch;
mod write;

pub use catalog::{read_catalog, Catalog, LayoutEntry, LayoutSources};
pub use watch::{watch_roots, Watch, DEBOUNCE};
pub use write::{layout_path, remove_layout, write_layout};

use forja_kernel::{Ctx, KernelError, Registrar};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub(crate) const LAYOUT_DIR: &str = "layouts";

#[derive(Default)]
pub struct LayoutState {
    pub catalog: Catalog,
    pub generation: u64,
    pub project: Option<PathBuf>,
    pub queued: Option<u64>,
    pub ready: bool,
    pub seen_generation: u64,
    pub dirty_at: Option<Instant>,
    pub watch_requested: Vec<PathBuf>,
    pub watch: Option<Watch>,
    /// Último resultado de gravação/remoção, para o seletor mostrar falhas.
    pub last_write: Option<String>,
}

/// Catálogo compartilhado pelo módulo `content`.
pub struct LayoutStore {
    state: Mutex<LayoutState>,
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self {
            state: Mutex::new(LayoutState::default()),
        }
    }
}

impl LayoutStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with<T>(&self, run: impl FnOnce(&mut LayoutState) -> T) -> Option<T> {
        self.state.lock().ok().map(|mut state| run(&mut state))
    }

    pub(crate) fn state_lock(&self) -> Option<MutexGuard<'_, LayoutState>> {
        self.state.lock().ok()
    }

    pub(crate) fn has_watch(&self) -> bool {
        self.with(|state| state.watch.is_some()).unwrap_or(false)
    }

    fn next_generation(&self) -> u64 {
        let Ok(mut state) = self.state.lock() else {
            return 0;
        };
        state.generation = state.generation.wrapping_add(1);
        state.ready = false;
        state.queued = None;
        state.dirty_at = None;
        state.generation
    }

    /// Garante uma leitura para a raiz registrada e instala o observador.
    pub fn ensure(self: &Arc<Self>, cx: &Ctx<'_>, project: Option<PathBuf>, watch: bool) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.project != project {
            state.project = project;
            state.generation = state.generation.wrapping_add(1);
            state.ready = false;
            state.queued = None;
            state.dirty_at = None;
            state.seen_generation = 0;
        }
        drop(state);
        if !self.with(|state| state.ready).unwrap_or(false) {
            self.queue(cx);
        }
        watch::ensure_watcher(self, cx, watch);
    }

    /// Recarga manual: nova geração, descartando resultados em voo.
    pub fn request_reload(self: &Arc<Self>, cx: &Ctx<'_>) -> u64 {
        let generation = self.next_generation();
        self.queue(cx);
        generation
    }

    /// Marca o instante do último evento: coalesce até `DEBOUNCE` sem eventos.
    pub fn mark_dirty(&self, now: Instant) {
        let _ = self.with(|state| state.dirty_at = Some(now));
    }

    /// Verdadeiro quando o prazo de coalescência terminou.
    pub fn due(&self, now: Instant) -> bool {
        self.with(|state| watch::due(state, now)).unwrap_or(false)
    }

    /// Copia as entradas atuais e lê o catálogo inteiro em `Tasks`.
    pub fn queue(self: &Arc<Self>, cx: &Ctx<'_>) {
        let (sources, previous) = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            if state.queued == Some(state.generation) {
                return;
            }
            state.queued = Some(state.generation);
            (
                LayoutSources::from_paths(
                    crate::model::builtin_root(),
                    cx.user_dir().to_path_buf(),
                    state.project.clone(),
                    state.generation,
                ),
                state.catalog.entries.clone(),
            )
        };
        let store = Arc::clone(self);
        cx.spawn(
            move || read_catalog(&sources, &previous),
            move |cx, catalog| {
                if store.apply(catalog) {
                    cx.request_repaint();
                }
            },
        );
    }

    /// Publica o resultado somente se a geração e a raiz continuam atuais.
    pub fn apply(&self, mut catalog: Catalog) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        if catalog.generation != state.generation || catalog.project != state.project {
            return false;
        }
        catalog.revision = state.catalog.revision.wrapping_add(1);
        state.catalog = catalog;
        state.ready = true;
        true
    }

    /// Drena eventos do observador, respeita o debounce e agenda a leitura.
    pub fn tick(self: &Arc<Self>, cx: &Ctx<'_>, watch: bool, now: Instant) {
        let must_read = self.with(|state: &mut LayoutState| {
            if let Some(watch) = &state.watch {
                let seen = watch.generation.load(std::sync::atomic::Ordering::SeqCst);
                if seen > state.seen_generation {
                    state.seen_generation = seen;
                    state.dirty_at = Some(now);
                }
            }
            if watch::due(state, now) {
                state.dirty_at = None;
                return true;
            }
            false
        });
        if must_read.unwrap_or(false) {
            self.next_generation();
            self.queue(cx);
        }
        watch::ensure_watcher(self, cx, watch);
    }
}

/// Registra os comandos internos usados pelo catálogo e pelo seletor.
pub fn register(r: &mut Registrar<'_>, store: &Arc<LayoutStore>) {
    let list = Arc::clone(store);
    r.command("content.layouts.list", "content.command.layouts-list.title", move |cx, _| {
        let project = crate::state::current_project(cx);
        let watch = watch_enabled(cx);
        list.ensure(cx, project, watch);
        let snapshot = list
            .with(|state| (state.catalog.clone(), state.last_write.clone(), state.generation))
            .unwrap_or_default();
        let catalog = snapshot.0;
        let layouts = catalog
            .entries
            .values()
            .map(|entry| {
                json!({
                    "id": entry.spec.id,
                    "name": entry.spec.name,
                    "layer": entry.layer,
                    "source": entry.source,
                    "hash": entry.hash,
                    "text": entry.text,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "revision": catalog.revision,
            "generation": snapshot.2,
            "default_id": catalog.default_id,
            "layouts": layouts,
            "diagnostics": catalog.diagnostics,
            "last_write": snapshot.1,
        }))
    });
    let reload = Arc::clone(store);
    r.command("content.layouts.reload", "content.command.layouts-reload.title", move |cx, _| {
        let project = crate::state::current_project(cx);
        let watch = watch_enabled(cx);
        reload.ensure(cx, project, watch);
        Ok(json!({ "generation": reload.request_reload(cx) }))
    });
    let save = Arc::clone(store);
    r.command("content.layouts.save", "content.command.layouts-save.title", move |cx, args| {
        queue_write(&save, cx, &args, false)
    });
    let remove = Arc::clone(store);
    r.command("content.layouts.remove", "content.command.layouts-remove.title", move |cx, args| {
        queue_write(&remove, cx, &args, true)
    });
}

fn queue_write(
    store: &Arc<LayoutStore>,
    cx: &mut Ctx<'_>,
    args: &forja_kernel::Args,
    remove: bool,
) -> forja_kernel::Result<serde_json::Value> {
    let id = args.required_str("id")?.to_owned();
    let root = match args.str("layer").unwrap_or("user") {
        "user" => cx.user_dir().to_path_buf(),
        "project" => crate::state::current_project(cx)
            .map(|project| project.join(".forja"))
            .ok_or_else(|| KernelError::message("no project is active"))?,
        other => return Err(KernelError::message(format!("unknown layout layer `{other}`"))),
    };
    let text = if remove {
        String::new()
    } else {
        args.required_str("text")?.to_owned()
    };
    let expected = args.str("expected_hash").map(str::to_owned);
    let worker_id = id.clone();
    let result_id = id.clone();
    let op = if remove { "remove" } else { "save" }.to_owned();
    let store = Arc::clone(store);
    cx.spawn(
        move || {
            if remove {
                remove_layout(&root, &worker_id, expected.as_deref())
                    .map(|()| (None, String::new()))
            } else {
                write_layout(&root, &worker_id, &text, expected.as_deref())
                    .map(|hash| (Some(hash), String::new()))
            }
        },
        move |cx, result| {
            let ok = result.is_ok();
            let message = match result {
                Ok(_) => format!("{op} `{result_id}` ok"),
                Err(error) => format!("{op} `{result_id}` failed: {error}"),
            };
            let _ = store.with(|state| state.last_write = Some(message));
            if ok {
                let project = crate::state::current_project(cx);
                store.ensure(cx, project, watch_enabled(cx));
                store.request_reload(cx);
            }
            cx.request_repaint();
        },
    );
    Ok(json!({ "queued": true, "id": id }))
}

fn watch_enabled(cx: &Ctx<'_>) -> bool {
    cx.setting("content.reload.watch")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}
