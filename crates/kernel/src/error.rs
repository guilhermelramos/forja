//! Error type returned by kernel services and module handlers.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, KernelError>;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("registration failed: {0}")]
    Registration(String),
    #[error("{0}")]
    Message(String),
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("command `{id}` failed: {message}")]
    Command { id: String, message: String },
    #[error("invalid setting: {0}")]
    Settings(String),
    #[error("setting `{0}` cannot be overridden by a project")]
    UnsafeProjectSetting(String),
    #[error("no active project is loaded for project settings")]
    NoActiveProject,
    #[error("invalid key binding: {0}")]
    Keymap(String),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("toml error: {0}")]
    Toml(String),
    #[error("rpc error: {0}")]
    Rpc(#[from] forja_protocol::RpcError),
}

impl KernelError {
    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn registration(message: impl Into<String>) -> Self {
        Self::Registration(message.into())
    }

    pub fn is_unsafe_project_setting(&self) -> bool {
        matches!(self, Self::UnsafeProjectSetting(_))
    }

    pub fn is_no_active_project(&self) -> bool {
        matches!(self, Self::NoActiveProject)
    }
}

impl From<toml::de::Error> for KernelError {
    fn from(error: toml::de::Error) -> Self {
        Self::Toml(error.to_string())
    }
}

impl From<toml::ser::Error> for KernelError {
    fn from(error: toml::ser::Error) -> Self {
        Self::Toml(error.to_string())
    }
}
