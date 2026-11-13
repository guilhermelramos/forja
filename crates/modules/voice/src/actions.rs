//! Ações de voz: ditado do Windows, gravação local, transcrição pela OpenAI, chave, terminais e
//! inserção. A parte barata (ler a configuração, trocar o estado) roda na thread da interface. O
//! disco, o microfone e a rede rodam em `Ctx::spawn`. Nada é inserido sem a confirmação da tela.

use crate::gesture::{Action, Edge, Mode};
use crate::openai;
use crate::record::Recorder;
use crate::state::{
    fixtures_enabled, Captured, InsertKind, Notice, PendingInsert, Settings, Shared, TerminalChoice,
};
use crate::wav;
use crate::winkeys;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Chaves de tradução que este arquivo monta em tempo de execução.
pub const USED_KEYS: &[&str] = &[
    "voice-error-disabled",
    "voice-error-engine",
    "voice-error-state",
    "voice-error-input",
    "voice-error-audio",
    "voice-error-no-mic",
    "voice-error-not-recording",
    "voice-error-nothing",
    "voice-error-no-key",
    "voice-error-keyring",
    "voice-error-auth",
    "voice-error-http",
    "voice-error-network",
    "voice-error-timeout",
    "voice-error-response",
    "voice-error-unsupported",
    "voice-error-confirm",
    "voice-error-unavailable",
    "voice-error-insert",
    "voice-error-empty-key",
    "voice-notice-key-saved",
    "voice-notice-key-deleted",
    "voice-notice-inserted",
    "voice-notice-transcribed",
];

/// Ditado do Windows: um gesto da tecla. Liga ou desliga o ditado do sistema com Win+H.
pub fn dictate(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    mode: Mode,
    edge: Edge,
) -> Result<(), &'static str> {
    let settings = Settings::read(cx);
    if !settings.enabled {
        return Err("voice-error-disabled");
    }
    if settings.engine != "windows" {
        return Err("voice-error-engine");
    }
    let action = shared
        .with(|state| state.gesture.feed(mode, edge))
        .ok_or("voice-error-state")?;
    if action == Action::Nothing {
        return Ok(());
    }
    let backends = shared.backends();
    if backends.input.send(&winkeys::win_h()).is_err() {
        shared.with(|state| state.gesture.reset());
        return Err("voice-error-input");
    }
    Ok(())
}

/// Começa a gravar pelo microfone padrão.
pub fn start_recording(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), &'static str> {
    require_enabled(cx)?;
    if fixtures_enabled() {
        shared.with(|state| {
            state.recording_fixture = true;
            state.overlay = true;
        });
        return Ok(());
    }
    let busy = shared
        .with(|state| state.recorder.is_some())
        .ok_or("voice-error-state")?;
    if busy {
        return Ok(());
    }
    let recorder = Recorder::start()?;
    shared.with(|state| {
        state.recorder = Some(recorder);
        state.overlay = true;
        state.capture = None;
        state.transcript.clear();
        state.pending_insert = None;
        state.notice = None;
    });
    Ok(())
}

/// Para a gravação. O áudio é fechado em segundo plano e codificado como WAV.
pub fn stop_recording(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), &'static str> {
    if fixtures_enabled() {
        shared.with(|state| state.recording_fixture = false);
        return Ok(());
    }
    let recorder = shared
        .with(|state| state.recorder.take())
        .flatten()
        .ok_or("voice-error-not-recording")?;
    let settings = Settings::read(cx);
    let audio_dir = audio_dir(cx);
    let shared = shared.clone();
    cx.spawn(
        move || -> Result<Captured, &'static str> {
            finish_capture(recorder, settings.keep_audio, &audio_dir)
        },
        move |_cx, result: Result<Captured, &'static str>| {
            shared.with(|state| match result {
                Ok(captured) => {
                    state.capture = Some(captured);
                    state.notice = None;
                }
                Err(key) => state.notice = Some(Notice::key(key)),
            });
        },
    );
    Ok(())
}

