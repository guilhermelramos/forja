//! Testes do módulo inteiro, com backends falsos: o cofre é a memória e o teclado só registra os
//! passos. Nenhum teste abre o microfone, toca no teclado real ou grava no cofre do usuário.

use super::*;
use crate::keystore::memory::MemoryKeyStore;
use crate::winkeys::recording::RecordingInput;
use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
use serde_json::{json, Map, Value};
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("forja-voice-module-{name}-{}-{nanos}", std::process::id()))
}

/// Runtime com o módulo e backends falsos. Devolve também o registro de teclas para inspeção.
fn runtime(dir: &std::path::Path) -> (Runtime, Arc<RecordingInput>) {
    let input = Arc::new(RecordingInput::default());
    let module = VoiceModule::with_backends(Arc::new(MemoryKeyStore::default()), input.clone());
    let runtime = Runtime::new(
        vec![Box::new(module)],
        RuntimeOptions {
            data_dir: Some(dir.to_path_buf()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("module registers cleanly");
    (runtime, input)
}

fn args(pairs: Value) -> Args {
    match pairs {
        Value::Object(map) => Args::from_map(map.into_iter().collect::<Map<String, Value>>()),
        _ => Args::new(),
    }
}

fn enable(runtime: &mut Runtime) {
    runtime
        .set_setting("voice.mode.enabled", SettingValue::Bool(true))
        .expect("setting accepted");
}

#[test]
fn the_module_passes_the_kernel_contract() {
    let dir = temp_dir("contract");
    let (runtime, _input) = runtime(&dir);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dictation_is_refused_while_voice_is_off() {
    let dir = temp_dir("off");
    let (mut runtime, input) = runtime(&dir);
    let error = runtime.run_command("voice.dictate", Args::new()).expect_err("off by default");
    assert!(error.to_string().contains("voice-error-disabled"), "{error}");
    assert!(input.0.lock().unwrap().is_empty(), "no key may be sent while voice is off");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_toggle_sends_win_h_on_each_press() {
    let dir = temp_dir("toggle");
    let (mut runtime, input) = runtime(&dir);
    enable(&mut runtime);
    runtime.run_command("voice.dictate", Args::new()).expect("starts");
    assert_eq!(input.0.lock().unwrap().len(), 4, "one Win+H is four steps");
    runtime.run_command("voice.dictate", Args::new()).expect("stops");
    assert_eq!(input.0.lock().unwrap().len(), 8);
    assert_eq!(
        input.0.lock().unwrap()[..4],
        winkeys::win_h(),
        "the sequence is Win+H"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_hold_starts_on_press_and_stops_on_release() {
    let dir = temp_dir("hold");
    let (mut runtime, input) = runtime(&dir);
    enable(&mut runtime);
    runtime
        .run_command("voice.hold", args(json!({ "phase": "press" })))
        .expect("press starts");
    runtime
        .run_command("voice.hold", args(json!({ "phase": "release" })))
        .expect("release stops");
    let steps = input.0.lock().unwrap().clone();
    assert_eq!(steps.len(), 8);
    assert_eq!(steps[4..], winkeys::win_h());
    let error = runtime
        .run_command("voice.hold", args(json!({ "phase": "tap" })))
        .expect_err("phase checked");
    assert!(error.to_string().contains("press or release"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_whisper_engine_is_listed_but_not_available_for_dictation() {
    let dir = temp_dir("engine");
    let (mut runtime, input) = runtime(&dir);
    enable(&mut runtime);
    runtime
        .set_setting("voice.input.engine", SettingValue::Text("whisper_cpp".to_owned()))
        .expect("choice accepted");
    let error = runtime.run_command("voice.dictate", Args::new()).expect_err("not available");
    assert!(error.to_string().contains("voice-error-engine"), "{error}");
    assert!(input.0.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn inserting_needs_the_explicit_confirmation_flag() {
    let dir = temp_dir("confirm");
    let (mut runtime, _input) = runtime(&dir);
    enable(&mut runtime);
    let error = runtime
        .run_command(
            "voice.insert",
            args(json!({ "target": "terminal", "terminal_id": "t1" })),
        )
        .expect_err("not confirmed");
    assert!(error.to_string().contains("voice-error-confirm"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn confirmed_insert_with_no_transcript_has_nothing_to_send() {
    let dir = temp_dir("nothing");
    let (mut runtime, _input) = runtime(&dir);
    enable(&mut runtime);
    let error = runtime
        .run_command(
            "voice.insert",
            args(json!({ "target": "terminal", "terminal_id": "t1", "confirmed": true })),
        )
        .expect_err("empty");
    assert!(error.to_string().contains("voice-error-nothing"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_unknown_insert_target_is_refused() {
    let dir = temp_dir("target");
    let (mut runtime, _input) = runtime(&dir);
    let error = runtime
        .run_command("voice.insert", args(json!({ "target": "planilha", "confirmed": true })))
        .expect_err("unknown target");
    assert!(error.to_string().contains("terminal, editor or chat"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_empty_key_is_refused_before_it_reaches_the_vault() {
    let dir = temp_dir("key");
    let (mut runtime, _input) = runtime(&dir);
    let error = runtime
        .run_command("voice.key.save", args(json!({ "key": "   " })))
        .expect_err("empty key");
    assert!(error.to_string().contains("voice-error-empty-key"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transcribing_without_a_recording_has_nothing_to_send() {
    let dir = temp_dir("transcribe");
    let (mut runtime, _input) = runtime(&dir);
    enable(&mut runtime);
    let error = runtime.run_command("voice.transcribe", Args::new()).expect_err("no capture");
    assert!(error.to_string().contains("voice-error-nothing"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stopping_without_a_recording_says_so() {
    let dir = temp_dir("stop");
    let (mut runtime, _input) = runtime(&dir);
    enable(&mut runtime);
    let error = runtime.run_command("voice.record.stop", Args::new()).expect_err("not recording");
    assert!(error.to_string().contains("voice-error-not-recording"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}
