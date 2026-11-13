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

/// One command shown in the Bigorna and usable by the CLI.
pub struct CommandSpec {
    pub id: CommandId,
    pub module: String,
    pub title_key: String,
    pub availability: Availability,
    pub handler: CommandHandler,
}
