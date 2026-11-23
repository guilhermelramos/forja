//! Catálogo de arranjos das três origens, recarga e escrita atômica.
//!
//! Leitura, escrita, remoção e observador acontecem em `Tasks` do kernel. A
//! thread da interface só publica o resultado se a geração e a raiz do projeto
//! continuam sendo as mesmas do pedido.

use crate::model::{self, Diagnostic, Layer};
use forja_kernel::layout_spec::{validate_segment, LayoutSpecV1};
use forja_kernel::settings::project_settings_path;
use forja_kernel::{Ctx, KernelError, Registrar};
use notify::Watcher as _;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const LAYOUT_DIR: &str = "layouts";
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Um arranjo efetivo de uma das três camadas.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutEntry {
    pub spec: LayoutSpecV1,
    pub text: String,
    pub layer: Layer,
    pub source: PathBuf,
    pub hash: String,
}

/// Resultado de uma leitura em `Tasks`, com a geração que a originou.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub generation: u64,
    pub project: Option<PathBuf>,
    pub entries: BTreeMap<String, LayoutEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub default_id: Option<String>,
    pub revision: u64,
}

/// Dados copiados para o worker: nada de `Ctx` cruza a chamada de `Tasks`.
#[derive(Clone, Debug)]
pub struct LayoutSources {
    pub builtin: Option<PathBuf>,
    pub user: PathBuf,
    pub project: Option<PathBuf>,
    pub generation: u64,
}

impl LayoutSources {
    pub fn from_ctx(cx: &Ctx<'_>, project: Option<PathBuf>, generation: u64) -> Self {
        Self {
            builtin: model::builtin_root(),
            user: cx.user_dir().to_path_buf(),
            project,
            generation,
        }
    }

    /// Pastas lidas, da menor para a maior precedência.
    pub fn roots(&self) -> Vec<(Layer, PathBuf)> {
        let mut roots = Vec::new();
        if let Some(builtin) = &self.builtin {
            roots.push((Layer::Builtin, builtin.clone()));
        }
        roots.push((Layer::User, self.user.clone()));
        if let Some(project) = &self.project {
            roots.push((Layer::Project, project.join(".forja")));
        }
        roots
    }

    /// Pais das pastas de arranjos: captura criação da pasta e renomes atômicos.
    pub fn watch_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![self.user.clone(), self.user.join(LAYOUT_DIR)];
        if let Some(project) = &self.project {
            let local = project.join(".forja");
            roots.push(local.clone());
            roots.push(local.join(LAYOUT_DIR));
        }
        if let Some(builtin) = &self.builtin {
            roots.push(builtin.clone());
            roots.push(builtin.join(LAYOUT_DIR));
        }
        roots.retain(|root| root.is_dir());
        roots.sort();
        roots.dedup();
        roots
    }
}

/// Observador vivo enquanto a leitura automática estiver ligada.
pub struct Watch {
    _watcher: notify::RecommendedWatcher,
    generation: Arc<AtomicU64>,
}

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
        self.ensure_watcher(cx, watch);
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
        self.with(|state| {
            state
                .dirty_at
                .is_some_and(|at| now.duration_since(at) >= DEBOUNCE)
        })
        .unwrap_or(false)
    }

    /// Drena eventos do observador, respeita o debounce e agenda a leitura.
    pub fn tick(self: &Arc<Self>, cx: &Ctx<'_>, watch: bool, now: Instant) {
        let must_read = self.with(|state| {
            if let Some(watch) = &state.watch {
                let seen = watch.generation.load(Ordering::SeqCst);
                if seen > state.seen_generation {
                    state.seen_generation = seen;
                    state.dirty_at = Some(now);
                }
            }
            if state
                .dirty_at
                .is_some_and(|at| now.duration_since(at) >= DEBOUNCE)
            {
                state.dirty_at = None;
                return true;
            }
            false
        });
        if must_read.unwrap_or(false) {
            self.next_generation();
            self.queue(cx);
        }
        self.ensure_watcher(cx, watch);
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
                LayoutSources::from_ctx(cx, state.project.clone(), state.generation),
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

    fn ensure_watcher(self: &Arc<Self>, cx: &Ctx<'_>, enabled: bool) {
        let (roots, has_watch) = {
            let Ok(state) = self.state.lock() else {
                return;
            };
            let roots = if enabled {
                LayoutSources {
                    builtin: None,
                    user: cx.user_dir().to_path_buf(),
                    project: state.project.clone(),
                    generation: state.generation,
                }
                .watch_roots()
            } else {
                Vec::new()
            };
            (roots, state.watch.is_some())
        };
        let current = self
            .with(|state| state.watch_requested.clone())
            .unwrap_or_default();
        if roots == current && !(roots.is_empty() && has_watch) {
            return;
        }
        let _ = self.with(|state| state.watch_requested = roots.clone());
        if roots.is_empty() {
            let old = self.with(|state| state.watch.take()).flatten();
            if let Some(watch) = old {
                cx.spawn(move || drop(watch), |_cx, _| {});
            }
            return;
        }
        let generation = Arc::new(AtomicU64::new(0));
        let callback_generation = Arc::clone(&generation);
        let wake = cx.waker();
        let store = Arc::clone(self);
        cx.spawn_service(
            move || {
                let wake = Arc::clone(&wake);
                let generation = Arc::clone(&callback_generation);
                let watch_generation = Arc::clone(&generation);
                let mut watcher = notify::recommended_watcher(
                    move |result: notify::Result<notify::Event>| {
                        if result.is_ok() {
                            generation.fetch_add(1, Ordering::SeqCst);
                            wake();
                        }
                    },
                )
                .ok()?;
                for root in &roots {
                    let _ = watcher.watch(root, notify::RecursiveMode::Recursive);
                }
                Some(Watch { _watcher: watcher, generation: watch_generation })
            },
            move |cx, installed| {
                let Some(installed) = installed else {
                    return;
                };
                let stale = store
                    .with(|state| {
                        if state.watch_requested.is_empty() {
                            Some(installed)
                        } else {
                            state.watch.replace(installed)
                        }
                    })
                    .flatten();
                if let Some(watch) = stale {
                    cx.spawn(move || drop(watch), |_cx, _| {});
                }
            },
        );
    }
}

