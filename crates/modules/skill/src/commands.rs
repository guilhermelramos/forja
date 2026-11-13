//! Comandos e métodos RPC do módulo `skill`. Cada um valida os argumentos, chama uma ação e
//! responde na hora com `{started: true}`. O resultado do disco chega depois, pela foto
//! (`skill.list`) e pelo aviso da tela.

use crate::actions::{self, InstallRequest};
use crate::model::{Scope, SkillError};
use crate::state::{InstallKind, Shared};
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

/// Relê as raízes em segundo plano.
pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared) -> Result<Value> {
    actions::refresh(cx, shared);
    Ok(json!({ "started": true }))
}

/// Instala uma pasta ou um zip como skill de um dono e escopo.
pub fn install(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let request = install_request(args)?;
    actions::install(cx, shared, request).map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

/// Copia uma skill listada para outro dono e escopo.
pub fn copy(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let from = PathBuf::from(args.required_str("from")?);
    let owner = args.required_str("owner")?.to_owned();
    let scope = scope_arg(args)?;
    actions::copy(
        cx,
        shared,
        from,
        owner,
        scope,
        args.bool("overwrite").unwrap_or(false),
        args.bool("consent").unwrap_or(false),
    )
    .map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

/// Manda uma skill listada para a lixeira.
pub fn remove(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let path = PathBuf::from(args.required_str("path")?);
    actions::remove(cx, shared, path, args.bool("consent").unwrap_or(false)).map_err(to_kernel)?;
    Ok(json!({ "started": true }))
}

/// Os argumentos de `skill.install`, validados.
pub fn install_request(args: &Args) -> Result<InstallRequest> {
    let source = args.required_str("source")?;
    let kind = match args.str("kind").unwrap_or("folder") {
        "folder" => InstallKind::Folder,
        "zip" => InstallKind::Zip,
        other => {
            return Err(KernelError::message(format!(
                "kind must be folder or zip, got {other}"
            )))
        }
    };
    Ok(InstallRequest {
        kind,
        source: PathBuf::from(source),
        owner: args.required_str("owner")?.to_owned(),
        scope: scope_arg(args)?,
        overwrite: args.bool("overwrite").unwrap_or(false),
        consent: args.bool("consent").unwrap_or(false),
    })
}

/// A foto da lista para `skill.list` e para a tela.
pub fn snapshot(shared: &Shared) -> Value {
    actions::snapshot(shared)
}

fn scope_arg(args: &Args) -> Result<Scope> {
    let text = args.required_str("scope")?;
    Scope::parse(text)
        .ok_or_else(|| KernelError::message(format!("scope must be user or project, got {text}")))
}

/// Erro de ação para o chamador (comando, CLI ou RPC): a chave e o texto técnico.
pub fn to_kernel(error: SkillError) -> KernelError {
    KernelError::message(error.to_string())
}
