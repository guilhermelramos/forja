//! Módulo `routine` da Forja: rotinas agendadas (prompt para ajudante ou comando num terminal),
//! com pré-checagem, tolerância a horários perdidos, histórico e a página "Rotinas".
//!
//! Registra os comandos `routine.{save,delete,run}`, os métodos RPC/CLI `routine.{list,runs,run}`,
//! a aba "Rotinas" (slot `bench_tab`) e um passo periódico na barra de status, que roda o
//! agendador mesmo com a aba fechada. Contrato em `design/contracts.md`, seção "Rotinas".

mod actions;
mod commands;
mod draft;
mod execution;
mod fixtures;
mod model;
mod runner;
mod schedule;
mod state;
mod view;

#[cfg(test)]
mod tests;

use crate::state::Shared;
use forja_contracts::{
    names, BenchCreated, HelperStatus, HelperStatusChanged, NotifyRequested,
    TerminalCommandFinished, TerminalExited,
};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError};
use forja_kernel::ui_contract::View;
use forja_kernel::{Args, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot};
use serde_json::{json, Value};
use std::sync::Arc;

#[forja_kernel::module]
static ROUTINE: fn() -> Box<dyn Module> = || Box::new(RoutineModule::new());

/// Dono do estado das rotinas. A aba, a barra de status e os comandos compartilham o `Shared`.
pub struct RoutineModule {
    shared: Shared,
}

impl RoutineModule {
    pub fn new() -> Self {
        Self {
            shared: state::new_shared(),
        }
    }
}

