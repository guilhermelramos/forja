//! Runtime: owns the validated registry and every service a module can use.
//!
//! The shell creates one `Runtime`, renders modules through `Ctx` and calls
//! `poll_tasks` plus `handle_rpc_line` once per frame. The kernel itself never
//! spawns a window; tests build a runtime with explicit folders.

use crate::args::{Args, CommandId};
use crate::commands::{AvailabilityContext, CommandSpec};
use crate::error::{KernelError, Result};
use crate::events::EventBus;
use crate::i18n::{BundleSource, I18n, Locale, TimeFormat, WeekStart};
use crate::keymap::{BindingSource, Keymap, KeymapLayer};
use crate::logging::{self, LogConfig};
use crate::registry::{build_registry, Registry};
use crate::settings::{
    SettingSource, SettingSpec, SettingValue, Settings, SettingsChanged, SettingsWriter,
};
use crate::store::{LoadResult, Store};
use crate::tasks::{TaskId, Tasks, Wake};
use forja_protocol::{codec, Method, MethodCatalog, ResultSchema, RpcError};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct RuntimeOptions {
    pub data_dir: Option<PathBuf>,
    pub user_dir: Option<PathBuf>,
    pub locale: Option<String>,
    pub wake: Option<Wake>,
}

pub struct Runtime {
    registry: Registry,
    i18n: I18n,
    settings: Settings,
    settings_writer: SettingsWriter,
    keymap: Keymap,
    store: Store,
    data_dir: PathBuf,
    user_dir: PathBuf,
    bootstrap_path: PathBuf,
    storage_restart_pending: bool,
    cache_cleanup_started: bool,
    tasks: Tasks,
    events: EventBus,
    wake: Wake,
    warnings: Vec<String>,
    open_modals: BTreeMap<String, Value>,
    anvil_query: String,
    viewport_focused: bool,
    locale_setting_changed: bool,
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.tasks
            .wait_inflight(std::time::Duration::from_millis(250));
    }
}

