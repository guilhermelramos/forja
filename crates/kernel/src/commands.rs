//! Commands: stable ids, availability and handlers.

use crate::args::{Args, CommandId};
use crate::error::Result;
use crate::runtime::Ctx;
use serde_json::Value;
use std::sync::Arc;

/// `Fn` (not `FnMut`) so the shell can clone the handler out of the registry
/// before calling it; modules keep mutable state behind their own locks.
pub type CommandHandler =
    Arc<dyn for<'a, 'b> Fn(&'b mut Ctx<'a>, Args) -> Result<Value> + Send + Sync>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Availability {
    #[default]
    Always,
    NeedsProject,
    NeedsBench,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvailabilityContext {
    pub has_project: bool,
    pub has_bench: bool,
}

impl Availability {
    pub fn allows(self, context: AvailabilityContext) -> bool {
        match self {
            Self::Always => true,
            Self::NeedsProject => context.has_project,
            Self::NeedsBench => context.has_bench,
        }
    }
}

/// A value a command needs before it can run. The Anvil asks for these in order
/// instead of failing with a missing-argument error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandField {
    /// Argument name passed to the handler, for example `bench_id`.
    pub arg: &'static str,
    /// Fluent key of the prompt shown above the list or text field.
    pub label_key: &'static str,
    pub kind: CommandFieldKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandFieldKind {
    /// Free text typed in the Anvil search field.
    Text,
    /// One choice from a command result, filtered as the user types.
    Choice {
        /// Command that returns the options, for example `bench.list`.
        command: &'static str,
        /// Key of the option array in that result, for example `benches`.
        list: &'static str,
        /// Field with the visible label, for example `name`.
        label: &'static str,
        /// Field with the value passed to `arg`, for example `bench_id`.
        value: &'static str,
        /// Optional field shown as context on the right (never an internal id).
        detail: Option<&'static str>,
    },
}

/// One command shown in the Bigorna and usable by the CLI.
pub struct CommandSpec {
    pub id: CommandId,
    pub module: String,
    pub title_key: String,
    pub availability: Availability,
    /// Empty when the command can run without arguments.
    pub fields: Vec<CommandField>,
    pub handler: CommandHandler,
}
