//! `cargo xtask new-module <name>`.
//!
//! Creates `crates/modules/<name>` from the template and adds the dependency
//! to the `# forja:modules` region of `crates/shell/Cargo.toml`. No other file
//! outside the module folder is touched.

use crate::util::{pascal_case, read, repo_root, validate_name, write};
use std::path::Path;

pub fn run(name: &str) -> Result<(), String> {
    validate_name(name)?;
    let root = repo_root();
    let module_dir = root.join("crates").join("modules").join(name);
    if module_dir.exists() {
        return Err(format!(
            "{} already exists; choose another name or remove it",
            module_dir.display()
        ));
    }

    let pascal = pascal_case(name);
    let factory = name.to_ascii_uppercase();
    let files = [
        ("Cargo.toml", cargo_toml(name)),
        ("MODULE.md", module_md(name)),
        ("i18n/en.ftl", english(name)),
        ("i18n/pt-BR.ftl", portuguese(name)),
        ("keymap.toml", keymap(name)),
        ("src/lib.rs", lib_rs(name, &pascal, &factory)),
    ];
    for (relative, contents) in &files {
        write(&module_dir.join(relative), contents)?;
    }

    let shell_manifest = root.join("crates").join("shell").join("Cargo.toml");
    add_shell_dependency(&shell_manifest, name)?;

    println!("forja: module `{name}` created");
    for (relative, _) in &files {
        println!("  {}", module_dir.join(relative).display());
    }
    println!("  {} (only the # forja:modules region)", shell_manifest.display());
    Ok(())
}

fn cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "forja-module-{name}"
description = "Forja module {name}."
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
forja-kernel = {{ path = "../../kernel", features = ["egui"] }}
linkme.workspace = true
serde_json.workspace = true
"#
    )
}

fn module_md(name: &str) -> String {
    format!(
        r#"# forja-module-{name}

Módulo `{name}` da Forja. Gerado por `cargo xtask new-module {name}`.

## O que registra

- comando `{name}.greet` (Bigorna e CLI);
- aba `{name}.tab` na área de bancada;
- configuração `{name}.greeting.style` (`warm` ou `plain`);
- atalho `Ctrl+Alt+H` no escopo `{name}` (`keymap.toml`);
- método RPC `{name}.greet` com o parâmetro `name` (alias `nome`);
- bundles Fluent `i18n/en.ftl` e `i18n/pt-BR.ftl`.

## Contratos

- Toda chave i18n usada em runtime é anunciada com `Registrar::uses_i18n`.
- O teste do crate roda o contrato do kernel sobre o módulo.
- Nada fora de `crates/modules/{name}` precisa ser editado; o vínculo com o
  shell é feito pelo `xtask` na região `# forja:modules`.

## Como testar

```sh
cargo test -p forja-module-{name}
cargo run -p forja-app
forja {name} greet --nome Ana
```
"#
    )
}

fn english(name: &str) -> String {
    format!(
        r#"{name}-name = {pascal}
{name}-command-greet = Greet
{name}-tab-title = {pascal}
setting-{name}-greeting-style-label = Greeting style
setting-{name}-greeting-style-help = Chooses between a warm and a plain greeting.
{name}-greeting-warm = Hello, {{ $name }}!
{name}-greeting-plain = Hi, {{ $name }}.
{name}-method-greet-description = Greets a person and records the message.
{name}-method-greet-result = Greeting message.
{name}-default_name = world
{name}-view-button = Greet
{name}-view-history = Greetings
{name}-view-empty = No greeting yet. Press "Greet" to start.
{name}-view-error = Could not greet: {{ $message }}
"#,
        pascal = pascal_case(name)
    )
}

fn portuguese(name: &str) -> String {
    format!(
        r#"{name}-name = {pascal}
{name}-command-greet = Cumprimentar
{name}-tab-title = {pascal}
setting-{name}-greeting-style-label = Estilo do cumprimento
setting-{name}-greeting-style-help = Escolhe entre um cumprimento caloroso e um simples.
{name}-greeting-warm = Olá, {{ $name }}!
{name}-greeting-plain = Oi, {{ $name }}.
{name}-method-greet-description = Cumprimenta uma pessoa e registra a mensagem.
{name}-method-greet-result = Mensagem do cumprimento.
{name}-default_name = mundo
{name}-view-button = Cumprimentar
{name}-view-history = Cumprimentos
{name}-view-empty = Nenhum cumprimento ainda. Clique em "Cumprimentar" para começar.
{name}-view-error = Não foi possível cumprimentar: {{ $message }}
"#,
        pascal = pascal_case(name)
    )
}

