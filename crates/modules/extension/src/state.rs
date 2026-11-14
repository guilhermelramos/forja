//! Descoberta, estados, aprovação e ciclo de vida das extensões.

use crate::approval::{self, Approval, Registry};
use crate::manifest::{discover, Layer, Manifest};
use crate::runtime::{Host, HostCommand, HostStatus, PanelSnapshot, StartOptions};
use forja_contracts::{TrustDecided, TrustKind};
use forja_kernel::{Args, Ctx};
use notify::Watcher as _;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_millis(300);

/// Estado visível de uma extensão.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionState {
    Pending,
    Active,
    Changed,
    Invalid,
    Suspended,
    Conflict,
}

impl ExtensionState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Changed => "changed",
            Self::Invalid => "invalid",
            Self::Suspended => "suspended",
            Self::Conflict => "conflict",
        }
    }
}

/// Uma extensão descoberta.
pub struct ExtensionEntry {
    pub id: String,
    pub manifest: Option<Manifest>,
    pub state: ExtensionState,
    pub error: Option<String>,
    pub engine: Option<Arc<Host>>,
    pub pending_request: Option<String>,
    pub dev: bool,
    pub source: String,
}

impl ExtensionEntry {
    fn new(id: &str, source: &Path) -> Self {
        Self {
            id: id.to_owned(),
            manifest: None,
            state: ExtensionState::Invalid,
            error: None,
            engine: None,
            pending_request: None,
            dev: false,
            source: source.display().to_string(),
        }
    }
}

/// Observador de pastas.
pub struct Watch {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<()>,
}

/// Estado global do módulo.
pub struct CoreState {
    pub loaded: bool,
    pub project: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub revision: u64,
    pub entries: BTreeMap<String, ExtensionEntry>,
    pub registry: Option<Registry>,
    pub watch: Option<Watch>,
    pub watched: Vec<PathBuf>,
    pub dirty_at: Option<Instant>,
    pub last_tick: Option<Instant>,
    pub ready_sent: bool,
}

impl Default for CoreState {
    fn default() -> Self {
        Self {
            loaded: false,
            project: None,
            data_dir: PathBuf::new(),
            revision: 0,
            entries: BTreeMap::new(),
            registry: None,
            watch: None,
            watched: Vec::new(),
            dirty_at: None,
            last_tick: None,
            ready_sent: false,
        }
    }
}

/// Estado compartilhado do módulo.
pub struct ExtensionCore {
    pub state: Mutex<CoreState>,
}

impl ExtensionCore {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(CoreState::default()),
        }
    }

    pub fn with<T>(&self, run: impl FnOnce(&mut CoreState) -> T) -> Option<T> {
        self.state.lock().ok().map(|mut state| run(&mut state))
    }

    pub fn snapshot(&self) -> Vec<EntrySnapshot> {
        self.with(|state| {
            let data_dir = state.data_dir.clone();
            state
                .entries
                .values()
                .map(|entry| EntrySnapshot::from(entry, &data_dir))
                .collect()
        })
        .unwrap_or_default()
    }
}

impl Default for ExtensionCore {
    fn default() -> Self {
        Self::new()
    }
}

/// Cópia leve para a interface.
#[derive(Clone)]
pub struct EntrySnapshot {
    pub id: String,
    pub name: crate::manifest::Localized,
    pub description: crate::manifest::Localized,
    pub version: String,
    pub author: String,
    pub layer: String,
    pub dir: String,
    pub hash: String,
    pub state: ExtensionState,
    pub error: Option<String>,
    pub dev: bool,
    pub capabilities: Vec<(String, String)>,
    pub commands: Vec<crate::runtime::ExtCommandInfo>,
    pub settings: Vec<crate::runtime::SettingInfo>,
    pub values: BTreeMap<String, Value>,
    pub errors: Vec<String>,
    pub log: String,
    pub panels: Vec<PanelSnapshot>,
}