impl Runtime {
    /// Validates the registry, loads settings and keymaps and wires events.
    pub fn new(
        modules: Vec<Box<dyn crate::module::Module>>,
        options: RuntimeOptions,
    ) -> Result<Self> {
        let registry = build_registry(modules)?;
        let resolved_bootstrap = if options.data_dir.is_none() || options.user_dir.is_none() {
            Some(crate::bootstrap::resolve())
        } else {
            None
        };
        let data_dir = options.data_dir.clone().unwrap_or_else(|| {
            resolved_bootstrap
                .as_ref()
                .map(|r| r.data_dir.clone())
                .unwrap_or_else(crate::paths::default_data_dir)
        });
        let user_dir = options.user_dir.clone().unwrap_or_else(|| {
            resolved_bootstrap
                .as_ref()
                .map(|r| r.user_dir.clone())
                .unwrap_or_else(crate::paths::default_user_dir)
        });
        let bootstrap_path = {
            let env_data = std::env::var_os("FORJA_DATA_DIR")
                .is_some_and(|value| !value.is_empty());
            if options.data_dir.is_some() || env_data {
                data_dir.join(crate::bootstrap::BOOTSTRAP_FILE)
            } else {
                crate::bootstrap::bootstrap_file_path()
            }
        };
        let locale_setting = options
            .locale
            .clone()
            .or_else(|| std::env::var("FORJA_LOCALE").ok())
            .unwrap_or_else(|| "pt-BR".to_owned());
        let locale = Locale::parse(&locale_setting);

        let mut bundles = vec![BundleSource {
            module: "kernel".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        }];
        bundles.extend_from_slice(registry.bundles());
        let (mut i18n, mut warnings) = I18n::build(&bundles, locale);
        let (settings, setting_warnings) =
            Settings::load(data_dir.join("settings.toml"), registry.settings());
        logging::configure(data_dir.clone(), LogConfig::from_settings(&settings));
        if let Some(resolved) = resolved_bootstrap {
            for warning in resolved.warnings {
                logging::log(logging::LogLevel::Warn, format!("forja: {warning}"));
                warnings.push(warning);
            }
        }
        warnings.extend(setting_warnings);
        i18n.set_fallback(Locale::parse(
            settings
                .get("i18n.language.fallback")
                .and_then(SettingValue::as_str)
                .unwrap_or("en"),
        ));
        i18n.set_time_format(TimeFormat::parse(
            settings
                .get("i18n.time.format")
                .and_then(SettingValue::as_str)
                .unwrap_or("auto"),
        ));
        i18n.set_week_start(WeekStart::parse(
            settings
                .get("i18n.date.week_start")
                .and_then(SettingValue::as_str)
                .unwrap_or("auto"),
        ));

        let mut layers = read_keymap_packs(
            &user_dir.join("keymaps"),
            BindingSource::Pack,
            &mut warnings,
        );
        let legacy_keymap = user_dir.join("keymap.toml");
        if let Some(text) = read_optional(&legacy_keymap, &mut warnings) {
            layers.push(KeymapLayer {
                label: "keymap.toml".to_owned(),
                source: BindingSource::User,
                text,
            });
        }
        let custom_keymap = settings
            .get("keymap.file.custom_path")
            .and_then(SettingValue::as_str)
            .unwrap_or("");
        if let Some(path) = resolve_keymap_path(&user_dir, custom_keymap) {
            if path != legacy_keymap {
                if let Some(text) = read_optional(&path, &mut warnings) {
                    layers.push(KeymapLayer {
                        label: path.display().to_string(),
                        source: BindingSource::User,
                        text,
                    });
                }
            }
        }
        if let Some(text) = read_optional(&data_dir.join("keymap-platform.toml"), &mut warnings) {
            layers.push(KeymapLayer {
                label: "keymap-platform.toml".to_owned(),
                source: BindingSource::Platform,
                text,
            });
        }
        let block_conflicts = settings
            .get("keymap.conflict.policy")
            .and_then(SettingValue::as_str)
            .unwrap_or("warn")
            == "block";
        let (keymap, keymap_warnings) =
            Keymap::build_with_policy(registry.keybinds().to_vec(), layers, block_conflicts);
        warnings.extend(keymap_warnings);
        for conflict in keymap.conflicts() {
            warnings.push(format!(
                "shortcut conflict: {} in scope `{}` is used by `{}` and `{}`",
                conflict.chord, conflict.scope, conflict.first, conflict.second
            ));
        }

        let mut events = EventBus::new();
        events.register_signature("settings.changed", TypeId::of::<SettingsChanged>());
        for signature in registry.event_signatures() {
            events.register_signature(&signature.name, signature.type_id);
        }
        for subscription in registry.subscriptions() {
            events.subscribe(
                &subscription.module,
                subscription.type_id,
                Arc::clone(&subscription.handler),
            );
        }

        let workers = settings
            .get("app.performance.worker_threads")
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
            .unwrap_or(0.0);
        let tasks = Tasks::with_workers(crate::tasks::resolve_workers(workers));
        let wake = options.wake.unwrap_or_else(|| Arc::new(|| {}));
        tasks.set_wake(Arc::clone(&wake));

        let cleanup_days = settings
            .get("app.storage.cleanup_days")
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|n| n as f64)))
            .unwrap_or(30.0)
            .max(0.0)
            .round() as u64;
        let cache_cleanup_started = if cleanup_days > 0 && !registry.cache_dirs().is_empty() {
            let cleanup_root = data_dir.clone();
            let cleanup_dirs = registry.cache_dirs().to_vec();
            tasks.spawn(
                "kernel",
                move || {
                    crate::cache::clean_stale_caches(
                        &cleanup_root,
                        &cleanup_dirs,
                        cleanup_days,
                        std::time::SystemTime::now(),
                    )
                },
                |_cx, report| {
                    if report.files_removed > 0 {
                        logging::log(
                            logging::LogLevel::Info,
                            format!(
                                "forja: cache cleanup removed {} stale file(s) ({} bytes) across {} folder(s)",
                                report.files_removed, report.bytes_reclaimed, report.dirs_scanned
                            ),
                        );
                    }
                },
            );
            true
        } else {
            false
        };

        Ok(Self {
            registry,
            i18n,
            settings,
            settings_writer: SettingsWriter::start(),
            keymap,
            store: Store::new(data_dir.clone()),
            data_dir,
            user_dir,
            bootstrap_path,
            storage_restart_pending: false,
            cache_cleanup_started,
            tasks,
            events,
            wake,
            warnings,
            open_modals: BTreeMap::new(),
            anvil_query: String::new(),
            viewport_focused: true,
            locale_setting_changed: false,
        })
    }

    /// The shell records whether the window is focused at the start of each frame.
    pub fn set_viewport_focused(&mut self, focused: bool) {
        self.viewport_focused = focused;
    }

    pub fn viewport_focused(&self) -> bool {
        self.viewport_focused
    }

    /// Opens a modal view with an empty payload.
    pub fn open_modal(&mut self, view_id: &str) {
        self.open_modal_with(view_id, Value::Null);
    }

    /// Opens a modal view. `payload` is whatever the opener wants the view to read.
    pub fn open_modal_with(&mut self, view_id: &str, payload: Value) {
        self.open_modals.insert(view_id.to_owned(), payload);
    }

    /// Hides a modal view until something opens it again.
    pub fn close_modal(&mut self, view_id: &str) {
        self.open_modals.remove(view_id);
    }

    pub fn modal_is_open(&self, view_id: &str) -> bool {
        self.open_modals.contains_key(view_id)
    }

    /// Payload passed to the latest [`Self::open_modal_with`] for this id.
    pub fn modal_payload(&self, view_id: &str) -> Option<&Value> {
        self.open_modals.get(view_id)
    }

    /// Query text the shell is offering to Bigorna sources this frame.
    pub fn set_anvil_query(&mut self, query: &str) {
        self.anvil_query = query.to_owned();
    }

    pub fn anvil_query(&self) -> &str {
        &self.anvil_query
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    pub fn i18n(&self) -> &I18n {
        &self.i18n
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Changes whenever a setting value really changed; cheap to compare.
    pub fn settings_generation(&self) -> u64 {
        self.settings.generation()
    }

    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Folder Forja writes to (`%APPDATA%\Forja` or `FORJA_DATA_DIR`).
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Folder of user content (`~/.forja` or the runtime override).
    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    /// Path of `bootstrap.toml` updated when `app.storage.*` folders change.
    pub fn bootstrap_path(&self) -> &Path {
        &self.bootstrap_path
    }

    /// Overrides the `bootstrap.toml` destination (primarily for isolated tests).
    pub fn set_bootstrap_path(&mut self, path: PathBuf) {
        self.bootstrap_path = path;
    }

    /// True after `app.storage.data_dir` or `app.storage.content_dir` changed
    /// to a different folder that will take effect on the next startup.
    pub fn storage_restart_pending(&self) -> bool {
        self.storage_restart_pending
    }

    /// True when the startup cache cleanup task was queued.
    pub fn cache_cleanup_started(&self) -> bool {
        self.cache_cleanup_started
    }

    pub fn tasks(&self) -> &Tasks {
        &self.tasks
    }

    pub fn events(&self) -> &EventBus {
        &self.events
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn waker(&self) -> Wake {
        Arc::clone(&self.wake)
    }

    pub fn set_wake(&mut self, wake: Wake) {
        self.tasks.set_wake(Arc::clone(&wake));
        self.wake = wake;
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.i18n.set_locale(locale);
    }

    /// Reads the user and project keymap packs again. The project layer sits
    /// above the user packs and below `~/.forja/keymap.toml`.
    pub fn reload_keymap(&mut self, project: Option<&Path>) {
        let mut warnings = Vec::new();
        let mut layers = read_keymap_packs(
            &self.user_dir.join("keymaps"),
            BindingSource::Pack,
            &mut warnings,
        );
        if let Some(project) = project {
            layers.extend(read_keymap_packs(
                &project.join(".forja").join("keymaps"),
                BindingSource::Project,
                &mut warnings,
            ));
        }
        if let Some(text) = read_optional(&self.user_dir.join("keymap.toml"), &mut warnings) {
            layers.push(KeymapLayer {
                label: "keymap.toml".to_owned(),
                source: BindingSource::User,
                text,
            });
        }
        let custom_keymap = self
            .settings
            .get("keymap.file.custom_path")
            .and_then(SettingValue::as_str)
            .unwrap_or("");
        if let Some(path) = resolve_keymap_path(&self.user_dir, custom_keymap) {
            if path != self.user_dir.join("keymap.toml") {
                if let Some(text) = read_optional(&path, &mut warnings) {
                    layers.push(KeymapLayer {
                        label: path.display().to_string(),
                        source: BindingSource::User,
                        text,
                    });
                }
            }
        }
        if let Some(text) = read_optional(&self.data_dir.join("keymap-platform.toml"), &mut warnings)
        {
            layers.push(KeymapLayer {
                label: "keymap-platform.toml".to_owned(),
                source: BindingSource::Platform,
                text,
            });
        }
        let block_conflicts = self
            .settings
            .get("keymap.conflict.policy")
            .and_then(SettingValue::as_str)
            .unwrap_or("warn")
            == "block";
        let (keymap, keymap_warnings) =
            Keymap::build_with_policy(self.registry.keybinds().to_vec(), layers, block_conflicts);
        warnings.extend(keymap_warnings);
        for conflict in keymap.conflicts() {
            warnings.push(format!(
                "shortcut conflict: {} in scope `{}` is used by `{}` and `{}`",
                conflict.chord, conflict.scope, conflict.first, conflict.second
            ));
        }
        self.keymap = keymap;
        self.warnings.extend(warnings);
    }

    /// Locale tried after the current one when a key is missing (`i18n.language.fallback`).
    pub fn set_fallback_locale(&mut self, locale: Locale) {
        self.i18n.set_fallback(locale);
    }

    pub fn set_setting(&mut self, key: &str, value: SettingValue) -> Result<()> {
        let before = self.settings.generation();
        if matches!(key, "app.storage.data_dir" | "app.storage.content_dir") {
            let Some(raw) = value.as_str() else {
                return Err(KernelError::Settings(format!(
                    "`{key}` expected text, got {}",
                    value.type_name()
                )));
            };
            let default_dir = if key == "app.storage.data_dir" {
                crate::paths::base_default_data_dir()
            } else {
                crate::paths::base_default_user_dir()
            };
            let resolved_target = match crate::bootstrap::update_storage_setting(
                &self.bootstrap_path,
                key,
                raw,
                &default_dir,
            ) {
                Ok(target) => target,
                Err(error) => {
                    logging::log(
                        logging::LogLevel::Warn,
                        format!("forja: rejected `{key}` change: {error}"),
                    );
                    return Err(error);
                }
            };
            self.settings
                .set_memory(key, value, self.registry.settings())?;
            let current_dir = if key == "app.storage.data_dir" {
                &self.data_dir
            } else {
                &self.user_dir
            };
            self.storage_restart_pending =
                self.storage_restart_pending || !crate::bootstrap::same_path(&resolved_target, current_dir);
            self.settings_writer.submit(self.settings.snapshot());
            if self.settings.generation() != before {
                self.notify_settings_changed();
            }
            return Ok(());
        }
        self.settings
            .set_memory(key, value, self.registry.settings())?;
        self.apply_i18n_setting(key);
        if key.starts_with("app.log.") {
            logging::configure(
                self.data_dir.clone(),
                LogConfig::from_settings(&self.settings),
            );
        }
        self.settings_writer.submit(self.settings.snapshot());
        if self.settings.generation() != before {
            self.notify_settings_changed();
        }
        Ok(())
    }

    /// Directory of the currently active project, when one is loaded.
    pub fn active_project(&self) -> Option<&Path> {
        self.settings.active_project()
    }

    /// Switches the active project (or clears it when `None`), reloads its
    /// `<project>\.forja\ajustes.toml` and notifies setting subscribers.
    pub fn set_active_project(&mut self, project_dir: Option<&Path>) {
        let before = self.settings.generation();
        let warnings = self
            .settings
            .load_project(project_dir, self.registry.settings());
        self.warnings.extend(warnings);
        if self.settings.generation() != before {
            self.notify_settings_changed();
        }
    }

    /// Reloads `<project>\.forja\ajustes.toml` for `project_dir` and notifies subscribers.
    pub fn reload_project_settings(&mut self, project_dir: Option<&Path>) {
        self.set_active_project(project_dir);
    }

    /// Validates that `key` is project-safe (`scope = "project"`), updates the
    /// active project's override in memory, queues an atomic write to
    /// `<project>\.forja\ajustes.toml` and notifies subscribers.
    pub fn set_project_setting(&mut self, key: &str, value: SettingValue) -> Result<()> {
        let before = self.settings.generation();
        self.settings
            .set_project_memory(key, value, self.registry.settings())?;
        let snapshot = self.settings.project_snapshot()?;
        self.settings_writer.submit(snapshot);
        if self.settings.generation() != before {
            self.notify_settings_changed();
        }
        Ok(())
    }

    /// Removes a project override for `key` so it falls back to `user > default`,
    /// queues an atomic write to `<project>\.forja\ajustes.toml` and notifies subscribers.
    pub fn unset_project_setting(&mut self, key: &str) -> Result<()> {
        let before = self.settings.generation();
        self.settings
            .unset_project_memory(key, self.registry.settings())?;
        let snapshot = self.settings.project_snapshot()?;
        self.settings_writer.submit(snapshot);
        if self.settings.generation() != before {
            self.notify_settings_changed();
        }
        Ok(())
    }

    fn notify_settings_changed(&mut self) {
        let event = SettingsChanged {
            generation: self.settings.generation(),
            project_dir: self.settings.active_project().map(Path::to_path_buf),
        };
        let _ = self.emit(&event);
        (self.wake)();
    }

    /// Keeps the locale preferences in sync when the settings UI writes them.
    fn apply_i18n_setting(&mut self, key: &str) {
        let text = self
            .settings
            .get(key)
            .and_then(SettingValue::as_str)
            .map(str::to_owned);
        let Some(text) = text else {
            return;
        };
        match key {
            "i18n.language.mode" => {
                self.locale_setting_changed = true;
                match text.as_str() {
                    "en" => self.i18n.set_locale(Locale::En),
                    "pt_BR" => self.i18n.set_locale(Locale::PtBr),
                    _ => {}
                }
            }
            "i18n.language.fallback" => self.i18n.set_fallback(Locale::parse(&text)),
            "i18n.time.format" => self.i18n.set_time_format(TimeFormat::parse(&text)),
            "i18n.date.week_start" => self.i18n.set_week_start(WeekStart::parse(&text)),
            _ => {}
        }
    }

    /// Returns true once after `i18n.language.mode` was updated at runtime so the
    /// shell can release any startup `--locale` lock and apply live switches.
    pub fn take_locale_setting_changed(&mut self) -> bool {
        std::mem::take(&mut self.locale_setting_changed)
    }

    /// Waits for queued setting writes. Tests use it before reading the file;
    /// the shell never needs to.
    pub fn flush_settings(&self) {
        self.settings_writer.flush();
    }

    pub fn ctx(&mut self, module_id: &str) -> Ctx<'_> {
        Ctx {
            runtime: self,
            module_id: module_id.to_owned(),
        }
    }

    /// Instantiates module views and returns the shell's main context helper.
    pub fn is_command_available(&self, id: &str, context: AvailabilityContext) -> bool {
        self.registry
            .command(id)
            .map(|command| command.availability.allows(context))
            .unwrap_or(false)
    }

    pub fn available_commands(&self, context: AvailabilityContext) -> Vec<&CommandSpec> {
        self.registry
            .commands()
            .iter()
            .filter(|command| command.availability.allows(context))
            .collect()
    }

    pub fn run_command(&mut self, id: &str, args: Args) -> Result<Value> {
        let (handler, module_id) = {
            let command = self
                .registry
                .command(id)
                .ok_or_else(|| KernelError::UnknownCommand(id.to_owned()))?;
            (Arc::clone(&command.handler), command.module.clone())
        };
        let mut cx = Ctx {
            runtime: self,
            module_id,
        };
        handler(&mut cx, args).map_err(|error| KernelError::Command {
            id: id.to_owned(),
            message: error.to_string(),
        })
    }

    /// Runs completed task callbacks on the calling (UI) thread.
    pub fn poll_tasks(&mut self) -> usize {
        let completions = self.tasks.take_completed();
        let count = completions.len();
        for completion in completions {
            let mut cx = Ctx {
                runtime: self,
                module_id: completion.module_id,
            };
            (completion.run)(&mut cx);
        }
        count
    }

    pub fn emit<T: 'static>(&mut self, event: &T) -> bool {
        let type_id = TypeId::of::<T>();
        let registered = self.events.has_signature(type_id);
        for (module_id, handler) in self.events.handlers_for(type_id) {
            let mut cx = Ctx {
                runtime: self,
                module_id,
            };
            handler(&mut cx, event);
        }
        registered
    }

    /// Catalog returned by `meta.methods`: module methods plus the meta entry.
    /// Descriptions are resolved with the current locale for CLI/MCP help.
    pub fn method_catalog(&self) -> MethodCatalog {
        let mut methods: Vec<Method> = self
            .registry
            .methods()
            .iter()
            .map(|method| method.schema.clone())
            .collect();
        methods.push(meta_methods_schema());
        for method in &mut methods {
            method.description = self.i18n.t(&method.description_key);
            if let Some(result) = &mut method.result {
                result.description = self.i18n.t(&result.description_key);
            }
        }
        MethodCatalog::new(methods)
    }

    pub fn call_method(
        &mut self,
        name: &str,
        params: Map<String, Value>,
    ) -> std::result::Result<Value, RpcError> {
        if name == "meta.methods" {
            return serde_json::to_value(self.method_catalog())
                .map_err(|error| RpcError::new(codec::INTERNAL_ERROR, error.to_string()));
        }
        let (handler, schema, module_id) = {
            let method = self
                .registry
                .method(name)
                .ok_or_else(|| RpcError::method_not_found(name))?;
            (
                Arc::clone(&method.handler),
                method.schema.clone(),
                method.module.clone(),
            )
        };
        schema
            .check_params(&params)
            .map_err(RpcError::invalid_params)?;
        let mut cx = Ctx {
            runtime: self,
            module_id,
        };
        handler(&mut cx, params)
    }

    /// Dispatches one JSON-RPC line. Returns `None` for notifications.
    pub fn handle_rpc_line(&mut self, line: &str) -> Option<String> {
        let request = match codec::parse(line) {
            Ok(request) => request,
            Err(invalid) => return Some(codec::error_response(invalid.id, &invalid.error)),
        };
        let result = self.call_method(&request.method, request.params);
        let id = request.id?;
        Some(match result {
            Ok(value) => codec::ok_response(&id, value),
            Err(error) => codec::error_response(id, &error),
        })
    }
}

