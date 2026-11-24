//! Módulo `mcp`: exposição do servidor MCP e receita de configuração.
//!
//! A ponte stdio vive em `crates/mcp` (`forja-mcp`). Este módulo decide quais
//! métodos do catálogo entram na lista exposta, lê as configurações do grupo
//! `mcp` e mostra o snippet de cada ajudante. Ele nunca escreve na configuração
//! de outro CLI.

use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{
    Args, Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use state::{Authorize, McpState};
use views::ApprovalView;

mod state;
mod views;

/// Estado protegido do módulo.
pub(crate) type Shared = Arc<Mutex<McpState>>;

#[forja_kernel::module]
static MCP: fn() -> Box<dyn Module> = || Box::new(McpModule::new());

/// Módulo MCP da Forja.
pub struct McpModule {
    state: Shared,
}

impl McpModule {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(McpState::default())),
        }
    }
}

impl Module for McpModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("mcp", "mcp.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        r.command("mcp.setup", "mcp.command.setup", |cx, args| {
            Ok(setup(cx, &args))
        });
        r.command("mcp.tools", "mcp.command.tools", |cx, _| Ok(tools(cx)));
        let authorize_state = Arc::clone(&self.state);
        r.command("mcp.authorize", "mcp.command.authorize", move |cx, args| {
            authorize(cx, &authorize_state, &args)
        });
        let approval_state = Arc::clone(&self.state);
        r.command("mcp.approval", "mcp.command.approval", move |cx, args| {
            approval(cx, &approval_state, &args)
        });
        let authorize_rpc = Arc::clone(&self.state);
        r.rpc(
            Method::new("mcp.authorize", "mcp.method.authorize.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("tool", ParamKind::String))
                .param(ParamSchema::optional("client", ParamKind::String))
                .param(ParamSchema::optional("summary", ParamKind::String))
                .param(ParamSchema::optional("effect", ParamKind::String))
                .result(ResultSchema::new("mcp.method.authorize.result")),
            move |cx, params| {
                let started = std::time::Instant::now();
                let mut args = Args::from_map(params);
                if args.str("effect").is_none() {
                    args.insert("effect", json!("write"));
                }
                let outcome = authorize(cx, &authorize_rpc, &args);
                record(cx, "mcp.authorize", started, outcome.is_ok());
                outcome
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
            },
        );
        let approval_rpc = Arc::clone(&self.state);
        r.rpc(
            Method::new("mcp.approval", "mcp.method.approval.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::required("request_id", ParamKind::String))
                .result(ResultSchema::new("mcp.method.approval.result")),
            move |cx, params| {
                let started = std::time::Instant::now();
                let outcome = approval(cx, &approval_rpc, &Args::from_map(params));
                record(cx, "mcp.approval", started, outcome.is_ok());
                outcome
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
            },
        );
        r.rpc(
            Method::new("mcp.tools", "mcp.method.tools.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("mcp.method.tools.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "enabled": { "type": "boolean" },
                        "tools": { "type": "array" }
                    },
                    "required": ["enabled", "tools"]
                }))),
            |cx, _| {
                let started = std::time::Instant::now();
                let result = Ok(tools(cx));
                record(cx, "mcp.tools", started, true);
                result
            },
        );
        r.rpc(
            Method::new("mcp.setup", "mcp.method.setup.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::optional("profile_id", ParamKind::String).alias("profile"))
                .result(ResultSchema::new("mcp.method.setup.result")),
            |cx, params| {
                let started = std::time::Instant::now();
                let mut args = Args::new();
                if let Some(profile) = params.get("profile_id").and_then(Value::as_str) {
                    args.insert("profile_id", json!(profile));
                }
                let result = Ok(setup(cx, &args));
                record(cx, "mcp.setup", started, true);
                result
            },
        );

        for key in [
            "mcp.view.title",
            "mcp.view.server",
            "mcp.view.enabled",
            "mcp.view.disabled",
            "mcp.view.tools",
            "mcp.view.setup",
            "mcp.view.no-write",
            "mcp.command.authorize",
            "mcp.command.approval",
            "mcp.method.authorize.description",
            "mcp.method.authorize.result",
            "mcp.method.approval.description",
            "mcp.method.approval.result",
            "mcp.approval.title",
            "mcp.approval.client",
            "mcp.approval.tool",
            "mcp.approval.effect",
            "mcp.approval.pending",
            "mcp.approval.once",
            "mcp.approval.until-close",
            "mcp.approval.deny",
        ] {
            r.uses_i18n(key);
        }
        r.view(
            Slot::SettingsSection,
            "mcp.settings",
            "mcp.view.title",
            60,
            || Box::new(McpSettings),
        );
        let approval_state = Arc::clone(&self.state);
        r.view(
            Slot::Modal,
            "mcp.approval",
            "mcp.approval.title",
            40,
            move || Box::new(ApprovalView::new(Arc::clone(&approval_state))),
        );
    }
}

