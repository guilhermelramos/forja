//! Dados de captura de tela (`FORJA_FIXTURES=1`): destinos SSH, testes e distros do WSL de exemplo.
//! Só entram quando não há destinos reais, e nunca são gravados no arquivo real.

use crate::ssh::ProbeError;
use crate::state::ProbeResult;
use crate::target::{LocalForward, SshTarget};
use crate::wsl::WslDistro;
use std::collections::BTreeMap;

pub fn targets() -> Vec<SshTarget> {
    vec![
        SshTarget {
            id: "ssh-build".into(),
            label: "build".into(),
            host: "build.example.com".into(),
            user: "ci".into(),
            port: 2222,
            identity_file: "C:/Users/dev/.ssh/id_ed25519".into(),
            proxy_jump: String::new(),
            forwards: vec![LocalForward {
                local_port: 7001,
                remote_host: "db.internal".into(),
                remote_port: 5432,
                active: true,
            }],
        },
        SshTarget {
            id: "ssh-staging".into(),
            label: "staging".into(),
            host: "10.0.4.12".into(),
            user: "ana".into(),
            port: 22,
            identity_file: String::new(),
            proxy_jump: "build".into(),
            forwards: Vec::new(),
        },
    ]
}

/// Último teste: o primeiro conectou devagar; o segundo foi recusado.
pub fn probes(now: i64) -> BTreeMap<String, ProbeResult> {
    let mut probes = BTreeMap::new();
    probes.insert(
        "ssh-build".to_owned(),
        ProbeResult {
            ok: true,
            error: None,
            elapsed_ms: 412,
            at: now - 120,
        },
    );
    probes.insert(
        "ssh-staging".to_owned(),
        ProbeResult {
            ok: false,
            error: Some(ProbeError::AuthFailed),
            elapsed_ms: 980,
            at: now - 300,
        },
    );
    probes
}

pub fn distros() -> Vec<WslDistro> {
    vec![
        WslDistro {
            name: "Ubuntu-24.04".into(),
            state: "Running".into(),
            version: 2,
            is_default: true,
        },
        WslDistro {
            name: "Debian".into(),
            state: "Stopped".into(),
            version: 2,
            is_default: false,
        },
    ]
}