/// Services available to commands, views and handlers.
pub struct Ctx<'a> {
    runtime: &'a mut Runtime,
    module_id: String,
}

impl<'a> Ctx<'a> {
    pub fn module_id(&self) -> &str {
        &self.module_id
    }

    pub fn t(&self, key: &str) -> String {
        self.runtime.i18n.t(key)
    }

    pub fn t_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.runtime.i18n.t_args(key, args)
    }

    pub fn i18n(&self) -> &I18n {
        &self.runtime.i18n
    }

    pub fn tasks(&self) -> &Tasks {
        &self.runtime.tasks
    }

    /// Spawns background work and runs `done` back on the UI thread.
    pub fn spawn<T, W, D>(&self, work: W, done: D) -> TaskId
    where
        T: Send + 'static,
        W: FnOnce() -> T + Send + 'static,
        D: for<'c, 'd> FnOnce(&'d mut Ctx<'c>, T) + Send + 'static,
    {
        let module_id = self.module_id.clone();
        self.runtime.tasks.spawn(&module_id, work, done)
    }

    pub fn store(&self) -> StoreHandle<'_> {
        StoreHandle {
            store: &self.runtime.store,
            module_id: &self.module_id,
        }
    }

    /// Folder Forja writes to (`%APPDATA%\Forja` or `FORJA_DATA_DIR`).
    pub fn data_dir(&self) -> &std::path::Path {
        &self.runtime.data_dir
    }

    /// Folder of user content (`~/.forja` or the runtime override).
    pub fn user_dir(&self) -> &std::path::Path {
        &self.runtime.user_dir
    }

    /// Module-owned cache subfolder inside `data_dir` (`<data>/cache/<module>`).
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime.data_dir.join("cache").join(&self.module_id)
    }

    /// True after `app.storage.*` folders changed and await app restart.
    pub fn storage_restart_pending(&self) -> bool {
        self.runtime.storage_restart_pending()
    }

    /// Asks the window to draw another frame soon.
    pub fn request_repaint(&self) {
        (self.runtime.wake)();
    }

    pub fn settings(&self) -> &Settings {
        &self.runtime.settings
    }

    /// Changes whenever a setting value or active project changed; cheap to compare.
    pub fn settings_generation(&self) -> u64 {
        self.runtime.settings_generation()
    }

    /// Effective setting value (`project > user > default`).
    pub fn setting(&self, key: &str) -> Option<&SettingValue> {
        self.runtime.settings.get(key)
    }

    /// User-level setting value (`user > default`), ignoring project overrides.
    pub fn user_setting(&self, key: &str) -> Option<&SettingValue> {
        self.runtime.settings.user_get(key)
    }

    /// Project override for `key`, or `None` when not overridden by the active project.
    pub fn project_setting(&self, key: &str) -> Option<&SettingValue> {
        self.runtime.settings.project_get(key)
    }

    /// Layer (`Default`, `User` or `Project`) that supplied the effective value for `key`.
    pub fn setting_source(&self, key: &str) -> Option<SettingSource> {
        self.runtime.settings.source_of(key)
    }

    pub fn setting_spec(&self, key: &str) -> Option<&SettingSpec> {
        self.runtime
            .registry
            .settings()
            .iter()
            .find(|spec| spec.key == key)
    }

    pub fn set_setting(&mut self, key: &str, value: SettingValue) -> Result<()> {
        self.runtime.set_setting(key, value)
    }

    pub fn active_project(&self) -> Option<&Path> {
        self.runtime.active_project()
    }

    pub fn set_active_project(&mut self, project_dir: Option<&Path>) {
        self.runtime.set_active_project(project_dir);
    }

    pub fn reload_project_settings(&mut self, project_dir: Option<&Path>) {
        self.runtime.reload_project_settings(project_dir);
    }

    pub fn set_project_setting(&mut self, key: &str, value: SettingValue) -> Result<()> {
        self.runtime.set_project_setting(key, value)
    }

    pub fn unset_project_setting(&mut self, key: &str) -> Result<()> {
        self.runtime.unset_project_setting(key)
    }

    pub fn keymap(&self) -> &Keymap {
        &self.runtime.keymap
    }

    /// Reads the user and project keymap packs again.
    pub fn reload_keymap(&mut self, project: Option<&Path>) {
        self.runtime.reload_keymap(project);
    }

    pub fn emit<T: 'static>(&mut self, event: &T) -> bool {
        self.runtime.emit(event)
    }

    pub fn run_command(&mut self, id: &str, args: Args) -> Result<Value> {
        self.runtime.run_command(id, args)
    }

    pub fn call_method(
        &mut self,
        name: &str,
        params: Map<String, Value>,
    ) -> std::result::Result<Value, RpcError> {
        self.runtime.call_method(name, params)
    }

    pub fn command_available(&self, id: &str, context: AvailabilityContext) -> bool {
        self.runtime.is_command_available(id, context)
    }

    pub fn registry(&self) -> &Registry {
        &self.runtime.registry
    }

    /// Shows the modal registered with `view_id` until [`Self::close_modal`].
    pub fn open_modal(&mut self, view_id: &str) {
        self.runtime.open_modal(view_id);
    }

    /// Same as [`Self::open_modal`], with a JSON payload the view can read back.
    pub fn open_modal_with(&mut self, view_id: &str, payload: Value) {
        self.runtime.open_modal_with(view_id, payload);
    }

    pub fn close_modal(&mut self, view_id: &str) {
        self.runtime.close_modal(view_id);
    }

    pub fn modal_is_open(&self, view_id: &str) -> bool {
        self.runtime.modal_is_open(view_id)
    }

    pub fn modal_payload(&self, view_id: &str) -> Option<&Value> {
        self.runtime.modal_payload(view_id)
    }

    /// The Bigorna query for this frame. Empty when the anvil is closed.
    pub fn anvil_query(&self) -> &str {
        self.runtime.anvil_query()
    }

    /// Whether the shell window was focused at the start of this frame.
    pub fn viewport_focused(&self) -> bool {
        self.runtime.viewport_focused()
    }
}

