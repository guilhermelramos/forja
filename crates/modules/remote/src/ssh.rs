//! Destinos SSH: validação, montagem da linha de comando do `ssh`, e o teste de conexão com
//! o mapeamento do stderr para erros que o usuário entende.
//!
//! Cada argumento é um `String` separado; nada é juntado por shell. O host sempre vem depois de
//! todas as opções, e valores que o `ssh` leria como opção (começando com `-`) são recusados.

use crate::target::{LocalForward, SshTarget};

/// Política da chave de host (`remote.ssh.host_key_policy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HostKeyPolicy {
    /// Só aceita chave já conhecida.
    Strict,
    /// Pergunta na primeira conexão (no terminal). Em teste automático, falha sem perguntar.
    #[default]
    Ask,
    /// Aceita uma chave nova e grava; recusa chave que mudou.
    AcceptNew,
}

impl HostKeyPolicy {
    pub fn parse(value: &str) -> Self {
        match value {
            "strict" => Self::Strict,
            "accept_new" => Self::AcceptNew,
            _ => Self::Ask,
        }
    }

    fn option(self) -> Option<&'static str> {
        match self {
            Self::Strict => Some("yes"),
            Self::Ask => None,
            Self::AcceptNew => Some("accept-new"),
        }
    }
}

/// Opções vindas das configurações `remote.ssh.*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshOptions {
    pub connect_timeout_s: u32,
    /// 0 desliga o keep-alive.
    pub keep_alive_s: u32,
    pub agent_forward: bool,
    pub reuse_connections: bool,
    pub host_key: HostKeyPolicy,
}

impl Default for SshOptions {
    fn default() -> Self {
        Self {
            connect_timeout_s: 20,
            keep_alive_s: 30,
            agent_forward: false,
            reuse_connections: true,
            host_key: HostKeyPolicy::Ask,
        }
    }
}

/// Confere um campo que vai como argumento do `ssh`.
fn check_token(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} is required"));
    }
    if value.starts_with('-') {
        return Err(format!("{field} must not start with '-'"));
    }
    if value.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("{field} must not contain spaces"));
    }
    Ok(())
}

/// Confere um destino antes de salvar ou usar.
pub fn validate_target(target: &SshTarget) -> Result<(), String> {
    check_token("Host", &target.host)?;
    if !target.user.is_empty() {
        check_token("User", &target.user)?;
    }
    if !target.proxy_jump.is_empty() {
        check_token("Proxy jump", &target.proxy_jump)?;
    }
    if target.identity_file.contains(['\n', '\r', '\0']) {
        return Err("Identity file contains control characters".into());
    }
    if target.port == 0 {
        return Err("Port must be between 1 and 65535".into());
    }
    for forward in &target.forwards {
        validate_forward(forward)?;
    }
    let mut ports: Vec<u16> = target.forwards.iter().map(|f| f.local_port).collect();
    ports.sort_unstable();
    if ports.windows(2).any(|w| w[0] == w[1]) {
        return Err("Two forwards use the same local port".into());
    }
    Ok(())
}

pub fn validate_forward(forward: &LocalForward) -> Result<(), String> {
    if forward.local_port == 0 {
        return Err("Local port must be between 1 and 65535".into());
    }
    if forward.remote_port == 0 {
        return Err("Remote port must be between 1 and 65535".into());
    }
    let host = forward.remote_host.trim();
    if host.is_empty() {
        return Err("Remote host is required".into());
    }
    if host.chars().any(|c| c.is_whitespace() || c.is_control() || c == ',') {
        return Err("Remote host must not contain spaces or commas".into());
    }
    if host.contains(':') && !(host.starts_with('[') && host.ends_with(']')) {
        return Err("IPv6 remote hosts must be written in [brackets]".into());
    }
    Ok(())
}

/// Opções comuns a todas as chamadas: porta, identidade, salto, usuário.
pub fn connection_options(target: &SshTarget) -> Vec<String> {
    let mut args = vec!["-p".to_string(), target.port.to_string()];
    if !target.identity_file.is_empty() {
        args.push("-i".into());
        args.push(target.identity_file.clone());
    }
    if !target.proxy_jump.is_empty() {
        args.push("-J".into());
        args.push(target.proxy_jump.clone());
    }
    if !target.user.is_empty() {
        args.push("-l".into());
        args.push(target.user.clone());
    }
    args
}

fn host_key_args(options: &SshOptions) -> Vec<String> {
    match options.host_key.option() {
        Some(value) => vec!["-o".into(), format!("StrictHostKeyChecking={value}")],
        None => Vec::new(),
    }
}

/// `ssh -o BatchMode=yes -o ConnectTimeout=N <destino> echo ok`. Sem perguntas: um teste nunca
/// fica esperando no terminal.
pub fn test_args(target: &SshTarget, options: &SshOptions) -> Vec<String> {
    let mut args = vec![
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        format!("ConnectTimeout={}", options.connect_timeout_s),
    ];
    args.extend(host_key_args(options));
    args.extend(connection_options(target));
    args.push(target.host.clone());
    args.push("echo".into());
    args.push("ok".into());
    args
}

