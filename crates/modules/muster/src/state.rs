//! Estado compartilhado do Mutirão e persistência no store do módulo.

use crate::model::{now_ms, MusterFile};
use forja_kernel::store::write_atomic_bytes;
use forja_kernel::{Ctx, LoadResult};
use std::sync::{Arc, Mutex};

/// Estado em memória.
#[derive(Debug, Default)]
pub struct MusterState {
    /// Dados persistidos.
    pub store: MusterFile,
    /// O arquivo já foi lido (ou confirmado ausente).
    pub loaded: bool,
    /// O arquivo existia e não pôde ser lido; gravar por cima apagaria dados.
    pub load_failed: bool,
    /// Aviso curto para a tela.
    pub notice: Option<String>,
    /// Prévia semeada para captura.
    pub preview_seeded: bool,
}

/// Ponteiro compartilhado.
pub type Shared = Arc<Mutex<MusterState>>;

/// Cria o estado compartilhado.
pub fn new_shared() -> Shared {
    Arc::new(Mutex::new(MusterState::default()))
}

/// Acesso curto ao estado. Nunca chame comandos dentro do fecho.
pub fn with<T>(shared: &Shared, change: impl FnOnce(&mut MusterState) -> T) -> Option<T> {
    shared.lock().ok().map(|mut state| change(&mut state))
}

/// Lê o arquivo uma vez.
pub fn ensure_loaded(cx: &mut Ctx<'_>, shared: &Shared) {
    {
        let Ok(state) = shared.lock() else {
            return;
        };
        if state.loaded {
            return;
        }
    }
    let result = cx.store().load::<MusterFile>();
    if let Ok(mut state) = shared.lock() {
        state.loaded = true;
        match result {
            LoadResult::Loaded(file) => state.store = file,
            LoadResult::Missing => state.store = MusterFile::default(),
            LoadResult::Quarantined(_) => {
                state.load_failed = true;
                state.notice = Some("muster-notice-load-failed".to_owned());
            }
        }
    }
}

/// Grava o arquivo em segundo plano.
pub fn persist(cx: &mut Ctx<'_>, shared: &Shared) {
    let snapshot = match shared.lock() {
        Ok(state) => {
            if state.load_failed {
                return;
            }
            state.store.clone()
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
        |_, _| {},
    );
}

/// Semeia uma prévia com dados fictícios para a captura da página.
pub fn seed_preview(shared: &Shared) {
    if std::env::var_os("FORJA_MUSTER_PREVIEW").is_none() {
        return;
    }
    let Ok(mut state) = shared.lock() else {
        return;
    };
    if state.preview_seeded {
        return;
    }
    state.preview_seeded = true;
    state.loaded = true;
    let now = now_ms();
    let run = state.store.create_run(
        "Pedido 123 — fatura",
        "Entregar a fatura com testes e revisão.",
        "coordinator",
        Some("C:/forja/pedido-123".to_owned()),
    );
    let first = state
        .store
        .add_task(
            &run.id,
            "Parser e índice",
            "",
            Vec::new(),
        )
        .expect("preview task");
    let second = state
        .store
        .add_task(
            &run.id,
            "Página do Acervo",
            "",
            vec![first.id.clone()],
        )
        .expect("preview task");
    state
        .store
        .add_task(
            &run.id,
            "Revisão final",
            "",
            vec![first.id.clone()],
        )
        .expect("preview task");
    let assignment = state
        .store
        .start(&first.id, "claude-code", "C:/forja/pedido-123")
        .expect("preview assignment");
    state
        .store
        .launched(
            &assignment.id,
            "claude-code",
            Some("1".to_owned()),
            None,
            "tui",
        )
        .ok();
    state
        .store
        .handoff(
            &assignment.id,
            crate::model::HandoffResult::Done,
            "Parser e índice prontos; 15 testes passando.",
            "cargo test -p forja-module-chat",
        )
        .ok();
    let second_assignment = state
        .store
        .start(&second.id, "codex", "C:/forja/pedido-123")
        .expect("preview assignment");
    state
        .store
        .launched(
            &second_assignment.id,
            "codex",
            Some("2".to_owned()),
            None,
            "tui",
        )
        .ok();
    state
        .store
        .note_from_helper(
            &second_assignment.id,
            "Os filtros por data estão prontos; falta revisar o vazio.",
            crate::model::NoteKind::Status,
        )
        .ok();
    if let Some(preview) = state.store.runs.iter_mut().find(|item| item.id == run.id) {
        preview.plan_approved = false;
        preview.budget_usd = 12.0;
        preview.spent_usd = 3.4;
    }
    let _ = now;
}

