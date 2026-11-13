//! Ponte MCP sem estado entre um cliente stdio e o app da Forja.
//!
//! O cliente fala MCP em JSON-RPC por linha no stdin/stdout. Cada chamada é
//! traduzida para um método do catálogo do app e encaminhada pelo pipe local.
//! A ponte não guarda segredos, não lê configuração e não abre o app.

pub mod player;

use serde_json::{json, Map, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Versão mais recente do protocolo que a ponte anuncia.
pub const LATEST_PROTOCOL: &str = "2026-07-28";
/// Versão anterior aceita no handshake `initialize`.
pub const PREVIOUS_PROTOCOL: &str = "2025-11-25";

/// Falha ao falar com o app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendError {
    /// Código estável para o cliente (`app_closed`, `timed_out`, ...).
    pub code: String,
    /// Mensagem legível.
    pub message: String,
}

impl BackendError {
    /// Erro com código e mensagem.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Transporte até o app. A produção usa o pipe; os testes usam um app falso.
pub trait Backend {
    /// Chama um método do catálogo.
    fn call(&self, method: &str, params: Value) -> Result<Value, BackendError>;
}

/// Ponte MCP.
pub struct Bridge<B> {
    backend: B,
    client: Mutex<Option<String>>,
}

impl<B: Backend> Bridge<B> {
    /// Cria a ponte sobre um backend.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            client: Mutex::new(None),
        }
    }

    fn set_client(&self, params: &Value) {
        let name = params
            .get("clientInfo")
            .and_then(|info| info.get("name"))
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_owned);
        if let Ok(mut client) = self.client.lock() {
            *client = name;
        }
    }

    fn client_name(&self) -> String {
        self.client
            .lock()
            .ok()
            .and_then(|client| client.clone())
            .unwrap_or_else(|| "MCP client".to_owned())
    }

    /// Processa uma linha JSON-RPC. Devolve a linha de resposta, ou `None` para
    /// notificações.
    pub fn handle_line(&self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(error) => {
                return Some(
                    json!({
                        "jsonrpc": "2.0",
                        "id": Value::Null,
                        "error": { "code": -32700, "message": format!("Parse error: {error}") }
                    })
                    .to_string(),
                )
            }
        };
        self.handle_message(value).map(|reply| reply.to_string())
    }

    /// Processa um valor JSON-RPC.
    pub fn handle_message(&self, value: Value) -> Option<Value> {
        let id = value.get("id").cloned();
        let Some(method) = value.get("method").and_then(Value::as_str) else {
            return id.map(|id| rpc_error(id, -32600, "Invalid Request"));
        };
        // Notificação: não há resposta.
        id.as_ref()?;
        let id = id.unwrap_or(Value::Null);
        let params = value.get("params").cloned().unwrap_or_else(|| json!({}));
        match method {
            "initialize" => {
                self.set_client(&params);
                Some(ok(id, initialize(&params)))
            }
            "ping" => Some(ok(id, json!({}))),
            "tools/list" => match self.tools() {
                Ok(tools) => Some(ok(id, json!({ "tools": tools }))),
                Err(error) => Some(tool_error(id, error)),
            },
            "tools/call" => Some(self.tools_call(id, &params)),
            other if other.starts_with("notifications/") => None,
            _ => Some(rpc_error(
                id,
                -32601,
                &format!("Method not found: {method}"),
            )),
        }
    }

    fn tools(&self) -> Result<Vec<Value>, BackendError> {
        let catalog = self
            .backend
            .call("meta.methods", Value::Object(Map::new()))?;
        let exposure = self
            .backend
            .call("mcp.tools", Value::Object(Map::new()))
            .ok();
        if exposure
            .as_ref()
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            == Some(false)
        {
            return Ok(Vec::new());
        }
        let exposed: Vec<&str> = exposure
            .as_ref()
            .and_then(|value| value.get("tools"))
            .and_then(Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(|tool| tool.get("id").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        let use_app_exposure = exposure.is_some();
        let mut tools = Vec::new();
        let Some(methods) = catalog.get("methods").and_then(Value::as_array) else {
            return Ok(tools);
        };
        for method in methods {
            let Some(id) = method.get("name").and_then(Value::as_str) else {
                continue;
            };
            if id == "meta.methods" || is_never_exposed(id) {
                continue;
            }
            let effect = method
                .get("effect")
                .and_then(Value::as_str)
                .unwrap_or("read");
            let exposed_by_app = if use_app_exposure {
                exposed.contains(&id)
            } else {
                method
                    .get("exposed_mcp")
                    .and_then(Value::as_bool)
                    .unwrap_or(effect == "read")
                    && effect != "write"
                    && effect != "destructive"
            };
            if !exposed_by_app || effect == "destructive" {
                continue;
            }
            let name = tool_name(id);
            if !valid_tool_name(&name) {
                continue;
            }
            let auto_approve = exposure
                .as_ref()
                .and_then(|value| value.get("tools"))
                .and_then(Value::as_array)
                .and_then(|tools| {
                    tools
                        .iter()
                        .find(|tool| tool.get("id").and_then(Value::as_str) == Some(id))
                })
                .and_then(|tool| tool.get("auto_approve"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut tool = json!({
                "name": name,
                "auto_approve": auto_approve,
                "description": method
                    .get("description")
                    .and_then(Value::as_str)
                    .filter(|description| !description.is_empty())
                    .unwrap_or(id),
                "inputSchema": input_schema(method),
                "annotations": {
                    "readOnlyHint": effect == "read",
                    "destructiveHint": false,
                    "idempotentHint": method
                        .get("idempotent")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    "openWorldHint": method
                        .get("network")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                }
            });
            if let Some(schema) = method
                .get("result")
                .and_then(|result| result.get("schema"))
                .filter(|schema| !schema.is_null())
            {
                tool["outputSchema"] = schema.clone();
            }
            tools.push(tool);
        }
        Ok(tools)
    }

    /// Pede aprovação para uma escrita não aprovada automaticamente.
    fn authorize_call(
        &self,
        tool: &str,
        effect: &str,
        auto_approve: bool,
        arguments: &Value,
    ) -> Result<(), BackendError> {
        if effect != "write" || auto_approve {
            return Ok(());
        }
        let summary = match arguments.as_object() {
            Some(object) if !object.is_empty() => {
                let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
                keys.sort_unstable();
                format!("parameters: {}", keys.join(", "))
            }
            _ => "no parameters".to_owned(),
        };
        let request = self.backend.call(
            "mcp.authorize",
            json!({
                "client": self.client_name(),
                "tool": tool,
                "summary": summary,
                "effect": effect
            }),
        )?;
        if request.get("approved").and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        if let Some(code) = request.get("code").and_then(Value::as_str) {
            if code != "pending" {
                return Err(BackendError::new(
                    code,
                    request
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("approval was not granted"),
                ));
            }
        }
        let Some(request_id) = request.get("request_id").and_then(Value::as_str) else {
            return Err(BackendError::new("denied", "approval was not granted"));
        };
        let deadline = Instant::now() + Duration::from_secs(650);
        loop {
            if Instant::now() >= deadline {
                return Err(BackendError::new(
                    "timed_out",
                    "the approval request timed out",
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
            let reply = self
                .backend
                .call("mcp.approval", json!({ "request_id": request_id }))?;
            match reply
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("pending")
            {
                "approved" => return Ok(()),
                "denied" => return Err(BackendError::new("denied", "denied by the user")),
                "timed_out" => {
                    return Err(BackendError::new(
                        "timed_out",
                        "the approval request timed out",
                    ))
                }
                _ => {}
            }
        }
    }

    fn tools_call(&self, id: Value, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let tools = match self.tools() {
            Ok(tools) => tools,
            Err(error) => return tool_error(id, error),
        };
        let Some(tool) = tools
            .iter()
            .find(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
        else {
            return rpc_error(id, -32602, &format!("Unknown or not exposed tool: {name}"));
        };
        let method = tool
            .get("name")
            .and_then(Value::as_str)
            .and_then(|name| name.strip_prefix("forja."))
            .unwrap_or("");
        let effect = tool
            .get("annotations")
            .and_then(|annotations| annotations.get("readOnlyHint"))
            .and_then(Value::as_bool)
            .map_or(
                "write",
                |read_only| if read_only { "read" } else { "write" },
            );
        let auto_approve = tool
            .get("auto_approve")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Err(error) = self.authorize_call(method, effect, auto_approve, &arguments) {
            return tool_error(id, error);
        }
        match self.backend.call(method, arguments) {
            Ok(value) => {
                let structured = if value.is_object() {
                    value.clone()
                } else {
                    json!({ "result": value })
                };
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": value.to_string() }],
                        "structuredContent": structured,
                        "isError": false
                    }
                })
            }
            Err(error) => {
                let code = error.code.clone();
                let message = error.message.clone();
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": message }],
                        "structuredContent": { "code": code, "message": message },
                        "isError": true
                    }
                })
            }
        }
    }
}

fn initialize(params: &Value) -> Value {
    let requested = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(LATEST_PROTOCOL);
    let version = if matches!(requested, LATEST_PROTOCOL | PREVIOUS_PROTOCOL) {
        requested
    } else {
        LATEST_PROTOCOL
    };
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "forja-mcp",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn input_schema(method: &Value) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();
    if let Some(params) = method.get("params").and_then(Value::as_array) {
        for param in params {
            let Some(name) = param.get("name").and_then(Value::as_str) else {
                continue;
            };
            let kind = param
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("string");
            let mut schema = json!({ "type": json_kind(kind) });
            if let Some(description) = param
                .get("description")
                .and_then(Value::as_str)
                .filter(|description| !description.is_empty())
            {
                schema["description"] = json!(description);
            }
            properties.insert(name.to_owned(), schema);
            if param.get("required").and_then(Value::as_bool) == Some(true) {
                required.push(json!(name));
            }
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn json_kind(kind: &str) -> &'static str {
    match kind {
        "integer" | "number" => "number",
        "boolean" => "boolean",
        "json" => "object",
        _ => "string",
    }
}

/// Nome MCP de um método.
pub fn tool_name(id: &str) -> String {
    format!("forja.{id}")
}

/// Regra de caracteres e tamanho do protocolo.
pub fn valid_tool_name(name: &str) -> bool {
    (1..=128).contains(&name.len())
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

/// Métodos que nunca podem ser expostos, mesmo com configuração.
pub fn is_never_exposed(id: &str) -> bool {
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

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

fn tool_error(id: Value, error: BackendError) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": error.message }],
            "structuredContent": { "code": error.code, "message": error.message },
            "isError": true
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeApp;

    impl Backend for FakeApp {
        fn call(&self, method: &str, params: Value) -> Result<Value, BackendError> {
            match method {
                "meta.methods" => Ok(json!({ "methods": [
                    {
                        "name": "bench.list", "description": "Lists benches",
                        "effect": "read", "network": false, "idempotent": true,
                        "exposed_mcp": true, "params": []
                    },
                    {
                        "name": "muster.handoff", "description": "Delivers a task",
                        "effect": "write", "network": false, "idempotent": false,
                        "exposed_mcp": false,
                        "params": [{ "name": "summary", "kind": "string", "required": true }]
                    },
                    {
                        "name": "helper.launch", "description": "Launches a helper",
                        "effect": "write", "network": false, "idempotent": false,
                        "exposed_mcp": false, "params": []
                    },
                    {
                        "name": "terminal.send", "description": "Never exposed",
                        "effect": "write", "network": false, "idempotent": false,
                        "exposed_mcp": true, "params": []
                    }
                ]})),
                "mcp.tools" => Ok(json!({
                    "enabled": true,
                    "tools": [
                        { "id": "bench.list", "auto_approve": false },
                        { "id": "muster.handoff", "auto_approve": false }
                    ]
                })),
                "mcp.authorize" => Ok(json!({ "approved": true })),
                "bench.list" => Ok(json!({ "benches": [{ "id": "b1" }] })),
                "muster.handoff" => Ok(json!({ "result": params.get("summary") })),
                _ => Err(BackendError::new("app_error", "unknown method")),
            }
        }
    }

    fn call<B: Backend>(bridge: &Bridge<B>, value: Value) -> Value {
        bridge.handle_message(value).expect("response")
    }

    #[test]
    fn initialize_echoes_a_supported_version() {
        let bridge = Bridge::new(FakeApp);
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                    "params": { "protocolVersion": PREVIOUS_PROTOCOL } }),
        );
        assert_eq!(reply["result"]["protocolVersion"], PREVIOUS_PROTOCOL);
        assert_eq!(reply["result"]["serverInfo"]["name"], "forja-mcp");
    }

    #[test]
    fn tools_list_respects_exposure_and_never_list() {
        let bridge = Bridge::new(FakeApp);
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        );
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "forja.bench.list");
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
        assert_eq!(tools[1]["name"], "forja.muster.handoff");
        assert_eq!(tools[1]["auto_approve"], false);
        assert!(tools
            .iter()
            .all(|tool| tool["name"] != "forja.helper.launch"));
        assert!(tools
            .iter()
            .all(|tool| tool["name"] != "forja.terminal.send"));
        assert_eq!(tools[1]["inputSchema"]["required"], json!(["summary"]));
        let denied = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 31, "method": "tools/call",
                    "params": { "name": "forja.helper.launch", "arguments": {} } }),
        );
        assert_eq!(denied["error"]["code"], -32602);
    }

    #[test]
    fn tools_call_returns_structured_content_and_errors() {
        let bridge = Bridge::new(FakeApp);
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                    "params": { "name": "forja.bench.list", "arguments": {} } }),
        );
        assert_eq!(reply["result"]["isError"], false);
        assert_eq!(
            reply["result"]["structuredContent"]["benches"][0]["id"],
            "b1"
        );
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                    "params": { "name": "forja.muster.handoff",
                                "arguments": { "summary": "done" } } }),
        );
        assert_eq!(reply["result"]["structuredContent"]["result"], "done");
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
                    "params": { "name": "forja.nope", "arguments": {} } }),
        );
        assert_eq!(reply["error"]["code"], -32602);
    }

    struct DeniedApp {
        called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl Backend for DeniedApp {
        fn call(&self, method: &str, _: Value) -> Result<Value, BackendError> {
            match method {
                "meta.methods" => Ok(json!({ "methods": [{
                    "name": "muster.handoff", "description": "Delivers",
                    "effect": "write", "network": false, "idempotent": false,
                    "exposed_mcp": false,
                    "params": [{ "name": "summary", "kind": "string", "required": true }]
                }]})),
                "mcp.tools" => Ok(json!({ "enabled": true, "tools": [
                    { "id": "muster.handoff", "auto_approve": false }
                ]})),
                "mcp.authorize" => Ok(json!({ "approved": false, "code": "denied" })),
                "muster.handoff" => {
                    self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(json!({ "result": "called" }))
                }
                _ => Err(BackendError::new("app_error", "unknown")),
            }
        }
    }

    #[test]
    fn a_denied_write_is_not_called() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let bridge = Bridge::new(DeniedApp {
            called: std::sync::Arc::clone(&called),
        });
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call",
                    "params": { "name": "forja.muster.handoff",
                                "arguments": { "summary": "done" } } }),
        );
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(reply["result"]["structuredContent"]["code"], "denied");
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    }

    struct PendingApp {
        polls: std::sync::atomic::AtomicUsize,
        called: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl Backend for PendingApp {
        fn call(&self, method: &str, _: Value) -> Result<Value, BackendError> {
            match method {
                "meta.methods" => Ok(json!({ "methods": [{
                    "name": "muster.handoff", "description": "Delivers",
                    "effect": "write", "network": false, "idempotent": false,
                    "exposed_mcp": false,
                    "params": [{ "name": "summary", "kind": "string", "required": true }]
                }]})),
                "mcp.tools" => Ok(json!({ "enabled": true, "tools": [
                    { "id": "muster.handoff", "auto_approve": false }
                ]})),
                "mcp.authorize" => Ok(json!({
                    "approved": false, "pending": true,
                    "request_id": "r1", "code": "pending"
                })),
                "mcp.approval" => {
                    let previous = self.polls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(if previous == 0 {
                        json!({ "status": "pending" })
                    } else {
                        json!({ "status": "approved", "approved": true })
                    })
                }
                "muster.handoff" => {
                    self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(json!({ "result": "called" }))
                }
                _ => Err(BackendError::new("app_error", "unknown")),
            }
        }
    }

    #[test]
    fn a_pending_write_waits_and_runs_after_approval() {
        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let bridge = Bridge::new(PendingApp {
            polls: std::sync::atomic::AtomicUsize::new(0),
            called: std::sync::Arc::clone(&called),
        });
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                    "params": { "name": "forja.muster.handoff",
                                "arguments": { "summary": "done" } } }),
        );
        assert_eq!(reply["result"]["isError"], false);
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn a_timed_out_write_is_not_called() {
        struct TimedOutApp {
            called: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        impl Backend for TimedOutApp {
            fn call(&self, method: &str, _: Value) -> Result<Value, BackendError> {
                match method {
                    "meta.methods" => Ok(json!({ "methods": [{
                        "name": "muster.handoff", "description": "Delivers",
                        "effect": "write", "network": false, "idempotent": false,
                        "exposed_mcp": false,
                        "params": [{ "name": "summary", "kind": "string", "required": true }]
                    }]})),
                    "mcp.tools" => Ok(json!({ "enabled": true, "tools": [
                        { "id": "muster.handoff", "auto_approve": false }
                    ]})),
                    "mcp.authorize" => Ok(json!({
                        "approved": false,
                        "pending": true,
                        "request_id": "r1",
                        "code": "pending"
                    })),
                    "mcp.approval" => Ok(json!({ "status": "timed_out" })),
                    "muster.handoff" => {
                        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(json!({ "result": "called" }))
                    }
                    _ => Err(BackendError::new("app_error", "unknown")),
                }
            }
        }

        let called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let bridge = Bridge::new(TimedOutApp {
            called: std::sync::Arc::clone(&called),
        });
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 10, "method": "tools/call",
                    "params": { "name": "forja.muster.handoff",
                                "arguments": { "summary": "done" } } }),
        );
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(reply["result"]["structuredContent"]["code"], "timed_out");
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn notifications_have_no_response_and_unknown_methods_are_errors() {
        let bridge = Bridge::new(FakeApp);
        assert!(bridge
            .handle_message(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .is_none());
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 6, "method": "resources/list" }),
        );
        assert_eq!(reply["error"]["code"], -32601);
    }

    #[test]
    fn app_closed_is_a_tool_error() {
        struct Closed;
        impl Backend for Closed {
            fn call(&self, _: &str, _: Value) -> Result<Value, BackendError> {
                Err(BackendError::new(
                    "app_closed",
                    "Forja is not open. Run `forja open`.",
                ))
            }
        }
        let bridge = Bridge::new(Closed);
        let reply = call(
            &bridge,
            json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/list" }),
        );
        assert_eq!(reply["result"]["isError"], true);
        assert_eq!(reply["result"]["structuredContent"]["code"], "app_closed");
    }

    #[test]
    fn tool_names_follow_the_protocol_rule() {
        assert!(valid_tool_name(&tool_name("muster.handoff")));
        assert!(!valid_tool_name("forja.this name"));
        assert!(is_never_exposed("terminal.send"));
        assert!(is_never_exposed("mcp.setup"));
        assert!(!is_never_exposed("muster.handoff"));
    }
}
