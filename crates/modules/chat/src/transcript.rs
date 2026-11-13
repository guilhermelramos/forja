//! Leitura de transcrições e atualização incremental por linha.
//!
//! Cada CLI guarda o histórico em um formato próprio:
//! - Claude Code: JSONL com `type`/`message`;
//! - Codex: JSONL com `response_item`, `event_msg` e itens concluídos;
//! - OpenCode: JSON exportado com `info` e `messages[].parts`.
//!
//! O parser só usa o que está documentado nos perfis e nos fixtures; campos
//! desconhecidos são ignorados.

use crate::model::{now_ms, ChatBlock, ChatMessage, ChatRole, ChatUsage, TodoItem, ToolStatus};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Limite de texto guardado por bloco de ferramenta ou diff.
pub const MAX_TOOL_TEXT: usize = 40_000;

/// Formato de transcrição entendido pelo módulo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
    /// OpenCode.
    OpenCode,
}

impl Format {
    /// Deduz pelo id do perfil.
    pub fn for_profile(profile_id: &str) -> Self {
        match profile_id {
            "codex" => Self::Codex,
            "opencode" => Self::OpenCode,
            _ => Self::Claude,
        }
    }
}

/// Lê a transcrição inteira.
pub fn read_transcript(format: Format, path: &Path) -> Vec<ChatMessage> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    match format {
        Format::Codex => parse_codex(&text),
        Format::OpenCode => parse_opencode(&text),
        Format::Claude => parse_claude(&text),
    }
}

/// Leitor incremental. Linhas incompletas ficam para a próxima consulta.
#[derive(Debug, Default)]
pub struct TranscriptTail {
    offset: u64,
    pending: Vec<u8>,
}

impl TranscriptTail {
    /// Leitor do começo do arquivo.
    pub fn new() -> Self {
        Self::default()
    }

    /// Volta ao começo.
    pub fn reset(&mut self) {
        self.offset = 0;
        self.pending.clear();
    }

    /// Lê só o que foi acrescentado desde a última chamada.
    pub fn poll(&mut self, format: Format, path: &Path) -> Vec<ChatMessage> {
        let Ok(mut file) = std::fs::File::open(path) else {
            return Vec::new();
        };
        let Ok(metadata) = file.metadata() else {
            return Vec::new();
        };
        if metadata.len() < self.offset {
            self.reset();
        }
        if metadata.len() == self.offset {
            return Vec::new();
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return Vec::new();
        }
        let mut chunk = Vec::new();
        if file.read_to_end(&mut chunk).is_err() {
            return Vec::new();
        }
        self.offset = metadata.len();
        if matches!(format, Format::OpenCode) {
            // Export é um documento JSON único; relê inteiro quando muda.
            self.reset();
            return read_transcript(format, path);
        }
        self.pending.extend_from_slice(&chunk);
        let Some(last_newline) = self.pending.iter().rposition(|byte| *byte == b'\n') else {
            return Vec::new();
        };
        let complete: Vec<u8> = self.pending.drain(..=last_newline).collect();
        let text = String::from_utf8_lossy(&complete);
        let mut messages = match format {
            Format::Codex => parse_codex(&text),
            Format::OpenCode => parse_opencode(&text),
            Format::Claude => parse_claude(&text),
        };
        messages.retain(|message| !message.blocks.is_empty());
        messages
    }
}