/// Lê todas as camadas. Falha de um arquivo conserva a última versão válida
/// daquele mesmo caminho.
pub fn read_catalog(sources: &LayoutSources, previous: &BTreeMap<String, LayoutEntry>) -> Catalog {
    let mut entries = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (layer, root) in sources.roots() {
        let mut seen_in_layer = BTreeSet::new();
        for path in model::files_in(&root, LAYOUT_DIR, "toml") {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, layer, error.to_string()));
                    keep_previous(&mut entries, previous, &path);
                    continue;
                }
            };
            let spec = match LayoutSpecV1::parse(&text) {
                Ok(spec) => spec,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, layer, error.to_string()));
                    keep_previous(&mut entries, previous, &path);
                    continue;
                }
            };
            if spec.id != stem {
                diagnostics.push(Diagnostic::new(
                    &path,
                    layer,
                    format!("file `{stem}.toml` declares id `{}`", spec.id),
                ));
                keep_previous(&mut entries, previous, &path);
                continue;
            }
            if let Err(error) = spec.validate() {
                diagnostics.push(Diagnostic::new(&path, layer, error.to_string()));
                keep_previous(&mut entries, previous, &path);
                continue;
            }
            if !seen_in_layer.insert(spec.id.clone()) {
                diagnostics.push(Diagnostic::new(
                    &path,
                    layer,
                    format!("layout `{}` repeats in this layer", spec.id),
                ));
                continue;
            }
            entries.insert(
                spec.id.clone(),
                LayoutEntry {
                    spec,
                    text: text.clone(),
                    layer,
                    source: path.clone(),
                    hash: crate::model::sha256_hex(&text),
                },
            );
        }
    }
    Catalog {
        generation: sources.generation,
        project: sources.project.clone(),
        entries,
        diagnostics,
        default_id: read_default_id(sources.project.as_deref()),
        revision: 0,
    }
}

fn read_default_id(project: Option<&Path>) -> Option<String> {
    let text = std::fs::read_to_string(project_settings_path(project?)).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let id = value
        .get("appearance")?
        .get("layout")?
        .get("default")?
        .as_str()?;
    validate_segment("appearance.layout.default", id).ok()?;
    Some(id.to_owned())
}

fn keep_previous(
    entries: &mut BTreeMap<String, LayoutEntry>,
    previous: &BTreeMap<String, LayoutEntry>,
    path: &Path,
) {
    if let Some(entry) = previous.values().find(|entry| entry.source == path) {
        entries.insert(entry.spec.id.clone(), entry.clone());
    }
}

/// Caminho seguro para `<root>/layouts/<id>.toml`, inclusive com links e junções.
pub fn layout_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    validate_segment("id", id).map_err(|error| error.to_string())?;
    let directory = root.join(LAYOUT_DIR);
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let root_canon = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let dir_canon = std::fs::canonicalize(&directory).map_err(|error| error.to_string())?;
    if !dir_canon.starts_with(&root_canon) {
        return Err("layouts folder escapes the content root".to_owned());
    }
    let name = format!("{id}.toml");
    let path = dir_canon.join(&name);
    if let Ok(metadata) = std::fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() {
            return Err(format!("`{name}` is a link"));
        }
    }
    Ok(path)
}

fn current_hash(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| crate::model::sha256_hex(&text))
}

/// Gravação atômica. `expected_hash` detecta revisão concorrente.
pub fn write_layout(
    root: &Path,
    id: &str,
    text: &str,
    expected_hash: Option<&str>,
) -> Result<String, String> {
    let spec = LayoutSpecV1::parse(text).map_err(|error| error.to_string())?;
    if spec.id != id {
        return Err(format!("file must be named `{}.toml`", spec.id));
    }
    let path = layout_path(root, id)?;
    if let Some(expected) = expected_hash {
        if current_hash(&path).as_deref() != Some(expected) {
            return Err("layout changed since it was opened".to_owned());
        }
    }
    let hash = crate::model::sha256_hex(text);
    forja_kernel::store::write_atomic_bytes(&path, text.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(hash)
}

/// Remoção atômica: rename para um irmão temporário e só depois delete.
pub fn remove_layout(root: &Path, id: &str, expected_hash: Option<&str>) -> Result<(), String> {
    let path = layout_path(root, id)?;
    if let Some(expected) = expected_hash {
        if current_hash(&path).as_deref() != Some(expected) {
            return Err("layout changed since it was opened".to_owned());
        }
    }
    if !path.is_file() {
        return Err(format!("layout `{id}` does not exist in this layer"));
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let temporary = path.with_extension(format!("remove-{}-{nanos}", std::process::id()));
    std::fs::rename(&path, &temporary).map_err(|error| error.to_string())?;
    if let Err(error) = std::fs::remove_file(&temporary) {
        let _ = std::fs::rename(&temporary, &path);
        return Err(error.to_string());
    }
    Ok(())
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