impl Default for RoutineModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for RoutineModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("routine", "routine.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        r.contract_event::<TerminalCommandFinished>(names::TERMINAL_COMMAND_FINISHED);
        r.contract_event::<TerminalExited>(names::TERMINAL_EXITED);
        r.contract_event::<HelperStatusChanged>(names::HELPER_STATUS_CHANGED);
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);
        r.contract_event::<NotifyRequested>(names::NOTIFY_REQUESTED);

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &TerminalCommandFinished| {
            actions::terminal_done(cx, &shared, &event.terminal.terminal_id, event.exit_code, false);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &TerminalExited| {
            actions::terminal_done(cx, &shared, &event.terminal.terminal_id, event.exit_code, true);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &HelperStatusChanged| {
            let finished = event.status == HelperStatus::Finished;
            actions::helper_status(cx, &shared, &event.bench_path, finished);
        });

        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &BenchCreated| {
            actions::bench_created(cx, &shared, &event.0.name, &event.0.path);
        });

        let shared = Arc::clone(&self.shared);
        r.command("routine.save", "routine.command.save", move |cx, args| {
            let routine = args.get("routine").cloned().unwrap_or(Value::Null);
            commands::save(cx, &shared, routine)
        });

        let shared = Arc::clone(&self.shared);
        r.command("routine.delete", "routine.command.delete", move |cx, args| {
            let id = args.required_str("id")?.to_owned();
            commands::delete(cx, &shared, &id)
        });

        let shared = Arc::clone(&self.shared);
        r.command("routine.run", "routine.command.run", move |cx, args| {
            let id = args.required_str("id")?.to_owned();
            commands::run_now(cx, &shared, &id)
        });

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("routine.list", "routine.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("routine.method.list.result").schema(json!({
                    "type": "object",
                    "properties": { "routines": { "type": "array" } },
                    "required": ["routines"]
                }))),
            move |cx, _params| Ok(commands::list_snapshot(cx, &shared)),
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("routine.runs", "routine.method.runs.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(
                    ParamSchema::optional("routine_id", ParamKind::String)
                        .describe("Rotina cujo histórico se quer; vazio lista tudo."),
                )
                .param(
                    ParamSchema::optional("limit", ParamKind::Integer)
                        .describe("Quantas execuções, da mais recente. Padrão 20."),
                )
                .result(ResultSchema::new("routine.method.runs.result")),
            move |_cx, params| {
                let args = Args::from_map(params);
                let (routine_id, limit) = commands::runs_args(&args);
                Ok(commands::runs_snapshot(&shared, routine_id.as_deref(), limit))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("routine.run", "routine.method.run.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("id", ParamKind::String).describe("Id da rotina."))
                .result(ResultSchema::new("routine.method.run.result")),
            move |cx, params| {
                let id = params
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("id is required"))?
                    .to_owned();
                commands::run_now(cx, &shared, &id).map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("routine.save", "routine.method.save.description")
                .effect(MethodEffect::Write)
                .param(
                    ParamSchema::required("routine", ParamKind::Json)
                        .describe("Objeto JSON da rotina."),
                )
                .result(ResultSchema::new("routine.method.save.result")),
            move |cx, params| {
                let routine = match params.get("routine") {
                    Some(Value::String(text)) => serde_json::from_str::<Value>(text)
                        .map_err(|error| RpcError::invalid_params(error.to_string()))?,
                    Some(value) => value.clone(),
                    None => return Err(RpcError::invalid_params("routine is required")),
                };
                commands::save(cx, &shared, routine)
                    .map_err(|error: KernelError| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.view(Slot::Rail, "routine.page", "routine.tab.title", 40, move || {
            Box::new(view::RoutinesView::new(Arc::clone(&shared))) as Box<dyn View>
        });
        r.view_glyph("routine.page", forja_ui_kit::ROUTINES);

        let shared = Arc::clone(&self.shared);
        r.view(Slot::StatusBar, "routine.tick", "routine.tab.title", 20, move || {
            Box::new(view::TickView {
                shared: Arc::clone(&shared),
            }) as Box<dyn View>
        });

        for key in view::USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
        for key in model::USED_KEYS.iter().copied() {
            r.uses_i18n(key);
        }
    }
}

/// Configurações do grupo "Rotinas" do catálogo (`design/settings.toml`).
fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "routine.mode.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-routine-mode-enabled-label",
        "setting-routine-mode-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "routine.schedule.timezone",
        SettingKind::Choice {
            options: vec!["system".to_owned(), "utc".to_owned()],
        },
        SettingValue::Text("system".to_owned()),
        "setting-routine-schedule-timezone-label",
        "setting-routine-schedule-timezone-help",
    ));
    r.setting(SettingSpec::new(
        "routine.schedule.missed",
        SettingKind::Choice {
            options: vec!["skip".to_owned(), "run_once".to_owned(), "ask".to_owned()],
        },
        SettingValue::Text("run_once".to_owned()),
        "setting-routine-schedule-missed-label",
        "setting-routine-schedule-missed-help",
    ));
    r.setting(SettingSpec::new(
        "routine.schedule.grace_s",
        SettingKind::Integer {
            min: Some(0),
            max: Some(3600),
        },
        SettingValue::Integer(120),
        "setting-routine-schedule-grace_s-label",
        "setting-routine-schedule-grace_s-help",
    ));
    r.setting(SettingSpec::new(
        "routine.run.timeout_min",
        SettingKind::Integer {
            min: Some(1),
            max: Some(720),
        },
        SettingValue::Integer(60),
        "setting-routine-run-timeout_min-label",
        "setting-routine-run-timeout_min-help",
    ));
    r.setting(SettingSpec::new(
        "routine.run.concurrency",
        SettingKind::Integer {
            min: Some(1),
            max: Some(8),
        },
        SettingValue::Integer(1),
        "setting-routine-run-concurrency-label",
        "setting-routine-run-concurrency-help",
    ));
    r.setting(SettingSpec::new(
        "routine.run.target",
        SettingKind::Choice {
            options: vec!["ask".to_owned(), "fixed".to_owned(), "new".to_owned()],
        },
        SettingValue::Text("ask".to_owned()),
        "setting-routine-run-target-label",
        "setting-routine-run-target-help",
    ));
    r.setting(SettingSpec::new(
        "routine.logs.retention_days",
        SettingKind::Integer {
            min: Some(1),
            max: Some(365),
        },
        SettingValue::Integer(30),
        "setting-routine-logs-retention_days-label",
        "setting-routine-logs-retention_days-help",
    ));
    r.setting(SettingSpec::new(
        "routine.notifications.failure",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-routine-notifications-failure-label",
        "setting-routine-notifications-failure-help",
    ));
    r.setting(SettingSpec::new(
        "routine.notifications.success",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-routine-notifications-success-label",
        "setting-routine-notifications-success-help",
    ));
}
