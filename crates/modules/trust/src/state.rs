//! Registro de aprovações por hash e fila de pedidos pendentes.

use forja_contracts::{TrustEntry, TrustKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

/// Uma decisão guardada ou mantida só na sessão.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    /// Tipo do conteúdo.
    pub kind: TrustKind,
    /// Caminho canônico.
    pub source_path: String,
    /// SHA-256 exato do conteúdo mostrado.
    pub content_hash: String,
    /// Verdadeiro para aprovação, falso para recusa.
    pub approved: bool,
    /// Data ISO 8601.
    pub decided_at: String,
    /// Resumo curto para a lista.
    pub summary: String,
    /// Aprovação que morre ao fechar o app.
    #[serde(default)]
    pub session_only: bool,
}

/// Pedido aguardando decisão na interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRequest {
    /// Correlação devolvida por `trust.request`.
    pub request_id: String,
    /// Tipo do conteúdo.
    pub kind: TrustKind,
    /// Caminho canônico.
    pub source_path: String,
    /// Hash do conteúdo.
    pub content_hash: String,
    /// Resumo mostrado no modal.
    pub summary: String,
}

/// Arquivo persistido.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrustFile {
    /// Decisões persistentes.
    #[serde(default)]
    pub approvals: Vec<Approval>,
}

/// Estado em memória do módulo.
#[derive(Default)]
pub struct TrustState {
    /// Decisões em memória.
    pub approvals: Vec<Approval>,
    /// Pedidos aguardando decisão.
    pub pending: VecDeque<PendingRequest>,
    store_path: Option<PathBuf>,
    loaded: bool,
    next_request: u64,
}

impl TrustState {
    /// Carrega o arquivo do store uma vez.
    pub fn ensure_loaded(&mut self, path: &Path) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.store_path = Some(path.to_path_buf());
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(file) = serde_json::from_str::<TrustFile>(&text) else {
            return;
        };
        self.approvals = file.approvals;
    }

    /// Caminho do store, quando já carregado.
    pub fn store_path(&self) -> Option<&Path> {
        self.store_path.as_deref()
    }

    /// Consulta a decisão mais recente para o hash exato.
    pub fn check(
        &self,
        kind: TrustKind,
        source_path: &str,
        content_hash: &str,
    ) -> (bool, Option<String>) {
        self.approvals
            .iter()
            .rev()
            .find(|entry| {
                entry.kind == kind
                    && entry.source_path == source_path
                    && entry.content_hash == content_hash
            })
            .map_or((false, None), |entry| {
                (entry.approved, Some(entry.decided_at.clone()))
            })
    }

    /// Cria um pedido quando o conteúdo ainda não tem decisão para este hash.
    pub fn request(
        &mut self,
        kind: TrustKind,
        source_path: String,
        content: &str,
        summary: String,
    ) -> (Option<String>, bool) {
        let hash = sha256_hex(content);
        let (approved, _) = self.check(kind, &source_path, &hash);
        if approved {
            return (None, true);
        }
        let request_id = format!(
            "trust-{}-{}",
            std::process::id(),
            self.next_request
        );
        self.next_request += 1;
        self.pending.push_back(PendingRequest {
            request_id: request_id.clone(),
            kind,
            source_path,
            content_hash: hash,
            summary,
        });
        (Some(request_id), false)
    }

    /// Aprova o conteúdo sem modal. Usado só pelo modo de desenvolvimento de
    /// extensões, que já foi ligado explicitamente pelo usuário.
    pub fn grant(
        &mut self,
        kind: TrustKind,
        source_path: String,
        content: &str,
        summary: String,
    ) {
        let approval = Approval {
            kind,
            source_path,
            content_hash: sha256_hex(content),
            approved: true,
            decided_at: chrono::Utc::now().to_rfc3339(),
            summary,
            session_only: false,
        };
        self.approvals.retain(|entry| {
            !(entry.kind == approval.kind
                && entry.source_path == approval.source_path
                && entry.content_hash == approval.content_hash)
        });
        self.approvals.push(approval);
    }

    /// Decide um pedido e devolve a aprovação criada.
    pub fn decide(
        &mut self,
        request_id: &str,
        approved: bool,
        session_only: bool,
    ) -> Option<Approval> {
        let index = self
            .pending
            .iter()
            .position(|request| request.request_id == request_id)?;
        let request = self.pending.remove(index)?;
        let approval = Approval {
            kind: request.kind,
            source_path: request.source_path,
            content_hash: request.content_hash,
            approved,
            decided_at: chrono::Utc::now().to_rfc3339(),
            summary: request.summary,
            session_only,
        };
        self.approvals.retain(|entry| {
            !(entry.kind == approval.kind
                && entry.source_path == approval.source_path
                && entry.content_hash == approval.content_hash)
        });
        self.approvals.push(approval.clone());
        Some(approval)
    }

    /// Remove decisões de um caminho, ou só a do hash informado.
    pub fn revoke(
        &mut self,
        kind: TrustKind,
        source_path: &str,
        content_hash: Option<&str>,
    ) -> bool {
        let before = self.approvals.len();
        self.approvals.retain(|entry| {
            !(entry.kind == kind
                && entry.source_path == source_path
                && content_hash.is_none_or(|hash| entry.content_hash == hash))
        });
        before != self.approvals.len()
    }

    /// Lista para a seção de Ajustes.
    pub fn list(&self) -> Vec<TrustEntry> {
        self.approvals
            .iter()
            .map(|entry| TrustEntry {
                kind: entry.kind,
                source_path: entry.source_path.clone(),
                content_hash: entry.content_hash.clone(),
                approved: entry.approved,
                decided_at: entry.decided_at.clone(),
                summary: entry.summary.clone(),
                session_only: entry.session_only,
            })
            .collect()
    }

    /// Conteúdo que deve ir para o disco.
    pub fn file(&self) -> TrustFile {
        TrustFile {
            approvals: self
                .approvals
                .iter()
                .filter(|entry| !entry.session_only)
                .cloned()
                .collect(),
        }
    }
}

/// SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Converte o texto do contrato em `TrustKind`.
pub fn parse_kind(value: &str) -> Option<TrustKind> {
    match value {
        "setup" => Some(TrustKind::Setup),
        "teardown" => Some(TrustKind::Teardown),
        "recipe" => Some(TrustKind::Recipe),
        "command" => Some(TrustKind::Command),
        "extension" => Some(TrustKind::Extension),
        "helper_profile" => Some(TrustKind::HelperProfile),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-trust-{label}-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn approval_is_bound_to_the_exact_hash() {
        let mut state = TrustState::default();
        let path = temp_path("hash");
        state.ensure_loaded(&path);
        let (request, _) = state.request(
            TrustKind::Recipe,
            "C:/project/.forja/recipes/x.toml".into(),
            "command = \"echo one\"",
            "Recipe x".into(),
        );
        let approval = state
            .decide(&request.expect("request"), true, false)
            .expect("approval");
        assert_eq!(approval.content_hash, sha256_hex("command = \"echo one\""));
        assert!(state.check(
            TrustKind::Recipe,
            "C:/project/.forja/recipes/x.toml",
            &approval.content_hash
        ).0);
        assert!(!state.check(
            TrustKind::Recipe,
            "C:/project/.forja/recipes/x.toml",
            &sha256_hex("command = \"echo two\"")
        ).0);
    }

    #[test]
    fn revoke_and_session_only_decisions() {
        let mut state = TrustState::default();
        state.ensure_loaded(&temp_path("revoke"));
        let (request, _) = state.request(
            TrustKind::Extension,
            "C:/project/.forja/extensions/x".into(),
            "code",
            "Extension x".into(),
        );
        let approval = state
            .decide(&request.expect("request"), true, true)
            .expect("approval");
        assert!(state.file().approvals.is_empty());
        assert!(state.revoke(TrustKind::Extension, &approval.source_path, None));
        assert!(!state.check(TrustKind::Extension, &approval.source_path, &approval.content_hash).0);
    }

    #[test]
    fn already_approved_content_does_not_open_another_request() {
        let mut state = TrustState::default();
        state.ensure_loaded(&temp_path("repeat"));
        let source = "C:/project/forja.toml".to_owned();
        let (request, _) = state.request(TrustKind::Setup, source.clone(), "npm install", "Setup".into());
        state.decide(&request.expect("request"), true, false);
        let (again, approved) = state.request(TrustKind::Setup, source, "npm install", "Setup".into());
        assert_eq!(again, None);
        assert!(approved);
    }
}
