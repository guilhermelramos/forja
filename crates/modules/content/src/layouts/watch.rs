//! Observador de arquivos, filtro de eventos e janela de debounce.

use super::{LayoutState, LayoutStore};
use forja_kernel::Ctx;
use notify::Watcher as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Silêncio depois do último evento relevante antes de reler.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Observador vivo enquanto a leitura automática estiver ligada.
pub struct Watch {
    pub(crate) _watcher: notify::RecommendedWatcher,
    pub(crate) generation: Arc<AtomicU64>,
}

/// Raizes que produzem eventos relevantes.
#[derive(Clone, Debug, Default)]
pub struct WatchRoots {
    pub user: Option<PathBuf>,
    pub project: Option<PathBuf>,
    pub builtin: Option<PathBuf>,
}

impl WatchRoots {
    pub fn from_context(user: PathBuf, project: Option<PathBuf>, builtin: Option<PathBuf>) -> Self {
        Self {
            user: Some(user),
            project,
            builtin,
        }
    }

    /// Diretórios pai e pastas de arranjos, sem tocar o disco na thread da UI.
    pub fn candidates(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Some(user) = &self.user {
            roots.push(user.clone());
            roots.push(user.join(super::LAYOUT_DIR));
        }
        if let Some(project) = &self.project {
            let local = project.join(".forja");
            roots.push(project.clone());
            roots.push(local.clone());
            roots.push(local.join(super::LAYOUT_DIR));
        }
        if let Some(builtin) = &self.builtin {
            roots.push(builtin.clone());
            roots.push(builtin.join(super::LAYOUT_DIR));
        }
        roots.sort();
        roots.dedup();
        roots
    }

    /// Um evento só interessa se toca um `.toml` de arranjo ou cria uma das
    /// pastas que ainda não existiam.
    pub fn relevant(&self, event: &notify::Event) -> bool {
        let creation = matches!(event.kind, notify::event::EventKind::Create(_));
        event.paths.iter().any(|path| {
            self.layout_file(path)
                || (creation && self.layout_folder(path))
        })
    }

    fn layout_file(&self, path: &Path) -> bool {
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("toml"))
        {
            return false;
        }
        self.folder_for(path).is_some()
    }

    fn folder_for(&self, path: &Path) -> Option<&Path> {
        for root in [&self.user, &self.builtin] {
            if let Some(root) = root {
                let layouts = root.join(super::LAYOUT_DIR);
                if path.starts_with(&layouts) && path.parent() == Some(layouts.as_path()) {
                    return Some(root);
                }
            }
        }
        let project = self.project.as_ref()?;
        let layouts = project.join(".forja").join(super::LAYOUT_DIR);
        (path.starts_with(&layouts) && path.parent() == Some(layouts.as_path())).then_some(project)
    }

    fn layout_folder(&self, path: &Path) -> bool {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if name != ".forja" && name != super::LAYOUT_DIR {
            return false;
        }
        if name == ".forja" {
            return self.project.as_ref().is_some_and(|root| path == root.join(".forja"));
        }
        self.user
            .as_ref()
            .is_some_and(|root| path == root.join(super::LAYOUT_DIR))
            || self.builtin.as_ref().is_some_and(|root| path == root.join(super::LAYOUT_DIR))
            || self
                .project
                .as_ref()
                .is_some_and(|root| path == root.join(".forja").join(super::LAYOUT_DIR))
    }
}

/// Compatível com a API antiga: candidatos sem filtrar existência.
pub fn watch_roots(
    user: &Path,
    project: Option<&Path>,
    builtin: Option<&Path>,
) -> Vec<PathBuf> {
    WatchRoots::from_context(
        user.to_path_buf(),
        project.map(Path::to_path_buf),
        builtin.map(Path::to_path_buf),
    )
    .candidates()
}

/// Instala ou descarta o observador. Toda E/S acontece em `Tasks`.
pub fn ensure_watcher(store: &Arc<LayoutStore>, cx: &Ctx<'_>, enabled: bool) {
    let candidates = if enabled {
        let state = store.state_lock();
        let root = state.as_ref().and_then(|state| state.project.clone());
        WatchRoots::from_context(
            cx.user_dir().to_path_buf(),
            root,
            crate::model::builtin_root(),
        )
        .candidates()
    } else {
        Vec::new()
    };
    let (current, token, has_watch) = store
        .with(|state: &mut LayoutState| {
            (state.watch_requested.clone(), state.watch_token, state.watch.is_some())
        })
        .unwrap_or_default();
    if candidates == current && candidates.is_empty() == !has_watch {
        return;
    }
    let token = if candidates == current { token } else { token.wrapping_add(1) };
    let _ = store.with(|state: &mut LayoutState| {
        state.watch_requested = candidates.clone();
        state.watch_token = token;
        if !enabled {
            // Desativar também cancela o prazo pendente.
            state.dirty_at = None;
            state.deadline_scheduled = false;
        }
    });
    if candidates.is_empty() {
        let old = store.with(|state: &mut LayoutState| state.watch.take()).flatten();
        if let Some(watch) = old {
            cx.spawn(move || drop(watch), |_cx, _| {});
        }
        return;
    }
    let roots = WatchRoots::from_context(
        cx.user_dir().to_path_buf(),
        store.with(|state: &mut LayoutState| state.project.clone()).flatten(),
        crate::model::builtin_root(),
    );
    let generation = Arc::new(AtomicU64::new(0));
    let callback_generation = Arc::clone(&generation);
    let wake = cx.waker();
    let store = Arc::clone(store);
    cx.spawn_service(
        move || {
            let existing = roots
                .candidates()
                .into_iter()
                .filter(|root| root.is_dir())
                .collect::<Vec<_>>();
            if existing.is_empty() {
                return None;
            }
            let wake = Arc::clone(&wake);
            let generation = Arc::clone(&callback_generation);
            let watch_generation = Arc::clone(&generation);
            let filter = roots.clone();
            let mut watcher = notify::recommended_watcher(
                move |result: notify::Result<notify::Event>| {
                    if let Ok(event) = result {
                        if filter.relevant(&event) {
                            generation.fetch_add(1, Ordering::SeqCst);
                            wake();
                        }
                    }
                },
            )
            .ok()?;
            for root in &existing {
                let _ = watcher.watch(root, notify::RecursiveMode::Recursive);
            }
            Some(Watch { _watcher: watcher, generation: watch_generation })
        },
        move |cx, installed| {
            let Some(installed) = installed else {
                return;
            };
            let stale = store
                .with(|state: &mut LayoutState| {
                    if state.watch_token != token || state.watch_requested.is_empty() {
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

/// Verdadeiro quando o prazo de coalescência terminou.
pub fn due(state: &LayoutState, now: std::time::Instant) -> bool {
    state
        .dirty_at
        .is_some_and(|at| now.duration_since(at) >= DEBOUNCE)
}
