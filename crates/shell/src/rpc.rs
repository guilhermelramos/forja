//! RPC host: owns the named pipe and hands request lines to the UI thread.

use forja_kernel::tasks::Wake;
use std::io;

#[cfg(windows)]
pub use forja_protocol::pipe::RpcCall;

#[cfg(windows)]
pub struct RpcHost {
    receiver: std::sync::mpsc::Receiver<RpcCall>,
}

#[cfg(windows)]
impl RpcHost {
    pub fn start(wake: Wake) -> io::Result<Self> {
        let (sender, receiver) = std::sync::mpsc::channel();
        forja_protocol::pipe::spawn(&forja_protocol::pipe_name(), sender, wake)?;
        Ok(Self { receiver })
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

    pub fn try_recv(&self) -> Option<RpcCall> {
        None
    }
}
