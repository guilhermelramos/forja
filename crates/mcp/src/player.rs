//! `forja-mcp --player`: MCP tools for a play-test agent.
//!
//! The agent uses Forja like a person: it reads what is on screen, clicks,
//! types and presses keys. The tools call the `ui.*` methods that only exist
//! when the app runs with `forja-app --test-driver`. Nothing else in the app
//! catalog is exposed. `report_finding` appends a bug or observation to the
//! JSONL file given with `--findings <path>`.
//!
//! Replies are small on purpose. An action sends the action, what appeared or
//! disappeared and only the widgets that are new or changed; it never repeats
//! the whole screen, which the agent already has from its last `ui_look`.
//! `ui_look` shows the screen compact, optionally limited by `region` or
//! `filter`. `--max-steps` caps the actions of one session.

use crate::{Backend, BackendError};
use serde_json::{json, Value};
use std::cell::Cell;
use std::io::Write as _;
use std::path::PathBuf;

/// Lines of a full screen sent back to the model per `ui_look`.
const MAX_LOOK_LINES: usize = 110;
/// New or changed widgets printed after an action.
const MAX_CHANGED_LINES: usize = 24;
/// A widget name longer than this is cut in the text the model reads.
const MAX_NAME_CHARS: usize = 60;

/// MCP server for play tests.
pub struct Player<B> {
    backend: B,
    findings: Option<PathBuf>,
    steps: Cell<usize>,
    max_steps: Option<usize>,
}

impl<B: Backend> Player<B> {
    pub fn new(backend: B, findings: Option<PathBuf>) -> Self {
        Self {
            backend,
            findings,
            steps: Cell::new(0),
            max_steps: None,
        }
    }

