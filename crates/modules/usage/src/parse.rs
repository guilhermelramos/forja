//! Transcript parsing for Claude, Codex and OpenCode.
//!
//! The parsers take text so fixtures stay in tests. The only file access is in
//! `read_lines`, called by the scanner.

use crate::model::{UsageRecord, UsageTotals};
use serde_json::Value;
use std::path::Path;

const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

pub fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(text
        .lines()
        .filter(|line| line.len() <= MAX_LINE_BYTES && !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

pub fn parse_claude(lines: &[String], session_hint: &str, project_hint: &str) -> Vec<UsageRecord> {
    let mut records = Vec::new();
    let mut session_id = session_hint.to_owned();
    let mut project = project_hint.to_owned();
    let mut title = String::new();
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(id) = value.get("sessionId").and_then(Value::as_str) {
            session_id = id.to_owned();
        }
        if let Some(cwd) = value.get("cwd").and_then(Value::as_str) {
            project = cwd.to_owned();
        }
        if title.is_empty() && value.get("type").and_then(Value::as_str) == Some("user") {
            title = value
                .pointer("/message/content")
                .and_then(text_content)
                .map(|text| summarize_title(&text))
                .unwrap_or_default();
        }
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(usage) = value.pointer("/message/usage") else {
            continue;
        };
        let mut totals = UsageTotals {
            input_tokens: u64_field(usage, "input_tokens"),
            output_tokens: u64_field(usage, "output_tokens"),
            cache_read_tokens: u64_field(usage, "cache_read_input_tokens"),
            cache_write_tokens: u64_field(usage, "cache_creation_input_tokens"),
            total_tokens: 0,
        };
        if totals.is_empty() {
            continue;
        }
        totals.total_tokens = totals.input_tokens + totals.output_tokens;
        records.push(UsageRecord {
            agent: "claude".into(),
            session_id: session_id.clone(),
            project: project.clone(),
            model: value
                .pointer("/message/model")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned(),
            title: title.clone(),
            timestamp_ms: parse_timestamp(value.get("timestamp")),
            totals,
        });
    }
    records
}

pub fn parse_codex(lines: &[String], session_hint: &str, project_hint: &str) -> Vec<UsageRecord> {
    let mut records = Vec::new();
    let mut session_id = session_hint.to_owned();
    let mut project = project_hint.to_owned();
    let mut model = String::new();
    let mut title = String::new();
    let mut previous_total: Option<UsageTotals> = None;
    for line in lines {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let kind = value.get("type").and_then(Value::as_str).unwrap_or("");
        let payload = value.get("payload").cloned().unwrap_or(Value::Null);
        match kind {
            "session_meta" => {
                if let Some(id) = payload.get("id").and_then(Value::as_str) {
                    session_id = id.to_owned();
                }
                if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
                    project = cwd.to_owned();
                }
            }
            "turn_context" => {
                if let Some(name) = payload.get("model").and_then(Value::as_str) {
                    model = name.to_owned();
                }
            }
            "response_item" if title.is_empty() => {
                if payload.get("role").and_then(Value::as_str) == Some("user") {
                    if let Some(text) = payload.get("content").and_then(text_content) {
                        title = summarize_title(&text);
                    }
                }
            }
            "event_msg" if payload.get("type").and_then(Value::as_str) == Some("token_count") => {
                let Some(info) = payload.get("info") else {
                    continue;
                };
                let totals = if let Some(last) = info.get("last_token_usage") {
                    codex_totals(last)
                } else if let Some(total) = info.get("total_token_usage") {
                    let current = codex_totals(total);
                    let delta = match &previous_total {
                        Some(previous) => UsageTotals {
                            input_tokens: current
                                .input_tokens
                                .saturating_sub(previous.input_tokens),
                            output_tokens: current
                                .output_tokens
                                .saturating_sub(previous.output_tokens),
                            cache_read_tokens: current
                                .cache_read_tokens
                                .saturating_sub(previous.cache_read_tokens),
                            cache_write_tokens: current
                                .cache_write_tokens
                                .saturating_sub(previous.cache_write_tokens),
                            total_tokens: current
                                .total_tokens
                                .saturating_sub(previous.total_tokens),
                        },
                        None => current,
                    };
                    previous_total = Some(current);
                    delta
                } else {
                    continue;
                };
                if totals.is_empty() {
                    continue;
                }
                records.push(UsageRecord {
                    agent: "codex".into(),
                    session_id: session_id.clone(),
                    project: project.clone(),
                    model: if model.is_empty() {
                        "unknown".into()
                    } else {
                        model.clone()
                    },
                    title: title.clone(),
                    timestamp_ms: parse_timestamp(value.get("timestamp")),
                    totals,
                });
            }
            _ => {}
        }
    }
    records
}

fn codex_totals(value: &Value) -> UsageTotals {
    let input = u64_field(value, "input_tokens");
    let output = u64_field(value, "output_tokens");
    UsageTotals {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: u64_field(value, "cached_input_tokens"),
        cache_write_tokens: u64_field(value, "cache_write_input_tokens"),
        total_tokens: u64_field(value, "total_tokens").max(input.saturating_add(output)),
    }
}

pub fn parse_opencode(text: &str, path: &Path) -> Vec<UsageRecord> {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    collect_opencode(&value, path, &mut records);
    records
}

