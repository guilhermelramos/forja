//! `forja-mcp --player`: MCP tools for a play-test agent.
//!
//! The agent uses Forja like a person: it reads what is on screen, clicks,
//! types and presses keys. The tools call the `ui.*` methods that only exist
//! when the app runs with `forja-app --test-driver`. Nothing else in the app
//! catalog is exposed. `report_finding` appends a bug or observation to the
//! JSONL file given with `--findings <path>`.

use crate::{Backend, BackendError};
use serde_json::{json, Value};
use std::io::Write as _;
use std::path::PathBuf;

/// Lines of the widget list sent back to the model per call.
const MAX_VIEW_LINES: usize = 250;

/// MCP server for play tests.
pub struct Player<B> {
    backend: B,
    findings: Option<PathBuf>,
}

impl<B: Backend> Player<B> {
    pub fn new(backend: B, findings: Option<PathBuf>) -> Self {
        Self { backend, findings }
    }

    /// Handles one JSON-RPC line; `None` for notifications.
    pub fn handle_line(&self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                return Some(
                    json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "parse error" } })
                        .to_string(),
                )
            }
        };
        let id = message.get("id").cloned()?;
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match message.get("method").and_then(Value::as_str).unwrap_or("") {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(crate::LATEST_PROTOCOL),
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "forja-player", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Ferramentas para usar a Forja como uma pessoa: ui_look, ui_click, ui_type, ui_key, ui_scroll, ui_wait e report_finding."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(self.call_tool(&params)),
            other => Err(json!({ "code": -32601, "message": format!("method not found: {other}") })),
        };
        Some(
            match result {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
            }
            .to_string(),
        )
    }

    fn call_tool(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        if name == "report_finding" {
            return self.report(&arguments);
        }
        let method = match name {
            "ui_look" => "ui.snapshot",
            "ui_click" => "ui.click",
            "ui_type" => "ui.type",
            "ui_key" => "ui.key",
            "ui_scroll" => "ui.scroll",
            "ui_wait" => "ui.wait",
            _ => return error_content(&format!("unknown tool {name}")),
        };
        match self.backend.call(method, arguments) {
            Ok(result) => success_content(name, &result),
            Err(BackendError { code, message }) => error_content(&format!("{code}: {message}")),
        }
    }

    fn report(&self, arguments: &Value) -> Value {
        let title = arguments.get("title").and_then(Value::as_str).unwrap_or("").trim();
        if title.is_empty() {
            return error_content("title is required");
        }
        let Some(path) = &self.findings else {
            return error_content("this server was started without --findings");
        };
        let line = json!({
            "title": title,
            "kind": arguments.get("kind").and_then(Value::as_str).unwrap_or("bug"),
            "severity": arguments.get("severity").and_then(Value::as_str).unwrap_or("medium"),
            "steps": arguments.get("steps").cloned().unwrap_or(Value::Null),
            "expected": arguments.get("expected").cloned().unwrap_or(Value::Null),
            "actual": arguments.get("actual").cloned().unwrap_or(Value::Null),
            "screen": arguments.get("screen").cloned().unwrap_or(Value::Null),
        });
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| writeln!(file, "{line}"));
        match written {
            Ok(()) => json!({ "content": [{ "type": "text", "text": "registrado" }], "isError": false }),
            Err(error) => error_content(&error.to_string()),
        }
    }
}

fn success_content(tool: &str, result: &Value) -> Value {
    let mut text = String::new();
    if let Some(clicked) = result.pointer("/action/clicked") {
        text.push_str(&format!(
            "Ação: clique em #{} {} \"{}\"\n",
            clicked["id"],
            clicked["role"].as_str().unwrap_or(""),
            clicked["name"].as_str().unwrap_or("")
        ));
    }
    let list = |key: &str| -> String {
        result["changes"][key]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|item| item.replace('\n', " / "))
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default()
    };
    if tool != "ui_look" {
        text.push_str(&format!("Apareceu: {}\nSumiu: {}\n", list("appeared"), list("disappeared")));
    }
    text.push_str(&format!(
        "Janela {}x{} px. Elementos (#id papel \"nome\" (x0,y0)-(x1,y1) [estado]):\n",
        result["window"]["width"], result["window"]["height"]
    ));
    let view = result["view"].as_str().unwrap_or("");
    let total = view.lines().count();
    for line in view.lines().take(MAX_VIEW_LINES) {
        text.push_str(line);
        text.push('\n');
    }
    if total > MAX_VIEW_LINES {
        text.push_str(&format!(
            "... mais {} elementos abaixo (role a tela com ui_scroll para vê-los).\n",
            total - MAX_VIEW_LINES
        ));
    }
    let mut content = vec![json!({ "type": "text", "text": text })];
    if let Some(png) = result["png_base64"].as_str() {
        content.push(json!({ "type": "image", "data": png, "mimeType": "image/png" }));
    }
    json!({ "content": content, "isError": false })
}

