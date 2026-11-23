//! Settings schema and persistence in `%APPDATA%\Forja\settings.toml` and
//! per-project overrides in `<project>\.forja\ajustes.toml`.
//!
//! Keys follow `<module>.<group>.<name>` in snake_case. Modules declare the
//! schema; the runtime resolves values with precedence `project > user > default`
//! and writes only project-safe keys (`scope = "project"`) to the project file.

use crate::error::{KernelError, Result};
use crate::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Relative file name inside `<project>\.forja\`.
pub const PROJECT_SETTINGS_FILE: &str = "ajustes.toml";

/// Returns `<project_dir>\.forja\ajustes.toml`.
pub fn project_settings_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".forja").join(PROJECT_SETTINGS_FILE)
}

/// Catalog keys that may be overridden in `<project>\.forja\ajustes.toml`.
///
/// Security, network, credential, permission, updater, storage-path and
/// global window/performance settings are never included here.
pub const PROJECT_SAFE_KEYS: &[&str] = &[
    "bench.worktree.name_template",
    "bench.worktree.branch_prefix",
    "bench.worktree.base_branch",
    "bench.layout.tabs",
    "bench.stage.creation_default",
    "git.commit.sign_off",
    "terminal.shell.start_dir",
    "terminal.cursor.style",
    "terminal.font.family",
    "terminal.font.size",
    "terminal.theme.name",
    "terminal.theme.follow_app",
    "terminal.color.opacity",
    "terminal.recipes.show_scope",
    "terminal.recipes.shell",
    "editor.indent.auto_detect",
    "editor.indent.size",
    "editor.indent.tabs",
    "editor.wrap.mode",
    "editor.wrap.column",
    "editor.save.format",
    "editor.save.trim_trailing",
    "editor.save.final_newline",
    "editor.files.encoding",
    "editor.files.eol",
    "helper.default.profile",
    "request.default.source",
    "request.local.prefix",
    "counter.github.label_filter",
    "counter.linear.team",
    "counter.jira.project",
];

/// Returns true when `key` is in the catalog's project-safe list.
pub fn is_project_safe_key(key: &str) -> bool {
    PROJECT_SAFE_KEYS.contains(&key)
}

/// Returns true for keys that must never be overridden by a project file
/// (security, permissions, MCP approvals, network/proxy, updater, data paths,
/// window geometry, performance, extensions sandbox, etc.).
pub fn is_forbidden_project_key(key: &str) -> bool {
    const FORBIDDEN_PREFIXES: &[&str] = &[
        "mcp.",
        "network.",
        "privacy.",
        "update.",
        "app.",
        "appearance.window.",
        "extension.",
        "remote.",
        "accounts.",
        "usage.",
        "voice.",
        "notifications.",
        "i18n.",
        "keymap.",
        "anvil.",
        "ports.",
        "browser.",
        "content.trust.",
    ];
    const FORBIDDEN_KEYS: &[&str] = &[
        "helper.default.permissions",
        "helper.hooks.enabled",
        "helper.hooks.timeout_s",
        "bench.worktree.location",
        "bench.worktree.path",
        "bench.worktree.auto_prune_days",
        "bench.setup.policy",
        "bench.teardown.policy",
        "terminal.shell.default",
        "terminal.shell.path",
        "terminal.shell.env",
        "terminal.osc52.enabled",
        "terminal.paste.warn_multiline",
        "terminal.paste.bracketed",
        "terminal.links.confirm",
        "terminal.recipes.confirm",
        "terminal.recipes.press_enter",
        "git.commit.sign",
        "git.commit.gpg_key",
        "git.sync.auto_pull",
        "git.protected.branches",
        "explorer.tree.follow_symlinks",
        "counter.github.host",
        "counter.gitlab.host",
    ];
    FORBIDDEN_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
        || FORBIDDEN_KEYS.contains(&key)
}

/// Where a setting may be configured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingScope {
    /// Global user setting stored only in `%APPDATA%\Forja\settings.toml`.
    #[default]
    User,
    /// May be overridden per project in `<project>\.forja\ajustes.toml`.
    Project,
}

impl SettingScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

/// Layer that supplied the current effective value of a setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    Default,
    User,
    Project,
}

/// Event emitted whenever effective settings change (user edit, project edit
/// or active project switch).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsChanged {
    pub generation: u64,
    pub project_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SettingValue {
    Bool(bool),
    Integer(i64),
    Number(f64),
    Text(String),
    List(Vec<String>),
}

impl SettingValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::Number(_) => "number",
            Self::Text(_) => "text",
            Self::List(_) => "list",
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(value) => Some(value),
            _ => None,
        }
    }

    fn to_toml(&self) -> toml::Value {
        match self {
            Self::Bool(value) => toml::Value::Boolean(*value),
            Self::Integer(value) => toml::Value::Integer(*value),
            Self::Number(value) => toml::Value::Float(*value),
            Self::Text(value) => toml::Value::String(value.clone()),
            Self::List(values) => {
                toml::Value::Array(values.iter().cloned().map(toml::Value::String).collect())
            }
        }
    }

    fn from_toml(value: &toml::Value) -> Option<Self> {
        match value {
            toml::Value::Boolean(value) => Some(Self::Bool(*value)),
            toml::Value::Integer(value) => Some(Self::Integer(*value)),
            toml::Value::Float(value) => Some(Self::Number(*value)),
            toml::Value::String(value) => Some(Self::Text(value.clone())),
            toml::Value::Array(values) => {
                let mut list = Vec::with_capacity(values.len());
                for value in values {
                    list.push(value.as_str()?.to_owned());
                }
                Some(Self::List(list))
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingKind {
    #[serde(rename = "toggle")]
    Bool,
    Integer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    Number {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
    },
    Text {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_len: Option<usize>,
    },
    Choice {
        options: Vec<String>,
    },
    Path,
    List {
        #[serde(default)]
        item: String,
    },
    Color,
}

impl SettingKind {
    pub fn validate(&self, value: &SettingValue) -> std::result::Result<(), String> {
        match (self, value) {
            (Self::Bool, SettingValue::Bool(_)) => Ok(()),
            (Self::Integer { min, max }, SettingValue::Integer(value)) => {
                if let Some(min) = min {
                    if value < min {
                        return Err(format!("must be at least {min}"));
                    }
                }
                if let Some(max) = max {
                    if value > max {
                        return Err(format!("must be at most {max}"));
                    }
                }
                Ok(())
            }
            (Self::Number { min, max }, SettingValue::Number(value)) => {
                if let Some(min) = min {
                    if value < min {
                        return Err(format!("must be at least {min}"));
                    }
                }
                if let Some(max) = max {
                    if value > max {
                        return Err(format!("must be at most {max}"));
                    }
                }
                Ok(())
            }
            (Self::Text { max_len }, SettingValue::Text(value)) => {
                if let Some(max_len) = max_len {
                    if value.chars().count() > *max_len {
                        return Err(format!("must have at most {max_len} characters"));
                    }
                }
                Ok(())
            }
            (Self::Choice { options }, SettingValue::Text(value)) => {
                if options.iter().any(|option| option == value) {
                    Ok(())
                } else {
                    Err(format!("must be one of: {}", options.join(", ")))
                }
            }
            (Self::Path, SettingValue::Text(_)) => Ok(()),
            (Self::Color, SettingValue::Text(value)) => {
                let valid = value.len() == 7
                    && value.starts_with('#')
                    && value[1..].chars().all(|c| c.is_ascii_hexdigit());
                if valid {
                    Ok(())
                } else {
                    Err("must be a color in #RRGGBB form".to_owned())
                }
            }
            (Self::List { item }, SettingValue::List(values)) => {
                let mut seen = BTreeSet::new();
                for value in values {
                    if value.trim().is_empty() {
                        return Err(format!("list items cannot be empty ({item})"));
                    }
                    if !seen.insert(value) {
                        return Err(format!("list repeats `{value}`"));
                    }
                }
                Ok(())
            }
            (kind, value) => Err(format!(
                "expected {}, got {}",
                kind.name(),
                value.type_name()
            )),
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Bool => "toggle",
            Self::Integer { .. } => "integer",
            Self::Number { .. } => "number",
            Self::Text { .. } => "text",
            Self::Choice { .. } => "choice",
            Self::Path => "path",
            Self::List { .. } => "list",
            Self::Color => "color",
        }
    }
}

/// One setting declared by a module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingSpec {
    pub key: String,
    pub kind: SettingKind,
    pub default: SettingValue,
    pub label_key: String,
    pub help_key: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default)]
    pub scope: SettingScope,
}

impl SettingSpec {
    pub fn new(
        key: impl Into<String>,
        kind: SettingKind,
        default: SettingValue,
        label_key: impl Into<String>,
        help_key: impl Into<String>,
    ) -> Self {
        let key = key.into();
        let scope = if is_project_safe_key(&key) {
            SettingScope::Project
        } else {
            SettingScope::User
        };
        Self {
            key,
            kind,
            default,
            label_key: label_key.into(),
            help_key: help_key.into(),
            order: 0,
            scope,
        }
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn scope(mut self, scope: SettingScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn allows_project_override(&self) -> bool {
        self.scope == SettingScope::Project && !is_forbidden_project_key(&self.key)
    }

    pub fn module(&self) -> &str {
        self.key.split('.').next().unwrap_or("")
    }

    pub fn group(&self) -> &str {
        self.key.split('.').nth(1).unwrap_or("")
    }

    /// Structural validation used at finalization time.
    pub fn validate(&self) -> std::result::Result<(), String> {
        let segments: Vec<&str> = self.key.split('.').collect();
        if segments.len() != 3 {
            return Err(format!(
                "setting key `{}` must have exactly <module>.<group>.<name>",
                self.key
            ));
        }
        for segment in &segments {
            if segment.is_empty()
                || !segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                return Err(format!(
                    "setting key `{}` has an invalid segment `{segment}`",
                    self.key
                ));
            }
        }
        if self.label_key.trim().is_empty() || self.help_key.trim().is_empty() {
            return Err(format!("setting `{}` needs label and help keys", self.key));
        }
        if self.scope == SettingScope::Project && is_forbidden_project_key(&self.key) {
            return Err(format!(
                "setting `{}` cannot use project scope",
                self.key
            ));
        }
        self.kind
            .validate(&self.default)
            .map_err(|reason| format!("setting `{}` default is invalid: {reason}", self.key))?;
        Ok(())
    }

    pub fn validate_value(&self, value: &SettingValue) -> Result<()> {
        self.kind
            .validate(value)
            .map_err(|reason| KernelError::Settings(format!("`{}` {reason}", self.key)))
    }
}

/// Values currently in memory across default, user and project layers.
#[derive(Debug)]
pub struct Settings {
    path: PathBuf,
    defaults: BTreeMap<String, SettingValue>,
    user_values: BTreeMap<String, SettingValue>,
    user_explicit: BTreeSet<String>,
    original: toml::Table,
    project_dir: Option<PathBuf>,
    project_values: BTreeMap<String, SettingValue>,
    project_original: toml::Table,
    values: BTreeMap<String, SettingValue>,
    generation: u64,
}

impl Settings {
    /// Loads the user file, falls back to schema defaults and reports invalid or
    /// unknown entries as warnings instead of failing startup.
    pub fn load(path: PathBuf, specs: &[SettingSpec]) -> (Self, Vec<String>) {
        let default_map = defaults(specs);
        let mut user_values = default_map.clone();
        let mut user_explicit = BTreeSet::new();
        let mut warnings = Vec::new();
        let mut original = toml::Table::new();
        if let Ok(text) = std::fs::read_to_string(&path) {
            match toml::from_str::<toml::Table>(&text) {
                Ok(table) => {
                    let mut flat = BTreeMap::new();
                    flatten_table("", &table, &mut flat);
                    for (key, value) in flat {
                        let Some(spec) = specs.iter().find(|spec| spec.key == key) else {
                            warnings.push(format!("settings.toml: unknown setting `{key}` ignored"));
                            continue;
                        };
                        let Some(value) = SettingValue::from_toml(&value) else {
                            warnings.push(format!(
                                "settings.toml: `{key}` has an unsupported value type"
                            ));
                            continue;
                        };
                        if let Err(reason) = spec.validate_value(&value) {
                            warnings.push(format!(
                                "settings.toml: `{key}` ignored ({reason}); using default"
                            ));
                            continue;
                        }
                        user_explicit.insert(key.clone());
                        user_values.insert(key, value);
                    }
                    original = table;
                }
                Err(error) => warnings.push(format!("settings.toml could not be parsed: {error}")),
            }
        }
        let values = user_values.clone();
        (
            Self {
                path,
                defaults: default_map,
                user_values,
                user_explicit,
                original,
                project_dir: None,
                project_values: BTreeMap::new(),
                project_original: toml::Table::new(),
                values,
                generation: 0,
            },
            warnings,
        )
    }

    /// Switches the active project (or clears it when `None`), reloads
    /// `<project>\.forja\ajustes.toml` and recomputes effective settings with
    /// precedence `project > user > default`.
    ///
    /// Keys outside the project-safe list and invalid files or values never
    /// fail: they are ignored, fall back to the user value and log a warning.
    pub fn load_project(
        &mut self,
        project_dir: Option<&Path>,
        specs: &[SettingSpec],
    ) -> Vec<String> {
        let next_dir = project_dir.map(Path::to_path_buf);
        let dir_changed = self.project_dir != next_dir;
        self.project_dir = next_dir;
        self.project_values.clear();
        self.project_original = toml::Table::new();
        let mut warnings = Vec::new();

        if let Some(dir) = project_dir {
            let path = project_settings_path(dir);
            match std::fs::read_to_string(&path) {
                Ok(text) => match toml::from_str::<toml::Table>(&text) {
                    Ok(table) => {
                        let mut flat = BTreeMap::new();
                        flatten_table("", &table, &mut flat);
                        for (key, raw_value) in flat {
                            let Some(spec) = specs.iter().find(|spec| spec.key == key) else {
                                record_project_warning(
                                    &mut warnings,
                                    format!("{PROJECT_SETTINGS_FILE}: unknown setting `{key}` ignored"),
                                );
                                continue;
                            };
                            if !spec.allows_project_override() {
                                record_project_warning(
                                    &mut warnings,
                                    format!(
                                        "{PROJECT_SETTINGS_FILE}: setting `{key}` is not allowed in project scope and was ignored"
                                    ),
                                );
                                continue;
                            }
                            let Some(value) = SettingValue::from_toml(&raw_value) else {
                                record_project_warning(
                                    &mut warnings,
                                    format!(
                                        "{PROJECT_SETTINGS_FILE}: `{key}` has an unsupported value type; using user value"
                                    ),
                                );
                                continue;
                            };
                            if let Err(reason) = spec.validate_value(&value) {
                                record_project_warning(
                                    &mut warnings,
                                    format!(
                                        "{PROJECT_SETTINGS_FILE}: `{key}` ignored ({reason}); using user value"
                                    ),
                                );
                                continue;
                            }
                            let _ = insert_dotted(&mut self.project_original, &key, value.to_toml());
                            self.project_values.insert(key, value);
                        }
                    }
                    Err(error) => record_project_warning(
                        &mut warnings,
                        format!("{PROJECT_SETTINGS_FILE} could not be parsed: {error}"),
                    ),
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => record_project_warning(
                    &mut warnings,
                    format!(
                        "{PROJECT_SETTINGS_FILE} could not be read ({}): {error}",
                        path.display()
                    ),
                ),
            }
        }

        let mut effective = self.user_values.clone();
        for (key, value) in &self.project_values {
            effective.insert(key.clone(), value.clone());
        }
        if self.values != effective || dir_changed {
            self.values = effective;
            self.generation += 1;
        }
        warnings
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn active_project(&self) -> Option<&Path> {
        self.project_dir.as_deref()
    }

    pub fn project_path(&self) -> Option<PathBuf> {
        self.project_dir.as_deref().map(project_settings_path)
    }

    /// Effective setting value (`project > user > default`).
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key)
    }

    pub fn get_or<'a>(&'a self, key: &str, fallback: &'a SettingValue) -> &'a SettingValue {
        self.values.get(key).unwrap_or(fallback)
    }

    /// Effective setting values (`project > user > default`).
    pub fn values(&self) -> &BTreeMap<String, SettingValue> {
        &self.values
    }

    /// User-level setting value (`user > default`), ignoring any active project override.
    pub fn user_get(&self, key: &str) -> Option<&SettingValue> {
        self.user_values.get(key)
    }

    pub fn user_values(&self) -> &BTreeMap<String, SettingValue> {
        &self.user_values
    }

    /// Project override for `key`, or `None` when the active project does not override it.
    pub fn project_get(&self, key: &str) -> Option<&SettingValue> {
        self.project_values.get(key)
    }

    pub fn project_values(&self) -> &BTreeMap<String, SettingValue> {
        &self.project_values
    }

    /// Schema default for `key`.
    pub fn default_get(&self, key: &str) -> Option<&SettingValue> {
        self.defaults.get(key)
    }

    /// Layer that supplied the current effective value for `key`.
    pub fn source_of(&self, key: &str) -> Option<SettingSource> {
        if self.project_values.contains_key(key) {
            Some(SettingSource::Project)
        } else if self.user_explicit.contains(key) {
            Some(SettingSource::User)
        } else if self.defaults.contains_key(key) {
            Some(SettingSource::Default)
        } else {
            None
        }
    }

    /// Counter bumped every time a value or active project changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Validates against `specs`, updates user memory and writes `settings.toml`
    /// synchronously. The shell uses [`Settings::set_memory`] plus
    /// [`SettingsWriter`] to keep disk work off the UI thread.
    pub fn set(&mut self, key: &str, value: SettingValue, specs: &[SettingSpec]) -> Result<()> {
        self.set_memory(key, value, specs)?;
        self.save()
    }

    pub fn set_memory(
        &mut self,
        key: &str,
        value: SettingValue,
        specs: &[SettingSpec],
    ) -> Result<()> {
        let spec = specs
            .iter()
            .find(|spec| spec.key == key)
            .ok_or_else(|| KernelError::Settings(format!("unknown setting `{key}`")))?;
        spec.validate_value(&value)?;
        self.user_explicit.insert(key.to_owned());
        let user_changed = self.user_values.get(key) != Some(&value);
        if user_changed {
            self.user_values.insert(key.to_owned(), value.clone());
        }
        let effective_changed = if !self.project_values.contains_key(key) {
            if self.values.get(key) != Some(&value) {
                self.values.insert(key.to_owned(), value);
                true
            } else {
                false
            }
        } else {
            false
        };
        if user_changed || effective_changed {
            self.generation += 1;
        }
        Ok(())
    }

    /// Validates that `key` is project-safe (`scope = "project"`), updates the
    /// active project's overrides in memory and writes `<project>\.forja\ajustes.toml`
    /// atomically.
    pub fn set_project(
        &mut self,
        key: &str,
        value: SettingValue,
        specs: &[SettingSpec],
    ) -> Result<()> {
        self.set_project_memory(key, value, specs)?;
        self.save_project()
    }

    /// Validates that `key` is project-safe (`scope = "project"`) and updates
    /// the active project's override in memory without touching disk.
    pub fn set_project_memory(
        &mut self,
        key: &str,
        value: SettingValue,
        specs: &[SettingSpec],
    ) -> Result<()> {
        if self.project_dir.is_none() {
            return Err(KernelError::NoActiveProject);
        }
        let spec = specs
            .iter()
            .find(|spec| spec.key == key)
            .ok_or_else(|| KernelError::Settings(format!("unknown setting `{key}`")))?;
        if !spec.allows_project_override() {
            return Err(KernelError::UnsafeProjectSetting(key.to_owned()));
        }
        spec.validate_value(&value)?;
        let project_changed = self.project_values.get(key) != Some(&value);
        self.project_values.insert(key.to_owned(), value.clone());
        let effective_changed = self.values.get(key) != Some(&value);
        self.values.insert(key.to_owned(), value);
        if project_changed || effective_changed {
            self.generation += 1;
        }
        Ok(())
    }

    /// Removes a project override in memory and writes `<project>\.forja\ajustes.toml`.
    pub fn unset_project(&mut self, key: &str, specs: &[SettingSpec]) -> Result<()> {
        self.unset_project_memory(key, specs)?;
        self.save_project()
    }

    /// Removes a project override in memory so the key falls back to `user > default`.
    pub fn unset_project_memory(&mut self, key: &str, specs: &[SettingSpec]) -> Result<()> {
        if self.project_dir.is_none() {
            return Err(KernelError::NoActiveProject);
        }
        let spec = specs
            .iter()
            .find(|spec| spec.key == key)
            .ok_or_else(|| KernelError::Settings(format!("unknown setting `{key}`")))?;
        if !spec.allows_project_override() {
            return Err(KernelError::UnsafeProjectSetting(key.to_owned()));
        }
        let removed = self.project_values.remove(key).is_some();
        remove_dotted(&mut self.project_original, key);
        if removed {
            if let Some(fallback) = self.user_values.get(key).or_else(|| self.defaults.get(key)) {
                self.values.insert(key.to_owned(), fallback.clone());
            }
            self.generation += 1;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            path: self.path.clone(),
            values: self.user_values.clone(),
            original: self.original.clone(),
        }
    }

    pub fn project_snapshot(&self) -> Result<SettingsSnapshot> {
        let path = self.project_path().ok_or(KernelError::NoActiveProject)?;
        Ok(SettingsSnapshot {
            path,
            values: self.project_values.clone(),
            original: self.project_original.clone(),
        })
    }

    pub fn save(&self) -> Result<()> {
        save_values(&self.path, &self.user_values, &self.original)
    }

    pub fn save_project(&self) -> Result<()> {
        self.project_snapshot()?.save()
    }
}

/// Immutable copy sent to the writer thread.
#[derive(Clone, Debug)]
pub struct SettingsSnapshot {
    path: PathBuf,
    values: BTreeMap<String, SettingValue>,
    original: toml::Table,
}

impl SettingsSnapshot {
    pub fn save(&self) -> Result<()> {
        save_values(&self.path, &self.values, &self.original)
    }
}

/// Serializes writes on one background thread so the UI never blocks on disk.
pub(crate) struct SettingsWriter {
    sender: Option<std::sync::mpsc::Sender<SettingsWrite>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

enum SettingsWrite {
    Save(SettingsSnapshot),
    Flush(std::sync::mpsc::Sender<()>),
}

impl SettingsWriter {
    pub(crate) fn start() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<SettingsWrite>();
        let thread = std::thread::Builder::new()
            .name("forja-settings-writer".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    match message {
                        SettingsWrite::Save(snapshot) => {
                            if let Err(error) = snapshot.save() {
                                crate::logging::log(
                                    crate::logging::LogLevel::Error,
                                    format!("forja: could not save settings: {error}"),
                                );
                            }
                        }
                        SettingsWrite::Flush(ack) => {
                            let _ = ack.send(());
                        }
                    }
                }
            })
            .expect("settings writer thread starts");
        Self {
            sender: Some(sender),
            thread: Some(thread),
        }
    }

    pub(crate) fn submit(&self, snapshot: SettingsSnapshot) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(SettingsWrite::Save(snapshot));
        }
    }

    /// Waits until every queued write is done. Used by tests and shutdown.
    pub(crate) fn flush(&self) {
        let (ack, wait) = std::sync::mpsc::channel();
        if let Some(sender) = &self.sender {
            if sender.send(SettingsWrite::Flush(ack)).is_ok() {
                let _ = wait.recv_timeout(std::time::Duration::from_secs(5));
            }
        }
    }
}

impl Drop for SettingsWriter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn record_project_warning(warnings: &mut Vec<String>, message: String) {
    crate::logging::log(crate::logging::LogLevel::Warn, format!("forja: {message}"));
    warnings.push(message);
}

fn save_values(
    path: &Path,
    values: &BTreeMap<String, SettingValue>,
    original: &toml::Table,
) -> Result<()> {
    let mut root = original.clone();
    for (key, value) in values {
        insert_dotted(&mut root, key, value.to_toml())?;
    }
    let text = toml::to_string_pretty(&root)?;
    write_atomic_bytes(path, text.as_bytes())?;
    Ok(())
}

fn defaults(specs: &[SettingSpec]) -> BTreeMap<String, SettingValue> {
    specs
        .iter()
        .map(|spec| (spec.key.clone(), spec.default.clone()))
        .collect()
}

fn insert_dotted(table: &mut toml::Table, key: &str, value: toml::Value) -> Result<()> {
    let mut segments = key.split('.').peekable();
    let mut current = table;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            if current.get(segment).is_some_and(toml::Value::is_table) {
                return Err(KernelError::Settings(format!(
                    "cannot overwrite table at `{key}`"
                )));
            }
            current.insert(segment.to_owned(), value);
            return Ok(());
        }
        let entry = current
            .entry(segment.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            return Err(KernelError::Settings(format!(
                "cannot overwrite value along `{key}`"
            )));
        }
        current = entry.as_table_mut().expect("entry was made a table");
    }
    Ok(())
}

fn remove_dotted(table: &mut toml::Table, key: &str) {
    let segments: Vec<&str> = key.split('.').collect();
    remove_segments(table, &segments);
}

fn remove_segments(table: &mut toml::Table, segments: &[&str]) -> bool {
    let Some((first, rest)) = segments.split_first() else {
        return table.is_empty();
    };
    if rest.is_empty() {
        table.remove(*first);
        return table.is_empty();
    }
    let should_remove = if let Some(toml::Value::Table(child)) = table.get_mut(*first) {
        remove_segments(child, rest)
    } else {
        false
    };
    if should_remove {
        table.remove(*first);
    }
    table.is_empty()
}

fn flatten_table(prefix: &str, table: &toml::Table, out: &mut BTreeMap<String, toml::Value>) {
    for (key, value) in table {
        let key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(nested) => flatten_table(&key, nested, out),
            other => {
                out.insert(key, other.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_specs() -> Vec<SettingSpec> {
        vec![
            SettingSpec::new(
                "hello.greeting.style",
                SettingKind::Choice {
                    options: vec!["warm".into(), "plain".into()],
                },
                SettingValue::Text("warm".into()),
                "hello.setting.style.label",
                "hello.setting.style.help",
            )
            .scope(SettingScope::Project),
            SettingSpec::new(
                "hello.greeting.times",
                SettingKind::Integer {
                    min: Some(1),
                    max: Some(5),
                },
                SettingValue::Integer(1),
                "hello.setting.times.label",
                "hello.setting.times.help",
            ),
        ]
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-settings-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn schema_validation_rejects_structural_problems() {
        let mut spec = hello_specs()[0].clone();
        assert!(spec.validate().is_ok());
        spec.key = "hello".into();
        assert!(spec.validate().is_err());
        spec.key = "hello.Greeting.style".into();
        assert!(spec.validate().is_err());
        spec.key = "hello.greeting.style.extra".into();
        assert!(spec.validate().is_err());

        let forbidden = SettingSpec::new(
            "mcp.tools.auto_approve",
            SettingKind::List {
                item: "tool".into(),
            },
            SettingValue::List(Vec::new()),
            "mcp.label",
            "mcp.help",
        )
        .scope(SettingScope::Project);
        assert!(forbidden.validate().is_err());
    }

    #[test]
    fn values_are_checked_against_kind() {
        let specs = hello_specs();
        assert!(specs[0]
            .validate_value(&SettingValue::Text("warm".into()))
            .is_ok());
        assert!(specs[0]
            .validate_value(&SettingValue::Text("cold".into()))
            .is_err());
        assert!(specs[1].validate_value(&SettingValue::Integer(9)).is_err());
        assert!(specs[1].validate_value(&SettingValue::Bool(true)).is_err());
    }

    #[test]
    fn color_list_and_path_kinds_are_checked() {
        let color = SettingKind::Color;
        assert!(color.validate(&SettingValue::Text("#F2762E".into())).is_ok());
        assert!(color
            .validate(&SettingValue::Text("#F2762EFF".into()))
            .is_err());
        assert!(color.validate(&SettingValue::Text("orange".into())).is_err());

        let list = SettingKind::List {
            item: "branch".into(),
        };
        assert!(list
            .validate(&SettingValue::List(vec!["main".into(), "dev".into()]))
            .is_ok());
        assert!(list
            .validate(&SettingValue::List(vec!["main".into(), "main".into()]))
            .is_err());
        assert!(list
            .validate(&SettingValue::List(vec!["main".into(), " ".into()]))
            .is_err());

        let path = SettingKind::Path;
        assert!(path.validate(&SettingValue::Text(String::new())).is_ok());
    }

    #[test]
    fn defaults_fill_missing_file_and_roundtrip() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("settings.toml");
        let specs = hello_specs();
        let (mut settings, warnings) = Settings::load(path.clone(), &specs);
        assert!(warnings.is_empty());
        assert_eq!(
            settings.get("hello.greeting.style"),
            Some(&SettingValue::Text("warm".into()))
        );
        settings
            .set(
                "hello.greeting.style",
                SettingValue::Text("plain".into()),
                &specs,
            )
            .unwrap();
        assert!(path.exists());

        let (reloaded, warnings) = Settings::load(path.clone(), &specs);
        assert!(warnings.is_empty());
        assert_eq!(
            reloaded.get("hello.greeting.style"),
            Some(&SettingValue::Text("plain".into()))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_file_values_fall_back_to_defaults_with_warnings() {
        let dir = temp_dir("bad");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        std::fs::write(
            &path,
            "hello.greeting.times = 99\nhello.greeting.style = \"cold\"\n",
        )
        .unwrap();
        let specs = hello_specs();
        let (settings, warnings) = Settings::load(path, &specs);
        assert_eq!(warnings.len(), 2);
        assert_eq!(
            settings.get("hello.greeting.times"),
            Some(&SettingValue::Integer(1))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn saving_registered_setting_preserves_unknown_toml_values() {
        let dir = temp_dir("deferred");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let input = "[helper.sessions]\nresume_policy = 'never'\n[future]\nwhen = 2026-01-02\nitems = [{ name = 'one', active = true }]\n[future.empty]\n";
        std::fs::write(&path, input).unwrap();
        let before: toml::Table = toml::from_str(input).unwrap();
        let specs = hello_specs();
        let (mut settings, _) = Settings::load(path.clone(), &specs);
        settings
            .set_memory(
                "hello.greeting.style",
                SettingValue::Text("plain".into()),
                &specs,
            )
            .unwrap();
        settings.snapshot().save().unwrap();
        let after: toml::Table =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after.get("helper"), before.get("helper"));
        assert_eq!(after.get("future"), before.get("future"));
        assert_eq!(after["hello"]["greeting"]["style"].as_str(), Some("plain"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn conflicting_unknown_structure_is_not_overwritten() {
        let dir = temp_dir("conflict");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let input = "hello = 'unrecognized structure'\n";
        std::fs::write(&path, input).unwrap();
        let (settings, _) = Settings::load(path.clone(), &hello_specs());
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), input);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_settings_precedence_is_project_over_user_over_default() {
        let dir = temp_dir("precedence");
        let user_path = dir.join("settings.toml");
        let project = dir.join("proj-a");
        let specs = vec![SettingSpec::new(
            "terminal.font.size",
            SettingKind::Integer {
                min: Some(8),
                max: Some(32),
            },
            SettingValue::Integer(13),
            "setting-terminal-font-size-label",
            "setting-terminal-font-size-help",
        )];

        let (mut settings, warnings) = Settings::load(user_path.clone(), &specs);
        assert!(warnings.is_empty());
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(13))
        );
        assert_eq!(
            settings.source_of("terminal.font.size"),
            Some(SettingSource::Default)
        );

        // User overrides default (13 -> 15).
        settings
            .set(
                "terminal.font.size",
                SettingValue::Integer(15),
                &specs,
            )
            .unwrap();
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(15))
        );
        assert_eq!(
            settings.source_of("terminal.font.size"),
            Some(SettingSource::User)
        );

        // Project overrides user (15 -> 18).
        let warnings = settings.load_project(Some(&project), &specs);
        assert!(warnings.is_empty());
        settings
            .set_project(
                "terminal.font.size",
                SettingValue::Integer(18),
                &specs,
            )
            .unwrap();
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(18))
        );
        assert_eq!(
            settings.user_get("terminal.font.size"),
            Some(&SettingValue::Integer(15))
        );
        assert_eq!(
            settings.project_get("terminal.font.size"),
            Some(&SettingValue::Integer(18))
        );
        assert_eq!(
            settings.default_get("terminal.font.size"),
            Some(&SettingValue::Integer(13))
        );
        assert_eq!(
            settings.source_of("terminal.font.size"),
            Some(SettingSource::Project)
        );

        // Removing the project override falls back to the user value (15), not default (13).
        settings.unset_project("terminal.font.size", &specs).unwrap();
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(15))
        );
        assert_eq!(
            settings.source_of("terminal.font.size"),
            Some(SettingSource::User)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn writing_unsafe_key_to_project_is_refused_with_typed_error() {
        let dir = temp_dir("unsafe-write");
        let user_path = dir.join("settings.toml");
        let project = dir.join("proj");
        let specs = vec![
            SettingSpec::new(
                "mcp.tools.auto_approve",
                SettingKind::List {
                    item: "tool".into(),
                },
                SettingValue::List(Vec::new()),
                "setting-mcp-tools-auto_approve-label",
                "setting-mcp-tools-auto_approve-help",
            ),
            SettingSpec::new(
                "helper.default.permissions",
                SettingKind::Choice {
                    options: vec!["ask".into(), "auto".into()],
                },
                SettingValue::Text("ask".into()),
                "setting-helper-default-permissions-label",
                "setting-helper-default-permissions-help",
            ),
        ];

        let (mut settings, _) = Settings::load(user_path, &specs);
        settings.load_project(Some(&project), &specs);

        let err = settings
            .set_project(
                "mcp.tools.auto_approve",
                SettingValue::List(vec!["fs.write".into()]),
                &specs,
            )
            .expect_err("MCP auto_approve must be refused in project scope");
        assert!(err.is_unsafe_project_setting(), "unexpected error: {err:?}");
        assert!(matches!(
            err,
            KernelError::UnsafeProjectSetting(ref key) if key == "mcp.tools.auto_approve"
        ));

        let err = settings
            .set_project(
                "helper.default.permissions",
                SettingValue::Text("auto".into()),
                &specs,
            )
            .expect_err("permissions must be refused in project scope");
        assert!(err.is_unsafe_project_setting());
        assert!(!project_settings_path(&project).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unsafe_keys_and_invalid_project_files_are_ignored_with_log_warnings() {
        let dir = temp_dir("invalid-project");
        crate::logging::initialize(dir.clone(), crate::logging::LogConfig::default());
        let user_path = dir.join("settings.toml");
        let project_bad_toml = dir.join("proj-bad-toml");
        let project_unsafe = dir.join("proj-unsafe");
        std::fs::create_dir_all(project_bad_toml.join(".forja")).unwrap();
        std::fs::create_dir_all(project_unsafe.join(".forja")).unwrap();

        let specs = vec![
            SettingSpec::new(
                "terminal.font.size",
                SettingKind::Integer {
                    min: Some(8),
                    max: Some(32),
                },
                SettingValue::Integer(13),
                "setting-terminal-font-size-label",
                "setting-terminal-font-size-help",
            ),
            SettingSpec::new(
                "mcp.tools.auto_approve",
                SettingKind::List {
                    item: "tool".into(),
                },
                SettingValue::List(Vec::new()),
                "setting-mcp-tools-auto_approve-label",
                "setting-mcp-tools-auto_approve-help",
            ),
        ];

        let (mut settings, _) = Settings::load(user_path, &specs);
        settings
            .set("terminal.font.size", SettingValue::Integer(16), &specs)
            .unwrap();

        // 1. Corrupt/invalid TOML in `.forja\ajustes.toml` keeps user value (16) and warns.
        std::fs::write(
            project_settings_path(&project_bad_toml),
            "terminal.font.size = [unclosed",
        )
        .unwrap();
        let warnings = settings.load_project(Some(&project_bad_toml), &specs);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("ajustes.toml could not be parsed"));
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(16))
        );

        // 2. Unsafe key and out-of-range value in `.forja\ajustes.toml` are ignored with warnings.
        std::fs::write(
            project_settings_path(&project_unsafe),
            "mcp.tools.auto_approve = [\"shell.exec\"]\nterminal.font.size = 999\n",
        )
        .unwrap();
        let warnings = settings.load_project(Some(&project_unsafe), &specs);
        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .any(|w| w.contains("mcp.tools.auto_approve") && w.contains("not allowed in project scope")));
        assert!(warnings
            .iter()
            .any(|w| w.contains("terminal.font.size") && w.contains("using user value")));
        assert_eq!(
            settings.get("mcp.tools.auto_approve"),
            Some(&SettingValue::List(Vec::new()))
        );
        assert_eq!(
            settings.get("terminal.font.size"),
            Some(&SettingValue::Integer(16))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn switching_active_project_reloads_file_and_bumps_generation() {
        let dir = temp_dir("switch-project");
        let user_path = dir.join("settings.toml");
        let proj_a = dir.join("proj-a");
        let proj_b = dir.join("proj-b");
        std::fs::create_dir_all(proj_a.join(".forja")).unwrap();
        std::fs::create_dir_all(proj_b.join(".forja")).unwrap();
        std::fs::write(
            project_settings_path(&proj_a),
            "[editor.indent]\nsize = 2\n",
        )
        .unwrap();
        std::fs::write(
            project_settings_path(&proj_b),
            "[editor.indent]\nsize = 8\n",
        )
        .unwrap();

        let specs = vec![SettingSpec::new(
            "editor.indent.size",
            SettingKind::Integer {
                min: Some(1),
                max: Some(8),
            },
            SettingValue::Integer(4),
            "setting-editor-indent-size-label",
            "setting-editor-indent-size-help",
        )];

        let (mut settings, _) = Settings::load(user_path, &specs);
        let gen0 = settings.generation();
        assert_eq!(
            settings.get("editor.indent.size"),
            Some(&SettingValue::Integer(4))
        );

        settings.load_project(Some(&proj_a), &specs);
        let gen1 = settings.generation();
        assert!(gen1 > gen0);
        assert_eq!(
            settings.get("editor.indent.size"),
            Some(&SettingValue::Integer(2))
        );

        settings.load_project(Some(&proj_b), &specs);
        let gen2 = settings.generation();
        assert!(gen2 > gen1);
        assert_eq!(
            settings.get("editor.indent.size"),
            Some(&SettingValue::Integer(8))
        );

        settings.load_project(None, &specs);
        let gen3 = settings.generation();
        assert!(gen3 > gen2);
        assert_eq!(
            settings.get("editor.indent.size"),
            Some(&SettingValue::Integer(4))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_save_is_atomic_and_never_pollutes_user_settings() {
        let dir = temp_dir("atomic-project");
        let user_path = dir.join("settings.toml");
        let project = dir.join("proj");
        let specs = vec![SettingSpec::new(
            "editor.save.format",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "setting-editor-save-format-label",
            "setting-editor-save-format-help",
        )];

        let (mut settings, _) = Settings::load(user_path.clone(), &specs);
        settings.load_project(Some(&project), &specs);
        settings
            .set_project("editor.save.format", SettingValue::Bool(true), &specs)
            .unwrap();

        let proj_file = project_settings_path(&project);
        assert!(proj_file.is_file());
        assert!(!user_path.exists());

        // No temporary sibling file is left behind in `.forja`.
        let leftovers: Vec<_> = std::fs::read_dir(project.join(".forja"))
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains("tmp-"))
            .collect();
        assert!(leftovers.is_empty());

        let parsed: toml::Table =
            toml::from_str(&std::fs::read_to_string(&proj_file).unwrap()).unwrap();
        assert_eq!(parsed["editor"]["save"]["format"].as_bool(), Some(true));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn catalog_scope_field_matches_kernel_project_safe_list() {
        #[derive(Deserialize)]
        struct RawCatalog {
            setting: Vec<RawSetting>,
        }
        #[derive(Deserialize)]
        struct RawSetting {
            key: String,
            scope: Option<String>,
        }

        let raw: RawCatalog =
            toml::from_str(include_str!("../../../design/settings.toml")).expect("valid catalog");
        let mut catalog_project_keys = BTreeSet::new();
        for entry in &raw.setting {
            let scope = entry
                .scope
                .as_deref()
                .unwrap_or_else(|| panic!("setting `{}` is missing `scope`", entry.key));
            match scope {
                "user" => {}
                "project" => {
                    assert!(
                        !is_forbidden_project_key(&entry.key),
                        "forbidden key `{}` marked as scope = \"project\"",
                        entry.key
                    );
                    catalog_project_keys.insert(entry.key.clone());
                }
                other => panic!("setting `{}` has invalid scope `{other}`", entry.key),
            }
        }
        let kernel_project_keys: BTreeSet<String> =
            PROJECT_SAFE_KEYS.iter().map(|k| (*k).to_owned()).collect();
        assert_eq!(catalog_project_keys, kernel_project_keys);
    }
}
