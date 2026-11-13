//! Wire contract shared by `forja-app`, the `forja` CLI and `forja-hook`.
//!
//! The transport is a named pipe owned by the shell. One JSON-RPC 2.0 request
//! goes in per line and one response line comes back. This crate has no
//! dependency on the kernel or on egui, so the CLI can compile and run without
//! the graphical stack.
//!
//! The method catalog is data only: modules own the handlers (in the kernel)
//! and the shell builds the catalog from the registry. [`schema::MethodCatalog`]
//! is what `meta.methods` returns and what the CLI turns into subcommands.

pub mod client;
pub mod codec;
#[cfg(windows)]
pub mod pipe;
pub mod schema;

pub use client::{call, call_raw, notify, ClientError};
pub use codec::{RpcError, RpcRequest};
pub use schema::{
    Method, MethodCatalog, MethodEffect, MethodSchema, ParamKind, ParamSchema, ResultSchema,
};

/// Largest request line accepted by the server and by the client.
pub const MAX_LINE_BYTES: usize = 1024 * 1024;

/// Default pipe name for the current Windows user.
///
/// Override with `FORJA_PIPE` for isolated tests; normal runs always use
/// `\\.\pipe\forja-<usuario>`.
pub fn pipe_name() -> String {
    if let Ok(name) = std::env::var("FORJA_PIPE") {
        if !name.is_empty() {
            return name;
        }
    }
    let user = std::env::var("USERNAME")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "usuario".to_owned());
    let safe: String = user
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    format!(r"\\.\pipe\forja-{safe}")
}
