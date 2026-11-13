//! The module contract and automatic discovery through `linkme`.
//!
//! A module crate exposes:
//!
//! ```ignore
//! #[forja_kernel::module]
//! static HELLO: fn() -> Box<dyn forja_kernel::Module> = || Box::new(HelloModule);
//! ```
//!
//! The shell links the crate and calls [`discovered_modules`]; no central
//! list of modules exists.

use crate::manifest::ModuleManifest;
use crate::registry::Registrar;

/// One feature of the product. Registration is the only entry point.
pub trait Module: Send {
    /// Metadata used for ordering, settings groups and contract checks.
    fn manifest(&self) -> ModuleManifest;

    /// Registers commands, views, settings, key bindings, events and RPC
    /// methods. Must not do heavy IO: the shell calls this during startup.
    fn register(&mut self, registrar: &mut Registrar<'_>);
}

/// Constructor stored in the distributed slice.
pub type ModuleFactory = fn() -> Box<dyn Module>;

#[linkme::distributed_slice]
pub static MODULE_FACTORIES: [ModuleFactory] = [..];

/// Instantiates every linked module. Order is not guaranteed; the registry
/// sorts by manifest dependencies.
pub fn discovered_modules() -> Vec<Box<dyn Module>> {
    MODULE_FACTORIES.iter().map(|factory| factory()).collect()
}

/// Re-export so `#[forja_kernel::module]` users can refer to the attribute.
pub use forja_kernel_macros::module;
