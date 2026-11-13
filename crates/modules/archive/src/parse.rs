//! Parsers de transcrição do Acervo.
//!
//! Cada parser devolve o texto pesquisável, o título, o projeto e as datas.
//! Formatos não confirmados nos perfis não são inventados.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

/// Formato suportado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionFormat {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
    /// OpenCode.
    OpenCode,
}

/// Sessão pronta para indexar.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArchiveSession {
    /// Id da sessão.
    pub session_id: String,
    /// Id do perfil.
    pub helper_id: String,
    /// Projeto.
    pub project: String,
    /// Arquivo.
    pub path: String,
    /// Início em ms.
    pub started_ms: i64,
    /// Última atualização em ms.
    pub updated_ms: i64,
    /// Título curto.
    pub title: String,
    /// Texto completo.
    pub text: String,
}

/// Lê um arquivo de sessão.
pub fn parse_file(
    format: SessionFormat,
    helper_id: &str,
    path: &Path,
) -> Result<Option<ArchiveSession>, String> {
    let mut session = match format {
        SessionFormat::Claude => parse_claude(path)?,
        SessionFormat::Codex => parse_codex(path)?,
        SessionFormat::OpenCode => parse_opencode(path)?,
    };
    if let Some(session) = &mut session {
        session.helper_id = helper_id.to_owned();
        if session.path.is_empty() {
            session.path = path.display().to_string();
        }
    }
    Ok(session)
}

fn read_lines(path: &Path) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut lines = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.len() <= MAX_LINE_BYTES && !line.trim().is_empty() {
            lines.push(line);
        }
    }
    Ok(lines)
}

fn parse_claude(path: &Path) -> Result<Option<ArchiveSession>, String> {
    let mut session_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("session")
        .to_owned();
    let mut project = String::new();
    let mut title = String::new();
    let mut text = String::new();
    let mut started = 0i64;
    let mut updated = 0i64;
    for line in read_lines(path)? {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(id) = value.get("sessionId").and_then(Value::as_str) {
            session_id = id.to_owned();
        }
        if let Some(cwd) = value.get("cwd").and_then(Value::as_str) {
            project = cwd.to_owned();
        }
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp_str)
            .unwrap_or(0);
        if timestamp > 0 {
            if started == 0 || timestamp < started {
                started = timestamp;
            }
            updated = updated.max(timestamp);
        }
        match value.get("type").and_then(Value::as_str).unwrap_or("") {
            "user" => {
                if let Some(message) = value.get("message") {
                    if let Some(prompt) = message
                        .get("content")
                        .and_then(|content| message_content_text(content, "text"))
                    {
                        if title.is_empty() {
                            title = summarize_title(&prompt);
                        }
                        push_text(&mut text, &prompt);
                    }
                }
            }
            "assistant" => {
                if let Some(message) = value.get("message") {
                    if let Some(reply) = message
                        .get("content")
                        .and_then(|content| message_content_text(content, "text"))
                    {
                        push_text(&mut text, &reply);
                    }
                }
            }
            _ => {}
        }
    }
    if text.is_empty() && title.is_empty() {
        return Ok(None);
    }
    Ok(Some(ArchiveSession {
        session_id,
        helper_id: "claude-code".to_owned(),
        project,
        path: path.display().to_string(),
        started_ms: started,
        updated_ms: updated,
        title,
        text,
    }))
}

fn parse_codex(path: &Path) -> Result<Option<ArchiveSession>, String> {
    let mut session_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("session")
        .to_owned();
    let mut project = String::new();
    let mut title = String::new();
    let mut text = String::new();
    let mut started = 0i64;
    let mut updated = 0i64;
    for line in read_lines(path)? {
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let timestamp = value
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(parse_timestamp_str)
            .unwrap_or(0);
        if timestamp > 0 {
            if started == 0 || timestamp < started {
                started = timestamp;
            }
            updated = updated.max(timestamp);
        }
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
            "response_item" => {
                if payload.get("role").and_then(Value::as_str) == Some("user") {
                    if let Some(prompt) = payload
                        .get("content")
                        .and_then(|content| message_content_text(content, "input_text"))
                    {
                        if title.is_empty() {
                            title = summarize_title(&prompt);
                        }
                        push_text(&mut text, &prompt);
                    }
                } else if payload.get("role").and_then(Value::as_str) == Some("assistant") {
                    if let Some(reply) = payload
                        .get("content")
                        .and_then(|content| message_content_text(content, "output_text"))
                    {
                        push_text(&mut text, &reply);
                    }
                }
            }
            "event_msg" if payload.get("type").and_then(Value::as_str) == Some("agent_message") => {
                if let Some(reply) = payload.get("message").and_then(Value::as_str) {
                    push_text(&mut text, reply);
                }
            }
            _ => {}
        }
    }
    if text.is_empty() && title.is_empty() {
        return Ok(None);
    }
    Ok(Some(ArchiveSession {
        session_id,
        helper_id: "codex".to_owned(),
        project,
        path: path.display().to_string(),
        started_ms: started,
        updated_ms: updated,
        title,
        text,
    }))
}