/// Claude Code: `type`, `timestamp` e `message`.
pub fn parse_claude(text: &str) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut tool_names: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        let timestamp = parse_timestamp(value.get("timestamp"));
        match kind {
            "assistant" => {
                let Some(message) = value.get("message") else {
                    continue;
                };
                let mut blocks = Vec::new();
                if let Some(content) = message.get("content") {
                    claude_blocks(content, &mut blocks, &mut tool_names);
                }
                let usage = message.get("usage").map(claude_usage);
                if !blocks.is_empty() {
                    messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        blocks,
                        timestamp_ms: timestamp,
                        usage,
                    });
                }
            }
            "user" => {
                let Some(message) = value.get("message") else {
                    continue;
                };
                let Some(content) = message.get("content") else {
                    continue;
                };
                let mut blocks = Vec::new();
                let mut has_text = false;
                if let Some(items) = content.as_array() {
                    for item in items {
                        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
                        match item_type {
                            "tool_result" => {
                                let id = item
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                let name =
                                    tool_names.get(id).cloned().unwrap_or_else(|| "tool".into());
                                let output = item
                                    .get("content")
                                    .and_then(|content| text_from_content(content, "text"))
                                    .unwrap_or_default();
                                let status = if item
                                    .get("is_error")
                                    .and_then(Value::as_bool)
                                    .unwrap_or(false)
                                {
                                    ToolStatus::Error
                                } else {
                                    ToolStatus::Ok
                                };
                                blocks.push(ChatBlock::ToolCall {
                                    name,
                                    input: String::new(),
                                    output: Some(truncate(&output, MAX_TOOL_TEXT)),
                                    status,
                                });
                            }
                            "text" => {
                                if let Some(text) = item.get("text").and_then(Value::as_str) {
                                    if !text.trim().is_empty() {
                                        blocks.push(ChatBlock::Text(text.to_owned()));
                                        has_text = true;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else if let Some(text) = text_from_content(content, "text") {
                    blocks.push(ChatBlock::Text(text));
                    has_text = true;
                }
                if has_text || !blocks.is_empty() {
                    messages.push(ChatMessage {
                        role: if has_text {
                            ChatRole::User
                        } else {
                            ChatRole::Tool
                        },
                        blocks,
                        timestamp_ms: timestamp,
                        usage: None,
                    });
                }
            }
            _ => {}
        }
    }
    messages
}

fn claude_blocks(
    content: &Value,
    blocks: &mut Vec<ChatBlock>,
    tool_names: &mut HashMap<String, String>,
) {
    let Some(items) = content.as_array() else {
        if let Some(text) = text_from_content(content, "text") {
            blocks.push(ChatBlock::Text(text));
        }
        return;
    };
    for item in items {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        match item_type {
            "text" => {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        blocks.push(ChatBlock::Text(text.to_owned()));
                    }
                }
            }
            "thinking" => {
                if let Some(text) = item.get("thinking").and_then(Value::as_str) {
                    blocks.push(ChatBlock::Thinking(text.to_owned()));
                }
            }
            "tool_use" => {
                let name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_owned();
                let id = item.get("id").and_then(Value::as_str).unwrap_or("");
                if !id.is_empty() {
                    tool_names.insert(id.to_owned(), name.clone());
                }
                let input = item.get("input").cloned().unwrap_or(Value::Null);
                if name == "TodoWrite" {
                    if let Some(items) = input.get("todos").and_then(Value::as_array) {
                        blocks.push(ChatBlock::Todo {
                            items: items.iter().map(todo_item).collect(),
                        });
                    }
                } else if let Some(edit) = claude_edit(&name, &input) {
                    blocks.push(edit);
                } else {
                    blocks.push(ChatBlock::ToolCall {
                        name,
                        input: truncate(&pretty_json(&input), MAX_TOOL_TEXT),
                        output: None,
                        status: ToolStatus::Running,
                    });
                }
            }
            _ => {}
        }
    }
}

fn claude_edit(name: &str, input: &Value) -> Option<ChatBlock> {
    if !matches!(name, "Write" | "Edit" | "MultiEdit" | "NotebookEdit") {
        return None;
    }
    let path = input
        .get("file_path")
        .or_else(|| input.get("notebook_path"))
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)?;
    let mut diff = String::new();
    if let Some(content) = input.get("content").and_then(Value::as_str) {
        diff.push_str(&format!("--- {path}\n+++ {path}\n"));
        for line in content.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
    } else if let Some(edits) = input.get("edits").and_then(Value::as_array) {
        diff.push_str(&format!("--- {path}\n+++ {path}\n"));
        for edit in edits {
            if let Some(old) = edit.get("old_string").and_then(Value::as_str) {
                for line in old.lines() {
                    diff.push('-');
                    diff.push_str(line);
                    diff.push('\n');
                }
            }
            if let Some(new) = edit.get("new_string").and_then(Value::as_str) {
                for line in new.lines() {
                    diff.push('+');
                    diff.push_str(line);
                    diff.push('\n');
                }
            }
        }
    } else {
        let old = input
            .get("old_string")
            .and_then(Value::as_str)
            .unwrap_or("");
        let new = input
            .get("new_string")
            .and_then(Value::as_str)
            .unwrap_or("");
        diff.push_str(&format!("--- {path}\n+++ {path}\n"));
        for line in old.lines() {
            diff.push('-');
            diff.push_str(line);
            diff.push('\n');
        }
        for line in new.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
    }
    Some(ChatBlock::FileEdit {
        path: path.to_owned(),
        diff: truncate(&diff, MAX_TOOL_TEXT),
    })
}

