//! Catálogo de arranjos, recarga e escrita atômica.
//!
//! Leitura, escrita, remoção e observador acontecem em `Tasks` do kernel. A
//! thread da interface só publica o resultado se a geração e a raiz do projeto
//! continuam sendo as mesmas do pedido.

mod catalog;
mod watch;
mod write;

pub use catalog::{read_catalog, read_limited, Catalog, CatalogCache, LayoutEntry, LayoutSources};
pub use watch::{watch_roots, Watch, WatchRoots, DEBOUNCE};
pub use write::{layout_path, project_layout_root, remove_layout, write_layout};

use forja_kernel::{Args, Ctx, KernelError, Registrar};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub(crate) const LAYOUT_DIR: &str = "layouts";

/// Raiz registrada do projeto ativo. `bench.current.path` aponta para o
/// checkout da bancada, que pode ser outro worktree do mesmo projeto.
pub fn current_project_root(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("project.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(Value::as_str)
                .filter(|path| !path.is_empty())
                .map(PathBuf::from)
        })
}

pub struct LayoutState {
    pub catalog: Catalog,
    pub cache: CatalogCache,
    pub generation: u64,
    pub project: Option<PathBuf>,
    pub queued: Option<u64>,
    pub ready: bool,
    pub seen_generation: u64,
    pub dirty_at: Option<Instant>,
    pub deadline_scheduled: bool,
    pub watch_requested: Vec<PathBuf>,
    pub watch_token: u64,
    pub watch: Option<Watch>,
    /// Último resultado de gravação/remoção, para o seletor mostrar falhas.
    pub last_write: Option<String>,
    pub write_log: Vec<String>,
    /// Hash confirmado pela última mutação, usado como revisão gravada.
    pub last_write_hash: Option<String>,
    /// Raiz embutida guardada. A UI só lê este valor; nunca faz E/S para obtê-lo.
    pub builtin_root: Option<PathBuf>,
    /// Verdadeiro depois que a resolução em `Tasks` foi agendada (uma só vez).
    pub builtin_root_pending: bool,
}

impl Default for LayoutState {
    fn default() -> Self {
        Self {
            catalog: Catalog::default(),
            cache: CatalogCache::default(),
            generation: 0,
            project: None,
            queued: None,
            ready: false,
            seen_generation: 0,
            dirty_at: None,
            deadline_scheduled: false,
            watch_requested: Vec::new(),
            watch_token: 0,
            watch: None,
            last_write: None,
            write_log: Vec::new(),
            last_write_hash: None,
            builtin_root: crate::model::builtin_root_hint(),
            builtin_root_pending: false,
        }
    }
}

/// Catálogo compartilhado pelo módulo `content`.
pub struct LayoutStore {
    state: Mutex<LayoutState>,
    /// Um lock por destino serializa checagem de hash e mutação.
    locks: Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>,
}

