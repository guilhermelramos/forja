//! RPC host: owns the named pipe and hands request lines to the UI thread.
//!
//! Controlled by `app.cli.enabled`. When `--test-driver` is active, the host
//! stays running even if `app.cli.enabled` is false, because the test driver
//! delivers `ui.*` commands over the same named pipe.

use forja_kernel::tasks::Wake;
use std::io;

/// Returns whether the local named-pipe RPC server should be running.
///
/// Screenshot runs never expose RPC. Test-driver runs always keep it active
/// because `--test-driver` uses the same named pipe as the CLI.
pub(crate) fn should_run_rpc_server(cli_enabled: bool, test_driver: bool, screenshot: bool) -> bool {
    !screenshot && (test_driver || cli_enabled)
}

#[cfg(windows)]
pub use forja_protocol::pipe::RpcCall;

#[cfg(windows)]
pub struct RpcHost {
    receiver: std::sync::mpsc::Receiver<RpcCall>,
    _server: forja_protocol::pipe::PipeServer,
}

#[cfg(windows)]
impl RpcHost {
    pub fn start(wake: Wake) -> io::Result<Self> {
        Self::start_named(&forja_protocol::pipe_name(), wake)
    }

    pub fn start_named(name: &str, wake: Wake) -> io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let server = forja_protocol::pipe::spawn_server(name, sender, wake)?;
        Ok(Self {
            receiver,
            _server: server,
        })
    }

    pub fn try_recv(&self) -> Option<RpcCall> {
        self.receiver.try_recv().ok()
    }
}

#[cfg(not(windows))]
pub struct RpcCall {
    pub line: String,
    pub reply: std::sync::mpsc::Sender<Option<String>>,
}

#[cfg(not(windows))]
pub struct RpcHost;

#[cfg(not(windows))]
impl RpcHost {
    pub fn start(_wake: Wake) -> io::Result<Self> {
        Ok(Self)
    }

    pub fn start_named(_name: &str, _wake: Wake) -> io::Result<Self> {
        Ok(Self)
    }

    pub fn try_recv(&self) -> Option<RpcCall> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn rpc_gate_keeps_test_driver_active_and_blocks_screenshots() {
        assert!(should_run_rpc_server(true, false, false));
        assert!(!should_run_rpc_server(false, false, false));
        assert!(should_run_rpc_server(false, true, false));
        assert!(!should_run_rpc_server(true, false, true));
        assert!(!should_run_rpc_server(true, true, true));
    }

    #[cfg(windows)]
    #[test]
    fn rpc_host_drop_closes_pipe_and_allows_restart() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let pipe = format!(r"\\.\pipe\forja-shell-rpc-test-{}-{nanos}", std::process::id());
        let host = RpcHost::start_named(&pipe, Arc::new(|| {})).expect("start");
        drop(host);
        let restarted = RpcHost::start_named(&pipe, Arc::new(|| {})).expect("restart");
        drop(restarted);
    }
}
