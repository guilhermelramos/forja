//! Registro de aprovações por hash, fora do repositório.

use crate::manifest::Capabilities;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Uma aprovação de extensão ligada ao hash exato.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub id: String,
    pub version: String,
    pub hash: String,
    pub capabilities: Capabilities,
    pub approved_at: String,
    #[serde(default)]
    pub dev: bool,
}

/// Arquivo persistido.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApprovalFile {
    #[serde(default)]
    pub approvals: Vec<Approval>,
}

/// Lê o registro; arquivo ausente ou corrompido vira lista vazia.
pub fn load(path: &Path) -> Vec<Approval> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<ApprovalFile>(&text)
        .map(|file| file.approvals)
        .unwrap_or_default()
}

/// Grava o registro de forma atômica.
pub fn save(path: &Path, approvals: &[Approval]) -> std::io::Result<()> {
    let file = ApprovalFile {
        approvals: approvals.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&file)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    forja_kernel::store::write_atomic_bytes(path, &bytes)
}

/// Registro de aprovações do processo.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub path: PathBuf,
    pub approvals: Vec<Approval>,
}

impl Registry {
    pub fn open(path: PathBuf) -> Self {
        let approvals = load(&path);
        Self { path, approvals }
    }

    pub fn find(&self, id: &str) -> Option<&Approval> {
        self.approvals.iter().rev().find(|entry| entry.id == id)
    }

    pub fn put(&mut self, approval: Approval) {
        self.approvals
            .retain(|entry| !(entry.id == approval.id && entry.hash == approval.hash));
        self.approvals.push(approval);
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.approvals.len();
        self.approvals.retain(|entry| entry.id != id);
        before != self.approvals.len()
    }

    pub fn persist(&self) -> std::io::Result<()> {
        save(&self.path, &self.approvals)
    }
}

/// Data ISO 8601 do momento.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_is_keyed_by_id_and_hash() {
        let dir = std::env::temp_dir().join(format!(
            "forja-ext-approval-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("approvals.json");
        let mut registry = Registry::open(path.clone());
        registry.put(Approval {
            id: "x".into(),
            version: "1.0.0".into(),
            hash: "a".into(),
            capabilities: Capabilities::default(),
            approved_at: now(),
            dev: false,
        });
        registry.persist().unwrap();
        let reopened = Registry::open(path);
        assert_eq!(reopened.find("x").map(|entry| entry.hash.as_str()), Some("a"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