impl Default for LayoutStore {
    fn default() -> Self {
        Self {
            state: Mutex::new(LayoutState::default()),
            locks: Mutex::new(BTreeMap::new()),
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

    /// Valor guardado da raiz embutida: leitura sem E/S, segura na thread da UI.
    pub fn builtin_root(&self) -> Option<PathBuf> {
        self.with(|state| state.builtin_root.clone()).unwrap_or(None)
    }

    /// Agenda uma única vez a resolução definitiva da raiz embutida em `Tasks`.
    /// O `is_dir`/`current_exe` roda no worker; a UI só publica o resultado.
    pub fn ensure_builtin_root(self: &Arc<Self>, cx: &Ctx<'_>) {
        let already = self
            .with(|state| {
                if state.builtin_root_pending {
                    true
                } else {
                    state.builtin_root_pending = true;
                    false
                }
            })
            .unwrap_or(true);
        if already {
            return;
        }
        let store = Arc::clone(self);
        cx.spawn(crate::model::builtin_root, move |cx, resolved| {
            let changed = store
                .with(|state| {
                    let changed = state.builtin_root != resolved;
                    state.builtin_root = resolved;
                    changed
                })
                .unwrap_or(false);
            if changed {
                store.request_reload(cx);
            }
            cx.request_repaint();
        });
    }

    fn destination_lock(&self, destination: &Path) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap_or_else(|error| error.into_inner());
        Arc::clone(
            locks
                .entry(destination.to_path_buf())
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
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
            state.project = project.clone();
            state.generation = state.generation.wrapping_add(1);
            state.ready = false;
            state.queued = None;
            state.dirty_at = None;
            state.seen_generation = 0;
            state.watch_token = state.watch_token.wrapping_add(1);
            // Publica imediatamente o snapshot da nova raiz: nunca devolve o
            // catálogo da bancada/projeto anterior enquanto a leitura corre.
            let cached = state.cache.project == project;
            state.catalog = Catalog {
                generation: state.generation,
                project,
                entries: if cached {
                    state.cache.entries.clone()
                } else {
                    BTreeMap::new()
                },
                ..Catalog::default()
            };
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
        self.ensure_builtin_root(cx);
        let (sources, previous) = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            if state.queued == Some(state.generation) {
                return;
            }
            state.queued = Some(state.generation);
            (
                LayoutSources::from_ctx(
                    cx,
                    state.builtin_root.clone(),
                    state.project.clone(),
                    state.generation,
                ),
                state.cache.clone(),
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
        state.cache = CatalogCache {
            project: catalog.project.clone(),
            entries: catalog.entries.clone(),
            valid: catalog.valid.clone(),
        };
        state.catalog = catalog;
        state.ready = true;
        true
    }

    /// Sinal do disco: invalida leituras em voo e inicia a janela de debounce.
    pub fn signal(&self, now: Instant) {
        let _ = self.with(|state: &mut LayoutState| {
            state.dirty_at = Some(now);
            state.generation = state.generation.wrapping_add(1);
            state.ready = false;
            state.queued = None;
        });
    }

    /// Drena eventos do observador, respeita o debounce e agenda a leitura.
    /// O único prazo de egui é pedido no sinal; quadros subsequentes não
    /// voltam a acordar a UI continuamente.
    pub fn tick(self: &Arc<Self>, cx: &Ctx<'_>, watch: bool, now: Instant) {
        let signaled = self
            .with(|state: &mut LayoutState| {
                if let Some(watch) = &state.watch {
                    let seen = watch.generation.load(std::sync::atomic::Ordering::SeqCst);
                    if seen > state.seen_generation {
                        state.seen_generation = seen;
                        return true;
                    }
                }
                false
            })
            .unwrap_or(false);
        if signaled {
            self.signal(now);
            if watch {
                cx.request_repaint_after(DEBOUNCE);
                let _ = self.with(|state: &mut LayoutState| state.deadline_scheduled = true);
            }
        }
        let must_read = self
            .with(|state: &mut LayoutState| {
                let due = watch::due(state, now);
                if due {
                    state.dirty_at = None;
                    state.deadline_scheduled = false;
                }
                due
            })
            .unwrap_or(false);
        if must_read {
            self.next_generation();
            self.queue(cx);
        }
        watch::ensure_watcher(self, cx, watch);
    }
}

/// Tick de serviço registrado no kernel: chamado uma vez por quadro pelo shell.
pub fn tick(store: &Arc<LayoutStore>, cx: &mut Ctx<'_>) {
    let watch = cx
        .setting("content.reload.watch")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    store.tick(cx, watch, Instant::now());
}

/// Registra os comandos internos usados pelo catálogo e pelo seletor.
pub fn register(r: &mut Registrar<'_>, store: &Arc<LayoutStore>) {
    let list = Arc::clone(store);
    r.command("content.layouts.list", "content.command.layouts-list.title", move |cx, _| {
        let project = current_project_root(cx);
        let watch = watch_enabled(cx);
        list.ensure(cx, project, watch);
        let snapshot = list
            .with(|state| {
                (
                    state.catalog.clone(),
                    state.last_write.clone(),
                    state.write_log.clone(),
                    state.last_write_hash.clone(),
                    state.generation,
                )
            })
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
            "generation": snapshot.4,
            "default_id": catalog.default_id,
            "layouts": layouts,
            "diagnostics": catalog.diagnostics,
            "last_write": snapshot.1,
            "last_write_hash": snapshot.3,
            "known_views": catalog.views,
            "write_log": snapshot.2,
        }))
    });
    let reload = Arc::clone(store);
    r.command("content.layouts.reload", "content.command.layouts-reload.title", move |cx, _| {
        let project = current_project_root(cx);
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
    let layer = args.str("layer").unwrap_or("user").to_owned();
    let project = current_project_root(cx);
    let root = match layer.as_str() {
        "user" => cx.user_dir().to_path_buf(),
        "project" => project
            .clone()
            .ok_or_else(|| KernelError::message("no project is active"))
            .and_then(|project| project_layout_root(&project).map_err(KernelError::message))?,
        other => return Err(KernelError::message(format!("unknown layout layer `{other}`"))),
    };
    let destination = root.join(LAYOUT_DIR).join(format!("{id}.toml"));
    let lock = store.destination_lock(&destination);
    let text = if remove {
        String::new()
    } else {
        args.required_str("text")?.to_owned()
    };
    let expected = args.str("expected_hash").map(str::to_owned);
    let worker_id = id.clone();
    let result_id = id.clone();
    let store = Arc::clone(store);
    cx.spawn(
        move || {
            // Checagem de revisão e mutação são uma única seção crítica por
            // destino: duas gravações não passam pelo mesmo hash.
            let _guard = lock.lock().unwrap_or_else(|error| error.into_inner());
            if remove {
                remove_layout(&root, &worker_id, expected.as_deref()).map(|()| None)
            } else {
                write_layout(&root, &worker_id, &text, expected.as_deref()).map(Some)
            }
        },
        move |cx, result| {
            let (hash, detail) = match &result {
                Ok(hash) => (hash.clone(), String::new()),
                Err(error) => (None, error.clone()),
            };
            let ok = result.is_ok();
            let key = match (remove, ok) {
                (false, true) => "content.layouts.save-ok",
                (false, false) => "content.layouts.save-failed",
                (true, true) => "content.layouts.remove-ok",
                (true, false) => "content.layouts.remove-failed",
            };
            let message = cx.t_args(
                key,
                &[("id", &result_id), ("detail", &detail), ("hash", hash.as_deref().unwrap_or(""))],
            );
            let _ = store.with(|state| {
                state.last_write = Some(message.clone());
                state.write_log.push(message.clone());
                if state.write_log.len() > 20 {
                    state.write_log.remove(0);
                }
                state.last_write_hash = hash.clone();
            });
            let root_current = layer != "project" || current_project_root(cx) == project;
            if ok && root_current {
                let generation = store.request_reload(cx);
                store.ensure(cx, project.clone(), watch_enabled(cx));
                let _ = store.with(|state| {
                    state.last_write = Some(format!("{message} · {generation} · {hash:?}"));
                });
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
