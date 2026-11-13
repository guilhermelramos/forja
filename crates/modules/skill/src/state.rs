//! Estado compartilhado do módulo: a última leitura, o que está em andamento, o aviso, a operação
//! que espera confirmação e o conflito que pede sobrescrita. A tela e os comandos seguram o lock
//! só para ler ou trocar campos; disco e rede ficam em `Ctx::spawn`.

use crate::model::{Listing, Scope};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Origem de uma instalação: uma pasta com `SKILL.md`, ou um zip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallKind {
    Folder,
    Zip,
}

/// Operação que a tela pediu e que o usuário ainda precisa confirmar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Install {
        kind: InstallKind,
        source: PathBuf,
        owner: String,
        owner_name: String,
        scope: Scope,
    },
    Copy {
        from: PathBuf,
        owner: String,
        owner_name: String,
        scope: Scope,
    },
    Remove {
        path: PathBuf,
        folder: String,
        owner: String,
        owner_name: String,
        scope: Scope,
    },
}

/// Aviso da última operação: chave de tradução e o texto técnico que vai junto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub key: String,
    pub detail: String,
}

#[derive(Default)]
pub struct State {
    pub listing: Listing,
    /// Já houve uma leitura (mesmo que vazia). Evita reler a cada quadro.
    pub scanned: bool,
    pub scanning: bool,
    pub notice: Option<Notice>,
    pub pending: Option<Pending>,
    /// Caminho da skill que já existe, quando a última instalação ou cópia pediu sobrescrita.
    pub conflict: Option<String>,
    pub selected: Option<String>,
    /// `SKILL.md` do item selecionado: (caminho, texto). Preenchido em `ctx.spawn`.
    pub body: Option<(String, String)>,
}

#[derive(Clone, Default)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        Self::default()
    }

    /// Roda `change` com o lock. `None` se o lock estiver envenenado.
    pub fn with<T>(&self, change: impl FnOnce(&mut State) -> T) -> Option<T> {
        self.0.lock().ok().map(|mut state| change(&mut state))
    }
}

/// Modo de captura (`FORJA_FIXTURES=1`): a lista vem de dados de exemplo, sem ler o disco.
pub fn fixtures_enabled() -> bool {
    std::env::var("FORJA_FIXTURES")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}
