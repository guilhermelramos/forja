//! Estado compartilhado do módulo de voz, e os backends (cofre da chave e envio de teclas) que as
//! ações usam. Os backends ficam ao lado do estado para que os testes troquem por versões falsas.
//! A tela e os comandos seguram o lock só para ler ou trocar campos.

use crate::gesture::Gesture;
use crate::keystore::KeyStore;
use crate::record::Recorder;
use crate::winkeys::InputSender;
use forja_kernel::{Ctx, SettingValue};
use std::sync::{Arc, Mutex};

/// Os dois acessos ao sistema, separados para os testes.
pub struct Backends {
    pub keys: Arc<dyn KeyStore>,
    pub input: Arc<dyn InputSender>,
}

/// Áudio gravado, já codificado como WAV. A duração serve para a tela.
#[derive(Clone, Debug)]
pub struct Captured {
    pub wav: Arc<Vec<u8>>,
    pub seconds: f32,
}

/// Um terminal que pode receber o texto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalChoice {
    pub id: String,
    pub title: String,
}

/// Para onde o texto vai. Editor e chat só entram se o comando existir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertKind {
    Terminal,
    Editor,
    Chat,
}

/// Inserção pedida e ainda não confirmada pelo usuário.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingInsert {
    pub kind: InsertKind,
    pub terminal_id: Option<String>,
    pub text: String,
}

/// Aviso: chave de tradução e um texto técnico opcional (nunca a chave ou o áudio).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub key: String,
    pub detail: String,
}

impl Notice {
    pub fn key(key: &str) -> Self {
        Self {
            key: key.to_owned(),
            detail: String::new(),
        }
    }
}

#[derive(Default)]
pub struct State {
    /// Gesto da tecla de ditado do Windows (alternar ou segurar).
    pub gesture: Gesture,
    /// Gravação local em andamento, se houver.
    pub recorder: Option<Recorder>,
    /// Gravação fake dos dados de captura: liga a aparência de "gravando" sem microfone.
    pub recording_fixture: bool,
    pub level: f32,
    pub capture: Option<Captured>,
    pub transcribing: bool,
    pub transcript: String,
    pub targets: Vec<TerminalChoice>,
    pub pending_insert: Option<PendingInsert>,
    /// `None` enquanto a chave não foi lida; `Some(true/false)` depois.
    pub key_present: Option<bool>,
    pub notice: Option<Notice>,
    /// A janela de voz está aberta.
    pub overlay: bool,
}

#[derive(Clone)]
pub struct Shared {
    state: Arc<Mutex<State>>,
    backends: Arc<Backends>,
}

impl Shared {
    pub fn new(backends: Backends) -> Self {
        Self {
            state: Arc::new(Mutex::new(State::default())),
            backends: Arc::new(backends),
        }
    }

    /// Roda `change` com o lock. `None` se o lock estiver envenenado.
    pub fn with<T>(&self, change: impl FnOnce(&mut State) -> T) -> Option<T> {
        self.state.lock().ok().map(|mut state| change(&mut state))
    }

    pub fn backends(&self) -> Arc<Backends> {
        Arc::clone(&self.backends)
    }
}

/// As configurações de voz lidas da tela de Ajustes, na thread da interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub engine: String,
    pub language: String,
    pub keep_audio: bool,
}

impl Settings {
    pub fn read(cx: &Ctx<'_>) -> Self {
        Self {
            enabled: flag(cx, "voice.mode.enabled"),
            engine: text(cx, "voice.input.engine", "windows"),
            language: text(cx, "voice.input.language", "auto"),
            keep_audio: flag(cx, "voice.privacy.keep_audio"),
        }
    }
}

fn flag(cx: &Ctx<'_>, key: &str) -> bool {
    cx.setting(key).and_then(SettingValue::as_bool).unwrap_or(false)
}

fn text(cx: &Ctx<'_>, key: &str, fallback: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

/// Modo de captura (`FORJA_FIXTURES=1`): dados de exemplo, sem microfone nem cofre.
pub fn fixtures_enabled() -> bool {
    std::env::var("FORJA_FIXTURES")
        .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// A janela de voz de exemplo só abre com `FORJA_FIXTURES_VOICE=1`, para não cobrir as outras
/// telas nas capturas.
pub fn overlay_fixture_enabled() -> bool {
    fixtures_enabled()
        && std::env::var("FORJA_FIXTURES_VOICE")
            .map(|value| matches!(value.trim(), "1" | "true" | "yes"))
            .unwrap_or(false)
}