/// Sessão interativa: TTY forçado, para que a senha e as perguntas de chave apareçam no terminal.
/// Com `remote_dir`, o shell de login remoto começa nessa pasta.
/// Flags de multiplexing OpenSSH quando `reuse_connections` está ativo.
#[cfg_attr(windows, allow(dead_code))]
pub fn reuse_options(options: &SshOptions) -> Vec<String> {
    if !options.reuse_connections {
        return Vec::new();
    }
    vec![
        "-o".into(),
        "ControlMaster=auto".into(),
        "-o".into(),
        "ControlPersist=60".into(),
    ]
}

pub fn terminal_args(target: &SshTarget, options: &SshOptions, remote_dir: Option<&str>) -> Vec<String> {
    let mut args = vec!["-tt".to_string()];
    if options.agent_forward {
        args.push("-A".into());
    }
    if options.keep_alive_s > 0 {
        args.push("-o".into());
        args.push(format!("ServerAliveInterval={}", options.keep_alive_s));
    }
    #[cfg(not(windows))]
    args.extend(reuse_options(options));
    args.extend(host_key_args(options));
    args.extend(connection_options(target));
    args.push(target.host.clone());
    if let Some(dir) = remote_dir.map(str::trim).filter(|d| !d.is_empty()) {
        args.push(format!("cd {} && exec \"${{SHELL:-/bin/sh}}\" -l", posix_quote(dir)));
    }
    args
}

/// Um `ssh -N -L` em segundo plano. Sem perguntas (`BatchMode`): a chave precisa estar liberada
/// pelo agente ou não ter senha. O ouvinte fica só em loopback.
pub fn forward_args(target: &SshTarget, options: &SshOptions, forward: &LocalForward) -> Vec<String> {
    let mut args = vec![
        "-N".to_string(),
        "-o".into(),
        "BatchMode=yes".into(),
        "-o".into(),
        format!("ConnectTimeout={}", options.connect_timeout_s),
        "-o".into(),
        "ExitOnForwardFailure=yes".into(),
    ];
    if options.keep_alive_s > 0 {
        args.push("-o".into());
        args.push(format!("ServerAliveInterval={}", options.keep_alive_s));
    }
    args.extend(host_key_args(options));
    args.extend(connection_options(target));
    args.push("-L".into());
    args.push(format!(
        "127.0.0.1:{}:{}:{}",
        forward.local_port, forward.remote_host, forward.remote_port
    ));
    args.push(target.host.clone());
    args
}

/// Uma palavra para um shell POSIX (usada no diretório inicial remoto).
pub fn posix_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Por que uma conexão falhou, em termos que o usuário pode agir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeError {
    AuthFailed,
    HostUnreachable,
    UnknownHost,
    /// A chave do host não está em known_hosts (o modo sem perguntas não confia nela sozinho).
    UnknownHostKey,
    /// A chave guardada difere da do servidor: pode ser um intermediário, ou o servidor foi refeito.
    HostKeyChanged,
    Other(String),
}

impl ProbeError {
    /// Chave i18n da explicação.
    pub fn key(&self) -> &'static str {
        match self {
            Self::AuthFailed => "remote-error-auth_failed",
            Self::HostUnreachable => "remote-error-host_unreachable",
            Self::UnknownHost => "remote-error-unknown_host",
            Self::UnknownHostKey => "remote-error-unknown_host_key",
            Self::HostKeyChanged => "remote-error-host_key_changed",
            Self::Other(_) => "remote-error-other",
        }
    }
}

/// Mapeia o stderr do `ssh` para um erro. As frases mais específicas vêm primeiro.
pub fn classify_failure(stderr: &str) -> ProbeError {
    let has = |needle: &str| stderr.contains(needle);
    if has("REMOTE HOST IDENTIFICATION HAS CHANGED") {
        ProbeError::HostKeyChanged
    } else if has("Host key verification failed")
        || has("No ECDSA host key is known")
        || has("No ED25519 host key is known")
        || has("No RSA host key is known")
        || has("Are you sure you want to continue connecting")
    {
        ProbeError::UnknownHostKey
    } else if has("Permission denied") || has("Too many authentication failures") {
        ProbeError::AuthFailed
    } else if has("Could not resolve hostname") {
        ProbeError::UnknownHost
    } else if has("Connection timed out")
        || has("Operation timed out")
        || has("Connection refused")
        || has("No route to host")
        || has("Network is unreachable")
    {
        ProbeError::HostUnreachable
    } else {
        let first = stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("no details from ssh");
        ProbeError::Other(first.to_string())
    }
}

/// Decide o resultado do teste pelo processo: precisa sair com 0 e imprimir `ok`.
pub fn probe_outcome(success: bool, stdout: &str, stderr: &str) -> Result<(), ProbeError> {
    if success && stdout.lines().any(|l| l.trim() == "ok") {
        Ok(())
    } else if success {
        Err(ProbeError::Other("unexpected output from remote host".into()))
    } else {
        Err(classify_failure(stderr))
    }
}

/// Chaves de explicação de falha de conexão, para o teste de contrato.
pub const USED_KEYS: &[&str] = &[
    "remote-error-auth_failed",
    "remote-error-host_unreachable",
    "remote-error-unknown_host",
    "remote-error-unknown_host_key",
    "remote-error-host_key_changed",
    "remote-error-other",
];

/// Nome do programa `ssh` nesta plataforma.
pub fn ssh_program() -> &'static str {
    if cfg!(windows) {
        "ssh.exe"
    } else {
        "ssh"
    }
}

#[cfg(test)]
mod tests;