fn timeout_s(cx: &Ctx<'_>) -> i64 {
    cx.setting("mcp.server.timeout_s")
        .and_then(SettingValue::as_i64)
        .unwrap_or(120)
        .clamp(1, 600)
}

fn authorize(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, forja_kernel::KernelError> {
    let tool = args.required_str("tool")?.to_owned();
    let client = args.str("client").unwrap_or("MCP client").to_owned();
    let summary = args.str("summary").unwrap_or_default().to_owned();
    let effect = args.str("effect").unwrap_or("write").to_owned();
    if auto_approved(cx, &tool) {
        return Ok(json!({ "approved": true, "auto": true }));
    }
    let timeout = timeout_s(cx);
    let now = chrono::Utc::now().timestamp_millis();
    let outcome = {
        let mut state = shared
            .lock()
            .map_err(|_| forja_kernel::KernelError::Message("mcp state is poisoned".to_owned()))?;
        state.cleanup(timeout, now);
        state.authorize(&client, &tool, &summary, &effect, now)
    };
    match outcome {
        Authorize::Approved => Ok(json!({ "approved": true })),
        Authorize::QueueFull => Ok(json!({
            "approved": false,
            "code": "queue_full"
        })),
        Authorize::Pending(id) => {
            cx.open_modal_with("mcp.approval", json!({ "request_id": id }));
            Ok(json!({
                "approved": false,
                "pending": true,
                "request_id": id,
                "code": "pending"
            }))
        }
    }
}

fn approval(cx: &Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value, forja_kernel::KernelError> {
    let id = args.required_str("request_id")?.to_owned();
    let timeout = timeout_s(cx);
    let now = chrono::Utc::now().timestamp_millis();
    let status = shared
        .lock()
        .map_err(|_| forja_kernel::KernelError::Message("mcp state is poisoned".to_owned()))?
        .status(&id, timeout, now);
    Ok(json!({
        "status": status.as_str(),
        "approved": status == state::ApprovalStatus::Approved
    }))
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "mcp.server.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-mcp-server-enabled-label",
        "setting-mcp-server-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "mcp.server.expose",
        SettingKind::List {
            item: "ferramenta".to_owned(),
        },
        SettingValue::List(Vec::new()),
        "setting-mcp-server-expose-label",
        "setting-mcp-server-expose-help",
    ));
    r.setting(SettingSpec::new(
        "mcp.server.auto_approve",
        SettingKind::List {
            item: "ferramenta".to_owned(),
        },
        SettingValue::List(Vec::new()),
        "setting-mcp-server-auto_approve-label",
        "setting-mcp-server-auto_approve-help",
    ));
    r.setting(SettingSpec::new(
        "mcp.log.requests",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-mcp-log-requests-label",
        "setting-mcp-log-requests-help",
    ));
    r.setting(SettingSpec::new(
        "mcp.tools.auto_approve",
        SettingKind::List {
            item: "ferramenta".to_owned(),
        },
        SettingValue::List(Vec::new()),
        "setting-mcp-tools-auto_approve-label",
        "setting-mcp-tools-auto_approve-help",
    ));
    r.setting(SettingSpec::new(
        "mcp.server.timeout_s",
        SettingKind::Integer {
            min: Some(1),
            max: Some(600),
        },
        SettingValue::Integer(120),
        "setting-mcp-server-timeout_s-label",
        "setting-mcp-server-timeout_s-help",
    ));
}