impl EntrySnapshot {
    fn from(entry: &ExtensionEntry, data_dir: &Path) -> Self {
        let manifest = entry.manifest.as_ref();
        let (commands, settings, values, errors, panels) = entry
            .engine
            .as_ref()
            .and_then(|engine| {
                engine.shared.lock().ok().map(|shared| {
                    (
                        shared.commands.clone(),
                        shared.settings_schema.clone(),
                        shared.settings.clone(),
                        shared.errors.clone(),
                        shared.panels.values().cloned().collect::<Vec<_>>(),
                    )
                })
            })
            .unwrap_or_default();
        let log = tail(
            &data_dir
                .join("logs")
                .join("extensions")
                .join(format!("{}.log", entry.id)),
            120,
        );
        Self {
            id: entry.id.clone(),
            name: manifest
                .map(|manifest| manifest.name.clone())
                .unwrap_or_default(),
            description: manifest
                .map(|manifest| manifest.description.clone())
                .unwrap_or_default(),
            version: manifest
                .map(|manifest| manifest.version.clone())
                .unwrap_or_default(),
            author: manifest
                .map(|manifest| manifest.author.clone())
                .unwrap_or_default(),
            layer: manifest
                .map(|manifest| manifest.layer.label().to_owned())
                .unwrap_or_else(|| "unknown".to_owned()),
            dir: manifest
                .map(|manifest| manifest.dir.clone())
                .unwrap_or_else(|| entry.source.clone()),
            hash: manifest
                .map(|manifest| manifest.hash.clone())
                .unwrap_or_default(),
            state: entry.state,
            error: entry.error.clone(),
            dev: entry.dev,
            capabilities: manifest
                .map(|manifest| manifest.capabilities.entries())
                .unwrap_or_default(),
            commands,
            settings,
            values,
            errors,
            log,
            panels,
        }
    }
}

