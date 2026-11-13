//! Conversão dos argumentos JSON de `terminal.open`.

use crate::core::OpenRequest;
use forja_kernel::Args;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(super) fn open_request(args: &Args) -> OpenRequest {
    OpenRequest {
        cwd: args.str("cwd").map(PathBuf::from),
        command: args.str("command").map(str::to_owned),
        args: args.get("args").and_then(value_text),
        environment: args.get("env").and_then(value_environment).unwrap_or_default(),
        title: args.str("title").map(str::to_owned),
        shell: args.str("shell").map(str::to_owned),
    }
}

pub(super) fn open_request_map(params: &Map<String, Value>) -> OpenRequest {
    OpenRequest {
        cwd: params.get("cwd").and_then(Value::as_str).map(PathBuf::from),
        command: params.get("command").and_then(Value::as_str).map(str::to_owned),
        args: params.get("args").and_then(value_text),
        environment: params
            .get("env")
            .and_then(value_environment)
            .unwrap_or_default(),
        title: params.get("title").and_then(Value::as_str).map(str::to_owned),
        shell: params.get("shell").and_then(Value::as_str).map(str::to_owned),
    }
}

fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(quote_argument)
                .collect::<Vec<_>>()
                .join(" "),
        ),
        _ => None,
    }
}

fn value_environment(value: &Value) -> Option<BTreeMap<String, String>> {
    match value {
        Value::Object(map) => Some(
            map.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_owned()))
                })
                .collect(),
        ),
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .filter_map(|item| item.split_once('='))
                .map(|(key, value)| (key.trim().to_owned(), value.to_owned()))
                .collect(),
        ),
        _ => None,
    }
}

fn quote_argument(value: &str) -> String {
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            result.push_str(&"\\".repeat(slashes));
        }
        slashes = 0;
        result.push(ch);
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}

pub(super) fn parse_id(value: &str) -> Option<u64> {
    value.parse().ok()
}