/// Lista efetiva de ferramentas MCP.
pub fn tools(cx: &Ctx<'_>) -> Value {
    let enabled = cx
        .setting("mcp.server.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    if !enabled {
        return json!({ "enabled": false, "tools": [] });
    }
    let expose = list_setting(cx, "mcp.server.expose");
    let auto_approve: Vec<String> = ["mcp.server.auto_approve", "mcp.tools.auto_approve"]
        .iter()
        .flat_map(|key| list_setting(cx, key))
        .filter(|id| !never_exposed(id))
        .collect();
    let mut tools = Vec::new();
    for method in cx.registry().methods() {
        let schema = &method.schema;
        if never_exposed(&schema.name) {
            continue;
        }
        let effect = schema.effect;
        if effect == MethodEffect::Destructive {
            continue;
        }
        let exposed = if expose.iter().any(|id| id == &schema.name) {
            true
        } else {
            effect == MethodEffect::Read && default_exposed(&schema.name)
        };
        if !exposed {
            continue;
        }
        let name = format!("forja.{}", schema.name);
        if !valid_name(&name) {
            continue;
        }
        tools.push(json!({
            "id": schema.name,
            "name": name,
            "description": schema.description,
            "effect": effect.as_str(),
            "network": schema.network,
            "idempotent": schema.idempotent,
            "auto_approve": effect == MethodEffect::Write
                && auto_approve.iter().any(|id| id == &schema.name)
        }));
    }
    json!({ "enabled": true, "tools": tools })
}

/// Whether `tool` runs without asking, from either approval list. A method
/// the code never exposes is never approved, whatever the list says.
fn auto_approved(cx: &Ctx<'_>, tool: &str) -> bool {
    !never_exposed(tool)
        && ["mcp.server.auto_approve", "mcp.tools.auto_approve"]
            .iter()
            .any(|key| list_setting(cx, key).iter().any(|id| id == tool))
}

/// One line for the app log: which request, how it ended, how long it took.
/// Arguments never go in: they carry project data.
fn request_line(request: &str, ok: bool, elapsed: std::time::Duration) -> String {
    format!(
        "mcp {request} {} {} ms",
        if ok { "ok" } else { "error" },
        elapsed.as_millis()
    )
}

/// Logs a request the MCP server received, when `mcp.log.requests` is on.
fn record(cx: &Ctx<'_>, request: &str, started: std::time::Instant, ok: bool) {
    let enabled = cx
        .setting("mcp.log.requests")
        .and_then(SettingValue::as_bool)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    forja_kernel::logging::log(
        forja_kernel::logging::LogLevel::Info,
        request_line(request, ok, started.elapsed()),
    );
}

fn list_setting(cx: &Ctx<'_>, key: &str) -> Vec<String> {
    match cx.setting(key) {
        Some(SettingValue::List(values)) => values.clone(),
        _ => Vec::new(),
    }
}

/// Métodos cuja exposição é proibida por código, sem configuração.
pub fn never_exposed(id: &str) -> bool {
    id.starts_with("mcp.")
        || id.starts_with("trust.")
        || id.starts_with("approval.")
        || id.starts_with("permission.")
        || id.starts_with("extension.")
        || id.starts_with("helper.hooks.")
        || matches!(
            id,
            "settings.set"
                | "terminal.send"
                | "bench.remove"
                | "bench.archive"
                | "hook.event"
                | "file.open"
                | "editor.save"
        )
}

