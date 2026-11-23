//! Módulo `hello` da Forja.
//!
//! Gerado por `cargo xtask new-module hello`. O módulo demonstra o contrato
//! completo: comando, aba, configuração, atalho, i18n e método RPC.

use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{
    Args, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[forja_kernel::module]
static HELLO: fn() -> Box<dyn Module> = || Box::new(HelloModule { state: Shared::new() });

#[derive(Default)]
struct State {
    history: Vec<String>,
    error: Option<String>,
}

#[derive(Clone)]
struct Shared(Arc<Mutex<State>>);

impl Shared {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    fn push(&self, message: String) {
        if let Ok(mut state) = self.0.lock() {
            state.error = None;
            state.history.push(message);
        }
    }

    fn fail(&self, message: String) {
        if let Ok(mut state) = self.0.lock() {
            state.error = Some(message);
        }
    }

    fn snapshot(&self) -> (Vec<String>, Option<String>) {
        match self.0.lock() {
            Ok(state) => (state.history.clone(), state.error.clone()),
            Err(_) => (Vec::new(), None),
        }
    }
}

pub struct HelloModule {
    state: Shared,
}

impl Module for HelloModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("hello", "hello.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));

        r.setting(SettingSpec::new(
            "hello.greeting.style",
            SettingKind::Choice {
                options: vec!["warm".to_owned(), "plain".to_owned()],
            },
            SettingValue::Text("warm".to_owned()),
            "setting-hello-greeting-style-label",
            "setting-hello-greeting-style-help",
        ));

        let command_state = self.state.clone();
        r.command("hello.greet", "hello.command.greet", move |cx, args| {
            let message = greet(cx, args.str("name"), &command_state)?;
            Ok(json!({ "message": message }))
        });

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("hello.greet", "hello.method.greet.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(
                    ParamSchema::required("name", ParamKind::String)
                        .describe("Nome de quem recebe o cumprimento.")
                        .alias("nome"),
                )
                .result(
                    ResultSchema::new("hello.method.greet.result").schema(json!({
                        "type": "object",
                        "properties": { "message": { "type": "string" } },
                        "required": ["message"]
                    })),
                ),
            move |cx, params| {
                let name = params.get("name").and_then(Value::as_str);
                let message = greet(cx, name, &rpc_state)
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))?;
                Ok(json!({ "message": message }))
            },
        );

        for key in [
            "hello.greeting.warm",
            "hello.greeting.plain",
            "hello.default_name",
            "hello.view.button",
            "hello.view.history",
            "hello.view.empty",
            "hello.view.error",
        ] {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "hello.tab",
            "hello.tab.title",
            100,
            move || Box::new(HelloView { state: view_state.clone() }),
        );
    }
}

fn greet(
    cx: &mut forja_kernel::Ctx<'_>,
    name: Option<&str>,
    state: &Shared,
) -> forja_kernel::Result<String> {
    let style = cx
        .setting("hello.greeting.style")
        .and_then(SettingValue::as_str)
        .unwrap_or("warm")
        .to_owned();
    let name = name.map(str::to_owned).unwrap_or_else(|| cx.t("hello.default_name"));
    let message_key = if style == "plain" {
        "hello.greeting.plain"
    } else {
        "hello.greeting.warm"
    };
    let message = cx.t_args(message_key, &[("name", name.as_str())]);
    state.push(message.clone());
    Ok(message)
}

struct HelloView {
    state: Shared,
}

impl View for HelloView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut forja_kernel::Ctx<'_>) {
        ui.heading(cx.t("hello.tab.title"));
        if ui.button(cx.t("hello.view.button")).clicked() {
            if let Err(error) = cx.run_command("hello.greet", Args::new()) {
                let message = error.to_string();
                let text = cx.t_args("hello.view.error", &[("message", &message)]);
                self.state.fail(text);
            }
        }
        let (history, error) = self.state.snapshot();
        if let Some(error) = error {
            ui.label(egui::RichText::new(error).color(ui.visuals().error_fg_color));
        }
        ui.add_space(8.0);
        ui.label(cx.t("hello.view.history"));
        if history.is_empty() {
            ui.label(egui::RichText::new(cx.t("hello.view.empty")).weak());
        } else {
            for message in history.iter().rev() {
                ui.label(message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-hello-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(HelloModule { state: Shared::new() })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("module registers cleanly")
    }

    #[test]
    fn contract_and_rpc_acceptance() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());

        let params = Map::from_iter([("name".to_owned(), json!("Ana"))]);
        let value = runtime.call_method("hello.greet", params).unwrap();
        let message = value["message"].as_str().expect("message string");
        assert!(message.contains("Ana"), "message was {message}");

        let value = runtime.run_command("hello.greet", Args::new()).unwrap();
        assert!(value["message"].as_str().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