/// Codex: `response_item`, `event_msg` e itens concluídos.
pub fn parse_codex(text: &str) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    let mut saw_event_item = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let timestamp = parse_timestamp(value.get("timestamp"));
        if let Some(item) = value
            .get("payload")
            .and_then(|payload| payload.get("item"))
            .filter(|_| {
                value.get("payload").and_then(|payload| payload.get("type"))
                    == Some(&Value::String("item_completed".into()))
            })
        {
            saw_event_item = true;
            if let Some(message) = codex_item(item, timestamp) {
                messages.push(message);
            }
            continue;
        }
        if let Some(payload) = value.get("payload") {
            if payload.get("type").and_then(Value::as_str) == Some("agent_message") {
                if let Some(text) = payload.get("message").and_then(Value::as_str) {
                    saw_event_item = true;
                    messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        blocks: vec![ChatBlock::Text(text.to_owned())],
                        timestamp_ms: timestamp,
                        usage: None,
                    });
                }
            }
            // Rollouts gravam o prompt da pessoa como `response_item`, mesmo
            // quando o resto vem em `item_completed`.
            if value.get("type").and_then(Value::as_str) == Some("response_item")
                && payload.get("type").and_then(Value::as_str) == Some("message")
                && payload.get("role").and_then(Value::as_str) == Some("user")
            {
                if let Some(text) = payload
                    .get("content")
                    .and_then(|content| text_from_content(content, "input_text"))
                {
                    messages.push(ChatMessage {
                        role: ChatRole::User,
                        blocks: vec![ChatBlock::Text(text)],
                        timestamp_ms: timestamp,
                        usage: None,
                    });
                }
            }
        }
    }
    if !saw_event_item {
        messages = codex_response_items(text);
    }
    messages
}

fn codex_response_items(text: &str) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if value.get("type").and_then(Value::as_str) != Some("response_item") {
            continue;
        }
        let Some(payload) = value.get("payload") else {
            continue;
        };
        let timestamp = parse_timestamp(value.get("timestamp"));
        match payload.get("type").and_then(Value::as_str).unwrap_or("") {
            "message" => {
                let role = match payload.get("role").and_then(Value::as_str).unwrap_or("") {
                    "user" => ChatRole::User,
                    "assistant" => ChatRole::Assistant,
                    _ => continue,
                };
                if let Some(text) = payload
                    .get("content")
                    .and_then(|content| text_from_content(content, "input_text"))
                {
                    messages.push(ChatMessage {
                        role,
                        blocks: vec![ChatBlock::Text(text)],
                        timestamp_ms: timestamp,
                        usage: None,
                    });
                }
            }
            "function_call" => {
                let name = payload
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                let input = payload
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                messages.push(ChatMessage {
                    role: ChatRole::Assistant,
                    blocks: vec![ChatBlock::ToolCall {
                        name: name.into(),
                        input: truncate(input, MAX_TOOL_TEXT),
                        output: None,
                        status: ToolStatus::Running,
                    }],
                    timestamp_ms: timestamp,
                    usage: None,
                });
            }
            _ => {}
        }
    }
    messages
}

