//! Forjar: a regra de nome da bancada e a ligação com `bench.create` e `helper.launch`.
//!
//! Os comandos `bench.*` e `helper.*` pertencem a outros módulos. Se não existirem, o
//! Forjar avisa e não quebra: o pedido continua na lista.

use forja_contracts::{RequestItem, RequestRef};
use forja_kernel::{Args, Ctx, KernelError, Result};
use serde_json::{json, Value};

/// Largura máxima do trecho do título no nome da bancada.
const TITLE_LIMIT: usize = 40;

/// Nome da bancada derivado do pedido: `pedido-123-titulo-curto`.
/// Sem o título (configuração `request.create.branch_from_title` desligada): `pedido-123`.
pub fn bench_name(item: &RequestItem, include_title: bool) -> String {
    let base = format!("pedido-{}", slug(&item.key));
    if !include_title {
        return base;
    }
    let title: String = slug(&item.title).chars().take(TITLE_LIMIT).collect();
    let title = title.trim_matches('-');
    if title.is_empty() {
        base
    } else {
        format!("{base}-{title}")
    }
}

/// Minúsculas ASCII, acentos do português trocados pela letra base, o resto vira `-`.
pub fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let mapped = match ch.to_ascii_lowercase() {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        };
        if mapped.is_ascii_alphanumeric() {
            out.push(mapped);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

/// Pede a bancada ao módulo dono. Devolve o resultado cru de `bench.create`.
pub fn create_bench(cx: &mut Ctx<'_>, project_id: &str, name: &str, reference: &RequestRef) -> Result<Value> {
    let mut args = Args::new();
    args.insert("project_id", json!(project_id));
    args.insert("name", json!(name));
    args.insert("from_request", serde_json::to_value(reference)?);
    cx.run_command("bench.create", args)
}

/// Sobe o ajudante padrão na bancada, se a configuração pede e o comando existe.
/// Devolve `false` quando não subiu (desligado ou módulo ausente).
pub fn launch_helper(cx: &mut Ctx<'_>, bench_path: &str, reference: &RequestRef) -> Result<bool> {
    let auto_start = cx
        .setting("helper.launch.auto_start")
        .and_then(forja_kernel::SettingValue::as_bool)
        .unwrap_or(true);
    if !auto_start {
        return Ok(false);
    }
    let mut args = Args::new();
    args.insert("bench_path", json!(bench_path));
    args.insert("from_request", serde_json::to_value(reference)?);
    args.insert("initial_context", json!("request"));
    match cx.run_command("helper.launch", args) {
        Ok(_) => Ok(true),
        Err(KernelError::UnknownCommand(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

/// Verdadeiro quando o módulo de bancadas existe. Usado para escolher o aviso da tela.
pub fn bench_available(cx: &Ctx<'_>) -> bool {
    cx.registry().command("bench.create").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_request_number_and_title() {
        let mut item = RequestItem::new("github", "#123", "Corrigir login do GitHub!");
        assert_eq!(bench_name(&item, true), "pedido-123-corrigir-login-do-github");
        assert_eq!(bench_name(&item, false), "pedido-123");
        item.title = "  !!!  ".into();
        assert_eq!(bench_name(&item, true), "pedido-123");
    }

    #[test]
    fn keys_with_letters_and_accents_are_slugged() {
        let item = RequestItem::new("jira", "ENG-42", "Ação de cache é útil");
        assert_eq!(bench_name(&item, true), "pedido-eng-42-acao-de-cache-e-util");
        assert_eq!(slug("LOCAL-3"), "local-3");
    }

    #[test]
    fn long_titles_are_cut() {
        let item = RequestItem::new("github", "#9", "palavra ".repeat(20));
        assert!(bench_name(&item, true).len() <= "pedido-9-".len() + TITLE_LIMIT);
    }
}
