//! Destinos SSH salvos e encaminhamentos de porta. Não guardam segredo: senha e chave ficam
//! com o `ssh` e com o agente. O arquivo de identidade é só um caminho.

use serde::{Deserialize, Serialize};

pub const DEFAULT_SSH_PORT: u16 = 22;

/// Um encaminhamento local: `127.0.0.1:<local_port>` leva a `<remote_host>:<remote_port>` pelo host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocalForward {
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    /// Deve subir ao abrir o app (com `remote.forward.mode` em `auto`).
    pub active: bool,
}

/// Um host SSH salvo. `host` é o alias ou o endereço que o `ssh` resolve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SshTarget {
    pub id: String,
    pub label: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub identity_file: String,
    pub proxy_jump: String,
    pub forwards: Vec<LocalForward>,
}

impl Default for SshTarget {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            host: String::new(),
            user: String::new(),
            port: DEFAULT_SSH_PORT,
            identity_file: String::new(),
            proxy_jump: String::new(),
            forwards: Vec::new(),
        }
    }
}

/// Arquivo do store do módulo: destinos e o que já foi importado.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TargetFile {
    pub targets: Vec<SshTarget>,
}