fn parse_opencode(path: &Path) -> Result<Option<ArchiveSession>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return Ok(None);
    };
    let object = value
        .get("info")
        .or_else(|| value.get("message"))
        .unwrap_or(&value);
    let Some(object) = object.as_object() else {
        return Ok(None);
    };
    let session_id = object
        .get("sessionID")
        .or_else(|| object.get("sessionId"))
        .or_else(|| object.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("session")
        })
        .to_owned();
    let title = object
        .get("title")
        .and_then(Value::as_str)
        .map(summarize_title)
        .unwrap_or_default();
    let project = object
        .get("directory")
        .or_else(|| object.get("cwd"))
        .or_else(|| object.get("projectPath"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let timestamp = object
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
    let mut body = String::new();
    collect_text(&value, &mut body);
    if body.is_empty() && title.is_empty() && project.is_empty() {
        return Ok(None);
    }
    Ok(Some(ArchiveSession {
        session_id,
        helper_id: "opencode".to_owned(),
        project,
        path: path.display().to_string(),
        started_ms: timestamp,
        updated_ms: timestamp,
        title,
        text: body,
    }))
}

fn collect_text(value: &Value, text: &mut String) {
    match value {
        Value::String(string) => push_text(text, string),
        Value::Array(items) => {
            for item in items {
                collect_text(item, text);
            }
        }
        Value::Object(object) => {
            if let Some(string) = object.get("text").and_then(Value::as_str) {
                push_text(text, string);
            }
            if let Some(string) = object.get("content").and_then(Value::as_str) {
                push_text(text, string);
            }
            if let Some(parts) = object.get("parts").and_then(Value::as_array) {
                for part in parts {
                    collect_text(part, text);
                }
            }
            if let Some(messages) = object.get("messages").and_then(Value::as_array) {
                for message in messages {
                    collect_text(message, text);
                }
            }
        }
        _ => {}
    }
}

fn message_content_text(value: &Value, preferred: &str) -> Option<String> {
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

fn push_text(target: &mut String, text: &str) {
    let text = text.trim();
    if text.is_empty() || target.len() >= MAX_TEXT_BYTES {
        return;
    }
    if !target.is_empty() {
        target.push('\n');
    }
    let remaining = MAX_TEXT_BYTES.saturating_sub(target.len());
    if text.len() <= remaining {
        target.push_str(text);
    } else {
        let mut end = remaining;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        target.push_str(&text[..end]);
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

fn parse_timestamp_str(text: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|time| time.timestamp_millis())
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str, content: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-archive-parse-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn claude_fixture_has_title_text_project_and_dates() {
        let path = fixture(
            "claude-session.jsonl",
            include_str!("../fixtures/claude-session.jsonl"),
        );
        let session = parse_claude(&path)
            .unwrap()
            .expect("fixture parses");
        assert_eq!(session.session_id, "claude-session-1");
        assert_eq!(session.project, "C:/work/forja");
        assert!(session.title.contains("parser"));
        assert!(session.text.contains("Testes verdes"));
        assert!(session.started_ms > 0);
        assert!(session.updated_ms >= session.started_ms);
    }

    #[test]
    fn codex_fixture_has_title_text_project_and_dates() {
        let path = fixture(
            "codex-session.jsonl",
            include_str!("../fixtures/codex-session.jsonl"),
        );
        let session = parse_codex(&path).unwrap().expect("fixture parses");
        assert_eq!(session.session_id, "codex-session-1");
        assert_eq!(session.project, "C:/work/forja");
        assert!(session.title.contains("painel"));
        assert!(session.text.contains("Vou verificar"));
        assert!(session.started_ms > 0);
    }

    #[test]
    fn opencode_fixture_has_title_text_project_and_dates() {
        let path = fixture(
            "opencode-session.json",
            include_str!("../fixtures/opencode-session.json"),
        );
        let session = parse_opencode(&path).unwrap().expect("fixture parses");
        assert_eq!(session.session_id, "opencode-session-1");
        assert_eq!(session.project, "C:/work/forja");
        assert!(session.title.contains("view"));
        assert!(session.text.contains("Reveja"));
        assert!(session.started_ms > 0);
    }
}
