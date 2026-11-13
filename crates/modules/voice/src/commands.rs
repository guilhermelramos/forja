//! Comandos do módulo `voice`. Cada um valida os argumentos e chama uma ação. Não há métodos RPC:
//! a voz é de uso local, pela tecla, pela janela e pelos ajustes.

use crate::actions;
use crate::gesture::{Edge, Mode};
use crate::state::{InsertKind, Shared};
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{json, Value};

/// Ditado do Windows, alternando: um toque liga, o próximo desliga.
pub fn dictate(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::dictate(cx, shared, Mode::Toggle, Edge::Press).map_err(to_kernel)?;
    Ok(json!({ "ok": true }))
}

/// Ditado do Windows, segurando: `phase` é `press` ou `release`.
pub fn hold(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let edge = match args.required_str("phase")? {
        "press" => Edge::Press,
        "release" => Edge::Release,
        other => {
            return Err(KernelError::message(format!(
                "phase must be press or release, got {other}"
            )))
        }
    };
    actions::dictate(cx, shared, Mode::Hold, edge).map_err(to_kernel)?;
    Ok(json!({ "ok": true }))
}

pub fn record_start(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::start_recording(cx, shared).map_err(to_kernel)?;
    Ok(json!({ "recording": true }))
}

pub fn record_stop(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::stop_recording(cx, shared).map_err(to_kernel)?;
    Ok(json!({ "recording": false }))
}

/// Transcreve o último áudio pela OpenAI, em segundo plano.
pub fn transcribe(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::transcribe(cx, shared).map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

/// Insere a transcrição. Sem `confirmed: true` não insere: a tela pede a confirmação antes.
pub fn insert(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    if args.bool("confirmed") != Some(true) {
        return Err(to_kernel("voice-error-confirm"));
    }
    let kind = insert_kind(args.required_str("target")?)?;
    let terminal_id = args.str("terminal_id").map(str::to_owned);
    actions::request_insert(shared, kind, terminal_id).map_err(to_kernel)?;
    actions::confirm_insert(cx, shared).map_err(to_kernel)?;
    Ok(json!({ "inserted": true }))
}

pub fn discard(shared: &Shared) -> Result<Value> {
    actions::discard(shared);
    Ok(json!({ "discarded": true }))
}

/// Guarda a chave da OpenAI no cofre. O valor não aparece em nenhuma resposta.
pub fn key_save(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let key = args.required_str("key")?.to_owned();
    actions::save_key(cx, shared, key).map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

pub fn key_delete(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::delete_key(cx, shared).map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

pub fn targets_refresh(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::refresh_targets(cx, shared);
    Ok(json!({ "started": true }))
}

fn insert_kind(text: &str) -> Result<InsertKind> {
    match text {
        "terminal" => Ok(InsertKind::Terminal),
        "editor" => Ok(InsertKind::Editor),
        "chat" => Ok(InsertKind::Chat),
        other => Err(KernelError::message(format!(
            "target must be terminal, editor or chat, got {other}"
        ))),
    }
}

/// Erro de ação para o chamador: a chave de tradução, sem texto técnico.
fn to_kernel(key: &'static str) -> KernelError {
    KernelError::message(key)
}