/// Transcreve o último áudio pela OpenAI. Só roda por pedido explícito da tela.
pub fn transcribe(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), &'static str> {
    require_enabled(cx)?;
    if fixtures_enabled() {
        shared.with(|state| state.transcript = fixture_transcript());
        return Ok(());
    }
    let captured = shared
        .with(|state| state.capture.clone())
        .flatten()
        .ok_or("voice-error-nothing")?;
    let settings = Settings::read(cx);
    let language = openai::language_code(&settings.language).map(str::to_owned);
    let keys = shared.backends().keys.clone();
    let network = cx
        .run_command("network.proxy", forja_kernel::Args::new())
        .ok();
    shared.with(|state| {
        state.transcribing = true;
        state.notice = None;
    });
    let shared = shared.clone();
    cx.spawn(
        move || -> Result<String, &'static str> {
            let key = keys.get()?.ok_or("voice-error-no-key")?;
            openai::transcribe_with_policy(
                &key,
                &captured.wav,
                language.as_deref(),
                network.as_ref(),
            )
        },
        move |_cx, result: Result<String, &'static str>| {
            shared.with(|state| {
                state.transcribing = false;
                match result {
                    Ok(text) => {
                        state.transcript = text;
                        state.notice = Some(Notice::key("voice-notice-transcribed"));
                    }
                    Err(key) => state.notice = Some(Notice::key(key)),
                }
            });
        },
    );
    Ok(())
}

/// Lê, em segundo plano, se há uma chave guardada no cofre.
pub fn refresh_key_status(cx: &mut Ctx<'_>, shared: &Shared) {
    if fixtures_enabled() {
        shared.with(|state| state.key_present = Some(true));
        return;
    }
    let keys = shared.backends().keys.clone();
    let shared = shared.clone();
    cx.spawn(
        move || keys.get().map(|key| key.is_some()),
        move |_cx, result: Result<bool, &'static str>| {
            shared.with(|state| match result {
                Ok(present) => state.key_present = Some(present),
                Err(key) => {
                    state.key_present = Some(false);
                    state.notice = Some(Notice::key(key));
                }
            });
        },
    );
}

/// Guarda a chave no cofre. A chave nunca vai para o aviso nem para arquivo.
pub fn save_key(cx: &mut Ctx<'_>, shared: &Shared, key: String) -> Result<(), &'static str> {
    let key = key.trim().to_owned();
    if key.is_empty() {
        return Err("voice-error-empty-key");
    }
    if fixtures_enabled() {
        return Err("voice-error-keyring");
    }
    let keys = shared.backends().keys.clone();
    let shared = shared.clone();
    cx.spawn(
        move || keys.set(&key),
        move |_cx, result: Result<(), &'static str>| {
            shared.with(|state| match result {
                Ok(()) => {
                    state.key_present = Some(true);
                    state.notice = Some(Notice::key("voice-notice-key-saved"));
                }
                Err(key) => state.notice = Some(Notice::key(key)),
            });
        },
    );
    Ok(())
}

/// Apaga a chave do cofre.
pub fn delete_key(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), &'static str> {
    if fixtures_enabled() {
        return Err("voice-error-keyring");
    }
    let keys = shared.backends().keys.clone();
    let shared = shared.clone();
    cx.spawn(
        move || keys.delete(),
        move |_cx, result: Result<(), &'static str>| {
            shared.with(|state| match result {
                Ok(()) => {
                    state.key_present = Some(false);
                    state.notice = Some(Notice::key("voice-notice-key-deleted"));
                }
                Err(key) => state.notice = Some(Notice::key(key)),
            });
        },
    );
    Ok(())
}

/// Relê os terminais do projeto para a lista de destinos. Sem o módulo de terminal, a lista fica vazia.
pub fn refresh_targets(cx: &mut Ctx<'_>, shared: &Shared) {
    let targets: Vec<TerminalChoice> = match cx.run_command("terminal.list", Args::new()) {
        Ok(value) => parse_terminals(&value),
        Err(_) => Vec::new(),
    };
    shared.with(|state| state.targets = targets);
}

/// Deixa uma inserção pendente para a tela confirmar. Não insere nada ainda.
pub fn request_insert(
    shared: &Shared,
    kind: InsertKind,
    terminal_id: Option<String>,
) -> Result<(), &'static str> {
    let text = shared
        .with(|state| state.transcript.trim().to_owned())
        .ok_or("voice-error-state")?;
    if text.is_empty() {
        return Err("voice-error-nothing");
    }
    if kind == InsertKind::Terminal && terminal_id.is_none() {
        return Err("voice-error-unavailable");
    }
    shared.with(|state| {
        state.pending_insert = Some(PendingInsert {
            kind,
            terminal_id,
            text,
        });
    });
    Ok(())
}