/// Leituras que não entram por padrão por carregarem dados sensíveis.
fn default_exposed(id: &str) -> bool {
    !matches!(
        id,
        "archive.search" | "archive.list" | "terminal.read" | "settings.get" | "helper.hook"
    )
}

fn valid_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

/// Snippet de configuração por ajudante. Nunca escreve no CLI.
pub fn setup(cx: &Ctx<'_>, args: &Args) -> Value {
    let _ = cx;
    let filter = args.str("profile_id");
    let clients = [
        (
            "claude-code",
            "Claude Code",
            "json",
            "https://code.claude.com/docs",
            "{\n  \"mcpServers\": {\n    \"forja\": { \"command\": \"forja-mcp\" }\n  }\n}",
        ),
        (
            "codex",
            "OpenAI Codex CLI",
            "toml",
            "https://developers.openai.com/codex",
            "[mcp_servers.forja]\ncommand = \"forja-mcp\"",
        ),
        (
            "opencode",
            "OpenCode",
            "json",
            "https://opencode.ai/docs",
            "{\n  \"mcp\": {\n    \"forja\": { \"type\": \"local\", \"command\": [\"forja-mcp\"] }\n  }\n}",
        ),
    ];
    let clients: Vec<Value> = clients
        .iter()
        .filter(|(id, ..)| filter.is_none_or(|filter| filter == *id))
        .map(|(id, name, format, source, snippet)| {
            json!({
                "profile_id": id,
                "name": name,
                "format": format,
                "source": source,
                "snippet": snippet,
                "writes": false
            })
        })
        .collect();
    json!({ "clients": clients, "writes": false })
}

struct McpSettings;