fn tail(path: &Path, lines: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

/// Garante descoberta e ciclo de vida atualizados.
pub fn ensure(core: &ExtensionCore, cx: &mut Ctx<'_>) {
    let project = current_project(cx);
    let data_dir = cx.data_dir().to_path_buf();
    let registry_path = data_dir.join("approvals").join("extensions.json");
    let reload = {
        let Ok(mut state) = core.state.lock() else {
            return;
        };
        if state.registry.is_none() {
            state.registry = Some(Registry::open(registry_path));
        }
        state.data_dir = data_dir;
        let changed = state.project != project;
        !state.loaded || changed
    };
    if reload {
        scan(core, cx, project);
    }
    ensure_watcher(core, cx);
    tick(core, cx);
}

fn current_project(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
}

fn roots(cx: &Ctx<'_>, project: Option<&Path>) -> Vec<(Layer, PathBuf)> {
    let mut roots = vec![(Layer::User, cx.user_dir().join("extensions"))];
    if let Some(project) = project {
        roots.push((Layer::Project, project.join(".forja").join("extensions")));
    }
    roots
}

/// Relê as pastas e decide o estado de cada extensão.
pub fn scan(core: &ExtensionCore, cx: &mut Ctx<'_>, project: Option<PathBuf>) {
    let previous_project = core.with(|state| state.project.clone()).flatten();
    let ready = core.with(|state| state.ready_sent).unwrap_or(false);
    let enabled = cx
        .setting("extension.mode.enabled")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let dev_watch = cx
        .setting("extension.dev.watch")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let mut discovered: Vec<(Layer, PathBuf, Result<Manifest, String>)> = Vec::new();
    for (layer, root) in roots(cx, project.as_deref()) {
        for item in discover(&root, layer) {
            match item {
                Ok(manifest) => discovered.push((layer, PathBuf::from(&manifest.dir), Ok(manifest))),
                Err((dir, error)) => discovered.push((layer, dir, Err(error))),
            }
        }
    }
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    for (_, _, item) in &discovered {
        if let Ok(manifest) = item {
            *ids.entry(manifest.id.clone()).or_default() += 1;
        }
    }
    let mut next: BTreeMap<String, ExtensionEntry> = BTreeMap::new();
    let mut old = core
        .with(|state| std::mem::take(&mut state.entries))
        .unwrap_or_default();
    for (layer, dir, item) in discovered {
        let id = match &item {
            Ok(manifest) => manifest.id.clone(),
            Err(_) => dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown")
                .to_owned(),
        };
        let mut entry = ExtensionEntry::new(&id, &dir);
        match item {
            Ok(manifest) => {
                entry.id = manifest.id.clone();
                entry.source = manifest.dir.clone();
                if ids.get(&manifest.id).copied().unwrap_or(0) > 1 {
                    entry.state = ExtensionState::Conflict;
                    entry.error =
                        Some("the same id exists in the user and project layers".to_owned());
                    entry.manifest = Some(manifest);
                    next.insert(id, entry);
                    continue;
                }
                let approval = core
                    .with(|state| {
                        state
                            .registry
                            .as_ref()
                            .and_then(|registry| registry.find(&manifest.id).cloned())
                    })
                    .flatten();
                let trust_ok = trust_check(cx, &manifest);
                let mut state = if approval
                    .as_ref()
                    .is_some_and(|approval| approval.hash == manifest.hash)
                    && trust_ok
                {
                    ExtensionState::Active
                } else if approval.is_some() {
                    ExtensionState::Changed
                } else {
                    ExtensionState::Pending
                };
                let mut dev = false;
                if state == ExtensionState::Changed
                    && dev_watch
                    && layer == Layer::User
                    && approval
                        .as_ref()
                        .is_some_and(|approval| approval.capabilities == manifest.capabilities)
                    && trust_grant(cx, &manifest)
                {
                    record_approval(core, &manifest, true);
                    state = ExtensionState::Active;
                    dev = true;
                }
                if !enabled {
                    state = ExtensionState::Pending;
                    entry.error = Some("extensions are disabled in settings".to_owned());
                }
                entry.state = state;
                entry.dev = dev;
                entry.manifest = Some(manifest.clone());
                if state == ExtensionState::Active {
                    if let Some(previous) = old.get(&id) {
                        if previous
                            .manifest
                            .as_ref()
                            .is_some_and(|previous| previous.hash == manifest.hash)
                            && previous.state == ExtensionState::Active
                        {
                            entry.engine = old
                                .remove(&id)
                                .and_then(|mut previous| previous.engine.take());
                        }
                    }
                    if entry.engine.is_none() {
                        entry.engine = Some(Arc::new(start_engine(cx, &manifest)));
                    }
                }
            }
            Err(error) => {
                entry.state = ExtensionState::Invalid;
                entry.error = Some(error);
            }
        }
        next.insert(id, entry);
    }
    for (_, previous) in old {
        if let Some(engine) = previous.engine {
            engine.stop();
        }
    }
    let project_name = project
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    if let Ok(mut state) = core.state.lock() {
        state.entries = next;
        state.project = project.clone();
        state.loaded = true;
        state.ready_sent = true;
        state.revision = state.revision.wrapping_add(1);
    }
    if previous_project != project {
        if let Some(path) = &project {
            broadcast_event(
                core,
                "project.opened",
                json!({ "path": path.display().to_string(), "name": project_name }),
            );
        }
    }
    if !ready {
        broadcast_event(core, "app.ready", json!({}));
    }
}

/// Entrega um evento a todas as extensões ativas.
pub fn broadcast_event(core: &ExtensionCore, name: &str, payload: Value) {
    let engines: Vec<Arc<Host>> = core
        .with(|state| {
            state
                .entries
                .values()
                .filter(|entry| entry.state == ExtensionState::Active)
                .filter_map(|entry| entry.engine.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for engine in engines {
        engine.send(HostCommand::Event {
            name: name.to_owned(),
            payload: payload.clone(),
        });
    }
}

fn trust_check(cx: &mut Ctx<'_>, manifest: &Manifest) -> bool {
    let mut args = Args::new();
    args.insert("kind", json!("extension"));
    args.insert("source_path", json!(manifest.dir));
    args.insert("content_hash", json!(manifest.hash));
    cx.run_command("trust.check", args)
        .ok()
        .and_then(|value| value.get("approved").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn trust_grant(cx: &mut Ctx<'_>, manifest: &Manifest) -> bool {
    let mut args = Args::new();
    args.insert("kind", json!("extension"));
    args.insert("source_path", json!(manifest.dir));
    args.insert("content", json!(manifest.fingerprint));
    args.insert(
        "summary",
        json!(format!(
            "development reload: {} {}",
            manifest.id, manifest.version
        )),
    );
    cx.run_command("trust.grant", args).is_ok()
}

fn record_approval(core: &ExtensionCore, manifest: &Manifest, dev: bool) {
    let approval = Approval {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        hash: manifest.hash.clone(),
        capabilities: manifest.capabilities.clone(),
        approved_at: approval::now(),
        dev,
    };
    if let Ok(mut state) = core.state.lock() {
        if let Some(registry) = &mut state.registry {
            registry.put(approval);
            let _ = registry.persist();
        }
    }
}

fn start_engine(cx: &mut Ctx<'_>, manifest: &Manifest) -> Host {
    let memory_mb = cx
        .setting("extension.lua.memory_mb")
        .and_then(|value| value.as_i64())
        .unwrap_or(64)
        .clamp(8, 512) as usize;
    let timeout_ms = cx
        .setting("extension.lua.timeout_ms")
        .and_then(|value| value.as_i64())
        .unwrap_or(1000)
        .clamp(100, 10_000) as u64;
    let log_level = cx
        .setting("extension.log.level")
        .and_then(|value| value.as_str())
        .unwrap_or("warn")
        .to_owned();
    let sandbox_mode = cx
        .setting("extension.sandbox.mode")
        .and_then(|value| value.as_str())
        .unwrap_or("strict")
        .to_owned();
    let network_allowed = cx
        .setting("extension.network.allow")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let fs_outside_allowed = cx
        .setting("extension.fs.allow_outside_project")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let bench = cx.run_command("bench.current", Args::new()).ok();
    let project = bench
        .as_ref()
        .and_then(|value| value.get("path"))
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let context_project = project.clone();
    let context_bench = bench.clone();
    let host = Host::start(StartOptions {
        manifest: manifest.clone(),
        project,
        bench,
        locale: cx.i18n().locale().id().to_owned(),
        data_dir: cx.data_dir().to_path_buf(),
        memory_mb,
        timeout_ms,
        log_level,
        sandbox_mode,
        network_allowed,
        fs_outside_allowed,
    });
    host.send(HostCommand::Context {
        project: context_project,
        bench: context_bench,
    });
    host
}

/// Atende mensagens e detecta extensões suspensas.
pub fn tick(core: &ExtensionCore, cx: &mut Ctx<'_>) {
    let engines: Vec<(String, Arc<Host>)> = core
        .with(|state| {
            state
                .entries
                .iter()
                .filter_map(|(id, entry)| entry.engine.clone().map(|engine| (id.clone(), engine)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut suspended = Vec::new();
    let mut failed = Vec::new();
    for (id, engine) in engines {
        engine.tick(cx);
        if let Ok(shared) = engine.shared.lock() {
            match shared.status {
                HostStatus::Suspended => suspended.push(id),
                HostStatus::Failed => {
                    let error = shared.errors.last().cloned();
                    failed.push((id, error));
                }
                _ => {}
            }
        }
    }
    if !failed.is_empty() {
        if let Ok(mut state) = core.state.lock() {
            for (id, error) in failed {
                if let Some(entry) = state.entries.get_mut(&id) {
                    entry.state = ExtensionState::Invalid;
                    entry.error = error;
                }
            }
        }
    }
    for id in suspended {
        let engine = core.with(|state| {
            state
                .entries
                .get_mut(&id)
                .and_then(|entry| entry.engine.take())
        });
        if let Some(engine) = engine.flatten() {
            engine.stop();
            if let Ok(mut state) = core.state.lock() {
                if let Some(entry) = state.entries.get_mut(&id) {
                    entry.state = ExtensionState::Suspended;
                    entry.error = Some("three errors in 60 seconds".to_owned());
                }
            }
        }
    }
    let mut dirty = false;
    if let Ok(mut state) = core.state.lock() {
        let mut touched = false;
        if let Some(watch) = &state.watch {
            while watch.rx.try_recv().is_ok() {
                touched = true;
            }
        }
        if touched {
            state.dirty_at = Some(Instant::now());
        }
        if state
            .dirty_at
            .is_some_and(|at| at.elapsed() >= DEBOUNCE)
        {
            state.dirty_at = None;
            dirty = true;
        }
        state.last_tick = Some(Instant::now());
    }
    if dirty {
        let project = core.with(|state| state.project.clone()).flatten();
        scan(core, cx, project);
    }
}

fn ensure_watcher(core: &ExtensionCore, cx: &mut Ctx<'_>) {
    let project = core.with(|state| state.project.clone()).flatten();
    let roots: Vec<PathBuf> = roots(cx, project.as_deref())
        .into_iter()
        .map(|(_, root)| root)
        .filter(|root| root.is_dir())
        .collect();
    let current = core.with(|state| state.watched.clone()).unwrap_or_default();
    if roots.is_empty() || roots == current {
        return;
    }
    let (tx, rx) = channel();
    let mut watcher =
        match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
            if result.is_ok() {
                let _ = tx.send(());
            }
        }) {
            Ok(watcher) => watcher,
            Err(_) => return,
        };
    for root in &roots {
        let _ = watcher.watch(root, notify::RecursiveMode::Recursive);
    }
    if let Ok(mut state) = core.state.lock() {
        state.watch = Some(Watch {
            _watcher: watcher,
            rx,
        });
        state.watched = roots;
    }
}

/// Aprova uma extensão: pede o modal do `trust` ou conclui se já aprovado.
pub fn approve(core: &ExtensionCore, cx: &mut Ctx<'_>, id: &str) -> Result<Value, String> {
    ensure(core, cx);
    let manifest = core
        .with(|state| {
            state
                .entries
                .get(id)
                .and_then(|entry| entry.manifest.clone())
        })
        .flatten()
        .ok_or_else(|| format!("unknown extension `{id}`"))?;
    if core
        .with(|state| {
            state
                .entries
                .get(id)
                .map(|entry| entry.state == ExtensionState::Active)
        })
        .flatten()
        .unwrap_or(false)
    {
        return Ok(json!({ "approved": true, "already": true }));
    }
    let mut args = Args::new();
    args.insert("kind", json!("extension"));
    args.insert("source_path", json!(manifest.dir));
    args.insert("content", json!(manifest.fingerprint));
    args.insert(
        "summary",
        json!(format!(
            "{} {}: {}",
            manifest.name.resolve(cx.i18n().locale().id()),
            manifest.version,
            manifest
                .capabilities
                .entries()
                .into_iter()
                .map(|(key, scope)| {
                    let label = cx.t(&format!("extension.capability.{key}"));
                    if scope.is_empty() {
                        label
                    } else {
                        format!("{label} ({scope})")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        )),
    );
    let response = cx
        .run_command("trust.request", args)
        .map_err(|error| error.to_string())?;
    let already = response
        .get("approved")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if already {
        finish(core, cx, id, false)?;
        return Ok(json!({ "approved": true }));
    }
    let request = response
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Ok(mut state) = core.state.lock() {
        if let Some(entry) = state.entries.get_mut(id) {
            entry.pending_request = request.clone();
        }
    }
    Ok(json!({ "pending": true, "request_id": request }))
}

fn finish(core: &ExtensionCore, cx: &mut Ctx<'_>, id: &str, dev: bool) -> Result<(), String> {
    let manifest = core
        .with(|state| {
            state
                .entries
                .get(id)
                .and_then(|entry| entry.manifest.clone())
        })
        .flatten()
        .ok_or_else(|| format!("unknown extension `{id}`"))?;
    record_approval(core, &manifest, dev);
    let engine = Arc::new(start_engine(cx, &manifest));
    if let Ok(mut state) = core.state.lock() {
        if let Some(entry) = state.entries.get_mut(id) {
            if let Some(old) = entry.engine.take() {
                old.stop();
            }
            entry.engine = Some(engine);
            entry.state = ExtensionState::Active;
            entry.dev = dev;
            entry.error = None;
            entry.pending_request = None;
        }
    }
    Ok(())
}

/// Conclusão de um pedido do `trust`.
pub fn on_decided(core: &ExtensionCore, cx: &mut Ctx<'_>, event: &TrustDecided) {
    if event.kind != TrustKind::Extension {
        return;
    }
    let id = core
        .with(|state| {
            state
                .entries
                .iter()
                .find(|(_, entry)| {
                    entry.manifest.as_ref().is_some_and(|manifest| {
                        manifest.dir == event.source_path && manifest.hash == event.content_hash
                    })
                })
                .map(|(id, _)| id.clone())
        })
        .flatten();
    let Some(id) = id else {
        return;
    };
    if event.approved {
        let _ = finish(core, cx, &id, false);
    } else if let Ok(mut state) = core.state.lock() {
        if let Some(entry) = state.entries.get_mut(&id) {
            entry.state = ExtensionState::Pending;
            entry.pending_request = None;
            entry.error = Some("approval rejected".to_owned());
        }
    }
}

/// Revoga a aprovação e descarrega a extensão.
pub fn revoke(core: &ExtensionCore, cx: &mut Ctx<'_>, id: &str) -> Result<Value, String> {
    ensure(core, cx);
    let manifest = core
        .with(|state| {
            state
                .entries
                .get(id)
                .and_then(|entry| entry.manifest.clone())
        })
        .flatten()
        .ok_or_else(|| format!("unknown extension `{id}`"))?;
    let mut args = Args::new();
    args.insert("kind", json!("extension"));
    args.insert("source_path", json!(manifest.dir));
    args.insert("content_hash", json!(manifest.hash));
    let _ = cx.run_command("trust.revoke", args);
    if let Ok(mut state) = core.state.lock() {
        if let Some(registry) = &mut state.registry {
            registry.remove(id);
            let _ = registry.persist();
        }
        if let Some(entry) = state.entries.get_mut(id) {
            if let Some(engine) = entry.engine.take() {
                engine.stop();
            }
            entry.state = ExtensionState::Pending;
            entry.dev = false;
            entry.error = None;
        }
    }
    Ok(json!({ "revoked": true }))
}

/// Recarrega uma extensão aprovada.
pub fn reload(core: &ExtensionCore, cx: &mut Ctx<'_>, id: &str) -> Result<Value, String> {
    ensure(core, cx);
    let manifest = core
        .with(|state| {
            state
                .entries
                .get(id)
                .and_then(|entry| entry.manifest.clone())
        })
        .flatten()
        .ok_or_else(|| format!("unknown extension `{id}`"))?;
    let engine = Arc::new(start_engine(cx, &manifest));
    if let Ok(mut state) = core.state.lock() {
        if let Some(entry) = state.entries.get_mut(id) {
            if let Some(old) = entry.engine.take() {
                old.stop();
            }
            entry.engine = Some(engine);
            entry.state = ExtensionState::Active;
            entry.error = None;
        }
    }
    Ok(json!({ "reloaded": true }))
}

fn active_engine(core: &ExtensionCore, extension: &str) -> Result<Arc<Host>, String> {
    core.with(|state| {
        state
            .entries
            .get(extension)
            .filter(|entry| entry.state == ExtensionState::Active)
            .and_then(|entry| entry.engine.clone())
    })
    .flatten()
    .ok_or_else(|| format!("extension `{extension}` is not active"))
}

/// Executa um comando de uma extensão.
pub fn run(
    core: &ExtensionCore,
    cx: &mut Ctx<'_>,
    extension: &str,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    ensure(core, cx);
    let engine = active_engine(core, extension)?;
    let (reply, rx) = channel();
    engine.send(HostCommand::Run {
        command: command.to_owned(),
        args,
        reply,
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= deadline {
            return Err("[time] extension call exceeded the host budget".to_owned());
        }
        engine.tick(cx);
        match rx.try_recv() {
            Ok(result) => return result,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("extension thread stopped".to_owned())
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Executa uma ação de painel.
pub fn action(
    core: &ExtensionCore,
    cx: &mut Ctx<'_>,
    extension: &str,
    action: &str,
    event: Value,
) -> Result<Value, String> {
    ensure(core, cx);
    let engine = active_engine(core, extension)?;
    let (reply, rx) = channel();
    engine.send(HostCommand::Action {
        action: action.to_owned(),
        event,
        reply,
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Instant::now() >= deadline {
            return Err("[time] extension action exceeded the host budget".to_owned());
        }
        engine.tick(cx);
        match rx.try_recv() {
            Ok(result) => return result,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                return Err("extension thread stopped".to_owned())
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

/// Aplica uma configuração da extensão.
pub fn set_setting(core: &ExtensionCore, extension: &str, name: &str, value: Value) -> bool {
    active_engine(core, extension)
        .map(|engine| {
            engine.send(HostCommand::SettingSet {
                name: name.to_owned(),
                value,
            })
        })
        .unwrap_or(false)
}

/// Comandos de todas as extensões ativas.
pub fn commands(core: &ExtensionCore) -> Vec<crate::runtime::ExtCommandInfo> {
    core.with(|state| {
        let mut rows = Vec::new();
        for entry in state.entries.values() {
            if entry.state != ExtensionState::Active {
                continue;
            }
            if let Some(engine) = &entry.engine {
                if let Ok(shared) = engine.shared.lock() {
                    rows.extend(shared.commands.clone());
                }
            }
        }
        rows
    })
    .unwrap_or_default()
}

/// Painéis de um slot, de todas as extensões ativas.
pub fn panels(core: &ExtensionCore, slot: &str) -> Vec<PanelSnapshot> {
    core.with(|state| {
        let mut rows = Vec::new();
        for entry in state.entries.values() {
            if entry.state != ExtensionState::Active {
                continue;
            }
            if let Some(engine) = &entry.engine {
                if let Ok(shared) = engine.shared.lock() {
                    rows.extend(
                        shared
                            .panels
                            .values()
                            .filter(|panel| panel.slot == slot)
                            .cloned(),
                    );
                }
            }
        }
        rows
    })
    .unwrap_or_default()
}

/// Leitura do log de uma extensão, para a página.
pub fn read_log(core: &ExtensionCore, id: &str, lines: usize) -> String {
    core.with(|state| {
        tail(
            &state
                .data_dir
                .join("logs")
                .join("extensions")
                .join(format!("{id}.log")),
            lines,
        )
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::settings::SettingValue;
    use forja_kernel::{Module, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::time::Duration;

    struct TempGuard(PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-ext-state-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn runtime(dir: &Path) -> Runtime {
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(forja_module_trust::TrustModule::new()),
            Box::new(crate::ExtensionModule::new()),
        ];
        Runtime::new(
            modules,
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime builds")
    }

    fn copy_tree(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap().flatten() {
            let target = to.join(entry.file_name());
            if entry.path().is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    fn seed_approval(dir: &Path, manifest: &Manifest) {
        let data = dir.join("data");
        std::fs::create_dir_all(data.join("modules")).unwrap();
        std::fs::create_dir_all(data.join("approvals")).unwrap();
        let trust_path = data.join("modules/trust.json");
        let mut trust: serde_json::Value = std::fs::read_to_string(&trust_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| serde_json::json!({ "approvals": [] }));
        if let Some(rows) = trust.get_mut("approvals").and_then(serde_json::Value::as_array_mut) {
            rows.push(serde_json::json!({
                "kind": "extension",
                "source_path": manifest.dir,
                "content_hash": manifest.hash,
                "approved": true,
                "decided_at": "2026-11-13T00:00:00Z",
                "summary": "test",
                "session_only": false
            }));
        }
        std::fs::write(&trust_path, serde_json::to_vec_pretty(&trust).unwrap()).unwrap();
        let approval_path = data.join("approvals/extensions.json");
        let mut approval: serde_json::Value = std::fs::read_to_string(&approval_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| serde_json::json!({ "approvals": [] }));
        if let Some(rows) = approval
            .get_mut("approvals")
            .and_then(serde_json::Value::as_array_mut)
        {
            rows.push(serde_json::json!({
                "id": manifest.id,
                "version": manifest.version,
                "hash": manifest.hash,
                "capabilities": manifest.capabilities,
                "approved_at": "2026-11-13T00:00:00Z",
                "dev": false
            }));
        }
        std::fs::write(
            &approval_path,
            serde_json::to_vec_pretty(&approval).unwrap(),
        )
        .unwrap();
    }

    fn list(runtime: &mut Runtime) -> Value {
        runtime
            .call_method("extension.list", Map::new())
            .expect("extension.list")
    }

    fn wait_until(runtime: &mut Runtime, check: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..120 {
            let value = list(runtime);
            if check(&value) {
                return value;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        list(runtime)
    }

    fn install_example(dir: &Path, source: &Path, id: &str) -> Manifest {
        let target = dir.join("user").join("extensions").join(id);
        copy_tree(source, &target);
        let manifest = Manifest::read(&target, Layer::User).expect("example manifest");
        seed_approval(dir, &manifest);
        manifest
    }

    #[test]
    fn the_five_examples_load() {
        let dir = temp_dir("examples");
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../extensions");
        for id in [
            "todo-counter",
            "day-summary",
            "open-pr",
            "ports-panel",
            "routine-reminder",
        ] {
            install_example(&dir, &source.join(id), id);
        }
        let mut runtime = runtime(&dir);
        let value = wait_until(&mut runtime, |value| {
            value["extensions"].as_array().is_some_and(|items| {
                items.len() == 5
                    && items.iter().all(|item| item["state"] == "active")
                    && items.iter().any(|item| {
                        item["id"] == "todo-counter"
                            && item["commands"].as_array().is_some_and(|rows| !rows.is_empty())
                            && item["panels"].as_array().is_some_and(|rows| !rows.is_empty())
                    })
                    && items.iter().any(|item| {
                        item["id"] == "day-summary"
                            && item["settings"].as_array().is_some_and(|rows| !rows.is_empty())
                    })
            })
        });
        let items = value["extensions"].as_array().expect("array");
        assert_eq!(items.len(), 5, "{value}");
        assert!(items.iter().all(|item| item["state"] == "active"), "{value}");
        assert!(
            items.iter().any(|item| item["id"] == "todo-counter"
                && item["commands"].as_array().is_some_and(|commands| !commands.is_empty())
                && item["panels"].as_array().is_some_and(|panels| !panels.is_empty())),
            "{value}"
        );
        assert!(items.iter().any(|item| item["id"] == "day-summary"
            && item["settings"].as_array().is_some_and(|settings| !settings.is_empty())));
    }

    #[test]
    fn a_new_extension_registers_command_panel_and_setting_without_recompiling() {
        let dir = temp_dir("new");
        let target = dir.join("user").join("extensions").join("fresh");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join("extension.toml"),
            "id = \"fresh\"\nname = \"Fresh\"\nversion = \"1.0.0\"\nauthor = \"Forja\"\napi = \"1\"\n[capabilities]\ncommands = true\n\"ui.panel\" = true\nsettings = true\n",
        )
        .unwrap();
        std::fs::write(
            target.join("init.lua"),
            r#"
forja.settings.schema({ { name = "who", type = "text", default = "mundo", label = "Quem" } })
forja.commands.register({ id = "hello", title = "Hello", run = function(args) return { ok = true } end })
forja.ui.panel({ id = "fresh", title = "Fresh", slot = "right_panel", tree = { forja.ui.text({ value = "pronto" }) } })
"#,
        )
        .unwrap();
        let manifest = Manifest::read(&target, Layer::User).unwrap();
        seed_approval(&dir, &manifest);
        let mut runtime = runtime(&dir);
        let value = wait_until(&mut runtime, |value| {
            value["extensions"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["state"] == "active"
                        && item["commands"].as_array().is_some_and(|rows| !rows.is_empty())
                        && item["settings"].as_array().is_some_and(|rows| !rows.is_empty())
                        && item["panels"].as_array().is_some_and(|rows| !rows.is_empty())
                })
            })
        });
        let item = value["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "fresh")
            .expect("fresh");
        assert_eq!(item["state"], "active");
        assert_eq!(item["commands"][0]["id"], "ext.fresh.hello");
        assert_eq!(item["settings"][0]["name"], "who");
        assert_eq!(item["panels"][0]["id"], "fresh");
    }

    #[test]
    fn a_changed_hash_asks_for_approval_again_and_dev_mode_reloads() {
        let dir = temp_dir("hash");
        let target = dir.join("user").join("extensions").join("hashy");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            target.join("extension.toml"),
            "id = \"hashy\"\nname = \"Hashy\"\nversion = \"1.0.0\"\nauthor = \"Forja\"\napi = \"1\"\n[capabilities]\ncommands = true\n",
        )
        .unwrap();
        std::fs::write(
            target.join("init.lua"),
            "forja.commands.register({ id = 'one', title = 'One', run = function() return 1 end })\n",
        )
        .unwrap();
        let manifest = Manifest::read(&target, Layer::User).unwrap();
        seed_approval(&dir, &manifest);
        let mut runtime = runtime(&dir);
        wait_until(&mut runtime, |value| {
            value["extensions"].as_array().is_some_and(|items| {
                items.iter().any(|item| item["id"] == "hashy" && item["state"] == "active")
            })
        });
        std::fs::write(
            target.join("init.lua"),
            "forja.commands.register({ id = 'one', title = 'One', run = function() return 2 end })\n",
        )
        .unwrap();
        runtime
            .call_method("extension.rescan", Map::new())
            .expect("rescan");
        let value = list(&mut runtime);
        let item = value["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "hashy")
            .expect("hashy");
        assert_eq!(item["state"], "changed");
        let approved = runtime
            .call_method(
                "extension.approve",
                Map::from_iter([("id".to_owned(), serde_json::json!("hashy"))]),
            )
            .expect("approve");
        assert_eq!(approved["pending"], true);
        runtime
            .set_setting("extension.dev.watch", SettingValue::Bool(true))
            .expect("dev watch");
        runtime
            .call_method("extension.rescan", Map::new())
            .expect("rescan");
        let value = wait_until(&mut runtime, |value| {
            value["extensions"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["id"] == "hashy" && item["state"] == "active" && item["dev"] == true
                })
            })
        });
        let item = value["extensions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "hashy")
            .expect("hashy");
        assert_eq!(item["state"], "active");
        assert_eq!(item["dev"], true);
    }
}
