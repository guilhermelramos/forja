//! Registro e estado das opções de contexto, prontidão e retomada do ajudante.
//!
//! Mantém fora de `lib.rs` o registro das quatro chaves ligadas em #204, os
//! eventos de ativação de bancada/prontidão e a estado correspondente.

use crate::{context, ready, resume, Shared};
use forja_contracts::{names, BenchActivated, RequestRef, TerminalCommandFinished};
use forja_kernel::{Args, Ctx, Registrar, SettingKind, SettingSpec, SettingValue};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

/// Estado em memória do contexto inicial, prontidão e retomada.
#[derive(Default)]
pub struct State {
    /// Cache assíncrono dos arquivos de regras lidos por `(bancada, candidatos)`.
    pub rules_cache: BTreeMap<context::RulesCacheKey, Vec<context::RuleSnippet>>,
    /// Leituras assíncronas de regras em andamento.
    pub rules_loading: BTreeSet<context::RulesCacheKey>,
    /// Estado de prontidão de cada execução por `terminal_id`.
    pub ready_states: BTreeMap<String, ready::ReadyState>,
    /// Textos aguardando o ajudante sinalizar prontidão.
    pub deferred_sends: BTreeMap<String, Vec<String>>,
    /// Último aviso registrado para uma bancada.
    pub bench_notices: BTreeMap<String, String>,
    /// Retomadas pendentes (`resume_policy = "ask"`).
    pub pending_resumes: BTreeMap<String, resume::PendingResume>,
    /// Avisos quando `auto` cai para `never`.
    pub resume_notices: BTreeMap<String, String>,
    /// O arquivo persistente já foi carregado nesta sessão.
    pub file_loaded: bool,
    /// Uma gravação está em voo; as demais esperam na fila.
    pub write_inflight: bool,
    /// Snapshot mais recente aguardando a vez de gravar.
    pub pending_write: Option<crate::store::HelperFile>,
    /// Bancadas já avaliadas na restauração do app.
    pub restored_evaluated: BTreeSet<String>,
}

/// Identidade e texto necessários para entregar o contexto inicial pelo destino certo.
pub(crate) struct ContextDelivery {
    pub bench: PathBuf,
    pub candidates: Vec<String>,
    pub context_mode: String,
    pub request_ref: Option<RequestRef>,
    pub bench_ctx: Option<context::BenchContext>,
    pub explicit_prompt: Option<String>,
    pub chat_opened: bool,
    pub chat_session: Option<String>,
    pub terminal_id: Option<String>,
}

/// Carrega as regras (em tarefa do kernel quando necessário) e só depois monta
/// e entrega o primeiro prompt, na mesma ordem para chat e terminal.
pub(crate) fn deliver_context(cx: &mut Ctx<'_>, core: &Shared, delivery: ContextDelivery) {
    let key = context::cache_key(&delivery.bench, &delivery.candidates);
    let cached = core
        .with_state(|state| state.launch.rules_cache.get(&key).cloned())
        .ok()
        .flatten();
    if let Some(rules) = cached {
        let text = compose(&delivery, Some(&rules));
        send(cx, core, &delivery, text);
        return;
    }
    let owned = Arc::clone(core);
    let bench_bg = delivery.bench.clone();
    let candidates_bg = delivery.candidates.clone();
    cx.spawn(
        move || context::read_rules_files(&bench_bg, &candidates_bg),
        move |cx, rules| {
            let _ = owned.with_state(|state| {
                state.launch.rules_cache.insert(key, rules.clone());
            });
            let text = compose(&delivery, Some(&rules));
            send(cx, &owned, &delivery, text);
        },
    );
}

fn compose(delivery: &ContextDelivery, rules: Option<&[context::RuleSnippet]>) -> Option<String> {
    context::compose_initial_prompt(
        &delivery.context_mode,
        delivery.bench_ctx.as_ref(),
        delivery.request_ref.as_ref(),
        rules,
        delivery.explicit_prompt.as_deref(),
    )
}