/// Scoped JSON store for the module that owns this context.
pub struct StoreHandle<'a> {
    store: &'a Store,
    module_id: &'a str,
}

impl StoreHandle<'_> {
    pub fn path(&self) -> PathBuf {
        self.store.path_for(self.module_id)
    }

    pub fn load<T: DeserializeOwned>(&self) -> LoadResult<T> {
        self.store.load(self.module_id)
    }

    pub fn save<T: Serialize>(&self, value: &T) -> std::io::Result<()> {
        self.store.save(self.module_id, value)
    }
}

fn meta_methods_schema() -> Method {
    Method::new("meta.methods", "meta.methods.description")
        .result(ResultSchema::new("meta.methods.result"))
        .idempotent(true)
}

fn read_optional(path: &Path, warnings: &mut Vec<String>) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            None
        }
    }
}

/// Resolves `keymap.file.custom_path`. `~/.forja/...` follows the configured
/// user directory so tests and `FORJA_CONTENT_DIR` stay isolated.
fn resolve_keymap_path(user_dir: &Path, configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if configured.is_empty() {
        return None;
    }
    let normalized = configured.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("~/.forja/") {
        return Some(user_dir.join(rest));
    }
    if normalized == "~/.forja" {
        return Some(user_dir.to_path_buf());
    }
    if let Some(rest) = normalized.strip_prefix("~/") {
        return Some(
            crate::paths::home_dir()
                .unwrap_or_else(|| user_dir.to_path_buf())
                .join(rest),
        );
    }
    let path = Path::new(configured);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        user_dir.join(path)
    })
}

