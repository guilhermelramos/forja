//! Estado compartilhado do módulo e o ponto de contato com o disco e com os outros módulos.
//!
//! Regra: o estado nunca fica travado durante uma chamada de comando, de evento ou de disco.
//! Quem precisa mexer nele trava, muda, solta, e só então executa os efeitos devolvidos.

use crate::model::{RunStatus, Store};
use crate::runner::{MissedPolicy, Runner, Settings};
use forja_kernel::{Ctx, SettingValue};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

#[derive(Debug, Default)]
pub struct State {
    pub store: Store,
    pub loaded: bool,
    pub loading: bool,
    /// O arquivo existia e não pôde ser lido: gravar por cima apagaria o histórico.
    pub load_failed: bool,
    /// Dados de captura (FORJA_FIXTURES): nunca são gravados no arquivo real.
    pub fixture: bool,
    /// Chave i18n aviso para a tela (falha de leitura ou de gravação).
    pub notice: Option<String>,
    pub runner: Runner,
    pub last_tick: Option<SystemTime>,
    pub last_persist: Option<SystemTime>,
    /// Uma gravação está em andamento; pedidos seguintes são reunidos.
    pub persisting: bool,
    /// O estado mudou durante a gravação e precisa ser salvo novamente.
    pub persist_queued: bool,
}

pub type Shared = Arc<Mutex<State>>;

pub fn new_shared() -> Shared {
    Arc::new(Mutex::new(State::default()))
}

/// Acesso curto ao estado. Não chame comandos dentro da função.
pub fn with<T>(shared: &Shared, change: impl FnOnce(&mut State) -> T) -> Option<T> {
    shared.lock().ok().map(|mut state| change(&mut state))
}

/// Modo de captura (`FORJA_FIXTURES=1`), o mesmo sinal que os outros módulos usam.
pub fn fixtures_enabled() -> bool {
    std::env::var("FORJA_FIXTURES")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Configurações lidas agora. Chamar na thread da interface, antes de travar o estado.
pub fn settings(cx: &Ctx<'_>) -> Settings {
    let int = |key: &str, default: i64| cx.setting(key).and_then(SettingValue::as_i64).unwrap_or(default);
    let flag = |key: &str, default: bool| cx.setting(key).and_then(SettingValue::as_bool).unwrap_or(default);
    let missed = match cx.setting("routine.schedule.missed").and_then(SettingValue::as_str) {
        Some("skip") => MissedPolicy::Skip,
        Some("ask") => MissedPolicy::Ask,
        _ => MissedPolicy::RunOnce,
    };
    Settings {
        concurrency: int("routine.run.concurrency", 1).clamp(1, 8) as usize,
        timeout_s: int("routine.run.timeout_min", 60).clamp(1, 720) * 60,
        notify_failure: flag("routine.notifications.failure", true),
        notify_success: flag("routine.notifications.success", false),
        today: chrono::Local::now().format("%Y-%m-%d").to_string(),
        grace_s: int("routine.schedule.grace_s", 120).clamp(0, 3600),
        missed,
    }
}

/// `true` quando a agenda usa UTC; `false` quando segue o fuso do Windows.
pub fn schedule_in_utc(cx: &Ctx<'_>) -> bool {
    cx.setting("routine.schedule.timezone")
        .and_then(SettingValue::as_str)
        .is_some_and(|zone| zone == "utc")
}

/// Rotinas ligadas? `routine.mode.enabled` desligado pausa só os disparos automáticos.
pub fn scheduling_enabled(cx: &Ctx<'_>) -> bool {
    cx.setting("routine.mode.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true)
}

pub fn retention_days(cx: &Ctx<'_>) -> i64 {
    cx.setting("routine.logs.retention_days")
        .and_then(SettingValue::as_i64)
        .unwrap_or(30)
        .clamp(1, 365)
}

/// Chave i18n do status de uma execução.
pub fn status_key(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "routine-status-queued",
        RunStatus::Running => "routine-status-running",
        RunStatus::Succeeded => "routine-status-succeeded",
        RunStatus::Failed { .. } => "routine-status-failed",
        RunStatus::Skipped { .. } => "routine-status-skipped",
        RunStatus::Missed => "routine-status-missed",
    }
}
