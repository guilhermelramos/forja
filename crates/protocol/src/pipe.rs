//! Named-pipe transport for the local RPC.
//!
//! One JSON request per line in, one JSON reply per line out, then the
//! connection closes. Clients on other machines are refused by the kernel
//! (`PIPE_REJECT_REMOTE_CLIENTS`); the default DACL limits access to the
//! current user and administrators.

use super::codec;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::FromRawHandle;
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{
    FlushFileBuffers, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

/// Longest request line accepted. Longer requests are refused.
pub const MAX_LINE_BYTES: usize = super::MAX_LINE_BYTES;
/// How long a connection waits for the UI thread before it gets a timeout error.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(120);
/// Most RPC connections served at once.
pub const MAX_CONNECTIONS: usize = 16;

/// Shared "wake the UI" callback, called from connection threads.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// A raw request handed to the UI thread, with a channel for its reply line.
pub struct RpcCall {
    pub line: String,
    pub reply: Sender<Option<String>>,
}

/// Creates the first pipe instance (failing if another Forja already owns the
/// name) and starts the accept thread.
pub fn spawn(name: &str, calls: Sender<RpcCall>, wake: Wake) -> io::Result<()> {
    let first = create_instance(name, true)? as usize;
    let name = name.to_owned();
    std::thread::Builder::new()
        .name("forja-rpc".into())
        .spawn(move || accept_loop(&name, first, &calls, &wake))
        .map(|_| ())
}

fn create_instance(name: &str, first: bool) -> io::Result<HANDLE> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut open_mode = PIPE_ACCESS_DUPLEX;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
    // SAFETY: `wide` is NUL-terminated and lives across the call; a null
    // security pointer selects the default DACL.
    let handle = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            open_mode,
            mode,
            PIPE_UNLIMITED_INSTANCES,
            64 * 1024,
            64 * 1024,
            0,
            null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(handle)
    }
}

static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

fn acquire_connection(active: &AtomicUsize, limit: usize) -> bool {
    active
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
            (used < limit).then_some(used + 1)
        })
        .is_ok()
}

fn release_connection(active: &AtomicUsize) {
    active.fetch_sub(1, Ordering::SeqCst);
}

fn accept_loop(name: &str, mut current: usize, calls: &Sender<RpcCall>, wake: &Wake) {
    loop {
        let handle = current as HANDLE;
        // SAFETY: `handle` is a pipe instance owned by this thread.
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        // Create the next instance before serving this one so clients never
        // see a moment where the pipe does not exist.
        let next = loop {
            match create_instance(name, false) {
                Ok(handle) => break handle,
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        };
        if connected && acquire_connection(&ACTIVE_CONNECTIONS, MAX_CONNECTIONS) {
            let (calls, wake) = (calls.clone(), Arc::clone(wake));
            let handle = handle as usize;
            let spawned = std::thread::Builder::new()
                .name("forja-rpc-conn".into())
                .spawn(move || {
                    serve(handle as HANDLE, &calls, &wake);
                    release_connection(&ACTIVE_CONNECTIONS);
                });
            if spawned.is_err() {
                release_connection(&ACTIVE_CONNECTIONS);
            }
        } else if connected {
            // Too many connections at once: refuse this one instead of giving
            // it a thread.
            // SAFETY: the instance is connected and not used anywhere else.
            unsafe {
                DisconnectNamedPipe(handle);
                CloseHandle(handle);
            }
        } else {
            // SAFETY: the instance failed to connect and is not used anywhere else.
            unsafe { CloseHandle(handle) };
        }
        current = next as usize;
    }
}

/// Serves one connection: read a line, ask the UI thread, write the reply, close.
fn serve(handle: HANDLE, calls: &Sender<RpcCall>, wake: &Wake) {
    // SAFETY: this thread now owns the instance; `File` closes it on drop.
    let mut file = unsafe { File::from_raw_handle(handle as _) };
    let reply = match read_line(&mut file) {
        Ok(line) => ask_ui(line, calls, wake),
        Err(_) => None,
    };
    if let Some(text) = reply {
        let _ = file.write_all(text.as_bytes());
        let _ = file.write_all(b"\n");
        // SAFETY: the handle is still open (the File has not been dropped yet).
        unsafe { FlushFileBuffers(handle) };
    }
    // SAFETY: as above; disconnect before the File closes the handle.
    unsafe { DisconnectNamedPipe(handle) };
}

/// Hands a request to the UI thread and waits for its reply.
fn ask_ui(line: String, calls: &Sender<RpcCall>, wake: &Wake) -> Option<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let id = codec::request_id(&line);
    let is_notification = !line.trim().is_empty() && id.is_none();
    if calls.send(RpcCall { line, reply: tx }).is_err() {
        return Some(codec::error_response(
            serde_json::Value::Null,
            &codec::RpcError::new(codec::INTERNAL_ERROR, "Forja is shutting down"),
        ));
    }
    wake();
    match rx.recv_timeout(REPLY_TIMEOUT) {
        Ok(reply) => reply,
        Err(RecvTimeoutError::Timeout) if is_notification => None,
        Err(RecvTimeoutError::Timeout) => Some(codec::error_response(
            id.unwrap_or(serde_json::Value::Null),
            &codec::RpcError::new(codec::INTERNAL_ERROR, "Forja did not answer in time"),
        )),
        Err(RecvTimeoutError::Disconnected) => Some(codec::error_response(
            id.unwrap_or(serde_json::Value::Null),
            &codec::RpcError::new(codec::INTERNAL_ERROR, "Forja is shutting down"),
        )),
    }
}