fn read_keymap_packs(
    dir: &Path,
    source: BindingSource,
    warnings: &mut Vec<String>,
) -> Vec<KeymapLayer> {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "toml")
            })
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            warnings.push(format!("{}: {error}", dir.display()));
            return Vec::new();
        }
    };
    paths.sort();
    let mut layers = Vec::new();
    for path in paths {
        match std::fs::read_to_string(&path) {
            Ok(text) => layers.push(KeymapLayer {
                label: path.display().to_string(),
                source,
                text,
            }),
            Err(error) => warnings.push(format!("{}: {error}", path.display())),
        }
    }
    layers
}

/// Convenience for tests and the CLI: run one command by id.
pub fn command_id(id: &str) -> CommandId {
    CommandId::new(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ModuleManifest;
    use crate::module::Module;
    use crate::registry::Registrar;
    use crate::settings::{SettingKind, SettingSpec};
    use crate::ui::Slot;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestModule;

    impl Module for TestModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("test", "test.name", "0.1.0")
        }

        fn register(&mut self, registrar: &mut Registrar<'_>) {
            registrar.i18n(
                "test-name = Test\ntest-command-title = Run test\ntest-greeting = Hello, { $name }!\ntest-setting-label = Volume\ntest-setting-help = How loud.\n",
                "test-name = Teste\ntest-command-title = Rodar teste\ntest-greeting = Olá, { $name }!\ntest-setting-label = Volume\ntest-setting-help = Quão alto.\n",
            );
            registrar.setting(SettingSpec::new(
                "test.audio.volume",
                SettingKind::Integer {
                    min: Some(0),
                    max: Some(10),
                },
                SettingValue::Integer(5),
                "test.setting.label",
                "test.setting.help",
            ));
            registrar.command("test.run", "test.command.title", |cx, args| {
                let name = args.str("name").unwrap_or("mundo");
                Ok(json!({ "message": cx.t_args("test.greeting", &[("name", name)]) }))
            });
            registrar.uses_i18n("test.greeting");
            registrar.view_any(Slot::BenchTab, "test.tab", "test.command.title", 0, || {
                Box::new(String::new())
            });
        }
    }

    struct TempGuard(PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        TempGuard(std::env::temp_dir().join(format!(
            "forja-runtime-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )))
    }

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(TestModule)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime builds")
    }

    #[test]
    fn commands_run_with_i18n_and_settings() {
        let dir = temp_dir("commands");
        let mut runtime = runtime(&dir);
        let value = runtime
            .run_command("test.run", Args::from_map(Map::new()))
            .unwrap();
        assert_eq!(value["message"], "Olá, mundo!");
        let mut args = Map::new();
        args.insert("name".into(), json!("Ana"));
        let value = runtime
            .run_command("test.run", Args::from_map(args))
            .unwrap();
        assert_eq!(value["message"], "Olá, Ana!");
        runtime
            .set_setting("test.audio.volume", SettingValue::Integer(11))
            .expect_err("range is enforced");
        runtime
            .set_setting("test.audio.volume", SettingValue::Integer(8))
            .unwrap();
        runtime.flush_settings();
        assert!(dir.join("settings.toml").exists());
    }

    #[test]
    fn rpc_catalog_and_dispatch_work() {
        let dir = temp_dir("rpc");
        let mut runtime = runtime(&dir);
        let catalog = runtime.call_method("meta.methods", Map::new()).unwrap();
        assert!(catalog["methods"]
            .as_array()
            .unwrap()
            .iter()
            .any(|method| method["name"] == "meta.methods"));
        let error = runtime.call_method("test.nope", Map::new()).unwrap_err();
        assert_eq!(error.code, forja_protocol::codec::METHOD_NOT_FOUND);
        let reply = runtime
            .handle_rpc_line(r#"{"jsonrpc":"2.0","id":7,"method":"meta.methods","params":{}}"#)
            .expect("reply");
        assert!(reply.contains("\"id\":7"));
    }

    struct KeyModule;

    impl Module for KeyModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("keytest", "keytest.name", "0.1.0")
        }

        fn register(&mut self, registrar: &mut Registrar<'_>) {
            registrar.command("keytest.run", "keytest.command", |_, _| Ok(Value::Null));
            registrar.keymap(
                "[[binding]]\nid = \"keytest.run\"\nkey = \"Ctrl+Alt+K\"\nscope = \"global\"\n",
            );
        }
    }

    #[test]
    fn user_keymap_packs_and_the_user_file_override_module_defaults() {
        let dir = temp_dir("keymap");
        let user = dir.join("user");
        std::fs::create_dir_all(user.join("keymaps")).unwrap();
        std::fs::write(
            user.join("keymaps/mine.toml"),
            "[[binding]]\nid = \"keytest.run\"\nkey = \"Ctrl+Shift+K\"\nscope = \"global\"\n",
        )
        .unwrap();
        std::fs::write(
            user.join("keymap.toml"),
            "[[binding]]\nid = \"keytest.run\"\nkey = \"Ctrl+Alt+Shift+K\"\nscope = \"global\"\n",
        )
        .unwrap();
        let runtime = Runtime::new(
            vec![Box::new(KeyModule)],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(user),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .expect("runtime");
        let binding = runtime
            .keymap()
            .bindings()
            .iter()
            .find(|binding| binding.binding.command.as_str() == "keytest.run")
            .expect("binding");
        assert_eq!(binding.binding.chord.to_string(), "Ctrl+Alt+Shift+K");
    }

    #[test]
    fn project_keymap_pack_cannot_bind_builtin_actions() {
        let dir = temp_dir("project-keymap");
        let user = dir.join("user");
        let project = dir.join("project");
        std::fs::create_dir_all(user.join("keymaps")).unwrap();
        std::fs::create_dir_all(project.join(".forja").join("keymaps")).unwrap();
        std::fs::write(
            user.join("keymaps/mine.toml"),
            "[[binding]]\nid = \"keytest.run\"\nkey = \"Ctrl+Shift+K\"\nscope = \"global\"\n",
        )
        .unwrap();
        std::fs::write(
            project.join(".forja/keymaps/project.toml"),
            "[[binding]]\nid = \"keytest.run\"\nkey = \"Ctrl+Alt+P\"\nscope = \"global\"\n\n[[binding]]\nid = \"cmd.project\"\nkey = \"Ctrl+Alt+O\"\nscope = \"global\"\n",
        )
        .unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(KeyModule)],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(user),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .expect("runtime");
        runtime.reload_keymap(Some(&project));
        let binding = runtime
            .keymap()
            .bindings()
            .iter()
            .find(|binding| binding.binding.command.as_str() == "keytest.run")
            .expect("binding");
        assert_eq!(binding.binding.chord.to_string(), "Ctrl+Shift+K");
        assert!(runtime
            .keymap()
            .bindings()
            .iter()
            .any(|binding| binding.binding.command.as_str() == "cmd.project"));
        assert!(runtime
            .warnings()
            .iter()
            .any(|warning| warning.contains("cannot bind `keytest.run`")));
    }

    #[test]
    fn modal_gate_and_top_bar_slot() {
        let dir = temp_dir("modal");
        let mut runtime = runtime(&dir);
        assert_eq!(Slot::from_id("top_bar"), Some(Slot::TopBar));
        assert!(!runtime.modal_is_open("onboarding.wizard"));
        runtime.open_modal("onboarding.wizard");
        assert!(runtime.modal_is_open("onboarding.wizard"));
        assert_eq!(
            runtime.modal_payload("onboarding.wizard"),
            Some(&Value::Null)
        );
        runtime.open_modal_with("onboarding.wizard", json!({"step": 1}));
        assert_eq!(
            runtime
                .modal_payload("onboarding.wizard")
                .and_then(|value| value.get("step"))
                .and_then(Value::as_i64),
            Some(1)
        );
        runtime.close_modal("onboarding.wizard");
        assert!(!runtime.modal_is_open("onboarding.wizard"));
        runtime.set_anvil_query("main");
        assert_eq!(runtime.anvil_query(), "main");
    }

    #[test]
    fn tasks_return_to_the_ui_thread_with_module_context() {
        let dir = temp_dir("tasks");
        let mut runtime = runtime(&dir);
        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || {
            let _ = wake_tx.send(());
        }));
        let done = Arc::new(AtomicBool::new(false));
        let signal = Arc::clone(&done);
        let ctx = runtime.ctx("test");
        ctx.spawn(
            move || 21 + 21,
            move |cx, value| {
                assert_eq!(cx.module_id(), "test");
                assert_eq!(value, 42);
                signal.store(true, Ordering::SeqCst);
            },
        );
        while !done.load(Ordering::SeqCst) {
            runtime.poll_tasks();
            if !done.load(Ordering::SeqCst) {
                wake_rx
                    .recv_timeout(std::time::Duration::from_secs(60))
                    .expect("task did not signal completion");
            }
        }
    }

    #[test]
    fn language_mode_setting_updates_locale_live() {
        struct I18nTestModule;
        impl Module for I18nTestModule {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("i18n", "i18n.name", "0.1.0")
            }
            fn register(&mut self, registrar: &mut Registrar<'_>) {
                registrar.i18n("i18n-name = Language\n", "i18n-name = Idioma\n");
                registrar.setting(SettingSpec::new(
                    "i18n.language.mode",
                    SettingKind::Choice {
                        options: vec!["auto".into(), "pt_BR".into(), "en".into()],
                    },
                    SettingValue::Text("auto".into()),
                    "i18n.name",
                    "i18n.name",
                ));
            }
        }
        let dir = temp_dir("live-locale");
        let mut runtime = Runtime::new(
            vec![Box::new(I18nTestModule)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime builds");
        assert!(!runtime.take_locale_setting_changed());
        assert_eq!(runtime.i18n().locale(), Locale::PtBr);

        runtime
            .set_setting("i18n.language.mode", SettingValue::Text("en".into()))
            .expect("switch to en");
        assert!(runtime.take_locale_setting_changed());
        assert!(!runtime.take_locale_setting_changed());
        assert_eq!(runtime.i18n().locale(), Locale::En);

        runtime
            .set_setting("i18n.language.mode", SettingValue::Text("pt_BR".into()))
            .expect("switch back to pt_BR");
        assert!(runtime.take_locale_setting_changed());
        assert_eq!(runtime.i18n().locale(), Locale::PtBr);
    }

    #[test]
    fn runtime_project_settings_precedence_async_write_and_change_notifications() {
        use crate::settings::{project_settings_path, SettingScope};
        use std::sync::atomic::AtomicU64;

        let seen_generations = Arc::new(AtomicU64::new(0));
        let seen_clone = Arc::clone(&seen_generations);

        struct ProjSettingsModule {
            seen: Arc<AtomicU64>,
        }

        impl Module for ProjSettingsModule {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("projtest", "projtest.name", "0.1.0")
            }

            fn register(&mut self, registrar: &mut Registrar<'_>) {
                registrar.i18n("projtest-name = ProjTest\n", "projtest-name = ProjTest\n");
                registrar.setting(
                    SettingSpec::new(
                        "projtest.format.indent",
                        SettingKind::Integer {
                            min: Some(1),
                            max: Some(8),
                        },
                        SettingValue::Integer(4),
                        "projtest.name",
                        "projtest.name",
                    )
                    .scope(SettingScope::Project),
                );
                registrar.setting(SettingSpec::new(
                    "projtest.security.token",
                    SettingKind::Text { max_len: None },
                    SettingValue::Text("none".into()),
                    "projtest.name",
                    "projtest.name",
                ));
                let seen = Arc::clone(&self.seen);
                registrar.subscribe(move |_cx, event: &SettingsChanged| {
                    seen.store(event.generation, Ordering::SeqCst);
                });
            }
        }

        let dir = temp_dir("project-settings");
        let proj_a = dir.join("proj-a");
        let proj_b = dir.join("proj-b");
        std::fs::create_dir_all(proj_b.join(".forja")).unwrap();
        std::fs::write(
            project_settings_path(&proj_b),
            "projtest.format.indent = 2\nprojtest.security.token = \"stolen\"\n",
        )
        .unwrap();

        let mut runtime = Runtime::new(
            vec![Box::new(ProjSettingsModule { seen: seen_clone })],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime builds");

        // Writing project setting without an active project fails with NoActiveProject.
        let err = runtime
            .set_project_setting("projtest.format.indent", SettingValue::Integer(6))
            .expect_err("no project active");
        assert!(err.is_no_active_project());

        // User sets 6 (user > default 4).
        runtime
            .set_setting("projtest.format.indent", SettingValue::Integer(6))
            .unwrap();
        assert_eq!(
            runtime.settings().get("projtest.format.indent"),
            Some(&SettingValue::Integer(6))
        );
        assert_eq!(
            seen_generations.load(Ordering::SeqCst),
            runtime.settings_generation()
        );

        // Activate project A and write project override 8 asynchronously.
        runtime.set_active_project(Some(&proj_a));
        assert_eq!(runtime.active_project(), Some(proj_a.as_path()));
        let err = runtime
            .set_project_setting(
                "projtest.security.token",
                SettingValue::Text("secret".into()),
            )
            .expect_err("user-only setting refused");
        assert!(err.is_unsafe_project_setting());

        runtime
            .set_project_setting("projtest.format.indent", SettingValue::Integer(8))
            .unwrap();
        runtime.flush_settings();
        assert!(project_settings_path(&proj_a).is_file());
        assert_eq!(
            runtime.settings().get("projtest.format.indent"),
            Some(&SettingValue::Integer(8))
        );
        assert_eq!(
            seen_generations.load(Ordering::SeqCst),
            runtime.settings_generation()
        );

        // Switch to project B: loads indent = 2, ignores unsafe security.token with warning.
        let gen_before_b = runtime.settings_generation();
        runtime.set_active_project(Some(&proj_b));
        assert!(runtime.settings_generation() > gen_before_b);
        assert_eq!(
            seen_generations.load(Ordering::SeqCst),
            runtime.settings_generation()
        );
        assert_eq!(
            runtime.settings().get("projtest.format.indent"),
            Some(&SettingValue::Integer(2))
        );
        assert_eq!(
            runtime.settings().get("projtest.security.token"),
            Some(&SettingValue::Text("none".into()))
        );
        assert!(runtime
            .warnings()
            .iter()
            .any(|w| w.contains("projtest.security.token")));
    }

    #[test]
    fn storage_settings_write_bootstrap_toml_and_mark_restart_without_moving_data() {
        struct AppStorageStub;
        impl Module for AppStorageStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("app", "app.name", "0.1.0")
            }
            fn register(&mut self, registrar: &mut Registrar<'_>) {
                registrar.i18n("app-name = App\n", "app-name = App\n");
                registrar.setting(SettingSpec::new(
                    "app.storage.data_dir",
                    SettingKind::Path,
                    SettingValue::Text("%APPDATA%\\Forja".into()),
                    "app.name",
                    "app.name",
                ));
                registrar.setting(SettingSpec::new(
                    "app.storage.content_dir",
                    SettingKind::Path,
                    SettingValue::Text("~/.forja".into()),
                    "app.name",
                    "app.name",
                ));
            }
        }

        let dir = temp_dir("storage-bootstrap");
        let old_data = dir.join("old-data");
        let old_user = dir.join("old-user");
        let new_data = dir.join("new-data");
        let new_user = dir.join("new-user");
        let bootstrap = dir.join("bootstrap.toml");
        std::fs::create_dir_all(&old_data).unwrap();
        std::fs::write(old_data.join("existing.txt"), "keep-me").unwrap();

        let mut runtime = Runtime::new(
            vec![Box::new(AppStorageStub)],
            RuntimeOptions {
                data_dir: Some(old_data.clone()),
                user_dir: Some(old_user),
                locale: Some("pt-BR".into()),
                wake: None,
            },
        )
        .expect("runtime builds");
        runtime.set_bootstrap_path(bootstrap.clone());
        assert!(!runtime.storage_restart_pending());

        // Relative path is refused without updating bootstrap.toml.
        runtime
            .set_setting(
                "app.storage.data_dir",
                SettingValue::Text("relative/folder".into()),
            )
            .expect_err("relative path must be refused");
        assert!(!bootstrap.exists());
        assert!(!runtime.storage_restart_pending());

        // Valid paths update bootstrap.toml for next startup without moving existing files.
        runtime
            .set_setting(
                "app.storage.data_dir",
                SettingValue::Text(new_data.display().to_string()),
            )
            .unwrap();
        runtime
            .set_setting(
                "app.storage.content_dir",
                SettingValue::Text(new_user.display().to_string()),
            )
            .unwrap();
        assert!(runtime.storage_restart_pending());
        assert!(old_data.join("existing.txt").is_file());
        assert!(!new_data.join("existing.txt").exists());

        let config = crate::bootstrap::load_config_optional(&bootstrap).unwrap();
        assert_eq!(config.data_dir, Some(new_data.display().to_string()));
        assert_eq!(config.content_dir, Some(new_user.display().to_string()));
    }

    #[test]
    fn startup_cache_cleanup_runs_in_background_for_declared_dirs() {
        struct CacheModule;
        impl Module for CacheModule {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("app", "app.name", "0.1.0")
            }
            fn register(&mut self, registrar: &mut Registrar<'_>) {
                registrar.i18n("app-name = App\n", "app-name = App\n");
                registrar.setting(SettingSpec::new(
                    "app.storage.cleanup_days",
                    SettingKind::Integer {
                        min: Some(0),
                        max: Some(365),
                    },
                    SettingValue::Integer(30),
                    "app.name",
                    "app.name",
                ));
                registrar.cache_dir("cache/app");
            }
        }

        let dir = temp_dir("cache-cleanup");
        let data = dir.join("data");
        let cache = data.join("cache").join("app");
        let modules_dir = data.join("modules");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(&modules_dir).unwrap();

        let old_cache = cache.join("stale.cache");
        let fresh_cache = cache.join("fresh.cache");
        let user_state = modules_dir.join("app.json");
        std::fs::write(&old_cache, b"old").unwrap();
        std::fs::write(&fresh_cache, b"fresh").unwrap();
        std::fs::write(&user_state, b"{}").unwrap();

        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(45 * 86_400);
        let file = std::fs::OpenOptions::new().write(true).open(&old_cache).unwrap();
        file.set_modified(old_time).unwrap();
        drop(file);

        let (wake_tx, wake_rx) = std::sync::mpsc::channel();
        let mut runtime = Runtime::new(
            vec![Box::new(CacheModule)],
            RuntimeOptions {
                data_dir: Some(data.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".into()),
                wake: Some(Arc::new(move || {
                    let _ = wake_tx.send(());
                })),
            },
        )
        .expect("runtime builds");
        assert!(runtime.cache_cleanup_started());

        for _ in 0..50 {
            runtime.poll_tasks();
            if !old_cache.exists() {
                break;
            }
            let _ = wake_rx.recv_timeout(std::time::Duration::from_millis(50));
        }
        assert!(!old_cache.exists(), "stale cache file must be removed");
        assert!(fresh_cache.exists(), "fresh cache file must remain");
        assert!(user_state.exists(), "user/module data must never be deleted");

        // When cleanup_days = 0, startup cleanup does not run.
        let dir_zero = temp_dir("cache-cleanup-zero");
        let data_zero = dir_zero.join("data");
        std::fs::create_dir_all(&data_zero).unwrap();
        std::fs::write(
            data_zero.join("settings.toml"),
            "[app.storage]\ncleanup_days = 0\n",
        )
        .unwrap();
        let runtime_zero = Runtime::new(
            vec![Box::new(CacheModule)],
            RuntimeOptions {
                data_dir: Some(data_zero),
                user_dir: Some(dir_zero.join("user")),
                locale: Some("en".into()),
                wake: None,
            },
        )
        .expect("runtime builds");
        assert!(!runtime_zero.cache_cleanup_started());
    }

    #[test]
    fn escaping_cache_dir_is_rejected_at_registration() {
        struct BadCacheModule;
        impl Module for BadCacheModule {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("badcache", "badcache.name", "0.1.0")
            }
            fn register(&mut self, registrar: &mut Registrar<'_>) {
                registrar.cache_dir("../outside");
            }
        }
        let err = build_registry(vec![Box::new(BadCacheModule)])
            .err()
            .expect("escaping cache_dir must fail");
        assert!(err.to_string().contains("invalid"));
    }
}
