//! Observador de arquivos e janela de debounce.

use super::{LayoutState, LayoutStore};
use super::LAYOUT_DIR;
use forja_kernel::Ctx;
use notify::Watcher as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Silêncio depois do último evento do disco antes de reler.
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Observador vivo enquanto a leitura automática estiver ligada.
pub struct Watch {
    pub(crate) _watcher: notify::RecommendedWatcher,
    pub(crate) generation: Arc<AtomicU64>,
}

/// Pais das pastas de arranjos: captura criação da pasta e renomes atômicos.
pub fn watch_roots(user: &std::path::Path, project: Option<&std::path::Path>, builtin: Option<&std::path::Path>) -> Vec<PathBuf> {
    let mut roots = vec![user.to_path_buf(), user.join(LAYOUT_DIR)];
    if let Some(project) = project {
        let local = project.join(".forja");
        roots.push(local.clone());
        roots.push(local.join(LAYOUT_DIR));
    }
    if let Some(builtin) = builtin {
        roots.push(builtin.to_path_buf());
        roots.push(builtin.join(LAYOUT_DIR));
    }
    roots.retain(|root| root.is_dir());
    roots.sort();
    roots.dedup();
    roots
}

/// Instala ou descarta o observador conforme a raiz e a preferência atuais.
pub fn ensure_watcher(store: &Arc<LayoutStore>, cx: &Ctx<'_>, enabled: bool) {
    let roots = {
        let Some(state) = store.state_lock() else {
            return;
        };
        if enabled {
            watch_roots(
                cx.user_dir(),
                state.project.as_deref(),
                crate::model::builtin_root().as_deref(),
            )
        } else {
            Vec::new()
        }
    };
    let current = store
        .with(|state: &mut LayoutState| state.watch_requested.clone())
        .unwrap_or_default();
    if roots == current && !(roots.is_empty() && store.has_watch()) {
        return;
    }
    let _ = store.with(|state: &mut LayoutState| state.watch_requested = roots.clone());
    if roots.is_empty() {
        let old = store.with(|state: &mut LayoutState| state.watch.take()).flatten();
        if let Some(watch) = old {
            cx.spawn(move || drop(watch), |_cx, _| {});
        }
        return;
    }
    let generation = Arc::new(AtomicU64::new(0));
    let callback_generation = Arc::clone(&generation);
    let wake = cx.waker();
    let store = Arc::clone(store);
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
                .with(|state: &mut LayoutState| {
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

/// Verdadeiro quando o prazo de coalescência terminou.
pub fn due(state: &LayoutState, now: std::time::Instant) -> bool {
    state
        .dirty_at
        .is_some_and(|at| now.duration_since(at) >= DEBOUNCE)
}
