//! Parser do stream JSON dos turnos headless.
//!
//! Claude usa `-p --output-format stream-json` e Codex usa `exec --json`.
//! Cada linha é um objeto independente; o parser devolve os eventos que a
//! sessão de chat consome.

use crate::model::{ChatMessage, ChatRole, ChatUsage};
use crate::transcript::{codex_item, parse_claude, parse_timestamp, usage_from_value};
use serde_json::Value;

/// Evento de um turno em execução.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamEvent {
    /// Id da sessão informado pelo CLI.
    SessionId(String),
    /// Mensagem pronta para o modelo.
    Message(ChatMessage),
    /// Uso de tokens do turno.
    Usage(ChatUsage),
    /// Erro informado pelo CLI.
    Error(String),
    /// Fim do processo; o código pode vir do CLI ou do `wait`.
    Done(Option<i32>),
}

/// Interpreta uma linha do stream.
pub fn parse_stream_line(profile_id: &str, line: &str) -> Vec<StreamEvent> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    if profile_id == "codex" {
        parse_codex_event(&value)
    } else {
        parse_claude_event(&value)
    }
}

fn parse_claude_event(value: &Value) -> Vec<StreamEvent> {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    let mut events = Vec::new();
    if kind == "system"
        && value.get("subtype").and_then(Value::as_str) == Some("init")
    {
        if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
            events.push(StreamEvent::SessionId(session_id.to_owned()));
        }
    }
    match kind {
        "assistant" | "user" => {
            let messages = parse_claude(&value.to_string());
            events.extend(messages.into_iter().map(StreamEvent::Message));
            if let Some(usage) = value
                .get("message")
                .and_then(|message| message.get("usage"))
                .map(usage_from_value)
            {
                events.push(StreamEvent::Usage(usage));
            }
        }
        "result" => {
            if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
                events.push(StreamEvent::SessionId(session_id.to_owned()));
            }
            if let Some(usage) = value.get("usage") {
                events.push(StreamEvent::Usage(usage_from_value(usage)));
            }
            if let Some(result) = value.get("result").and_then(Value::as_str) {
                if !result.trim().is_empty() {
                    events.push(StreamEvent::Message(ChatMessage {
                        role: ChatRole::Assistant,
                        blocks: vec![crate::model::ChatBlock::Text(result.to_owned())],
                        timestamp_ms: parse_timestamp(value.get("timestamp")),
                        usage: None,
                    }));
                }
            }
        }
        "error" => {
            if let Some(message) = value
                .get("error")
                .and_then(|error| {
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .or_else(|| error.as_str())
                })
                .or_else(|| value.get("message").and_then(Value::as_str))
            {
                events.push(StreamEvent::Error(message.to_owned()));
            }
        }
        _ => {}
    }
    events
}

fn parse_codex_event(value: &Value) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    for key in ["thread_id", "session_id", "conversation_id"] {
        if let Some(id) = value.get(key).and_then(Value::as_str) {
            if !id.is_empty() {
                events.push(StreamEvent::SessionId(id.to_owned()));
            }
        }
    }
    let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
    if matches!(
        kind,
        "turn.completed" | "task_complete" | "response.completed" | "session.end"
    ) {
        if let Some(usage) = value.get("usage") {
            events.push(StreamEvent::Usage(usage_from_value(usage)));
        }
        events.push(StreamEvent::Done(None));
        return events;
    }
    if let Some(item) = value
        .get("item")
        .or_else(|| value.get("payload").and_then(|payload| payload.get("item")))
    {
        let timestamp = parse_timestamp(value.get("timestamp"));
        if let Some(message) = codex_item(item, timestamp) {
            events.push(StreamEvent::Message(message));
        }
        return events;
    }
    if let Some(payload) = value.get("payload") {
        if let Some(message) = codex_item(payload, parse_timestamp(value.get("timestamp"))) {
            events.push(StreamEvent::Message(message));
        }
    }
    if let Some(msg) = value.get("msg").and_then(Value::as_object) {
        let msg_type = msg.get("type").and_then(Value::as_str).unwrap_or("");
        if msg_type.contains("agent_message") {
            if let Some(text) = msg.get("message").and_then(Value::as_str) {
                events.push(StreamEvent::Message(ChatMessage {
                    role: ChatRole::Assistant,
                    blocks: vec![crate::model::ChatBlock::Text(text.to_owned())],
                    timestamp_ms: 0,
                    usage: None,
                }));
            }
        }
    }
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        if kind.contains("agent") || kind == "message" {
            events.push(StreamEvent::Message(ChatMessage {
                role: ChatRole::Assistant,
                blocks: vec![crate::model::ChatBlock::Text(message.to_owned())],
                timestamp_ms: 0,
                usage: None,
            }));
        }
    }
    if let Some(usage) = value.get("usage") {
        events.push(StreamEvent::Usage(usage_from_value(usage)));
    }
    if kind == "error" || kind == "turn.failed" {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message").and_then(Value::as_str))
        {
            events.push(StreamEvent::Error(message.to_owned()));
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<StreamEvent> {
        text.lines()
            .flat_map(|line| parse_stream_line("claude-code", line))
            .collect()
    }

    fn codex_lines(text: &str) -> Vec<StreamEvent> {
        text.lines()
            .flat_map(|line| parse_stream_line("codex", line))
            .collect()
    }

    #[test]
    fn claude_stream_fixture_has_session_text_and_usage() {
        let events = lines(include_str!("../fixtures/claude-stream.jsonl"));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::SessionId(id) if id == "abc")));
        assert!(events.iter().any(
            |event| matches!(event, StreamEvent::Message(message) if message.summary() == "Vou conferir.")
        ));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::Usage(usage) if usage.total_tokens == 6)));
    }

    #[test]
    fn codex_stream_fixture_has_thread_item_and_done() {
        let events = codex_lines(include_str!("../fixtures/codex-stream.jsonl"));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::SessionId(id) if id == "t1")));
        assert!(events.iter().any(
            |event| matches!(event, StreamEvent::Message(message) if message.summary() == "working")
        ));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::Done(_))));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::Usage(usage) if usage.total_tokens == 5)));
    }
}