fn keymap(name: &str) -> String {
    format!(
        r#"# Atalho padrão do módulo {name}. O escopo próprio evita conflito com os
# atalhos globais do produto.
[[binding]]
id = "{name}.greet"
key = "Ctrl+Alt+H"
scope = "{name}"
title_key = "{name}.command.greet"
"#
    )
}

fn lib_rs(name: &str, pascal: &str, factory: &str) -> String {
    format!(
        r#"//! Módulo `{name}` da Forja.
//!
//! Gerado por `cargo xtask new-module {name}`. O módulo demonstra o contrato
//! completo: comando, aba, configuração, atalho, i18n e método RPC.

use forja_kernel::forja_protocol::{{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema}};
use forja_kernel::ui_contract::{{egui, View}};
use forja_kernel::{{
    Args, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
}};
use serde_json::{{json, Value}};
use std::sync::{{Arc, Mutex}};

#[forja_kernel::module]
static {factory}: fn() -> Box<dyn Module> = || Box::new({pascal}Module {{ state: Shared::new() }});

#[derive(Default)]
struct State {{
    history: Vec<String>,
    error: Option<String>,
}}

#[derive(Clone)]
struct Shared(Arc<Mutex<State>>);

impl Shared {{
    fn new() -> Self {{
        Self(Arc::new(Mutex::new(State::default())))
    }}

    fn push(&self, message: String) {{
        if let Ok(mut state) = self.0.lock() {{
            state.error = None;
            state.history.push(message);
        }}
    }}

    fn fail(&self, message: String) {{
        if let Ok(mut state) = self.0.lock() {{
            state.error = Some(message);
        }}
    }}

    fn snapshot(&self) -> (Vec<String>, Option<String>) {{
        match self.0.lock() {{
            Ok(state) => (state.history.clone(), state.error.clone()),
            Err(_) => (Vec::new(), None),
        }}
    }}
}}

pub struct {pascal}Module {{
    state: Shared,
}}

impl Module for {pascal}Module {{
    fn manifest(&self) -> ModuleManifest {{
        ModuleManifest::new("{name}", "{name}.name", env!("CARGO_PKG_VERSION"))
    }}

    fn register(&mut self, r: &mut Registrar<'_>) {{
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));

        r.setting(SettingSpec::new(
            "{name}.greeting.style",
            SettingKind::Choice {{
                options: vec!["warm".to_owned(), "plain".to_owned()],
            }},
            SettingValue::Text("warm".to_owned()),
            "setting-{name}-greeting-style-label",
            "setting-{name}-greeting-style-help",
        ));

        let command_state = self.state.clone();
        r.command("{name}.greet", "{name}.command.greet", move |cx, args| {{
            let message = greet(cx, args.str("name"), &command_state)?;
            Ok(json!({{ "message": message }}))
        }});

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("{name}.greet", "{name}.method.greet.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(
                    ParamSchema::required("name", ParamKind::String)
                        .describe("Nome de quem recebe o cumprimento.")
                        .alias("nome"),
                )
                .result(
                    ResultSchema::new("{name}.method.greet.result").schema(json!({{
                        "type": "object",
                        "properties": {{ "message": {{ "type": "string" }} }},
                        "required": ["message"]
                    }})),
                ),
            move |cx, params| {{
                let name = params.get("name").and_then(Value::as_str);
                let message = greet(cx, name, &rpc_state)
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))?;
                Ok(json!({{ "message": message }}))
            }},
        );

        for key in [
            "{name}.greeting.warm",
            "{name}.greeting.plain",
            "{name}.default_name",
            "{name}.view.button",
            "{name}.view.history",
            "{name}.view.empty",
            "{name}.view.error",
        ] {{
            r.uses_i18n(key);
        }}

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "{name}.tab",
            "{name}.tab.title",
            100,
            move || Box::new({pascal}View {{ state: view_state.clone() }}),
        );
    }}
}}