    /// Caps the actions (`ui_click`, `ui_type`, `ui_key`, `ui_scroll` and
    /// `ui_wait`) of one session. Looking and reporting have no cap.
    pub fn with_max_steps(mut self, max_steps: Option<usize>) -> Self {
        self.max_steps = max_steps.filter(|max| *max > 0);
        self
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
                "instructions": "Ferramentas para usar a Forja como uma pessoa: ui_look, ui_click, ui_type, ui_key, ui_scroll, ui_wait e report_finding. As ações respondem só o que mudou; ui_look mostra a tela."
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
        if is_action(name) {
            if let Some(max) = self.max_steps {
                if self.steps.get() >= max {
                    return error_content(&format!(
                        "Limite de {max} ações desta sessão atingido. Pare de usar o app e escreva \
                         agora o resumo estruturado: telas visitadas, o que foi testado, achados e \
                         o que tentaria em seguida."
                    ));
                }
                self.steps.set(self.steps.get() + 1);
            }
        }
        match self.backend.call(method, arguments.clone()) {
            Ok(result) => {
                let mut text = response_text(name, &arguments, &result);
                if is_action(name) {
                    if let Some(max) = self.max_steps {
                        let left = max.saturating_sub(self.steps.get());
                        if left <= 3 {
                            text.push_str(&format!("Ações restantes: {left}.\n"));
                        }
                    }
                }
                success_content(text, &result)
            }
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
        let expectations = match arguments.get("expectations") {
            None | Some(Value::Null) => Value::Null,
            Some(value) => match normalize_expectations(value) {
                Ok(value) => value,
                Err(message) => return error_content(&message),
            },
        };
        let line = json!({
            "title": title,
            "kind": arguments.get("kind").and_then(Value::as_str).unwrap_or("bug"),
            "severity": arguments.get("severity").and_then(Value::as_str).unwrap_or("medium"),
            "steps": arguments.get("steps").cloned().unwrap_or(Value::Null),
            "expectations": expectations,
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

fn is_action(tool: &str) -> bool {
    matches!(tool, "ui_click" | "ui_type" | "ui_key" | "ui_scroll" | "ui_wait")
}

/// Validates the verifier's `expectations` list before it reaches the JSONL
/// file: every entry needs a non-empty item, a boolean `ok` and an observation.
fn normalize_expectations(value: &Value) -> Result<Value, String> {
    let Some(items) = value.as_array() else {
        return Err("expectations must be an array of {item, ok, observed}".to_owned());
    };
    let mut normalized = Vec::with_capacity(items.len());
    for (index, entry) in items.iter().enumerate() {
        let item = entry
            .get("item")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if item.is_empty() {
            return Err(format!("expectations[{index}].item is required"));
        }
        let Some(ok) = entry.get("ok").and_then(Value::as_bool) else {
            return Err(format!("expectations[{index}].ok must be true or false"));
        };
        let observed = entry
            .get("observed")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if observed.is_empty() {
            return Err(format!("expectations[{index}].observed is required"));
        }
        normalized.push(json!({ "item": item, "ok": ok, "observed": observed }));
    }
    Ok(Value::Array(normalized))
}

fn success_content(text: String, result: &Value) -> Value {
    let mut content = vec![json!({ "type": "text", "text": text })];
    if let Some(png) = result["png_base64"].as_str() {
        content.push(json!({ "type": "image", "data": png, "mimeType": "image/png" }));
    }
    json!({ "content": content, "isError": false })
}

fn error_content(message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": message }], "isError": true })
}

fn response_text(tool: &str, arguments: &Value, result: &Value) -> String {
    if tool == "ui_look" {
        look_text(result)
    } else {
        action_text(tool, arguments, result)
    }
}

/// The whole screen, compact: grouped repeats, short names, `@x,y` only for
/// widgets that can be acted on.
fn look_text(result: &Value) -> String {
    let elements = result["elements"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut text = String::new();
    if let Some(region) = result.get("region").filter(|region| !region.is_null()) {
        let rect = region["rect"].as_array();
        let at = |index: usize| rect.and_then(|rect| rect.get(index)).and_then(Value::as_f64).unwrap_or(0.0);
        text.push_str(&format!(
            "Região \"{}\" ({:.0}x{:.0} @{:.0},{:.0}): {} elementos.\n",
            region["label"].as_str().unwrap_or("?"),
            (at(2) - at(0)).max(0.0),
            (at(3) - at(1)).max(0.0),
            at(0),
            at(1),
            elements.len()
        ));
    } else {
        text.push_str(&format!(
            "Tela {:.0}x{:.0}: {} elementos.\n",
            result["window"]["width"].as_f64().unwrap_or(0.0),
            result["window"]["height"].as_f64().unwrap_or(0.0),
            elements.len()
        ));
    }
    if let Some(filter) = result.get("filter").and_then(Value::as_str) {
        text.push_str(&format!("Filtro: \"{filter}\".\n"));
    }
    text.push_str(&format!("@x,y = acionável; nomes até {MAX_NAME_CHARS} caracteres.\n"));
    if elements.is_empty() {
        text.push_str("Nenhum elemento; tente outro region ou filter.\n");
        return text;
    }
    let mut lines: Vec<(String, usize)> = Vec::new();
    let mut index = 0;
    while index < elements.len() {
        let mut count = 1;
        if !is_editable(&elements[index]) {
            while index + count < elements.len() && same_widget(&elements[index], &elements[index + count]) {
                count += 1;
            }
        }
        lines.push((widget_line(&elements[index], count, false), count));
        index += count;
    }
    let mut hidden = 0;
    for (line_index, (line, count)) in lines.iter().enumerate() {
        if line_index >= MAX_LOOK_LINES {
            hidden += count;
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    if hidden > 0 {
        text.push_str(&format!(
            "... mais {hidden} elementos (use ui_look com region ou filter).\n"
        ));
    }
    text
}

/// An action and its diff. No full screen here.
fn action_text(tool: &str, arguments: &Value, result: &Value) -> String {
    let mut text = String::new();
    match tool {
        "ui_click" => {
            if let Some(clicked) = result.pointer("/action/clicked") {
                text.push_str(&format!(
                    "Clique: #{} {} {}\n",
                    clicked["id"],
                    clicked["role"].as_str().unwrap_or("?"),
                    display_name(clicked)
                ));
            } else if let Some(at) = result.pointer("/action/at").and_then(Value::as_array) {
                let at = |index: usize| at.get(index).and_then(Value::as_f64).unwrap_or(0.0);
                text.push_str(&format!("Clique em ({:.0}, {:.0}).\n", at(0), at(1)));
            } else {
                text.push_str("Clique feito.\n");
            }
        }
        "ui_scroll" => {
            let dy = arguments["dy"].as_f64().unwrap_or(-300.0);
            if let Some(clicked) = result.pointer("/action/clicked") {
                text.push_str(&format!(
                    "Rolagem sobre #{} {} {} (dy {dy:.0}).\n",
                    clicked["id"],
                    clicked["role"].as_str().unwrap_or("?"),
                    display_name(clicked)
                ));
            } else {
                text.push_str(&format!("Rolagem (dy {dy:.0}).\n"));
            }
        }
        "ui_type" => {
            let typed = arguments["text"].as_str().unwrap_or("");
            text.push_str(&format!(
                "Digitou: \"{}\" ({} caracteres).\n",
                clip(typed, 80),
                typed.chars().count()
            ));
        }
        "ui_key" => {
            text.push_str(&format!(
                "Tecla: {}.\n",
                arguments["keys"].as_str().unwrap_or("?")
            ));
        }
        "ui_wait" => {
            text.push_str(&format!(
                "Esperou {} ms.\n",
                arguments["ms"].as_u64().unwrap_or(500)
            ));
        }
        _ => {}
    }
    let appeared = change_names(result, "appeared", "appeared_more");
    let disappeared = change_names(result, "disappeared", "disappeared_more");
    if let Some(line) = &appeared {
        text.push_str(&format!("Apareceu: {line}\n"));
    }
    if let Some(line) = &disappeared {
        text.push_str(&format!("Sumiu: {line}\n"));
    }
    if appeared.is_none() && disappeared.is_none() {
        text.push_str("Apareceu/Sumiu: nada.\n");
    }
    let new = element_list(result, "new");
    let changed = element_list(result, "changed");
    let new_more = result["changes"]["new_more"].as_u64().unwrap_or(0);
    let changed_more = result["changes"]["changed_more"].as_u64().unwrap_or(0);
    if !new.is_empty() {
        text.push_str(&format!("Novos ({}):\n", new.len() as u64 + new_more));
        for element in new.iter().take(MAX_CHANGED_LINES) {
            text.push_str("  ");
            text.push_str(&widget_line(element, 1, true));
            text.push('\n');
        }
        if new_more > 0 {
            text.push_str(&format!("  ... +{new_more} (use ui_look para ver)\n"));
        }
    }
    if !changed.is_empty() {
        text.push_str(&format!("Alterados ({}):\n", changed.len() as u64 + changed_more));
        for element in changed.iter().take(MAX_CHANGED_LINES) {
            text.push_str("  ");
            text.push_str(&widget_line(element, 1, true));
            text.push('\n');
        }
        if changed_more > 0 {
            text.push_str(&format!("  ... +{changed_more} (use ui_look para ver)\n"));
        }
    }
    if new.is_empty() && changed.is_empty() {
        text.push_str("Nenhum elemento novo ou alterado.\n");
    }
    // After typing, say where the text went when the diff does not show it.
    if tool == "ui_type" {
        if let Some(focus) = result.get("focus").filter(|focus| !focus.is_null()) {
            let in_diff = new
                .iter()
                .chain(changed.iter())
                .any(|element| element["id"] == focus["id"]);
            if !in_diff {
                text.push_str(&format!(
                    "Foco: #{} {} {}\n",
                    focus["id"],
                    focus["role"].as_str().unwrap_or("?"),
                    display_name(focus)
                ));
            }
        }
    }
    text
}

fn element_list<'a>(result: &'a Value, key: &str) -> &'a [Value] {
    result["changes"][key].as_array().map(Vec::as_slice).unwrap_or(&[])
}

fn change_names(result: &Value, key: &str, more_key: &str) -> Option<String> {
    let items = result["changes"][key].as_array()?;
    if items.is_empty() {
        return None;
    }
    let names: Vec<String> = items
        .iter()
        .filter_map(Value::as_str)
        .map(|name| clip(name, 40))
        .collect();
    let more = result["changes"][more_key].as_u64().unwrap_or(0);
    Some(if more > 0 {
        format!("{} (+{more})", names.join(" | "))
    } else {
        names.join(" | ")
    })
}

/// One compact line: `#id [×N] Role "name" = "value" @x,y [tags]`.
/// `always_position` is used by diffs, where the task asks for the position
/// of every new or changed widget; a full look omits it when nothing can be
/// done with the widget.
fn widget_line(element: &Value, count: usize, always_position: bool) -> String {
    let mut line = format!("#{}", element["id"]);
    if count > 1 {
        line.push_str(&format!(" ×{count}"));
    }
    line.push(' ');
    line.push_str(element["role"].as_str().unwrap_or("?"));
    line.push(' ');
    line.push_str(&display_name(element));
    if let Some(value) = element["value"].as_str().filter(|value| !value.is_empty()) {
        line.push_str(&format!(" = \"{}\"", clip(value, 40)));
    }
    if always_position || element["clickable"].as_bool() == Some(true) || is_editable(element) {
        line.push_str(&format!(" @{}", position(element)));
    }
    let tags = state_tags(element);
    if !tags.is_empty() {
        line.push_str(&format!(" [{}]", tags.join(" ")));
    }
    line
}

fn display_name(element: &Value) -> String {
    let name = element["name"].as_str().unwrap_or("").trim();
    if name.is_empty() {
        "(sem nome)".to_owned()
    } else if name.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) {
        "(ícone)".to_owned()
    } else {
        format!("\"{}\"", clip(name, MAX_NAME_CHARS))
    }
}

fn position(element: &Value) -> String {
    let rect = element["rect"].as_array();
    let value = |index: usize| {
        rect.and_then(|rect| rect.get(index))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    format!("{:.0},{:.0}", value(0), value(1))
}

fn state_tags(element: &Value) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if has_state(element, "focused") {
        tags.push("foco");
    }
    if has_state(element, "selected") {
        tags.push("sel");
    }
    if has_state(element, "disabled") {
        tags.push("desab");
    }
    if is_editable(element) {
        tags.push("edita");
    }
    tags
}

fn has_state(element: &Value, state: &str) -> bool {
    element["state"]
        .as_array()
        .is_some_and(|states| states.iter().any(|value| value.as_str() == Some(state)))
}

fn is_editable(element: &Value) -> bool {
    element["editable"].as_bool() == Some(true)
}

/// Widgets that would look the same on one line, even at different positions.
fn same_widget(left: &Value, right: &Value) -> bool {
    ["role", "name", "value", "state", "clickable", "editable"]
        .iter()
        .all(|key| left[*key] == right[*key])
}

fn clip(text: &str, max: usize) -> String {
    let text = text.replace('\n', " / ");
    if text.chars().count() <= max {
        text
    } else {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
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
            "description": "Mostra a tela de forma compacta (#id, papel, nome, posição, estado). Use só ao chegar numa tela nova. region limita a uma parte (cima, baixo, esquerda, direita, centro ou nome de uma janela) e filter mostra só nomes/papéis com o texto. image=true traz a captura.",
            "inputSchema": { "type": "object", "properties": {
                "image": { "type": "boolean" },
                "region": { "type": "string", "description": "Parte da tela: cima, baixo, esquerda, direita, centro ou o nome de uma janela aberta." },
                "filter": { "type": "string", "description": "Mostra só elementos cujo nome ou papel contém este texto." }
            } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "ui_click",
            "description": "Clica num elemento como uma pessoa (move o mouse, aperta e solta). Indique target (nome), id, ou x e y. Responde só o que apareceu/sumiu e os elementos novos ou alterados.",
            "inputSchema": { "type": "object", "properties": click }
        }),
        json!({
            "name": "ui_type",
            "description": "Digita texto no campo que está com foco (clique no campo antes). \\n vira Enter. Responde o campo alterado, não a tela.",
            "inputSchema": { "type": "object", "properties": { "text": { "type": "string" } }, "required": ["text"] }
        }),
        json!({
            "name": "ui_key",
            "description": "Aperta uma tecla ou atalho, ex.: \"Enter\", \"Escape\", \"Tab\", \"Ctrl+K\", \"Ctrl+Shift+P\", \"ArrowDown\". Responde só o que mudou.",
            "inputSchema": { "type": "object", "properties": { "keys": { "type": "string" } }, "required": ["keys"] }
        }),
        json!({
            "name": "ui_scroll",
            "description": "Rola a roda do mouse sobre um elemento ou ponto e responde só o que mudou.",
            "inputSchema": { "type": "object", "properties": scroll }
        }),
        json!({
            "name": "ui_wait",
            "description": "Espera (ms, até 20000) para tarefas em segundo plano terminarem e mostra só o que mudou.",
            "inputSchema": { "type": "object", "properties": { "ms": { "type": "integer" } } },
            "annotations": { "readOnlyHint": true }
        }),
        json!({
            "name": "report_finding",
            "description": "Registra um bug, falha de usabilidade ou observação, com passos para reproduzir. Em um veredito, inclua expectations com cada comportamento esperado e ok=true|false.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "kind": { "type": "string", "enum": ["bug", "usability", "accessibility", "text", "crash", "observation"] },
                    "severity": { "type": "string", "enum": ["low", "medium", "high", "critical"] },
                    "steps": { "type": "array", "items": { "type": "string" } },
                    "expectations": {
                        "type": "array",
                        "description": "Comportamentos esperados do veredito; fixed só é aceito se todos tiverem ok=true.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "item": { "type": "string" },
                                "ok": { "type": "boolean" },
                                "observed": { "type": "string" }
                            },
                            "required": ["item", "ok", "observed"]
                        }
                    },
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
    use std::sync::Mutex;

    struct Fake;

    impl Backend for Fake {
        fn call(&self, method: &str, params: Value) -> Result<Value, BackendError> {
            match method {
                "ui.click" => Ok(json!({
                    "window": { "width": 100, "height": 50 },
                    "changes": {
                        "appeared": ["Bigorna"], "disappeared": [],
                        "appeared_more": 0, "disappeared_more": 0,
                        "new": [{
                            "id": 7, "role": "MenuItem", "name": "Renomear",
                            "rect": [300, 120, 360, 140], "clickable": true, "editable": false, "state": []
                        }, {
                            "id": 8, "role": "Window", "name": "Dialogo",
                            "rect": [100, 100, 400, 300], "clickable": false, "editable": false, "state": []
                        }],
                        "changed": [],
                        "new_more": 0, "changed_more": 0
                    },
                    "elements": [
                        { "id": 1, "role": "Button", "name": "Ok", "rect": [0, 0, 10, 10],
                          "clickable": true, "editable": false, "state": [] },
                        { "id": 9, "role": "Label", "name": "Botão distante", "rect": [0, 40, 50, 50],
                          "clickable": false, "editable": false, "state": [] }
                    ],
                    "action": { "clicked": { "id": 1, "role": "Button", "name": params["target"],
                        "rect": [0, 0, 10, 10], "clickable": true, "editable": false, "state": [] } }
                })),
                "ui.snapshot" => Ok(screen(&params)),
                "ui.wait" => Ok(json!({
                    "window": { "width": 100, "height": 50 },
                    "changes": { "appeared": [], "disappeared": [], "appeared_more": 0,
                        "disappeared_more": 0, "new": [], "changed": [], "new_more": 0, "changed_more": 0 },
                    "elements": []
                })),
                _ => Err(BackendError::new("app_error", "no")),
            }
        }
    }

    fn screen(params: &Value) -> Value {
        let mut result = json!({
            "window": { "width": 1280, "height": 800 },
            "changes": { "appeared": [], "disappeared": [], "appeared_more": 0,
                "disappeared_more": 0, "new": [], "changed": [], "new_more": 0, "changed_more": 0 },
            "elements": [
                { "id": 1, "role": "Button", "name": "Ok", "rect": [0, 0, 10, 10],
                  "clickable": true, "editable": false, "state": [] },
                { "id": 2, "role": "TextInput", "name": "Busca", "value": "abc",
                  "rect": [200, 20, 300, 44], "clickable": true, "editable": true, "state": ["focused"] },
                { "id": 3, "role": "Image", "name": "\u{E000}", "rect": [0, 60, 16, 76],
                  "clickable": true, "editable": false, "state": [] },
                { "id": 4, "role": "Image", "name": "\u{E000}", "rect": [20, 60, 36, 76],
                  "clickable": true, "editable": false, "state": [] },
                { "id": 5, "role": "Image", "name": "\u{E000}", "rect": [40, 60, 56, 76],
                  "clickable": true, "editable": false, "state": [] },
                { "id": 6, "role": "Label", "name": "x".repeat(200),
                  "rect": [0, 80, 100, 90], "clickable": false, "editable": false, "state": [] }
            ]
        });
        if let Some(region) = params.get("region") {
            result["region"] = json!({ "label": region, "rect": [0, 0, 640, 400] });
        }
        if let Some(filter) = params.get("filter") {
            result["filter"] = filter.clone();
        }
        result
    }

    fn call<B: Backend>(player: &Player<B>, body: Value) -> Value {
        serde_json::from_str(&player.handle_line(&body.to_string()).unwrap()).unwrap()
    }

    fn call_tool<B: Backend>(player: &Player<B>, name: &str, arguments: Value) -> Value {
        call(
            player,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                    "params": { "name": name, "arguments": arguments } }),
        )
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
        let look = &reply["result"]["tools"][0]["inputSchema"]["properties"];
        assert!(look.get("region").is_some() && look.get("filter").is_some());
        let report = &reply["result"]["tools"][6]["inputSchema"]["properties"];
        let expectation = &report["expectations"]["items"];
        assert_eq!(expectation["required"], json!(["item", "ok", "observed"]));
    }

    #[test]
    fn click_reports_the_action_and_only_the_change() {
        let player = Player::new(Fake, None);
        let reply = call_tool(&player, "ui_click", json!({ "target": "Ok" }));
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Clique: #1 Button \"Ok\""));
        assert!(text.contains("Apareceu: Bigorna"));
        assert!(text.contains("Novos (2):"));
        assert!(text.contains("#7 MenuItem \"Renomear\" @300,120"));
        // A new non-clickable widget still reports where it is.
        assert!(text.contains("#8 Window \"Dialogo\" @100,100"));
        // The full screen from the last look is neither repeated nor truncated into the reply.
        assert!(!text.contains("Elementos ("));
        assert!(!text.contains("Botão distante"));
        assert!(text.len() < 600, "action reply was {} chars", text.len());
    }

    #[test]
    fn look_is_compact_and_groups_repeated_widgets() {
        let player = Player::new(Fake, None);
        let reply = call_tool(&player, "ui_look", json!({}));
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Tela 1280x800: 6 elementos."));
        assert!(text.contains("#3 ×3 Image (ícone) @0,60"));
        assert!(text.contains("#2 TextInput \"Busca\" = \"abc\" @200,20 [foco edita]"));
        assert!(text.contains("nomes até 60 caracteres"));
        assert!(!text.contains("(0,0)-(10,10)"));
        // A 200-char name is cut.
        assert!(!text.contains(&"x".repeat(200)));
        assert!(text.contains('…'));
    }

    #[test]
    fn look_sends_region_and_filter_to_the_app() {
        struct Capture {
            calls: Mutex<Vec<(String, Value)>>,
        }
        impl Backend for Capture {
            fn call(&self, method: &str, params: Value) -> Result<Value, BackendError> {
                self.calls.lock().unwrap().push((method.to_owned(), params.clone()));
                Ok(screen(&params))
            }
        }
        let player = Player::new(
            Capture {
                calls: Mutex::new(Vec::new()),
            },
            None,
        );
        let reply = call_tool(
            &player,
            "ui_look",
            json!({ "region": "janela de cima", "filter": "Busca" }),
        );
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Região \"janela de cima\""));
        assert!(text.contains("Filtro: \"Busca\""));
        let calls = player.backend.calls.lock().unwrap();
        assert_eq!(calls[0].0, "ui.snapshot");
        assert_eq!(calls[0].1["region"], "janela de cima");
        assert_eq!(calls[0].1["filter"], "Busca");
    }

    #[test]
    fn errors_are_tool_errors() {
        let player = Player::new(Fake, None);
        let reply = call_tool(&player, "ui_key", json!({ "keys": "Enter" }));
        assert_eq!(reply["result"]["isError"], true);
    }

    #[test]
    fn step_budget_stops_actions_and_keeps_looks_and_reports() {
        let path = std::env::temp_dir().join(format!("forja-player-budget-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let player = Player::new(Fake, Some(path.clone())).with_max_steps(Some(1));
        let first = call_tool(&player, "ui_click", json!({ "target": "Ok" }));
        assert_eq!(first["result"]["isError"], false);
        let second = call_tool(&player, "ui_click", json!({ "target": "Ok" }));
        assert_eq!(second["result"]["isError"], true);
        assert!(second["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Limite de 1 ações"));
        // Looking and reporting still work after the budget is spent.
        assert_eq!(call_tool(&player, "ui_look", json!({}))["result"]["isError"], false);
        assert_eq!(
            call_tool(
                &player,
                "report_finding",
                json!({ "title": "t", "steps": [], "expected": "a", "actual": "b" })
            )["result"]["isError"],
            false
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn findings_are_appended_as_jsonl() {
        let path = std::env::temp_dir().join(format!("forja-player-findings-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let player = Player::new(Fake, Some(path.clone()));
        let reply = call_tool(
            &player,
            "report_finding",
            json!({ "title": "Aba cortada", "steps": ["abrir"], "expected": "texto inteiro", "actual": "cortado" }),
        );
        assert_eq!(reply["result"]["isError"], false);
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("Aba cortada"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn report_writes_structured_expectations() {
        let path = std::env::temp_dir().join(format!("forja-player-expectations-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let player = Player::new(Fake, Some(path.clone()));
        let reply = call_tool(
            &player,
            "report_finding",
            json!({
                "title": "VEREDITO: fixed",
                "kind": "observation",
                "severity": "low",
                "steps": ["Abri o Quadro e cliquei no cartão"],
                "expectations": [
                    { "item": "O cartão abre a bancada", "ok": true, "observed": "A bancada abriu" },
                    { "item": "A janela de Ajustes não abre", "ok": true, "observed": "Não abriu nenhuma janela de Ajustes" }
                ],
                "expected": "O cartão abre a bancada",
                "actual": "A bancada abriu e nenhuma janela de Ajustes apareceu",
                "screen": "Quadro"
            }),
        );
        assert_eq!(reply["result"]["isError"], false);
        let saved = std::fs::read_to_string(&path).unwrap();
        let finding: Value = serde_json::from_str(saved.lines().next().unwrap()).unwrap();
        assert_eq!(finding["expectations"][0]["item"], "O cartão abre a bancada");
        assert_eq!(finding["expectations"][0]["ok"].as_bool(), Some(true));
        assert_eq!(
            finding["expectations"][1]["observed"],
            "Não abriu nenhuma janela de Ajustes"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn report_rejects_malformed_expectations() {
        let path = std::env::temp_dir().join(format!("forja-player-bad-expectations-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let player = Player::new(Fake, Some(path.clone()));
        let reply = call_tool(
            &player,
            "report_finding",
            json!({
                "title": "VEREDITO: fixed",
                "steps": ["testei"],
                "expected": "funciona",
                "actual": "funcionou",
                "expectations": [{ "item": "funciona", "ok": "sim", "observed": "funcionou" }]
            }),
        );
        assert_eq!(reply["result"]["isError"], true);
        let text = reply["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("expectations[0].ok"));
        assert!(!path.exists());
    }

    #[test]
    fn notifications_get_no_reply() {
        let player = Player::new(Fake, None);
        assert!(player
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
    }
}
