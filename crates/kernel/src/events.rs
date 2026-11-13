//! Typed event signatures and subscriptions.
//!
//! Modules declare their own event types once with `registrar.event::<T>("bench.created")`.
//! Cross-module events (types in `forja-contracts`) use `registrar.contract_event::<T>(name)`,
//! which any number of modules may call.
//! The bus keys handlers by `TypeId`, so emitting an unregistered type is a
//! no-op that the shell reports as a warning.

use crate::runtime::Ctx;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

pub type EventHandler =
    Arc<dyn for<'a, 'b> Fn(&'b mut Ctx<'a>, &dyn Any) + Send + Sync>;

#[derive(Clone, Debug)]
pub struct EventSignature {
    pub module: String,
    pub name: String,
    pub type_id: TypeId,
    /// Contract events (types from `forja-contracts`) may be declared by any module,
    /// publisher or subscriber, any number of times, and need no module-id prefix.
    pub shared: bool,
}

pub struct EventSubscription {
    pub module: String,
    pub name: String,
    pub type_id: TypeId,
    pub handler: EventHandler,
}

#[derive(Default)]
pub struct EventBus {
    names: HashMap<TypeId, String>,
    handlers: HashMap<TypeId, Vec<(String, EventHandler)>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_signature(&mut self, name: &str, type_id: TypeId) {
        self.names.entry(type_id).or_insert_with(|| name.to_owned());
    }

    pub fn subscribe(&mut self, module: &str, type_id: TypeId, handler: EventHandler) {
        self.handlers
            .entry(type_id)
            .or_default()
            .push((module.to_owned(), handler));
    }

    pub fn name_of(&self, type_id: TypeId) -> Option<&str> {
        self.names.get(&type_id).map(String::as_str)
    }

    pub fn has_signature(&self, type_id: TypeId) -> bool {
        self.names.contains_key(&type_id)
    }

    pub fn handlers_for(&self, type_id: TypeId) -> Vec<(String, EventHandler)> {
        self.handlers.get(&type_id).cloned().unwrap_or_default()
    }

    pub fn signature_count(&self) -> usize {
        self.names.len()
    }
}