fn greet(
    cx: &mut forja_kernel::Ctx<'_>,
    name: Option<&str>,
    state: &Shared,
) -> forja_kernel::Result<String> {{
    let style = cx
        .setting("{name}.greeting.style")
        .and_then(SettingValue::as_str)
        .unwrap_or("warm")
        .to_owned();
    let name = name.map(str::to_owned).unwrap_or_else(|| cx.t("{name}.default_name"));
    let message_key = if style == "plain" {{
        "{name}.greeting.plain"
    }} else {{
        "{name}.greeting.warm"
    }};
    let message = cx.t_args(message_key, &[("name", name.as_str())]);
    state.push(message.clone());
    Ok(message)
}}

struct {pascal}View {{
    state: Shared,
}}

impl View for {pascal}View {{
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut forja_kernel::Ctx<'_>) {{
        ui.heading(cx.t("{name}.tab.title"));
        if ui.button(cx.t("{name}.view.button")).clicked() {{
            if let Err(error) = cx.run_command("{name}.greet", Args::new()) {{
                let message = error.to_string();
                let text = cx.t_args("{name}.view.error", &[("message", &message)]);
                self.state.fail(text);
            }}
        }}
        let (history, error) = self.state.snapshot();
        if let Some(error) = error {{
            ui.label(egui::RichText::new(error).color(egui::Color32::from_rgb(0xE5, 0x53, 0x4B)));
        }}
        ui.add_space(8.0);
        ui.label(cx.t("{name}.view.history"));
        if history.is_empty() {{
            ui.label(egui::RichText::new(cx.t("{name}.view.empty")).weak());
        }} else {{
            for message in history.iter().rev() {{
                ui.label(message);
            }}
        }}
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use forja_kernel::{{contract, Runtime, RuntimeOptions}};
    use serde_json::Map;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {{
        std::env::temp_dir().join(format!(
            "forja-{name}-test-{{}}-{{}}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }}

    fn runtime(dir: &std::path::Path) -> Runtime {{
        Runtime::new(
            vec![Box::new({pascal}Module {{ state: Shared::new() }})],
            RuntimeOptions {{
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            }},
        )
        .expect("module registers cleanly")
    }}

    #[test]
    fn contract_and_rpc_acceptance() {{
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());

        let params = Map::from_iter([("name".to_owned(), json!("Ana"))]);
        let value = runtime.call_method("{name}.greet", params).unwrap();
        let message = value["message"].as_str().expect("message string");
        assert!(message.contains("Ana"), "message was {{message}}");

        let value = runtime.run_command("{name}.greet", Args::new()).unwrap();
        assert!(value["message"].as_str().is_some());
        let _ = std::fs::remove_dir_all(dir);
    }}
}}
"#
    )
}

fn add_shell_dependency(manifest: &Path, name: &str) -> Result<(), String> {
    let text = read(manifest)?;
    let crate_name = format!("forja-module-{name}");
    if text.contains(&format!("{crate_name} =")) {
        return Ok(());
    }
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let start = text
        .find("# forja:modules:start")
        .ok_or_else(|| format!("{} has no # forja:modules:start marker", manifest.display()))?;
    let end = text
        .find("# forja:modules:end")
        .ok_or_else(|| format!("{} has no # forja:modules:end marker", manifest.display()))?;
    if end < start {
        return Err(format!("{} markers are out of order", manifest.display()));
    }
    let line_start = text[..end]
        .rfind('\n')
        .map(|position| position + 1)
        .unwrap_or(end);
    let indentation = &text[line_start..end];
    let dependency = format!(
        "{indentation}{crate_name} = {{ path = \"../modules/{name}\" }}{newline}"
    );
    let mut updated = String::with_capacity(text.len() + dependency.len());
    updated.push_str(&text[..line_start]);
    updated.push_str(&dependency);
    updated.push_str(&text[line_start..]);
    write(manifest, &updated)
}