/// Reads bytes up to the first newline (or EOF), refusing oversized frames.
fn read_line(reader: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte)? {
            0 => break,
            _ if byte[0] == b'\n' => break,
            _ => {
                if bytes.len() >= MAX_LINE_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "request too large",
                    ));
                }
                bytes.push(byte[0]);
            }
        }
    }
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use std::sync::mpsc;

    fn unique_pipe() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!(r"\\.\pipe\forja-rpc-test-{}-{nanos}", std::process::id())
    }

    fn client_call(name: &str, request: &str) -> io::Result<String> {
        let mut pipe = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)?;
        pipe.write_all(request.as_bytes())?;
        pipe.write_all(b"\n")?;
        let mut reader = io::BufReader::new(pipe);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        Ok(line)
    }

    #[test]
    fn pipe_roundtrip_through_a_ui_thread_stand_in() {
        let name = unique_pipe();
        let (calls_tx, calls_rx) = mpsc::channel::<RpcCall>();
        spawn(&name, calls_tx, Arc::new(|| {})).expect("server starts");

        std::thread::spawn(move || {
            while let Ok(call) = calls_rx.recv() {
                let reply = codec::parse(&call.line).ok().map(|req| {
                    codec::ok_response(&req.id.unwrap_or_default(), serde_json::json!(req.method))
                });
                let _ = call.reply.send(reply);
            }
        });

        let reply = client_call(&name, r#"{"jsonrpc":"2.0","id":42,"method":"status.get"}"#)
            .expect("round trip");
        let value: serde_json::Value = serde_json::from_str(reply.trim()).unwrap();
        assert_eq!(value["id"], 42);
        assert_eq!(value["result"], "status.get");

        let reply = client_call(&name, r#"{"jsonrpc":"2.0","id":"b","method":"repo.list"}"#)
            .expect("second round trip");
        let value: serde_json::Value = serde_json::from_str(reply.trim()).unwrap();
        assert_eq!(value["result"], "repo.list");
    }

    #[test]
    fn a_second_server_on_the_same_name_is_refused() {
        let name = unique_pipe();
        let (tx1, _rx1) = mpsc::channel();
        spawn(&name, tx1, Arc::new(|| {})).expect("first server");
        let (tx2, _rx2) = mpsc::channel();
        assert!(spawn(&name, tx2, Arc::new(|| {})).is_err());
    }

    #[test]
    fn connection_slots_are_capped_and_released() {
        let active = AtomicUsize::new(0);
        for _ in 0..3 {
            assert!(acquire_connection(&active, 3));
        }
        assert!(!acquire_connection(&active, 3), "a fourth connection is refused");
        release_connection(&active);
        assert!(acquire_connection(&active, 3), "a freed slot is reused");
        assert_eq!(MAX_CONNECTIONS, 16);
    }

    #[test]
    fn read_line_stops_at_newline_and_refuses_huge_frames() {
        let mut input: &[u8] = b"{\"a\":1}\nignored";
        assert_eq!(read_line(&mut input).unwrap(), "{\"a\":1}");
        let big = vec![b'x'; MAX_LINE_BYTES + 1];
        let mut big: &[u8] = &big;
        assert!(read_line(&mut big).is_err());
    }
}
