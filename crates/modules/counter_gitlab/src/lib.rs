//! Módulo `counter_gitlab` da Forja: issues e pull requests do GitLab como fonte de pedidos.
//!
//! Registra `counter_gitlab.{describe,list,get,comment,transition,test,connect,disconnect}`.
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
static COUNTER_GITHUB: fn() -> Box<dyn Module> = || Box::new(GitlabCounter::new());

/// Estado mostrado na seção de Ajustes. Atualizado pelos resultados de `counter.result`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub account: Option<String>,
    pub error: Option<forja_contracts::CounterError>,
    pub busy: bool,
}

pub(crate) type Shared = Arc<Mutex<Status>>;

pub struct GitlabCounter {
    env: Env,
    status: Shared,
}

impl GitlabCounter {
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

impl Default for GitlabCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for GitlabCounter {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(
            "counter_gitlab",
            "counter_gitlab.name",
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
            "counter_gitlab.describe",
            "counter_gitlab.command.describe",
            |_cx, _args| {
                Ok(json!(CounterSource {
                    counter: ops::COUNTER.to_owned(),
                    module: ops::MODULE.to_owned(),
                    label_key: "counter_gitlab.name".to_owned(),
                }))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.list",
            "counter_gitlab.command.list",
            move |cx, args| {
                let args: ListArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::list(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.get",
            "counter_gitlab.command.get",
            move |cx, args| {
                let args: ItemArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::get(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.comment",
            "counter_gitlab.command.comment",
            move |cx, args| {
                let args: CommentArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::comment(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.transition",
            "counter_gitlab.command.transition",
            move |cx, args| {
                let args: TransitionArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::transition(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.test",
            "counter_gitlab.command.test",
            move |cx, args| {
                let args: OpArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::test(&env, &config, args.op_id))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.connect",
            "counter_gitlab.command.connect",
            move |cx, args| {
                let args: ConnectArgs = serde_json::from_value(args.into_json())?;
                let config = config(cx);
                spawn(cx, &env, move |env| ops::connect(&env, &config, args))
            },
        );

        let env = self.env.clone();
        r.command(
            "counter_gitlab.disconnect",
            "counter_gitlab.command.disconnect",
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
            "counter_gitlab.connection",
            "counter_gitlab.name",
            300,
            move || {
                Box::new(ConnectionView {
                    status: Arc::clone(&status),
                    token: String::new(),
                }) as Box<dyn View>
            },
        );

        for key in [
            "counter_gitlab-name",
            "counter_gitlab-command-describe",
            "counter_gitlab-command-list",
            "counter_gitlab-command-get",
            "counter_gitlab-command-comment",
            "counter_gitlab-command-transition",
            "counter_gitlab-command-test",
            "counter_gitlab-command-connect",
            "counter_gitlab-command-disconnect",
        ] {
            r.uses_i18n(key);
        }
        for suffix in forja_counter_kit::ERROR_SUFFIXES {
            r.uses_i18n(&format!("counter_gitlab-error-{suffix}"));
        }
        for key in view::I18N_KEYS {
            r.uses_i18n(key);
        }
    }
}

/// Lê as configurações do balcão na thread da interface; o trabalho usa só cópias.
fn config(cx: &Ctx<'_>) -> Config {
    let host = cx
        .setting("counter.gitlab.host")
        .and_then(SettingValue::as_str)
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .unwrap_or("gitlab.com")
        .to_owned();
    Config { host }
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
