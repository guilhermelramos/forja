//! JSON-RPC 2.0 framing: parse one request line, render one response line.
//!
//! Pure (no IO), so both the shell and the CLI test the same codec. Batch
//! requests are rejected with `-32600`; a request without an `id` is a
//! notification and gets no reply.

use serde_json::{json, Map, Value};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;
/// The app refused the request or could not carry it out.
pub const APP_ERROR: i64 = -32000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn app(message: impl Into<String>) -> Self {
        Self::new(APP_ERROR, message)
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(INVALID_PARAMS, message)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("Method not found: {method}"))
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for RpcError {}

/// A parsed request. `id` is `None` for a notification (no reply is sent).
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: Option<Value>,
    pub method: String,
    /// Always an object; absent params become an empty object.
    pub params: Map<String, Value>,
}

/// Public alias used by the session layer.
pub type RpcRequest = Request;

/// A request that could not be parsed. The reply carries `id` when readable.
#[derive(Debug, Clone, PartialEq)]
pub struct Invalid {
    pub id: Value,
    pub error: RpcError,
}

pub fn parse(line: &str) -> Result<Request, Invalid> {
    let value: Value = serde_json::from_str(line.trim()).map_err(|e| Invalid {
        id: Value::Null,
        error: RpcError::new(PARSE_ERROR, format!("Parse error: {e}")),
    })?;
    let object = match value {
        Value::Object(object) => object,
        Value::Array(_) => {
            return Err(Invalid {
                id: Value::Null,
                error: RpcError::new(INVALID_REQUEST, "Batch requests are not supported"),
            })
        }
        _ => {
            return Err(invalid_request(
                Value::Null,
                "Request must be a JSON object",
            ))
        }
    };
    let id_value = object.get("id").cloned();
    let reply_id = id_value.clone().unwrap_or(Value::Null);
    let id = match id_value {
        None => None,
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Null)) => Some(v),
        Some(_) => {
            return Err(invalid_request(
                Value::Null,
                "id must be a string, number or null",
            ))
        }
    };
    if object.get("jsonrpc") != Some(&json!("2.0")) {
        return Err(invalid_request(reply_id, "jsonrpc must be \"2.0\""));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Err(invalid_request(reply_id, "method must be a string"));
    };
    let params = match object.get("params") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(map)) => map.clone(),
        Some(_) => {
            return Err(Invalid {
                id: reply_id,
                error: RpcError::invalid_params("params must be an object"),
            })
        }
    };
    Ok(Request {
        id,
        method: method.to_owned(),
        params,
    })
}

fn invalid_request(id: Value, message: &str) -> Invalid {
    Invalid {
        id,
        error: RpcError::new(INVALID_REQUEST, message),
    }
}

/// Best-effort id of a raw request line, used when the UI thread never answered.
pub fn request_id(line: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    value.get("id").cloned()
}

pub fn ok_response(id: &Value, result: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

pub fn error_response(id: Value, error: &RpcError) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": error.code, "message": error.message},
    })
    .to_string()
}