/// Converte um item Codex em mensagem.
pub fn codex_item(item: &Value, timestamp: i64) -> Option<ChatMessage> {
    let kind = item.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "UserMessage" | "user_message" => {
            let text = item
                .get("content")
                .and_then(|content| text_from_content(content, "text"))
                .or_else(|| {
                    item.get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })?;
            Some(ChatMessage {
                role: ChatRole::User,
                blocks: vec![ChatBlock::Text(text)],
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        "AgentMessage" | "agent_message" => {
            let text = item
                .get("content")
                .and_then(|content| text_from_content(content, "text"))
                .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_owned))
                .or_else(|| {
                    item.get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })?;
            Some(ChatMessage {
                role: ChatRole::Assistant,
                blocks: vec![ChatBlock::Text(text)],
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        "Reasoning" | "reasoning" => {
            let text = item
                .get("summary")
                .and_then(|summary| text_from_content(summary, "text"))
                .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_owned))?;
            Some(ChatMessage {
                role: ChatRole::Assistant,
                blocks: vec![ChatBlock::Thinking(text)],
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        "CommandExecution" | "command_execution" => {
            let command = item
                .get("command")
                .map(|command| {
                    command
                        .as_array()
                        .map(|args| {
                            args.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .or_else(|| command.as_str().map(str::to_owned))
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            let output = item
                .get("aggregated_output")
                .and_then(Value::as_str)
                .unwrap_or("");
            let status = match item.get("exit_code").and_then(Value::as_i64) {
                Some(0) | None => ToolStatus::Ok,
                Some(_) => ToolStatus::Error,
            };
            Some(ChatMessage {
                role: ChatRole::Tool,
                blocks: vec![ChatBlock::ToolCall {
                    name: "shell".into(),
                    input: truncate(&command, MAX_TOOL_TEXT),
                    output: Some(truncate(output, MAX_TOOL_TEXT)),
                    status,
                }],
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        "FileChange" | "file_change" => {
            let changes = item.get("changes")?;
            let mut blocks = Vec::new();
            if let Some(changes) = changes.as_object() {
                for (path, change) in changes {
                    let diff = change
                        .get("unified_diff")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    blocks.push(ChatBlock::FileEdit {
                        path: path.clone(),
                        diff: truncate(&diff, MAX_TOOL_TEXT),
                    });
                }
            }
            (!blocks.is_empty()).then_some(ChatMessage {
                role: ChatRole::Tool,
                blocks,
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        "TodoList" | "todo_list" => {
            let items: Vec<TodoItem> = item
                .get("items")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(todo_item).collect::<Vec<_>>())
                .unwrap_or_default();
            (!items.is_empty()).then_some(ChatMessage {
                role: ChatRole::Assistant,
                blocks: vec![ChatBlock::Todo { items }],
                timestamp_ms: timestamp,
                usage: None,
            })
        }
        _ => None,
    }
}

/// OpenCode: um JSON exportado com `info` e `messages`.
///
/// Versões antigas gravavam um JSON por mensagem; o parser aceita tanto o
/// documento completo quanto um único objeto com `parts`.
pub fn parse_opencode(text: &str) -> Vec<ChatMessage> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut messages = Vec::new();
    if let Some(list) = value.get("messages").and_then(Value::as_array) {
        for message in list {
            if let Some(parsed) = opencode_message(message, value.get("info")) {
                messages.push(parsed);
            }
        }
        return messages;
    }
    if let Some(parsed) = opencode_message(&value, value.get("info")) {
        messages.push(parsed);
    }
    messages
}

fn opencode_message(value: &Value, root_info: Option<&Value>) -> Option<ChatMessage> {
    let info = value.get("info").unwrap_or(value);
    let role_text = info.get("role").and_then(Value::as_str).unwrap_or("");
    let role = match role_text {
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        "tool" => ChatRole::Tool,
        _ => ChatRole::Assistant,
    };
    let timestamp = info
        .get("time")
        .and_then(|time| {
            time.get("created")
                .or_else(|| time.get("updated"))
                .or_else(|| time.get("completed"))
                .and_then(|value| {
                    value
                        .as_i64()
                        .or_else(|| value.as_str().and_then(parse_timestamp_str))
                })
        })
        .or_else(|| {
            root_info.and_then(|info| {
                info.get("time")
                    .and_then(|time| time.get("created"))
                    .and_then(Value::as_i64)
            })
        })
        .unwrap_or_else(now_ms);
    let mut blocks = Vec::new();
    if let Some(parts) = value
        .get("parts")
        .or_else(|| value.get("content"))
        .and_then(Value::as_array)
    {
        for part in parts {
            opencode_part(part, &mut blocks);
        }
    } else if let Some(text) = info.get("text").and_then(Value::as_str) {
        blocks.push(ChatBlock::Text(text.to_owned()));
    }
    if blocks.is_empty() {
        return None;
    }
    Some(ChatMessage {
        role,
        blocks,
        timestamp_ms: timestamp,
        usage: None,
    })
}

fn opencode_part(part: &Value, blocks: &mut Vec<ChatBlock>) {
    let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "text" => {
            if let Some(text) = part.get("text").and_then(Value::as_str) {
                if !text.trim().is_empty() {
                    blocks.push(ChatBlock::Text(text.to_owned()));
                }
            }
        }
        "reasoning" | "thinking" => {
            if let Some(text) = part
                .get("text")
                .or_else(|| part.get("reasoning"))
                .and_then(Value::as_str)
            {
                blocks.push(ChatBlock::Thinking(text.to_owned()));
            }
        }
        "tool" | "tool_call" | "tool-invocation" => {
            let name = part
                .get("tool")
                .or_else(|| part.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("tool")
                .to_owned();
            let state = part.get("state").unwrap_or(part);
            let input = state
                .get("input")
                .or_else(|| part.get("input"))
                .map(pretty_json)
                .unwrap_or_default();
            let output = state
                .get("output")
                .or_else(|| state.get("metadata"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            let status = match state.get("status").and_then(Value::as_str).unwrap_or("") {
                "completed" | "success" => ToolStatus::Ok,
                "error" | "failed" => ToolStatus::Error,
                _ => ToolStatus::Running,
            };
            blocks.push(ChatBlock::ToolCall {
                name,
                input: truncate(&input, MAX_TOOL_TEXT),
                output: output.map(|text| truncate(&text, MAX_TOOL_TEXT)),
                status,
            });
        }
        "file" | "file_edit" => {
            if let Some(path) = part.get("path").and_then(Value::as_str) {
                let diff = part
                    .get("diff")
                    .or_else(|| part.get("patch"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                blocks.push(ChatBlock::FileEdit {
                    path: path.to_owned(),
                    diff: truncate(diff, MAX_TOOL_TEXT),
                });
            }
        }
        "todo" | "todos" => {
            let items = part
                .get("items")
                .or_else(|| part.get("todos"))
                .and_then(Value::as_array)
                .map(|items| items.iter().map(todo_item).collect::<Vec<_>>())
                .unwrap_or_default();
            if !items.is_empty() {
                blocks.push(ChatBlock::Todo { items });
            }
        }
        _ => {}
    }
}

fn todo_item(value: &Value) -> TodoItem {
    TodoItem {
        text: value
            .get("content")
            .or_else(|| value.get("text"))
            .or_else(|| value.get("title"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned(),
        status: value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
            .to_owned(),
    }
}

fn claude_usage(usage: &Value) -> ChatUsage {
    let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0)
        + usage
            .get("cache_read_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0)
        + usage
            .get("cache_creation_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0);
    let output = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ChatUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: input + output,
    }
}

/// Uso a partir de qualquer objeto com os nomes comuns.
pub fn usage_from_value(value: &Value) -> ChatUsage {
    let input_tokens = value
        .get("input_tokens")
        .or_else(|| value.get("prompt_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = value
        .get("output_tokens")
        .or_else(|| value.get("completion_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ChatUsage {
        input_tokens,
        output_tokens,
        total_tokens: value
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(input_tokens + output_tokens),
    }
}

/// Timestamp em milissegundos.
pub fn parse_timestamp(value: Option<&Value>) -> i64 {
    value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(parse_timestamp_str))
        })
        .unwrap_or(0)
}

fn parse_timestamp_str(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.timestamp_millis())
}

/// Texto de um conteúdo em string, array de blocos ou objeto.
pub fn text_from_content(value: &Value, preferred: &str) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
                        .or_else(|| {
                            (item.get("type").and_then(Value::as_str) == Some(preferred))
                                .then(|| item.get("text").and_then(Value::as_str))
                                .flatten()
                        })
                        .or_else(|| item.get("content").and_then(Value::as_str))
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!text.is_empty()).then_some(text)
        }
        Value::Object(_) => value.get("text").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Corta em um limite seguro de UTF-8.
pub fn truncate(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_owned();
    }
    let mut end = limit;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}.", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_fixture_has_text_tools_edits_todos_and_usage() {
        let messages = parse_claude(include_str!("../fixtures/claude-session.jsonl"));
        assert!(messages.iter().any(|message| message.role == ChatRole::User));
        assert!(messages
            .iter()
            .any(|message| message.role == ChatRole::Assistant));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::ToolCall { name, .. } if name == "Bash")
        )));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::ToolCall { name, output: Some(output), .. } if name == "Bash" && output.contains("ok"))
        )));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::FileEdit { path, diff } if path.ends_with("lib.rs") && diff.contains("+new"))
        )));
        assert!(messages
            .iter()
            .any(|message| message.blocks.iter().any(|block| matches!(block, ChatBlock::Todo { items } if items.len() == 2))));
        assert!(messages.iter().any(|message| message.usage.is_some()));
        assert!(messages
            .iter()
            .any(|message| message.blocks.iter().any(|block| matches!(block, ChatBlock::Thinking(_)))));
    }

    #[test]
    fn codex_fixture_has_items_and_legacy_messages() {
        let messages = parse_codex(include_str!("../fixtures/codex-session.jsonl"));
        assert!(messages.iter().any(|message| message.role == ChatRole::User));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::ToolCall { name, output: Some(output), .. } if name == "shell" && output.contains("ok"))
        )));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::FileEdit { path, .. } if path.ends_with("main.rs"))
        )));
        assert!(messages
            .iter()
            .any(|message| message.blocks.iter().any(|block| matches!(block, ChatBlock::Todo { .. }))));
        assert!(messages
            .iter()
            .any(|message| message.blocks.iter().any(|block| matches!(block, ChatBlock::Thinking(_)))));
    }

    #[test]
    fn opencode_fixture_has_parts() {
        let messages = parse_opencode(include_str!("../fixtures/opencode-session.json"));
        assert!(messages.iter().any(|message| message.role == ChatRole::User));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::ToolCall { name, output: Some(output), .. } if name == "bash" && output.contains("ok"))
        )));
        assert!(messages.iter().any(|message| message.blocks.iter().any(
            |block| matches!(block, ChatBlock::FileEdit { path, .. } if path.ends_with("view.rs"))
        )));
        assert!(messages
            .iter()
            .any(|message| message.blocks.iter().any(|block| matches!(block, ChatBlock::Thinking(_)))));
    }

    #[test]
    fn tail_reads_only_complete_new_lines() {
        let dir = std::env::temp_dir().join(format!(
            "forja-chat-tail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl");
        let first = r#"{"type":"user","message":{"content":"one"},"timestamp":"2026-11-12T00:00:00Z"}"#;
        std::fs::write(&path, format!("{first}\n")).unwrap();
        let mut tail = TranscriptTail::new();
        assert_eq!(tail.poll(Format::Claude, &path).len(), 1);
        assert!(tail.poll(Format::Claude, &path).is_empty());
        let second = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"two"}]}}"#;
        std::fs::write(&path, format!("{first}\n{second}")).unwrap();
        assert!(tail.poll(Format::Claude, &path).is_empty());
        std::fs::write(&path, format!("{first}\n{second}\n")).unwrap();
        let added = tail.poll(Format::Claude, &path);
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].summary(), "two");
        let _ = std::fs::remove_dir_all(dir);
    }
}