impl View for McpSettings {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let snapshot = tools(cx);
        let enabled = snapshot["enabled"].as_bool().unwrap_or(false);
        let count = snapshot["tools"].as_array().map(Vec::len).unwrap_or(0);
        ui.heading(cx.t("mcp.view.title"));
        ui.label(
            egui::RichText::new(if enabled {
                format!(
                    "{} · {}: {count}",
                    cx.t("mcp.view.server"),
                    cx.t("mcp.view.enabled")
                )
            } else {
                format!(
                    "{} · {}",
                    cx.t("mcp.view.server"),
                    cx.t("mcp.view.disabled")
                )
            })
            .weak(),
        );
        let setup = setup(cx, &Args::new());
        if let Some(clients) = setup["clients"].as_array() {
            ui.label(egui::RichText::new(cx.t("mcp.view.setup")).strong());
            for client in clients {
                if let Some(snippet) = client["snippet"].as_str() {
                    ui.label(
                        egui::RichText::new(client["name"].as_str().unwrap_or("MCP")).strong(),
                    );
                    ui.add(
                        egui::Label::new(egui::RichText::new(snippet).monospace())
                            .wrap_mode(egui::TextWrapMode::Extend),
                    );
                }
            }
            ui.label(egui::RichText::new(cx.t("mcp.view.no-write")).weak());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;

    // The kernel logger is a process-wide singleton: `Runtime::new` calls
    // `logging::configure` and moves its destination to that test's temp dir.
    // In a parallel run another test can redirect it between our `initialize`
    // and our log call, so the record lands in the wrong file. Hold one lock for
    // the whole lifetime of every test that owns a temp dir; the logger's
    // destination then cannot change while a test asserts on its own log.
    static LOG_DEST_LOCK: Mutex<()> = Mutex::new(());

    struct TempGuard(PathBuf, #[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl std::ops::Deref for TempGuard {
        type Target = PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_dir() -> TempGuard {
        let guard = LOG_DEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let dir = std::env::temp_dir().join(format!(
            "forja-mcp-module-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir, guard)
    }

    struct StubModule {
        id: &'static str,
        method: &'static str,
        effect: MethodEffect,
    }

    impl Module for StubModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new(self.id, "mcp.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.rpc(
                Method::new(self.method, "mcp.method.tools.description")
                    .effect(self.effect)
                    .result(ResultSchema::new("mcp.method.tools.result")),
                |_, _| Ok(json!({})),
            );
        }
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        runtime_with(dir, Arc::new(Mutex::new(McpState::default())))
    }

    fn runtime_with(dir: &std::path::Path, state: Shared) -> Runtime {
        Runtime::new(
            vec![
                Box::new(McpModule { state }),
                Box::new(StubModule {
                    id: "bench",
                    method: "bench.list",
                    effect: MethodEffect::Read,
                }),
                Box::new(StubModule {
                    id: "muster",
                    method: "muster.handoff",
                    effect: MethodEffect::Write,
                }),
                Box::new(StubModule {
                    id: "terminal",
                    method: "terminal.send",
                    effect: MethodEffect::Write,
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("mcp module registers")
    }

    #[test]
    fn contract_tools_are_read_only_safe_and_setup_never_writes() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        runtime
            .set_setting(
                "mcp.server.expose",
                SettingValue::List(vec!["muster.handoff".to_owned()]),
            )
            .unwrap();
        runtime
            .set_setting(
                "mcp.server.auto_approve",
                SettingValue::List(vec!["muster.handoff".to_owned()]),
            )
            .unwrap();
        let tools = runtime.call_method("mcp.tools", Map::new()).expect("tools");
        assert_eq!(tools["enabled"], true);
        let tools = tools["tools"].as_array().expect("array");
        assert!(!tools.is_empty());
        for tool in tools {
            let id = tool["id"].as_str().unwrap();
            assert!(!never_exposed(id), "{id}");
            assert_ne!(tool["effect"], "destructive");
            assert!(valid_name(tool["name"].as_str().unwrap()));
            if tool["effect"] == "read" {
                assert!(tool["auto_approve"] == false || tool["auto_approve"].is_null());
            }
        }
        assert!(tools.iter().any(|tool| tool["id"] == "bench.list"));
        assert!(tools.iter().any(|tool| tool["id"] == "muster.handoff"));
        assert!(tools.iter().all(|tool| tool["id"] != "terminal.send"));
        let setup = runtime.call_method("mcp.setup", Map::new()).expect("setup");
        assert_eq!(setup["writes"], false);
        let clients = setup["clients"].as_array().unwrap();
        assert_eq!(clients.len(), 3);
        assert!(clients
            .iter()
            .all(|client| client["snippet"].as_str().unwrap().contains("forja-mcp")));
        drop(runtime);
    }

    #[test]
    fn a_write_opens_the_modal_and_waits_for_the_decision() {
        let dir = temp_dir();
        let shared = Arc::new(Mutex::new(McpState::default()));
        let mut runtime = runtime_with(&dir, Arc::clone(&shared));
        runtime
            .set_setting(
                "mcp.server.expose",
                SettingValue::List(vec!["muster.handoff".to_owned()]),
            )
            .unwrap();
        let request = runtime
            .call_method(
                "mcp.authorize",
                Map::from_iter([
                    ("tool".to_owned(), json!("muster.handoff")),
                    ("client".to_owned(), json!("test")),
                    ("effect".to_owned(), json!("write")),
                ]),
            )
            .expect("authorize");
        assert_eq!(request["approved"], false);
        let id = request["request_id"].as_str().unwrap().to_owned();
        assert!(runtime.modal_is_open("mcp.approval"));
        let waiting = runtime
            .call_method(
                "mcp.approval",
                Map::from_iter([("request_id".to_owned(), json!(id.clone()))]),
            )
            .expect("approval");
        assert_eq!(waiting["status"], "pending");
        shared
            .lock()
            .unwrap()
            .decide(&id, true, false)
            .expect("decide");
        let decided = runtime
            .call_method(
                "mcp.approval",
                Map::from_iter([("request_id".to_owned(), json!(id))]),
            )
            .expect("approval");
        assert_eq!(decided["status"], "approved");
        assert_eq!(decided["approved"], true);
        drop(runtime);
    }

    #[test]
    fn auto_approved_writes_do_not_open_the_modal() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        runtime
            .set_setting(
                "mcp.server.auto_approve",
                SettingValue::List(vec!["muster.handoff".to_owned()]),
            )
            .unwrap();
        let request = runtime
            .call_method(
                "mcp.authorize",
                Map::from_iter([("tool".to_owned(), json!("muster.handoff"))]),
            )
            .expect("authorize");
        assert_eq!(request["approved"], true);
        assert!(!runtime.modal_is_open("mcp.approval"));
        drop(runtime);
    }

    #[test]
    fn the_tools_list_approves_only_what_it_names() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        runtime
            .set_setting(
                "mcp.server.expose",
                SettingValue::List(vec!["muster.handoff".to_owned()]),
            )
            .unwrap();
        // Empty by default: the write tool still asks every time.
        let before = runtime.call_method("mcp.tools", Map::new()).expect("tools");
        assert!(before["tools"]
            .as_array()
            .expect("array")
            .iter()
            .all(|tool| tool["auto_approve"] == false));
        runtime
            .set_setting(
                "mcp.tools.auto_approve",
                SettingValue::List(vec![
                    "muster.handoff".to_owned(),
                    "settings.set".to_owned(),
                ]),
            )
            .unwrap();
        let after = runtime.call_method("mcp.tools", Map::new()).expect("tools");
        let tools = after["tools"].as_array().expect("array");
        let handoff = tools
            .iter()
            .find(|tool| tool["id"] == "muster.handoff")
            .expect("handoff is exposed");
        assert_eq!(handoff["auto_approve"], true);
        // A method the code never exposes is not approved by being listed.
        assert!(tools.iter().all(|tool| tool["id"] != "settings.set"));
        let request = runtime
            .call_method(
                "mcp.authorize",
                Map::from_iter([("tool".to_owned(), json!("muster.handoff"))]),
            )
            .expect("authorize");
        assert_eq!(request["approved"], true);
        assert_eq!(request["auto"], true);
        drop(runtime);
    }

    #[test]
    fn request_log_lines_carry_no_arguments() {
        let line = request_line(
            "mcp.authorize",
            true,
            std::time::Duration::from_millis(7),
        );
        assert_eq!(line, "mcp mcp.authorize ok 7 ms");
        assert!(!request_line("mcp.tools", false, std::time::Duration::ZERO).contains("id"));
    }

    #[test]
    fn logging_records_the_request_when_the_setting_is_on() {
        let dir = temp_dir();
        forja_kernel::logging::initialize(dir.0.clone(), forja_kernel::logging::LogConfig::default());
        let mut runtime = runtime(&dir);
        runtime
            .set_setting("mcp.log.requests", SettingValue::Bool(true))
            .unwrap();
        // `LOG_DEST_LOCK` keeps another Runtime from moving the writer here.
        let log = forja_kernel::logging::current_log_path()
            .unwrap_or_else(|| forja_kernel::logging::logs_dir(dir.0.clone()).join("forja.log"));
        let before = std::fs::read_to_string(&log).unwrap_or_default().lines().count();
        runtime.call_method("mcp.tools", Map::new()).expect("tools");
        // Flush is a barrier in the logger channel: when it returns the record
        // is already on disk, so no polling or sleeping is needed.
        forja_kernel::logging::flush();
        let text = std::fs::read_to_string(&log)
            .unwrap_or_default()
            .lines()
            .skip(before)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("mcp mcp.tools ok"), "{text}");
        assert!(!text.contains("mcp.server.expose"), "{text}");
        drop(runtime);
    }

    #[test]
    fn disabled_server_has_no_tools() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        runtime
            .set_setting("mcp.server.enabled", SettingValue::Bool(false))
            .unwrap();
        let tools = runtime.call_method("mcp.tools", Map::new()).expect("tools");
        assert_eq!(tools["enabled"], false);
        assert_eq!(tools["tools"], json!([]));
        drop(runtime);
    }
}
