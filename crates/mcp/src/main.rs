//! `forja-mcp`: transporte stdio do servidor MCP da Forja.
//!
//! O stdout carrega apenas mensagens MCP válidas. Erros de transporte vão para
//! stderr. O processo termina quando o cliente fecha o stdin.

use forja_mcp::{Backend, BackendError, Bridge};
use std::io::{self, BufRead, Write};

struct ProtocolBackend;

impl Backend for ProtocolBackend {
    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, BackendError> {
        forja_protocol::call(method, params).map_err(|error| match error {
            forja_protocol::ClientError::NotRunning => BackendError::new(
                "app_closed",
                "Forja is not open. Run `forja open`.",
            ),
            forja_protocol::ClientError::Timeout => BackendError::new(
                "timed_out",
                "The Forja app did not answer in time.",
            ),
            forja_protocol::ClientError::Rpc(error) => {
                BackendError::new("app_error", error.message)
            }
            forja_protocol::ClientError::Io(error) => {
                BackendError::new("pipe_error", error.to_string())
            }
        })
    }
}

fn main() {
    // `--player [--findings <file>]`: only the play-test tools (needs `forja-app --test-driver`).
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--player") {
        let findings = args
            .iter()
            .position(|arg| arg == "--findings")
            .and_then(|index| args.get(index + 1))
            .map(std::path::PathBuf::from);
        let player = forja_mcp::player::Player::new(ProtocolBackend, findings);
        serve(|line| player.handle_line(line));
        return;
    }
    let bridge = Bridge::new(ProtocolBackend);
    serve(|line| bridge.handle_line(line));
}

fn serve(handle: impl Fn(&str) -> Option<String>) {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(&line) {
            if writeln!(stdout, "{reply}").is_err() {
                break;
            }
            let _ = stdout.flush();
        }
    }
}
