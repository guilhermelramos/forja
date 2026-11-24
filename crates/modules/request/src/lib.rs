//! Módulo `request` da Forja: a página de Pedidos.
//!
//! Reúne os pedidos de todos os balcões instalados (`counter.sources`) numa lista só, com
//! filtros, busca, gaveta de detalhes e o botão Forjar, que cria uma bancada a partir do
//! pedido. Também guarda pedidos locais no store do módulo.
//!
//! Registra comandos `request.{list,refresh,forge,comment,transition,open}`,
//! `request.local.create`, o método RPC `request.list` (CLI `forja request list --json`) e a
//! aba "Pedidos". Contratos em `design/contracts.md`, seção "Pedidos e balcões".

mod commands;
pub mod demo;
mod forge;
mod forge_preview;
mod local;
mod state;
mod sync;
mod view;

#[cfg(test)]
mod tests;

use crate::commands::ListParams;
use crate::state::{Shared, State};
use forja_contracts::{
    names, BenchActivated, BenchCreated, CounterResult, RequestState, StageChanged,
};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::ui_contract::View;
use forja_kernel::{
    Args, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
    Slot,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[forja_kernel::module]
static REQUEST: fn() -> Box<dyn Module> = || Box::new(RequestModule::new());

/// Dono do estado da página. A aba e os comandos compartilham o mesmo `Shared`.
pub struct RequestModule {
    shared: Shared,
}

impl RequestModule {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl Default for RequestModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RequestModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("request", "request.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        r.contract_event::<CounterResult>(names::COUNTER_RESULT);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);

        r.contract_event::<StageChanged>(names::STAGE_CHANGED);
        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &StageChanged| {
            sync::apply_stage(cx, &shared, event);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |_cx, result: &CounterResult| {
            commands::apply_result(&shared, result);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |_cx, event: &BenchActivated| {
            commands::set_project(&shared, &event.0.project_id, &event.0.path);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &BenchCreated| {
            // Forjar pedido antes de a bancada existir: o ajudante sobe agora.
            if let Some(pending) = commands::take_pending_forge(&shared, &event.0.name) {
                let _ = forge::launch_helper(cx, &event.0.path, &pending.reference);
            }
        });

        let shared = Arc::clone(&self.shared);
        r.command("request.list", "request.command.list", move |cx, args| {
            commands::list(cx, &shared, list_params(&args))
        });

        let shared = Arc::clone(&self.shared);
        r.command(
            "request.refresh",
            "request.command.refresh",
            move |cx, _args| {
                commands::refresh(cx, &shared)?;
                Ok(json!({ "started": true }))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command("request.forge", "request.command.forge", move |cx, args| {
            let counter = args.required_str("counter")?.to_owned();
            let key = args.required_str("key")?.to_owned();
            commands::forge(cx, &shared, &counter, &key)
        });

        let shared = Arc::clone(&self.shared);
        r.command(
            "request.comment",
            "request.command.comment",
            move |cx, args| {
                let counter = args.required_str("counter")?.to_owned();
                let key = args.required_str("key")?.to_owned();
                let body = args.required_str("body")?.to_owned();
                commands::comment(cx, &shared, &counter, &key, &body)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "request.transition",
            "request.command.transition",
            move |cx, args| {
                let counter = args.required_str("counter")?.to_owned();
                let key = args.required_str("key")?.to_owned();
                let to: RequestState = serde_json::from_value(json!(args.required_str("to")?))?;
                commands::transition(cx, &shared, &counter, &key, to)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command("request.open", "request.command.open", move |cx, args| {
            let counter = args.required_str("counter")?.to_owned();
            let key = args.required_str("key")?.to_owned();
            commands::open(cx, &shared, &counter, &key)
        });

        let shared = Arc::clone(&self.shared);
        r.command(
            "request.local.create",
            "request.command.local_create",
            move |cx, args| {
                let title = args.required_str("title")?.to_owned();
                let body = args.str("body").map(str::to_owned);
                commands::create_local(cx, &shared, &title, body.as_deref())
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("request.list", "request.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(
                    ParamSchema::optional("filter", ParamKind::String)
                        .describe("Filtro: mine, to_review, open, recent ou all."),
                )
                .param(
                    ParamSchema::optional("query", ParamKind::String)
                        .describe("Texto para filtrar título, chave, rótulos e autor."),
                )
                .result(
                    ResultSchema::new("request.method.list.result").schema(json!({
                        "type": "object",
                        "properties": {
                            "filter": { "type": "string" },
                            "items": { "type": "array" },
                            "loading": { "type": "boolean" }
                        },
                        "required": ["items"]
                    })),
                ),
            move |cx, params| {
                let args = Args::from_map(params);
                commands::list(cx, &shared, list_params(&args))
                    .map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new(
                "request.local.create",
                "request.method.local_create.description",
            )
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("title", ParamKind::String).alias("titulo"))
            .param(ParamSchema::optional("body", ParamKind::String).alias("corpo"))
            .result(ResultSchema::new("request.method.local_create.result")),
            move |cx, params| {
                let title = params
                    .get("title")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("title is required"))?
                    .to_owned();
                let body = params
                    .get("body")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                commands::create_local(cx, &shared, &title, body.as_deref())
                    .map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("request.forge", "request.method.forge.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("counter", ParamKind::String).alias("balcao"))
                .param(ParamSchema::required("key", ParamKind::String).alias("chave"))
                .result(ResultSchema::new("request.method.forge.result")),
            move |cx, params| {
                let counter = params
                    .get("counter")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("counter is required"))?
                    .to_owned();
                let key = params
                    .get("key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("key is required"))?
                    .to_owned();
                commands::forge(cx, &shared, &counter, &key)
                    .map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.view(
            Slot::Rail,
            "request.page",
            "request.tab.title",
            50,
            move || Box::new(view::RequestsView::new(Arc::clone(&shared))) as Box<dyn View>,
        );
        r.view_glyph("request.page", forja_ui_kit::REQUESTS.id());

        for key in USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
        for suffix in forja_counter_kit::ERROR_SUFFIXES {
            r.uses_i18n(&format!("request-error-{suffix}"));
        }
    }
}

fn list_params(args: &Args) -> ListParams {
    ListParams {
        filter: args.str("filter").map(str::to_owned),
        query: args.str("query").map(str::to_owned),
    }
}

/// Configurações do catálogo que o módulo `request` declara (`design/settings.toml`).
fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "request.default.source",
        SettingKind::Choice {
            options: ["local", "github", "gitlab", "linear", "jira"]
                .iter()
                .map(|option| (*option).to_owned())
                .collect(),
        },
        SettingValue::Text("local".to_owned()),
        "setting-request-default-source-label",
        "setting-request-default-source-help",
    ));
    r.setting(SettingSpec::new(
        "request.create.branch_from_title",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-request-create-branch_from_title-label",
        "setting-request-create-branch_from_title-help",
    ));
    r.setting(SettingSpec::new(
        "request.create.assign_self",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-request-create-assign_self-label",
        "setting-request-create-assign_self-help",
    ));
    r.setting(SettingSpec::new(
        "request.filters.default",
        SettingKind::Text { max_len: Some(32) },
        SettingValue::Text(String::new()),
        "setting-request-filters-default-label",
        "setting-request-filters-default-help",
    ));
    r.setting(SettingSpec::new(
        "request.refresh.interval_s",
        SettingKind::Integer {
            min: Some(0),
            max: Some(600),
        },
        SettingValue::Integer(60),
        "setting-request-refresh-interval_s-label",
        "setting-request-refresh-interval_s-help",
    ));
    r.setting(SettingSpec::new(
        "request.sync.stage_on_pr",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-request-sync-stage_on_pr-label",
        "setting-request-sync-stage_on_pr-help",
    ));
    r.setting(SettingSpec::new(
        "request.sync.close_on_merge",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-request-sync-close_on_merge-label",
        "setting-request-sync-close_on_merge-help",
    ));
    r.setting(SettingSpec::new(
        "request.local.prefix",
        SettingKind::Text { max_len: Some(16) },
        SettingValue::Text("LOCAL-".to_owned()),
        "setting-request-local-prefix-label",
        "setting-request-local-prefix-help",
    ));
    r.setting(SettingSpec::new(
        "request.local.counter_start",
        SettingKind::Integer {
            min: Some(1),
            max: Some(100_000),
        },
        SettingValue::Integer(1),
        "setting-request-local-counter_start-label",
        "setting-request-local-counter_start-help",
    ));
}

/// Chaves de texto que a página usa, declaradas com `uses_i18n` para o teste de contrato.
pub(crate) const USED_KEYS: &[&str] = &[
    "request-name",
    "request-tab-title",
    "request-filter-mine",
    "request-filter-to_review",
    "request-filter-open",
    "request-filter-recent",
    "request-filter-all",
    "request-search-hint",
    "request-source-label",
    "request-refresh",
    "request-loading",
    "request-empty",
    "request-no-project",
    "request-local-title-hint",
    "request-local-body-hint",
    "request-local-create",
    "request-local-loading",
    "request-detail-close",
    "request-detail-empty-body",
    "request-detail-comments",
    "request-detail-no-comments",
    "request-comment-hint",
    "request-action-forge",
    "request-action-comment",
    "request-action-close",
    "request-action-reopen",
    "request-state-open",
    "request-state-closed",
    "request-state-merged",
    "request-kind-issue",
    "request-kind-change",
    "request-draft",
    "request-local-tag",
    "request-notice-forged",
    "request-notice-forging",
    "request-notice-bench-missing",
    "request-notice-no-project",
    "request-notice-not-listed",
    "request-notice-empty-title",
    "request-notice-empty-comment",
    "request-notice-local-loading",
    "request-notice-local-load-failed",
    "request-notice-save-failed",
    "request-command-list",
    "request-command-refresh",
    "request-command-forge",
    "request-command-comment",
    "request-command-transition",
    "request-command-open",
    "request-command-local_create",
    "request-method-list-description",
    "request-method-list-result",
];