fn collect_opencode(value: &Value, path: &Path, records: &mut Vec<UsageRecord>) {
    let objects = [Some(value), value.get("message"), value.get("info")];
    for object in objects.into_iter().flatten() {
        let Some(object) = object.as_object() else {
            continue;
        };
        let session_id = object
            .get("sessionID")
            .or_else(|| object.get("sessionId"))
            .or_else(|| object.get("session_id"))
            .and_then(Value::as_str);
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .map(summarize_title)
            .unwrap_or_default();
        let project = object
            .get("directory")
            .or_else(|| object.get("projectPath"))
            .or_else(|| object.get("cwd"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let model = object
            .get("modelID")
            .or_else(|| object.get("modelId"))
            .or_else(|| object.get("model"))
            .and_then(|model| {
                model
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| model.get("id").and_then(Value::as_str).map(str::to_owned))
            })
            .unwrap_or_else(|| "unknown".into());
        let timestamp_ms = object
            .get("time")
            .and_then(|time| {
                time.get("created")
                    .or_else(|| time.get("updated"))
                    .and_then(|value| {
                        value
                            .as_i64()
                            .or_else(|| value.as_str().and_then(parse_timestamp_str))
                    })
            })
            .unwrap_or_else(now_ms);
        if let Some(session_id) = session_id {
            if let Some(tokens) = object.get("tokens") {
                let mut totals = UsageTotals {
                    input_tokens: u64_field(tokens, "input"),
                    output_tokens: u64_field(tokens, "output") + u64_field(tokens, "reasoning"),
                    cache_read_tokens: tokens
                        .get("cache")
                        .map(|cache| u64_field(cache, "read"))
                        .unwrap_or(0),
                    cache_write_tokens: tokens
                        .get("cache")
                        .map(|cache| u64_field(cache, "write"))
                        .unwrap_or(0),
                    total_tokens: 0,
                };
                totals.total_tokens = totals.input_tokens + totals.output_tokens;
                if !totals.is_empty() {
                    records.push(UsageRecord {
                        agent: "opencode".into(),
                        session_id: session_id.to_owned(),
                        project: project.clone(),
                        model: model.clone(),
                        title: title.clone(),
                        timestamp_ms,
                        totals,
                    });
                }
            } else if let Some(id) = object.get("id").and_then(Value::as_str) {
                records.push(UsageRecord {
                    agent: "opencode".into(),
                    session_id: id.to_owned(),
                    project: project.clone(),
                    model: String::new(),
                    title: title.clone(),
                    timestamp_ms,
                    totals: UsageTotals::default(),
                });
            }
        }
    }
    if let Some(children) = value.get("messages").and_then(Value::as_array) {
        for child in children {
            collect_opencode(child, path, records);
        }
    }
    let _ = path;
}

fn text_content(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(Value::as_str)
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

fn summarize_title(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    line.chars().take(120).collect()
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn parse_timestamp(value: Option<&Value>) -> i64 {
    value
        .and_then(Value::as_str)
        .and_then(parse_timestamp_str)
        .unwrap_or_else(now_ms)
}

fn parse_timestamp_str(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.timestamp_millis())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn claude_usage_and_project_are_read() {
        let text = r#"{"type":"user","sessionId":"s1","cwd":"C:/work/app","message":{"content":"Fix the login retry"}}
{"type":"assistant","sessionId":"s1","cwd":"C:/work/app","timestamp":"2026-11-10T10:00:00Z","message":{"model":"claude-sonnet","usage":{"input_tokens":100,"output_tokens":20,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}}
{"type":"assistant","sessionId":"s1","message":{"model":"claude-sonnet","usage":{"input_tokens":0,"output_tokens":0}}}"#;
        let records = parse_claude(&lines(text), "hint", "fallback");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].session_id, "s1");
        assert_eq!(records[0].project, "C:/work/app");
        assert_eq!(records[0].title, "Fix the login retry");
        assert_eq!(records[0].totals.total_tokens, 120);
        assert_eq!(records[0].totals.cache_read_tokens, 10);
    }

    #[test]
    fn codex_prefers_last_usage_and_falls_back_to_the_total_delta() {
        let text = r#"{"type":"session_meta","payload":{"id":"c1","cwd":"C:/work/codex"}}
{"type":"turn_context","payload":{"model":"gpt-5"}}
{"type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"input_tokens":50,"output_tokens":10,"total_tokens":60}}}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120}}}}
{"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":150,"output_tokens":30,"total_tokens":180}}}}"#;
        let records = parse_codex(&lines(text), "hint", "fallback");
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].totals.total_tokens, 60);
        assert_eq!(records[1].session_id, "c1");
        assert_eq!(records[1].project, "C:/work/codex");
        assert_eq!(records[1].model, "gpt-5");
        assert_eq!(records[1].totals.total_tokens, 120);
        assert_eq!(records[2].totals.total_tokens, 60);
    }

    #[test]
    fn opencode_tokens_and_sessions_are_collected() {
        let text = r#"{"id":"o1","sessionID":"o1","title":"Refactor parser","modelID":"qwen","time":{"created":1762764000000},"tokens":{"input":100,"output":20,"reasoning":5,"cache":{"read":10,"write":2}}}"#;
        let records = parse_opencode(text, Path::new("x.json"));
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].agent, "opencode");
        assert_eq!(records[0].title, "Refactor parser");
        assert_eq!(records[0].totals.total_tokens, 125);
        assert_eq!(records[0].totals.cache_read_tokens, 10);
    }
}
