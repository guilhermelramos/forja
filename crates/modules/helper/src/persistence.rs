//! Gravação serializada do arquivo persistente do helper.

use crate::store::HelperFile;
use crate::Shared;
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::Ctx;
use std::sync::Arc;

pub(crate) fn save_file(cx: &mut Ctx<'_>, core: &Shared) {
    let Some(file) = core
        .with_state(|state| {
            let file = state.file.clone();
            if state.launch.write_inflight {
                state.launch.pending_write = Some(file.clone());
                None
            } else {
                state.launch.write_inflight = true;
                Some(file)
            }
        })
        .ok()
        .flatten()
    else {
        return;
    };
    spawn_write(cx, core, file);
}

/// Grava um snapshot e, ao terminar, publica apenas o snapshot mais recente.
fn spawn_write(cx: &mut Ctx<'_>, core: &Shared, file: HelperFile) {
    let path = cx.store().path();
    let owned = Arc::clone(core);
    cx.spawn(
        move || {
            if let Ok(bytes) = serde_json::to_vec_pretty(&file) {
                let _ = write_atomic_bytes(&path, &bytes);
            }
        },
        move |cx, ()| {
            let next = owned
                .with_state(|state| {
                    state.launch.write_inflight = false;
                    let next = state.launch.pending_write.take();
                    if next.is_some() {
                        state.launch.write_inflight = true;
                    }
                    next
                })
                .ok()
                .flatten();
            if let Some(file) = next {
                spawn_write(cx, &owned, file);
            }
        },
    );
}

