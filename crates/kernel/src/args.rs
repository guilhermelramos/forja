//! Small value types shared by commands and RPC handlers.

use crate::error::{KernelError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Stable, dotted command id in English, for example `bench.create`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(String);

impl CommandId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CommandId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for CommandId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for CommandId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Argument object passed to a command or an RPC method.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args(Map<String, Value>);

impl Args {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_map(map: Map<String, Value>) -> Self {
        Self(map)
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.0.insert(key.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(Value::as_str)
    }

    pub fn required_str(&self, name: &str) -> Result<&str> {
        self.str(name)
            .ok_or_else(|| KernelError::Message(format!("missing required argument `{name}`")))
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.0.get(name).and_then(Value::as_bool)
    }

    pub fn i64(&self, name: &str) -> Option<i64> {
        self.0.get(name).and_then(Value::as_i64)
    }

    pub fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }

    pub fn into_map(self) -> Map<String, Value> {
        self.0
    }

    pub fn into_json(self) -> Value {
        Value::Object(self.0)
    }
}

impl From<Map<String, Value>> for Args {
    fn from(map: Map<String, Value>) -> Self {
        Self(map)
    }
}

impl From<Args> for Map<String, Value> {
    fn from(args: Args) -> Self {
        args.0
    }
}