/// Insere o texto pendente no destino escolhido. Só roda depois da confirmação da tela.
pub fn confirm_insert(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), &'static str> {
    let pending = shared
        .with(|state| state.pending_insert.take())
        .flatten()
        .ok_or("voice-error-confirm")?;
    let result = match pending.kind {
        InsertKind::Terminal => {
            let terminal_id = pending
                .terminal_id
                .clone()
                .ok_or("voice-error-unavailable")?;
            cx.run_command(
                "terminal.send",
                args(json!({ "terminal_id": terminal_id, "text": pending.text, "enter": false })),
            )
            .map(|_| ())
        }
        InsertKind::Editor => {
            if !command_exists(cx, "editor.insert") {
                return Err("voice-error-unavailable");
            }
            cx.run_command("editor.insert", args(json!({ "text": pending.text })))
                .map(|_| ())
        }
        InsertKind::Chat => {
            if !command_exists(cx, "chat.insert") {
                return Err("voice-error-unavailable");
            }
            cx.run_command("chat.insert", args(json!({ "text": pending.text })))
                .map(|_| ())
        }
    };
    match result {
        Ok(()) => {
            shared.with(|state| {
                state.transcript.clear();
                state.capture = None;
                state.notice = Some(Notice::key("voice-notice-inserted"));
            });
            Ok(())
        }
        Err(_) => Err("voice-error-insert"),
    }
}

/// Descarta a gravação, a transcrição e a inserção pendente. Não mexe numa gravação em andamento.
pub fn discard(shared: &Shared) {
    shared.with(|state| {
        state.capture = None;
        state.transcript.clear();
        state.pending_insert = None;
        state.notice = None;
        if state.recorder.is_none() {
            state.overlay = false;
        }
    });
}

/// Nível do microfone para a tela, lido do contador da gravação em curso.
pub fn current_level(shared: &Shared) -> f32 {
    shared
        .with(|state| state.recorder.as_ref().map(Recorder::level).unwrap_or(0.0))
        .unwrap_or(0.0)
}

/// Dados de captura: a janela aberta com uma gravação e uma transcrição de exemplo.
pub fn apply_fixtures(shared: &Shared) {
    shared.with(|state| {
        state.overlay = true;
        state.recording_fixture = true;
        state.level = 0.42;
        state.key_present = Some(true);
        state.targets = vec![TerminalChoice {
            id: "term-exemplo".to_owned(),
            title: "pwsh".to_owned(),
        }];
        state.transcript = fixture_transcript();
    });
}

fn require_enabled(cx: &Ctx<'_>) -> Result<(), &'static str> {
    if Settings::read(cx).enabled {
        Ok(())
    } else {
        Err("voice-error-disabled")
    }
}

/// Fecha a gravação e codifica o WAV. Guarda o áudio em disco só se a privacidade permitir.
fn finish_capture(
    recorder: Recorder,
    keep_audio: bool,
    audio_dir: &Path,
) -> Result<Captured, &'static str> {
    let capture = recorder.stop()?;
    let seconds = wav::seconds(capture.samples.len(), capture.sample_rate);
    let bytes =
        wav::encode(&capture.samples, capture.sample_rate).map_err(|_| "voice-error-audio")?;
    if keep_audio {
        let _ = write_audio(audio_dir, &bytes);
    }
    Ok(Captured {
        wav: Arc::new(bytes),
        seconds,
    })
}

fn write_audio(dir: &Path, bytes: &[u8]) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("voz-{}.wav", unix_nanos()));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

/// Pasta do áudio guardado (`<dados>/modules/voice-audio`).
fn audio_dir(cx: &Ctx<'_>) -> PathBuf {
    cx.store()
        .path()
        .parent()
        .map(|modules| modules.join("voice-audio"))
        .unwrap_or_else(|| std::env::temp_dir().join("forja-voice-audio"))
}

fn command_exists(cx: &Ctx<'_>, id: &str) -> bool {
    cx.registry()
        .commands()
        .iter()
        .any(|command| command.id.as_str() == id)
}

fn parse_terminals(value: &Value) -> Vec<TerminalChoice> {
    value
        .get("terminals")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(TerminalChoice {
                        id: item.get("terminal_id")?.as_str()?.to_owned(),
                        title: item
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn args(value: Value) -> Args {
    match value {
        Value::Object(map) => Args::from_map(map),
        _ => Args::new(),
    }
}

fn fixture_transcript() -> String {
    "Revisar o pedido de pagamento e listar os testes que faltam.".to_owned()
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}
