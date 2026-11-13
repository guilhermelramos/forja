//! Settings schema and persistence in `%APPDATA%\Forja\settings.toml`.
//!
//! Keys follow `<module>.<group>.<name>` in snake_case. Modules only declare
//! the schema; the shell renders the controls and stores values after
//! validating them against the same schema.

use crate::error::{KernelError, Result};
use crate::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
                let mut seen = std::collections::BTreeSet::new();
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
}

impl SettingSpec {
    pub fn new(
        key: impl Into<String>,
        kind: SettingKind,
        default: SettingValue,
        label_key: impl Into<String>,
        help_key: impl Into<String>,
    ) -> Self {
        Self {
            key: key.into(),
            kind,
            default,
            label_key: label_key.into(),
            help_key: help_key.into(),
            order: 0,
        }
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = order;
        self
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

/// Values currently in memory, plus the file they came from.
#[derive(Debug)]
pub struct Settings {
    path: PathBuf,
    values: BTreeMap<String, SettingValue>,
    original: toml::Table,
}

impl Settings {
    /// Loads the file, falls back to schema defaults and reports invalid or
    /// unknown entries as warnings instead of failing startup.
    pub fn load(path: PathBuf, specs: &[SettingSpec]) -> (Self, Vec<String>) {
        let mut values = defaults(specs);
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
                        values.insert(key, value);
                    }
                    original = table;
                }
                Err(error) => warnings.push(format!("settings.toml could not be parsed: {error}")),
            }
        }
        (
            Self {
                path,
                values,
                original,
            },
            warnings,
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key)
    }

    pub fn get_or<'a>(&'a self, key: &str, fallback: &'a SettingValue) -> &'a SettingValue {
        self.values.get(key).unwrap_or(fallback)
    }

    pub fn values(&self) -> &BTreeMap<String, SettingValue> {
        &self.values
    }

    /// Validates against `specs`, updates memory and writes the file
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
        self.values.insert(key.to_owned(), value);
        Ok(())
    }

    pub fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            path: self.path.clone(),
            values: self.values.clone(),
            original: self.original.clone(),
        }
    }

    pub fn save(&self) -> Result<()> {
        save_values(&self.path, &self.values, &self.original)
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
    sender: std::sync::mpsc::Sender<SettingsWrite>,
}

struct SettingsWrite {
    snapshot: SettingsSnapshot,
    ack: Option<std::sync::mpsc::Sender<()>>,
}

impl SettingsWriter {
    pub(crate) fn start() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel::<SettingsWrite>();
        std::thread::Builder::new()
            .name("forja-settings-writer".into())
            .spawn(move || {
                while let Ok(message) = receiver.recv() {
                    if let Err(error) = message.snapshot.save() {
                        eprintln!("forja: could not save settings: {error}");
                    }
                    if let Some(ack) = message.ack {
                        let _ = ack.send(());
                    }
                }
            })
            .expect("settings writer thread starts");
        Self { sender }
    }

    pub(crate) fn submit(&self, snapshot: SettingsSnapshot) {
        let _ = self.sender.send(SettingsWrite {
            snapshot,
            ack: None,
        });
    }

    /// Waits until every queued write is done. Used by tests and shutdown.
    pub(crate) fn flush(&self, snapshot: SettingsSnapshot) {
        let (ack, wait) = std::sync::mpsc::channel();
        if self
            .sender
            .send(SettingsWrite {
                snapshot,
                ack: Some(ack),
            })
            .is_ok()
        {
            let _ = wait.recv_timeout(std::time::Duration::from_secs(5));
        }
    }
}

fn save_values(path: &Path, values: &BTreeMap<String, SettingValue>, original: &toml::Table) -> Result<()> {
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
                return Err(KernelError::Settings(format!("cannot overwrite table at `{key}`")));
            }
            current.insert(segment.to_owned(), value);
            return Ok(());
        }
        let entry = current
            .entry(segment.to_owned())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if !entry.is_table() {
            return Err(KernelError::Settings(format!("cannot overwrite value along `{key}`")));
        }
        current = entry.as_table_mut().expect("entry was made a table");
    }
    Ok(())
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
            ),
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
        assert!(color.validate(&SettingValue::Text("#F2762EFF".into())).is_err());
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
        assert!(list.validate(&SettingValue::List(vec!["main".into(), " ".into()])).is_err());

        let path = SettingKind::Path;
        assert!(path.validate(&SettingValue::Text(String::new())).is_ok());
    }

    #[test]
    fn defaults_fill_missing_file_and_roundtrip() {
        let dir = std::env::temp_dir().join(format!(
            "forja-settings-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("settings.toml");
        let specs = hello_specs();
        let (mut settings, warnings) = Settings::load(path.clone(), &specs);
        assert!(warnings.is_empty());
        assert_eq!(
            settings.get("hello.greeting.style"),
            Some(&SettingValue::Text("warm".into()))
        );
        settings
            .set("hello.greeting.style", SettingValue::Text("plain".into()), &specs)
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
        let dir = std::env::temp_dir().join(format!(
            "forja-settings-bad-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        let dir = std::env::temp_dir().join(format!("forja-settings-deferred-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let input = "[helper.sessions]\nresume_policy = 'never'\n[future]\nwhen = 2026-01-02\nitems = [{ name = 'one', active = true }]\n[future.empty]\n";
        std::fs::write(&path, input).unwrap();
        let before: toml::Table = toml::from_str(input).unwrap();
        let specs = hello_specs();
        let (mut settings, _) = Settings::load(path.clone(), &specs);
        settings.set_memory("hello.greeting.style", SettingValue::Text("plain".into()), &specs).unwrap();
        settings.snapshot().save().unwrap();
        let after: toml::Table = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(after.get("helper"), before.get("helper"));
        assert_eq!(after.get("future"), before.get("future"));
        assert_eq!(after["hello"]["greeting"]["style"].as_str(), Some("plain"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn conflicting_unknown_structure_is_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("forja-settings-conflict-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let input = "hello = 'unrecognized structure'\n";
        std::fs::write(&path, input).unwrap();
        let (settings, _) = Settings::load(path.clone(), &hello_specs());
        assert!(settings.save().is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), input);
        let _ = std::fs::remove_dir_all(dir);
    }
}
