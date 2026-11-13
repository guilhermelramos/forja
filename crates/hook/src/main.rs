//! `forja-hook`: reads one payload from stdin and forwards it to the app.
//!
//! The per-agent interpretation arrives in Phase C. This skeleton only proves
//! the transport: it exits 0 immediately when the app is closed, and reports
//! transport failures when the app is open.

use serde_json::Value;
use std::io::Read;

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        std::process::exit(1);
    }
    if input.trim().is_empty() {
        return;
    }
    let payload = serde_json::from_str::<Value>(&input)
        .unwrap_or_else(|_| Value::String(input.trim().to_owned()));
    let mut profile_id = None;
    let mut event = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--agent" => profile_id = args.next(),
            "--event" => event = args.next(),
            _ => {}
        }
    }
    let envelope = serde_json::json!({
        "forja_profile_id": profile_id,
        "forja_event": event,
        "forja_token": std::env::var("FORJA_HOOK_TOKEN").ok(),
        "payload": payload
    });
    let params = serde_json::json!({ "payload": envelope });
    match forja_protocol::call("hook.event", params) {
        Ok(_) => {}
        Err(forja_protocol::ClientError::NotRunning) => {}
        Err(forja_protocol::ClientError::Rpc(error))
            if error.code == forja_protocol::codec::METHOD_NOT_FOUND =>
        {
            // The transport worked; the Phase C module will register the handler.
        }
        Err(error) => {
            eprintln!("forja-hook: {error}");
            std::process::exit(1);
        }
    }
}
