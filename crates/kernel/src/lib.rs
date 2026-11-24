//! Modular core of Forja.
//!
//! A feature is one crate that implements [`Module`] and publishes a factory
//! with `#[forja_kernel::module]`. The kernel discovers those factories through
//! `linkme`, validates a [`Registry`], then exposes services through [`Ctx`]:
//! commands, UI slots, settings, keyboard shortcuts, i18n, events, RPC methods,
//! background tasks, per-module storage and command execution.
//!
//! The default build has no egui dependency. Enable the `egui` feature to use
//! [`ui_contract::View`]; the registry itself stores views as `Box<dyn Any>`.
//!
//! Tests build a [`Runtime`] with explicit `data_dir` and `user_dir`; no test
//! touches `%APPDATA%` or `~/.forja`.

pub mod args;
pub mod bootstrap;
pub mod cache;
pub mod commands;
pub mod contract;
pub mod error;
pub mod events;
pub mod human_error;
pub mod i18n;
pub mod keymap;
pub mod layout_spec;
pub mod logging;
pub mod manifest;
pub mod module;
pub mod paths;
pub mod process;
pub mod registry;
pub mod rpc;
pub mod runtime;
pub mod settings;
pub mod store;
pub mod tasks;
pub mod test_pick;
pub mod ui;
#[cfg(feature = "egui")]
pub mod ui_contract;

pub use args::{Args, CommandId};
pub use bootstrap::{BootstrapConfig, CopyReport, ResolvedBootstrap, BOOTSTRAP_FILE};
pub use cache::CacheCleanupReport;
pub use commands::{
    Availability, AvailabilityContext, CommandField, CommandFieldKind, CommandSpec,
};
pub use error::{KernelError, Result};
pub use events::{EventBus, EventSignature};
pub use human_error::HumanError;
pub use i18n::{BundleSource, I18n, Locale};
pub use keymap::{
    BindingSource, KeyBinding, KeyChord, KeyConflict, Keymap, KeymapLayer, Scope,
};
pub use layout_spec::{
    LayoutChrome, LayoutError, LayoutNodeSpec, LayoutSpecV1, LocalizedName, SplitDir, TabSpec,
    TabTable, ViewRef,
};
pub use manifest::ModuleManifest;
pub use module::{discovered_modules, module, Module, ModuleFactory, MODULE_FACTORIES};
pub use registry::{build_registry, CacheDirSpec, Registrar, Registry, TickHandler};
pub use runtime::{Ctx, Runtime, RuntimeOptions, StoreHandle};
pub use settings::{
    is_forbidden_project_key, is_project_safe_key, project_settings_path, SettingKind,
    SettingScope, SettingSource, SettingSpec, SettingValue, SettingsChanged, PROJECT_SAFE_KEYS,
    PROJECT_SETTINGS_FILE,
};
pub use store::{LoadResult, Store};
pub use tasks::{TaskId, Tasks};
pub use ui::{AnvilHit, Slot, ViewContribution};

/// Re-export so module crates do not need a direct `forja-protocol` dependency.
pub use forja_protocol;