fn error_content(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn target_schema() -> Value {
    json!({
        "target": { "type": "string", "description": "Nome visível do elemento, como aparece em ui_look (ex.: \"Nova bancada\")." },
        "index": { "type": "integer", "description": "Qual ocorrência, quando há vários com o mesmo nome (0 = primeira)." },
        "id": { "type": "integer", "description": "#id do elemento em ui_look." },
        "x": { "type": "number", "description": "Posição x em pixels da janela (alternativa a target/id)." },
        "y": { "type": "number", "description": "Posição y em pixels da janela." }
    })
}

fn tools() -> Vec<Value> {
    let mut click = target_schema();
    click["button"] = json!({ "type": "string", "enum": ["left", "right", "middle"] });
    click["double"] = json!({ "type": "boolean", "description": "Clique duplo." });
    let mut scroll = target_schema();
    scroll["dy"] = json!({ "type": "number", "description": "Pixels; negativo desce a página, positivo sobe. Padrão -300." });
    vec![
        json!({
            "name": "ui_look",
            "description": "Mostra o que está na tela agora: cada elemento com #id, papel, nome, posição e estado. Use image=true para receber também a captura.",
            "inputSchema": { "type": "object", "properties": { "image": { "type": "boolean" } } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "ui_click",
            "description": "Clica num elemento como uma pessoa (move o mouse, aperta e solta). Indique target (nome), id, ou x e y. Responde o que apareceu e sumiu e a tela nova.",
            "inputSchema": { "type": "object", "properties": click }
        }),
        json!({
            "name": "ui_type",
            "description": "Digita texto no campo que está com foco (clique no campo antes). \\n vira Enter.",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] }
        }),
        json!({
            "name": "ui_key",
            "description": "Aperta uma tecla ou atalho, ex.: \"Enter\", \"Escape\", \"Tab\", \"Ctrl+K\", \"Ctrl+Shift+P\", \"ArrowDown\".",
            "inputSchema": { "type": "object", "properties": { "keys": { "type": "string" } }, "required": ["keys"] }
        }),
        json!({
            "name": "ui_scroll",
            "description": "Rola a roda do mouse sobre um elemento ou ponto.",
            "inputSchema": { "type": "object", "properties": scroll }
        }),
        json!({
            "name": "ui_wait",
            "description": "Espera (ms, até 20000) para tarefas em segundo plano terminarem e mostra a tela.",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer" } } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "report_finding",
            "description": "Registra um bug, falha de usabilidade ou observação, com passos para reproduzir.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": ["bug", "usability", "accessibility", "text", "crash", "observation"] },
                    "severity": { "type": "string", "enum": ["low", "medium", "high", "critical"] },
                    "steps": { "type": "array", "items": { "type": "string" } },
                    "expected": { "type": "string" },
                    "actual": { "type": "string" },
                    "screen": { "type": "string", "description": "Em que tela aconteceu." }
                },
                "required": ["title", "steps", "expected", "actual"]
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Backend for Fake {
        fn call(&self, method: &str, params: Value) -> Result<Value, BackendError> {
            match method {
                "ui.click" => Ok(json!({
                    "window": { "width": 100, "height": 50 },
                    "changes": { "appeared": ["Bigorna"], "disappeared": [] },
                    "view": "#1 Button \"Ok\" (0,0)-(10,10) [clicável]\n",
                    "action": { "clicked": { "id": 1, "role": "Button", "name": params["target"] } }
                })),
                _ => Err(BackendError::new("app_error", "no")),
            }
        }
    }

    fn call(player: &Player<Fake>, body: Value) -> Value {
        serde_json::from_str(&player.handle_line(&body.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn lists_only_player_tools() {
        let player = Player::new(Fake, None);
        let reply = call(&player, json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }));
        let names: Vec<&str> = reply["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["ui_look", "ui_click", "ui_type", "ui_key", "ui_scroll", "ui_wait", "report_finding"]);
    }

    #[test]
    fn click_reports_changes_and_errors_are_tool_errors() {
        let player = Player::new(Fake, None);
        let reply = call(&player, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "ui_click", "arguments": { "target": "Ok" } } }));
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Apareceu: Bigorna"));
        assert!(text.contains("#1 Button"));
        let reply = call(&player, json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "ui_key", "arguments": { "keys": "Enter" } } }));
        assert_eq!(reply["result"]["isError"], true);
    }

    #[test]
    fn findings_are_appended_as_jsonl() {
        let path = std::env::temp_dir().join(format!("forja-player-findings-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let player = Player::new(Fake, Some(path.clone()));
        let reply = call(&player, json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "report_finding", "arguments": {
                "title": "Aba cortada", "steps": ["abrir"], "expected": "texto inteiro", "actual": "cortado" } } }));
        assert_eq!(reply["result"]["isError"], false);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("Aba cortada"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn notifications_get_no_reply() {
        let player = Player::new(Fake, None);
        assert!(player
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }
}
