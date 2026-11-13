//! Módulo `counter_linear` da Forja: issues e pull requests do Linear como fonte de pedidos.
//!
//! Registra `counter_linear.{describe,list,get,comment,transition,test,connect,disconnect}`.
//! Os comandos de rede respondem `{started: true}` e publicam o resultado em `counter.result`.
//! O token fica no cofre do sistema, por host; a seção de Ajustes conecta, testa e desconecta.
//! Contrato: `design/contracts.md`, seção "Pedidos e balcões".

mod api;
mod ops;
mod view;

use forja_contracts::{
    names, CommentArgs, ConnectArgs, CounterOp, CounterResult, CounterSource, ItemArgs, ListArgs,
    OpArgs, TransitionArgs,
};
use forja_counter_kit::{default_secrets, default_transport, transport_for_policy};
use forja_kernel::ui_contract::View;
use forja_kernel::{Args, Ctx, Module, ModuleManifest, Registrar, Result, SettingValue, Slot};
use ops::{Config, Env};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use view::ConnectionView;

#[forja_kernel::module]
static COUNTER_GITHUB: fn() -> Box<dyn Module> = || Box::new(LinearCounter::new());

/// Estado mostrado na seção de Ajustes. Atualizado pelos resultados de `counter.result`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub account: Option<String>,
    pub error: Option<forja_contracts::CounterError>,
    pub busy: bool,
}

pub(crate) type Shared = Arc<Mutex<Status>>;

pub struct LinearCounter {
    env: Env,
    status: Shared,
}

impl LinearCounter {
    pub fn new() -> Self {
        Self::with_env(Env {
            transport: default_transport(),
            secrets: default_secrets(),
        })
    }

    /// Versão para testes: transporte e cofre são trocados sem tocar a rede nem o sistema.
    pub fn with_env(env: Env) -> Self {
        Self {
            env,
            status: Arc::new(Mutex::new(Status::default())),
        }
    }
}

impl Default for LinearCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LinearCounter {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(
            "counter_linear",
            "counter_linear.name",
            env!("CARGO_PKG_VERSION"),
        )
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        r.contract_event::<CounterResult>(names::COUNTER_RESULT);

        let status = Arc::clone(&self.status);
        r.subscribe(move |_cx, result: &CounterResult| {
            if result.counter == ops::COUNTER {
                apply_result(&status, result);
            }
        });

        r.command(
            "counter_linear.describe",
            "counter_linear.command.describe",
            |_cx, _args| {
                Ok(json!(CounterSource {
                    counter: ops::COUNTER.to_owned(),
                    module: ops::MODULE.to_owned(),
                    label_key: "counter_linear.name".to_owned(),
                }))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.list",
            "counter_linear.command.list",
            move |cx, args| {
                let args: ListArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::list(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.get",
            "counter_linear.command.get",
            move |cx, args| {
                let args: ItemArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::get(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.comment",
            "counter_linear.command.comment",
            move |cx, args| {
                let args: CommentArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::comment(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.transition",
            "counter_linear.command.transition",
            move |cx, args| {
                let args: TransitionArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::transition(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.test",
            "counter_linear.command.test",
            move |cx, args| {
                let args: OpArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::test(&env, &config, args.op_id))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.connect",
            "counter_linear.command.connect",
            move |cx, args| {
                let args: ConnectArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::connect(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_linear.disconnect",
            "counter_linear.command.disconnect",
            move |cx, args| {
                let args: OpArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| {
                    ops::disconnect(&env, &config, args.op_id)
                })
            },
        );

        let status = Arc::clone(&self.status);
        r.view(
            Slot::SettingsSection,
            "counter_linear.connection",
            "counter_linear.name",
            300,
            move || {
                Box::new(ConnectionView {
                    status: Arc::clone(&status),
                    token: String::new(),
                }) as Box<dyn View>
            },
        );

        for key in [
            "counter_linear-name",
            "counter_linear-command-describe",
            "counter_linear-command-list",
            "counter_linear-command-get",
            "counter_linear-command-comment",
            "counter_linear-command-transition",
            "counter_linear-command-test",
            "counter_linear-command-connect",
            "counter_linear-command-disconnect",
        ] {
            r.uses_i18n(key);
        }
        for suffix in forja_counter_kit::ERROR_SUFFIXES {
            r.uses_i18n(&format!("counter_linear-error-{suffix}"));
        }
        for key in view::I18N_KEYS {
            r.uses_i18n(key);
        }
    }
}

/// Lê a equipe configurada na thread da interface; o trabalho usa só uma cópia.
fn config(cx: &Ctx<'_>) -> Config {
    let team = cx
        .setting("counter.linear.team")
        .and_then(SettingValue::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_owned();
    Config { team }
}

/// Manda o trabalho para a thread de fundo e devolve já, com `{started: true}`.
fn spawn<F>(cx: &mut Ctx<'_>, base: &Env, work: F) -> Result<Value>
where
    F: FnOnce(Env) -> CounterResult + Send + 'static,
{
    let policy = cx.run_command("network.proxy", Args::new()).ok();
    let env = Env {
        transport: transport_for_policy(&base.transport, policy.as_ref()),
        secrets: Arc::clone(&base.secrets),
    };
    cx.spawn(
        move || work(env),
        |cx, result: CounterResult| {
            cx.emit(&result);
        },
    );
    Ok(json!({ "started": true }))
}

fn apply_result(status: &Shared, result: &CounterResult) {
    let Ok(mut status) = status.lock() else {
        return;
    };
    status.busy = false;
    status.error = result.error.clone();
    match result.op {
        CounterOp::Connect | CounterOp::Test => {
            if result.error.is_none() {
                status.account = result.account.clone();
            } else {
                status.account = None;
            }
        }
        CounterOp::Disconnect if result.error.is_none() => {
            status.account = None;
            status.error = None;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