/// Builds a request line for the client side.
pub fn request_line(id: Value, method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

/// Reads the result of a response line, turning a JSON-RPC error into `Err`.
pub fn response_result(line: &str) -> Result<Value, RpcError> {
    let value: Value = serde_json::from_str(line.trim())
        .map_err(|e| RpcError::new(PARSE_ERROR, format!("Parse error: {e}")))?;
    if let Some(error) = value.get("error") {
        let code = error.get("code").and_then(Value::as_i64).unwrap_or(INTERNAL_ERROR);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Unknown error")
            .to_owned();
        return Err(RpcError::new(code, message));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_with_numeric_string_and_null_ids() {
        let r = parse(r#"{"jsonrpc":"2.0","id":7,"method":"status.get"}"#).unwrap();
        assert_eq!(r.id, Some(json!(7)));
        assert_eq!(r.method, "status.get");
        assert!(r.params.is_empty());
        assert_eq!(
            parse(r#"{"jsonrpc":"2.0","id":"abc","method":"x"}"#)
                .unwrap()
                .id,
            Some(json!("abc"))
        );
        assert_eq!(
            parse(r#"{"jsonrpc":"2.0","id":null,"method":"x"}"#)
                .unwrap()
                .id,
            Some(Value::Null)
        );
    }

    #[test]
    fn missing_id_is_a_notification() {
        let r = parse(r#"{"jsonrpc":"2.0","method":"status.get"}"#).unwrap();
        assert_eq!(r.id, None);
    }

    #[test]
    fn params_must_be_an_object() {
        let r = parse(r#"{"jsonrpc":"2.0","id":1,"method":"m","params":{"path":"a"}}"#).unwrap();
        assert_eq!(r.params["path"], json!("a"));
        let err = parse(r#"{"jsonrpc":"2.0","id":3,"method":"m","params":[1]}"#).unwrap_err();
        assert_eq!(err.error.code, INVALID_PARAMS);
        assert_eq!(err.id, json!(3));
    }

    #[test]
    fn invalid_json_is_a_parse_error_with_null_id() {
        let err = parse("{not json").unwrap_err();
        assert_eq!(err.error.code, PARSE_ERROR);
        assert_eq!(err.id, Value::Null);
    }

    #[test]
    fn batches_are_rejected() {
        let err = parse(r#"[{"jsonrpc":"2.0","id":1,"method":"status.get"}]"#).unwrap_err();
        assert_eq!(err.error.code, INVALID_REQUEST);
        assert!(err.error.message.contains("Batch"));
    }

    #[test]
    fn wrong_version_method_or_id_type_is_invalid() {
        assert_eq!(
            parse(r#"{"jsonrpc":"1.0","id":1,"method":"m"}"#)
                .unwrap_err()
                .error
                .code,
            INVALID_REQUEST
        );
        let no_method = parse(r#"{"jsonrpc":"2.0","id":4}"#).unwrap_err();
        assert_eq!(no_method.error.code, INVALID_REQUEST);
        assert_eq!(no_method.id, json!(4));
        assert_eq!(
            parse(r#"{"jsonrpc":"2.0","id":{},"method":"m"}"#)
                .unwrap_err()
                .error
                .code,
            INVALID_REQUEST
        );
        assert_eq!(parse("42").unwrap_err().error.code, INVALID_REQUEST);
    }

    #[test]
    fn renders_success_and_error_responses() {
        let ok: Value = serde_json::from_str(&ok_response(&json!(9), json!({"a": 1}))).unwrap();
        assert_eq!(ok["jsonrpc"], "2.0");
        assert_eq!(ok["id"], 9);
        assert_eq!(ok["result"]["a"], 1);
        assert!(ok.get("error").is_none());

        let err: Value = serde_json::from_str(&error_response(
            json!("x"),
            &RpcError::method_not_found("nope"),
        ))
        .unwrap();
        assert_eq!(err["id"], "x");
        assert_eq!(err["error"]["code"], METHOD_NOT_FOUND);
        assert_eq!(err["error"]["message"], "Method not found: nope");
        assert!(err.get("result").is_none());
    }

    #[test]
    fn request_id_is_read_from_raw_lines() {
        assert_eq!(request_id(r#"{"id":5,"method":"x"}"#), Some(json!(5)));
        assert_eq!(request_id(r#"{"method":"x"}"#), None);
        assert_eq!(request_id("garbage"), None);
    }

    #[test]
    fn response_result_parses_ok_and_remote_errors() {
        assert_eq!(
            response_result(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#).unwrap(),
            json!({"ok": true})
        );
        let error = response_result(
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"nope"}}"#,
        )
        .unwrap_err();
        assert_eq!(error.code, METHOD_NOT_FOUND);
    }
}