fn send(cx: &mut Ctx<'_>, core: &Shared, delivery: &ContextDelivery, text: Option<String>) {
    let Some(text) = text else {
        return;
    };
    if delivery.chat_opened {
        // `chat.send` usa a sessão ativa: confirme a identidade antes de enviar.
        if let Ok(status) = cx.run_command("chat.status", Args::new()) {
            let same_session = delivery
                .chat_session
                .as_deref()
                .is_none_or(|expected| status.get("session_id").and_then(Value::as_str) == Some(expected));
            let same_bench = status
                .get("bench_path")
                .and_then(Value::as_str)
                .is_some_and(|path| crate::status::same_path(path, &delivery.bench.display().to_string()));
            if same_session && same_bench {
                let mut args = Args::new();
                args.insert("prompt", json!(text));
                let _ = cx.run_command("chat.send", args);
            }
        }
        return;
    }
    if let Some(terminal_id) = &delivery.terminal_id {
        let ready = core
            .with_state(|state| {
                state
                    .launch
                    .deferred_sends
                    .entry(terminal_id.clone())
                    .or_default()
                    .push(text);
                state
                    .launch
                    .ready_states
                    .get(terminal_id)
                    .is_some_and(|entry| entry.ready)
            })
            .unwrap_or(false);
        // As regras podem chegar depois do sinal de prontidão: entregue de vez.
        if ready {
            ready::flush_deferred_sends(cx, core, terminal_id);
        }
    }
}

const I18N_KEYS: &[&str] = &[
    "helper.command.resume-evaluate",
    "helper.ready.timeout-title",
    "helper.ready.timeout-body",
    "helper.resume.ask-title",
    "helper.resume.ask-body",
    "helper.resume.unsupported-title",
    "helper.resume.unsupported-body",
    "helper.resume.none-found",
    "setting.helper.launch.initial_context.option.request",
    "setting.helper.launch.initial_context.option.bench",
    "setting.helper.launch.initial_context.option.none",
    "setting.helper.sessions.resume_policy.option.ask",
    "setting.helper.sessions.resume_policy.option.auto",
    "setting.helper.sessions.resume_policy.option.never",
];

/// Registra chaves, eventos, comandos de retomada e a prontidão por terminal.
pub(crate) fn register(r: &mut Registrar<'_>, core: &Shared) {
    for key in I18N_KEYS {
        r.uses_i18n(key);
    }
    let mut add = |key: &str, kind: SettingKind, default: SettingValue| {
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{}-label", key.replace('.', "-")),
            format!("setting-{}-help", key.replace('.', "-")),
        ));
    };
    add(
        "helper.launch.initial_context",
        SettingKind::Choice {
            options: ["request", "bench", "none"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("request".to_owned()),
    );
    add(
        "helper.launch.include_rules",
        SettingKind::Bool,
        SettingValue::Bool(true),
    );
    add(
        "helper.launch.ready_timeout_s",
        SettingKind::Integer {
            min: Some(5),
            max: Some(120),
        },
        SettingValue::Integer(30),
    );
    add(
        "helper.sessions.resume_policy",
        SettingKind::Choice {
            options: ["ask", "auto", "never"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
    );

    r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
    r.contract_event::<TerminalCommandFinished>(names::TERMINAL_COMMAND_FINISHED);
    let bench_core = Arc::clone(core);
    r.subscribe(move |cx, event: &BenchActivated| {
        context::schedule_rules_preload(
            cx,
            &bench_core,
            PathBuf::from(&event.0.path),
            vec!["AGENTS.md".to_owned(), "CLAUDE.md".to_owned()],
        );
        resume::on_bench_activated(cx, &bench_core, &event.0);
    });

    resume::register(r, core);

    let finished_core = Arc::clone(core);
    r.subscribe(move |cx, event: &TerminalCommandFinished| {
        let _ = crate::commands::ensure_current(cx, &finished_core);
        crate::status::update_run(
            cx,
            &finished_core,
            &event.terminal,
            forja_contracts::HelperStatus::Idle,
            Some("command finished".to_owned()),
        );
    });
}

#[cfg(test)]
mod tests;
