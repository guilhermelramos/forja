//! Aprovação de receitas de projeto pelo módulo `trust`.
//!
//! A receita só executa quando o hash exato do arquivo está aprovado. Sem
//! decisão, o pedido abre o modal do trust e a execução espera o próximo clique.

use crate::recipes::Recipe;
use forja_kernel::{Args, Ctx};
use serde_json::json;

/// Confere ou pede aprovação para uma receita de projeto.
pub fn approve(cx: &mut Ctx<'_>, recipe: &Recipe) -> Result<(), String> {
    if recipe.scope != "project" || recipe.source_path.is_empty() {
        return Ok(());
    }
    if cx.registry().command("trust.check").is_none() {
        return Err(cx.t("terminal.view.trust-missing"));
    }
    let mut check = Args::new();
    check.insert("kind", json!("recipe"));
    check.insert("source_path", json!(recipe.source_path));
    check.insert("content_hash", json!(recipe.content_hash));
    let approved = cx
        .run_command("trust.check", check)
        .ok()
        .and_then(|value| value.get("approved").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if approved {
        return Ok(());
    }
    let mut request = Args::new();
    request.insert("kind", json!("recipe"));
    request.insert("source_path", json!(recipe.source_path));
    request.insert("content", json!(recipe.content));
    request.insert(
        "summary",
        json!(format!("receita {}: {}", recipe.id, recipe.command)),
    );
    let approved = cx
        .run_command("trust.request", request)
        .ok()
        .and_then(|value| value.get("approved").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if approved {
        return Ok(());
    }
    let locale = cx.i18n().locale().id().to_owned();
    let title = recipe.title(&locale).to_owned();
    Err(cx.t_args(
        "terminal.view.recipe-needs-trust",
        &[("recipe", &title)],
    ))
}
