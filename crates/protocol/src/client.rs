//! Client side of the local RPC pipe.
//!
//! The CLI and the hook use this module. It never starts the app: a missing
//! pipe is reported as [`ClientError::NotRunning`] so callers can choose exit
//! code 2 (CLI) or a silent exit (hook).

use crate::{codec, pipe_name};
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use thiserror::Error;

/// How long the CLI waits for the shell to answer.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Error)]
pub enum ClientError {
    /// No pipe exists (or it refused the connection): the app is not running.
    #[error("the Forja app is not running")]
    NotRunning,
    #[error("the Forja app did not answer in time")]
    Timeout,
    #[error("pipe error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Rpc(#[from] codec::RpcError),
}

/// Calls one method and returns its result object.
pub fn call(method: &str, params: serde_json::Value) -> Result<serde_json::Value, ClientError> {
    let line = codec::request_line(serde_json::json!(1), method, params);
    let reply = call_raw(&line)?;
    codec::response_result(&reply).map_err(ClientError::from)
}

/// Sends a raw JSON-RPC line and returns the raw reply line.
///
/// Runs the blocking pipe round trip on a helper thread so a hung app cannot
/// hang the CLI forever.
pub fn call_raw(line: &str) -> Result<String, ClientError> {
    let pipe = pipe_name();
    let line = line.to_owned();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("forja-rpc-client".into())
        .spawn(move || {
            let _ = tx.send(roundtrip(&pipe, &line));
        })
        .map_err(ClientError::Io)?;
    match rx.recv_timeout(CALL_TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(ClientError::Timeout),
    }
}

/// Sends a notification (no reply expected).
pub fn notify(method: &str, params: serde_json::Value) -> Result<(), ClientError> {
    let pipe = pipe_name();
    let line = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
    .to_string();
    let mut pipe = open(&pipe)?;
    pipe.write_all(line.as_bytes())?;
    pipe.write_all(b"\n")?;
    pipe.flush()?;
    Ok(())
}

fn roundtrip(pipe: &str, line: &str) -> Result<String, ClientError> {
    let mut pipe = open(pipe)?;
    pipe.write_all(line.as_bytes())?;
    pipe.write_all(b"\n")?;
    pipe.flush()?;
    let mut reader = BufReader::new(pipe);
    let mut reply = String::new();
    reader.read_line(&mut reply)?;
    Ok(reply)
}

fn open(name: &str) -> Result<std::fs::File, ClientError> {
    let mut last_error = None;
    for _ in 0..5 {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
        {
            Ok(file) => return Ok(file),
            Err(error) => {
                if is_not_running(&error) {
                    return Err(ClientError::NotRunning);
                }
                if error.raw_os_error() != Some(231) {
                    return Err(ClientError::Io(error));
                }
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
    Err(ClientError::Io(last_error.unwrap_or_else(|| {
        std::io::Error::other("pipe is busy")
    })))
}

fn is_not_running(error: &std::io::Error) -> bool {
    matches!(error.kind(), std::io::ErrorKind::NotFound)
        || matches!(error.raw_os_error(), Some(2 | 3 | 53 | 1058))
}
