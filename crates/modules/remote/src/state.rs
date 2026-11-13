//! Estado do módulo `remote` e as configurações `remote.*` lidas da thread da interface.
//!
//! Regra: o estado nunca fica travado durante um processo, uma leitura de disco ou uma chamada
//! de comando. Quem precisa mexer nele trava, muda, solta, e só então age.

use crate::forward::PortForwards;
use crate::ssh::{HostKeyPolicy, ProbeError, SshOptions};
use crate::target::SshTarget;
use crate::wsl::WslDistro;
use forja_kernel::{Ctx, SettingValue};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// Resultado do último teste de conexão de um destino.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeResult {
    pub ok: bool,
    pub error: Option<ProbeError>,
    pub elapsed_ms: u64,
    pub at: i64,
}

#[derive(Debug, Default)]
pub struct State {
    pub targets: Vec<SshTarget>,
    pub loaded: bool,
    pub loading: bool,
    /// O arquivo existia e não pôde ser lido: gravar por cima apagaria os destinos.
    pub load_failed: bool,
    /// Chave i18n de um aviso para a tela.
    pub notice: Option<String>,
    /// Último teste de cada destino (por id).
    pub probes: BTreeMap<String, ProbeResult>,
    /// Destinos com operação em andamento (teste, abrir, encaminhar).
    pub busy: BTreeSet<String>,
    /// Distros do WSL, quando já foram listadas.
    pub distros: Option<Vec<WslDistro>>,
    pub distros_loading: bool,
    /// Os encaminhamentos já foram restaurados nesta sessão.
    pub restored: bool,
    /// Dados de captura (FORJA_FIXTURES): nunca são gravados no arquivo real.
    pub fixture: bool,
}

pub type Shared = Arc<Mutex<State>>;

/// Gerenciador de encaminhamentos, fora do estado: processos não travam o estado.
pub type Forwards = Arc<Mutex<PortForwards>>;

pub fn new_shared() -> Shared {
    Arc::new(Mutex::new(State::default()))
}

pub fn new_forwards() -> Forwards {
    Arc::new(Mutex::new(PortForwards::default()))
}

/// Acesso curto ao estado. Não chame comandos nem processos dentro da função.
pub fn with<T>(shared: &Shared, change: impl FnOnce(&mut State) -> T) -> Option<T> {
    shared.lock().ok().map(|mut state| change(&mut state))
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn int(cx: &Ctx<'_>, key: &str, default: i64) -> i64 {
    cx.setting(key).and_then(SettingValue::as_i64).unwrap_or(default)
}

fn text(cx: &Ctx<'_>, key: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// Modo de captura (`FORJA_FIXTURES=1`), o mesmo sinal que os outros módulos usam.
pub fn fixtures_enabled() -> bool {
    std::env::var("FORJA_FIXTURES")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

pub fn ssh_enabled(cx: &Ctx<'_>) -> bool {
    cx.setting("remote.ssh.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true)
}

/// Opções de SSH como as configurações as descrevem.
pub fn default_host(cx: &Ctx<'_>) -> String {
    text(cx, "remote.ssh.default_host")
}

pub fn ssh_options(cx: &Ctx<'_>) -> SshOptions {
    let agent_forward = cx
        .setting("remote.ssh.agent_forward")
        .and_then(SettingValue::as_bool)
        .unwrap_or(false);
    let reuse_connections = cx
        .setting("remote.ssh.reuse_connections")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    SshOptions {
        connect_timeout_s: int(cx, "remote.ssh.connect_timeout_s", 20).clamp(5, 120) as u32,
        keep_alive_s: int(cx, "remote.ssh.keep_alive_s", 30).clamp(0, 300) as u32,
        agent_forward,
        reuse_connections,
        host_key: HostKeyPolicy::parse(&text(cx, "remote.ssh.host_key_policy")),
    }
}

/// Caminho do `ssh_config` com `~` resolvido para a pasta pessoal.
pub fn config_path(cx: &Ctx<'_>) -> PathBuf {
    let raw = text(cx, "remote.ssh.config_path");
    let raw = if raw.is_empty() { "~/.ssh/config".to_owned() } else { raw };
    expand_home(&raw, &home_dir())
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn expand_home(value: &str, home: &std::path::Path) -> PathBuf {
    for prefix in ["~/", "~\\"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

pub fn wsl_default_distro(cx: &Ctx<'_>) -> String {
    text(cx, "remote.wsl.default_distro")
}

pub fn wsl_default_user(cx: &Ctx<'_>) -> String {
    text(cx, "remote.wsl.default_user")
}

/// `ask`, `auto` ou `off` (`remote.forward.mode`).
pub fn forward_mode(cx: &Ctx<'_>) -> String {
    match text(cx, "remote.forward.mode").as_str() {
        "auto" => "auto".to_owned(),
        "off" => "off".to_owned(),
        _ => "ask".to_owned(),
    }
}

pub fn latency_warning_ms(cx: &Ctx<'_>) -> u64 {
    int(cx, "remote.latency.warning_ms", 300).clamp(50, 2000) as u64
}

/// Chaves de aviso e de erro que o estado e os comandos usam.
pub const USED_KEYS: &[&str] = &[
    "remote-notice-load-failed",
    "remote-notice-save-failed",
    "remote-error-loading",
    "remote-error-invalid",
];

/// Erros de arquivo e de processo do módulo, como texto curto para a tela.
pub fn short(text: &str) -> String {
    text.chars().take(160).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_is_expanded_to_the_home_folder() {
        let home = PathBuf::from("C:/Users/dev");
        assert_eq!(expand_home("~/.ssh/config", &home), home.join(".ssh/config"));
        assert_eq!(expand_home("D:/ssh/cfg", &home), PathBuf::from("D:/ssh/cfg"));
    }

    #[test]
    fn unknown_forward_modes_fall_back_to_ask_semantics_in_text() {
        assert_eq!(HostKeyPolicy::parse("nonsense"), HostKeyPolicy::Ask);
        assert_eq!(HostKeyPolicy::parse("strict"), HostKeyPolicy::Strict);
        assert_eq!(HostKeyPolicy::parse("accept_new"), HostKeyPolicy::AcceptNew);
    }

    #[test]
    fn the_shared_state_starts_empty_and_unloaded() {
        let shared = new_shared();
        let loaded = with(&shared, |state| state.loaded).unwrap();
        assert!(!loaded);

    }
}
