//! Compares `design/settings.toml` and `design/keymap.toml` against runtime registrations.

use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use toml::Value as TomlValue;

#[derive(Debug, Deserialize)]
pub struct SettingsCatalog {
    #[serde(default)]
    pub setting: Vec<CatalogSetting>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogSetting {
    pub key: String,
    pub module: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: TomlValue,
    pub min: Option<TomlValue>,
    pub max: Option<TomlValue>,
    #[serde(default)]
    pub options: Option<Vec<String>>,
    pub options_from: Option<String>,
    pub item: Option<String>,
    pub deferred: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KeymapCatalog {
    #[serde(default)]
    pub binding: Vec<CatalogBinding>,
}

#[derive(Debug, Deserialize)]
pub struct CatalogBinding {
    pub id: String,
    pub key: String,
    pub scope: String,
    #[serde(default)]
    pub family: String,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeCatalog {
    pub settings: Vec<RuntimeSetting>,
    pub keybinds: Vec<RuntimeKeybind>,
}

#[derive(Debug, Deserialize)]
pub struct RuntimeSetting {
    pub key: String,
    pub module: String,
    pub kind: RuntimeKind,
    pub default: JsonValue,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RuntimeKind {
    Toggle,
    Integer { min: Option<i64>, max: Option<i64> },
    Number { min: Option<f64>, max: Option<f64> },
    Text,
    Choice { options: Vec<String> },
    Path,
    List { item: String },
    Color,
}

impl RuntimeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
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

#[derive(Debug, Deserialize)]
pub struct RuntimeKeybind {
    pub id: String,
    pub key: String,
    pub scope: String,
    #[serde(default)]
    pub module: String,
}

#[derive(Debug)]
pub struct SettingComparison {
    pub key: String,
    pub catalog_module: String,
    pub registered: bool,
    pub runtime_module: Option<String>,
    pub type_mismatch: Option<String>,
    pub default_mismatch: Option<String>,
    pub range_or_options_note: Option<String>,
    pub module_mismatch: Option<String>,
    pub has_effect: bool,
    pub deferred: Option<String>,
    pub scope: Option<String>,
    pub scope_error: Option<String>,
}

#[derive(Debug)]
pub struct KeybindComparison {
    pub id: String,
    pub catalog_key: String,
    pub catalog_scope: String,
    pub family: String,
    pub registered: bool,
    pub runtime_key: Option<String>,
    pub runtime_scope: Option<String>,
    pub mismatch: Option<String>,
}

#[derive(Debug)]
pub struct CoverageReport {
    pub settings: Vec<SettingComparison>,
    pub extra_settings: Vec<RuntimeSetting>,
    pub no_effect_extra_settings: BTreeSet<String>,
    pub keybinds: Vec<KeybindComparison>,
    pub extra_keybinds: Vec<RuntimeKeybind>,
    pub runtime_settings_total: usize,
    pub runtime_keybinds_total: usize,
}

impl CoverageReport {
    pub fn invalid_deferred(&self) -> Vec<&SettingComparison> {
        self.settings.iter().filter(|item| {
            item.deferred.as_ref().is_some_and(|reason| item.registered || reason.trim().is_empty())
        }).collect()
    }
    pub fn invalid_scopes(&self) -> Vec<&SettingComparison> {
        self.settings
            .iter()
            .filter(|item| item.scope_error.is_some())
            .collect()
    }
    pub fn type_or_default_mismatches(&self) -> Vec<&SettingComparison> {
        self.settings
            .iter()
            .filter(|item| item.type_mismatch.is_some() || item.default_mismatch.is_some())
            .collect()
    }
}

pub fn evaluate(
    settings_cat: SettingsCatalog,
    keymap_cat: KeymapCatalog,
    runtime: RuntimeCatalog,
    usage: &BTreeMap<String, Vec<String>>,
) -> CoverageReport {
    let runtime_settings_total = runtime.settings.len();
    let runtime_keybinds_total = runtime.keybinds.len();

    let mut run_settings: BTreeMap<String, RuntimeSetting> = runtime
        .settings
        .into_iter()
        .map(|spec| (spec.key.clone(), spec))
        .collect();
    let mut run_keybinds: BTreeMap<String, RuntimeKeybind> = runtime
        .keybinds
        .into_iter()
        .map(|bind| (bind.id.clone(), bind))
        .collect();

    let mut settings = Vec::with_capacity(settings_cat.setting.len());
    for entry in settings_cat.setting {
        let effect_files = usage.get(&entry.key).cloned().unwrap_or_default();
        let has_effect = !effect_files.is_empty();
        let scope_error = check_scope(&entry);
        let scope = entry.scope.clone();
        if let Some(spec) = run_settings.remove(&entry.key) {
            let type_mismatch = check_type(&entry, &spec.kind);
            let default_mismatch = if type_mismatch.is_none() {
                check_default(&entry.default, &spec.default, &spec.kind)
            } else {
                None
            };
            let range_or_options_note = check_range_or_options(&entry, &spec.kind);
            let module_mismatch = if entry.module != spec.module {
                Some(format!(
                    "catálogo `{}` != runtime `{}`",
                    entry.module, spec.module
                ))
            } else {
                None
            };
            settings.push(SettingComparison {
                key: entry.key,
                catalog_module: entry.module,
                registered: true,
                runtime_module: Some(spec.module),
                type_mismatch,
                default_mismatch,
                range_or_options_note,
                module_mismatch,
                has_effect,
                deferred: entry.deferred,
                scope,
                scope_error,
            });
        } else {
            settings.push(SettingComparison {
                key: entry.key,
                catalog_module: entry.module,
                registered: false,
                runtime_module: None,
                type_mismatch: None,
                default_mismatch: None,
                range_or_options_note: None,
                module_mismatch: None,
                has_effect: false,
                deferred: entry.deferred,
                scope,
                scope_error,
            });
        }
    }

    let extra_settings: Vec<RuntimeSetting> = run_settings.into_values().collect();
    let no_effect_extra_settings: BTreeSet<String> = extra_settings
        .iter()
        .filter(|spec| usage.get(&spec.key).is_none_or(Vec::is_empty))
        .map(|spec| spec.key.clone())
        .collect();

    let mut keybinds = Vec::with_capacity(keymap_cat.binding.len());
    for entry in keymap_cat.binding {
        if let Some(bind) = run_keybinds.remove(&entry.id) {
            let mismatch = compare_keybind(&entry, &bind);
            keybinds.push(KeybindComparison {
                id: entry.id,
                catalog_key: entry.key,
                catalog_scope: entry.scope,
                family: entry.family,
                registered: true,
                runtime_key: Some(bind.key),
                runtime_scope: Some(bind.scope),
                mismatch,
            });
        } else {
            keybinds.push(KeybindComparison {
                id: entry.id,
                catalog_key: entry.key,
                catalog_scope: entry.scope,
                family: entry.family,
                registered: false,
                runtime_key: None,
                runtime_scope: None,
                mismatch: None,
            });
        }
    }

    let extra_keybinds: Vec<RuntimeKeybind> = run_keybinds.into_values().collect();

    CoverageReport {
        settings,
        extra_settings,
        no_effect_extra_settings,
        keybinds,
        extra_keybinds,
        runtime_settings_total,
        runtime_keybinds_total,
    }
}

fn check_scope(entry: &CatalogSetting) -> Option<String> {
    let Some(scope) = entry.scope.as_deref() else {
        return Some("campo `scope` ausente (esperado \"user\" ou \"project\")".to_owned());
    };
    match scope {
        "user" => None,
        "project" => {
            if is_forbidden_project_key(&entry.key) {
                Some(format!(
                    "chave `{}` não pode ter `scope = \"project\"` (segurança, rede ou global)",
                    entry.key
                ))
            } else {
                None
            }
        }
        other => Some(format!(
            "`scope` inválido `{other}` (esperado \"user\" ou \"project\")"
        )),
    }
}

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

fn check_type(entry: &CatalogSetting, kind: &RuntimeKind) -> Option<String> {
    let ok = match (entry.kind.as_str(), kind) {
        ("toggle", RuntimeKind::Toggle)
        | ("text", RuntimeKind::Text)
        | ("path", RuntimeKind::Path)
        | ("list", RuntimeKind::List { .. })
        | ("color", RuntimeKind::Color)
        | ("choice", RuntimeKind::Choice { .. }) => true,
        ("number", RuntimeKind::Integer { .. }) => entry.default.is_integer(),
        ("number", RuntimeKind::Number { .. }) => {
            entry.default.is_float() || entry.default.is_integer()
        }
        _ => false,
    };
    if ok {
        None
    } else {
        Some(format!(
            "catálogo `{}` != runtime `{}`",
            entry.kind,
            kind.label()
        ))
    }
}

fn check_default(
    catalog: &TomlValue,
    runtime: &JsonValue,
    kind: &RuntimeKind,
) -> Option<String> {
    let matches = match kind {
        RuntimeKind::Toggle => catalog.as_bool() == runtime.as_bool() && runtime.as_bool().is_some(),
        RuntimeKind::Integer { .. } => {
            catalog.as_integer() == runtime.as_i64() && runtime.as_i64().is_some()
        }
        RuntimeKind::Number { .. } => {
            let cat_f = toml_as_f64(catalog);
            let run_f = runtime.as_f64();
            matches!((cat_f, run_f), (Some(a), Some(b)) if (a - b).abs() <= 1e-9)
        }
        RuntimeKind::Text | RuntimeKind::Choice { .. } | RuntimeKind::Path | RuntimeKind::Color => {
            matches!((catalog.as_str(), runtime.as_str()), (Some(a), Some(b)) if a == b)
        }
        RuntimeKind::List { .. } => {
            let cat_list: Option<Vec<&str>> = catalog
                .as_array()
                .and_then(|arr| arr.iter().map(|v| v.as_str()).collect());
            let run_list: Option<Vec<&str>> = runtime
                .as_array()
                .and_then(|arr| arr.iter().map(|v| v.as_str()).collect());
            cat_list.is_some() && cat_list == run_list
        }
    };
    if matches {
        None
    } else {
        Some(format!("catálogo `{catalog}` != runtime `{runtime}`"))
    }
}

fn check_range_or_options(entry: &CatalogSetting, kind: &RuntimeKind) -> Option<String> {
    match kind {
        RuntimeKind::Integer { min, max } => {
            let cat_min = entry.min.as_ref().and_then(TomlValue::as_integer);
            let cat_max = entry.max.as_ref().and_then(TomlValue::as_integer);
            if cat_min != *min || cat_max != *max {
                Some(format!(
                    "faixa catálogo ({cat_min:?}, {cat_max:?}) != runtime ({min:?}, {max:?})"
                ))
            } else {
                None
            }
        }
        RuntimeKind::Number { min, max } => {
            let cat_min = entry.min.as_ref().and_then(toml_as_f64);
            let cat_max = entry.max.as_ref().and_then(toml_as_f64);
            if !opt_f64_eq(cat_min, *min) || !opt_f64_eq(cat_max, *max) {
                Some(format!(
                    "faixa catálogo ({cat_min:?}, {cat_max:?}) != runtime ({min:?}, {max:?})"
                ))
            } else {
                None
            }
        }
        RuntimeKind::Choice { options } => {
            if entry.options_from.is_some() {
                return None;
            }
            if entry.options.as_ref() != Some(options) {
                Some(format!(
                    "opções catálogo {:?} != runtime {:?}",
                    entry.options, options
                ))
            } else {
                None
            }
        }
        RuntimeKind::List { item } => {
            if let Some(cat_item) = &entry.item {
                if cat_item != item {
                    return Some(format!("item catálogo `{cat_item}` != runtime `{item}`"));
                }
            }
            None
        }
        _ => None,
    }
}

fn toml_as_f64(value: &TomlValue) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|i| i as f64))
}

fn opt_f64_eq(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() <= 1e-9,
        _ => false,
    }
}

fn compare_keybind(entry: &CatalogBinding, runtime: &RuntimeKeybind) -> Option<String> {
    let cat_chord = canonical_chord(&entry.key);
    let run_chord = canonical_chord(&runtime.key);
    let mut notes = Vec::new();
    match (cat_chord, run_chord) {
        (Ok(a), Ok(b)) if a == b => {}
        (Ok(a), Ok(b)) => notes.push(format!("tecla catálogo `{a}` != runtime `{b}`")),
        (Err(e), _) | (_, Err(e)) => notes.push(format!("atalho inválido: {e}")),
    }
    if entry.scope != runtime.scope {
        notes.push(format!(
            "escopo catálogo `{}` != runtime `{}`",
            entry.scope, runtime.scope
        ));
    }
    if notes.is_empty() {
        None
    } else {
        Some(notes.join("; "))
    }
}

/// Normalizes key chord aliases (`Ctrl+Left` == `Ctrl+ArrowLeft`, `Esc` == `Escape`, etc.).
pub fn canonical_chord(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("atalho vazio".to_owned());
    }
    let parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
    let (modifiers, key_part) = parts.split_at(parts.len() - 1);
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut meta = false;
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            "meta" | "win" | "super" | "cmd" => meta = true,
            other => return Err(format!("modificador desconhecido `{other}`")),
        }
    }
    let key = canonical_key(key_part[0])?;
    let mut out = Vec::new();
    if ctrl {
        out.push("Ctrl".to_owned());
    }
    if alt {
        out.push("Alt".to_owned());
    }
    if shift {
        out.push("Shift".to_owned());
    }
    if meta {
        out.push("Meta".to_owned());
    }
    out.push(key);
    Ok(out.join("+"))
}

fn canonical_key(raw: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Err("tecla vazia".to_owned());
    }
    let mapped = match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "Enter",
        "escape" | "esc" => "Escape",
        "space" | "spacebar" | " " => "Space",
        "tab" => "Tab",
        "backspace" => "Backspace",
        "delete" | "del" => "Delete",
        "insert" | "ins" => "Insert",
        "home" => "Home",
        "end" => "End",
        "pageup" | "pgup" => "PageUp",
        "pagedown" | "pgdn" => "PageDown",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "minus" | "-" => "Minus",
        "plus" | "+" => "Plus",
        "equal" | "=" => "Equal",
        "comma" | "," => "Comma",
        "period" | "." => "Period",
        "slash" | "/" => "Slash",
        "backslash" | "\\" => "Backslash",
        "semicolon" | ";" => "Semicolon",
        "quote" | "'" => "Quote",
        "backtick" | "grave" | "`" => "Backtick",
        "bracketleft" | "[" => "BracketLeft",
        "bracketright" | "]" => "BracketRight",
        other => {
            if other.chars().count() == 1 {
                return Ok(other.to_ascii_uppercase());
            }
            if let Some(num) = other.strip_prefix('f') {
                if matches!(num.parse::<u8>(), Ok(1..=24)) {
                    return Ok(other.to_ascii_uppercase());
                }
            }
            return Err(format!("tecla desconhecida `{other}`"));
        }
    };
    Ok(mapped.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_chord_unifies_arrows_and_symbols() {
        assert_eq!(canonical_chord("Alt+Left").unwrap(), "Alt+ArrowLeft");
        assert_eq!(canonical_chord("Ctrl+,").unwrap(), "Ctrl+Comma");
        assert_ne!(canonical_chord("Ctrl+Equal").unwrap(), "Ctrl+Plus");
    }

    #[test]
    fn type_and_default_detects_mismatches() {
        let entry = CatalogSetting {
            key: "demo.count".to_owned(),
            module: "demo".to_owned(),
            kind: "number".to_owned(),
            default: TomlValue::Integer(5),
            min: Some(TomlValue::Integer(1)),
            max: Some(TomlValue::Integer(10)),
            options: None,
            options_from: None,
            item: None,
            deferred: None,
            scope: Some("user".to_owned()),
        };
        let ok_kind = RuntimeKind::Integer {
            min: Some(1),
            max: Some(10),
        };
        assert!(check_type(&entry, &ok_kind).is_none());
        assert!(check_default(&entry.default, &JsonValue::from(5), &ok_kind).is_none());
        assert!(check_default(&entry.default, &JsonValue::from(6), &ok_kind).is_some());
        assert!(check_type(&entry, &RuntimeKind::Toggle).is_some());
    }

    #[test]
    fn deferred_settings_are_reported_and_registration_is_invalid() {
        let settings = || toml::from_str::<SettingsCatalog>(r#"
            [[setting]]
            key = "demo.future"
            module = "demo"
            scope = "user"
            type = "toggle"
            default = true
            deferred = "Awaiting runtime"
        "#).unwrap();
        let keymap = || KeymapCatalog { binding: Vec::new() };
        let runtime = |registered| RuntimeCatalog {
            settings: if registered { vec![RuntimeSetting {
                key: "demo.future".into(), module: "demo".into(),
                kind: RuntimeKind::Toggle, default: JsonValue::Bool(true),
            }] } else { Vec::new() },
            keybinds: Vec::new(),
        };
        let report = evaluate(settings(), keymap(), runtime(false), &BTreeMap::new());
        assert!(report.invalid_deferred().is_empty());
        let text = super::super::report::render(&report);
        assert!(text.contains("`demo.future`: Awaiting runtime"));
        assert!(text.contains("Configurações do catálogo ausentes no app | 0"));
        assert!(!text.contains("Settings ausentes: `demo`"));
        let report = evaluate(settings(), keymap(), runtime(true), &BTreeMap::new());
        assert_eq!(report.invalid_deferred().len(), 1);
    }

    #[test]
    fn scope_validation_rejects_missing_unknown_or_unsafe_project_keys() {
        let parsed: SettingsCatalog = toml::from_str(
            r#"
            [[setting]]
            key = "terminal.font.size"
            module = "terminal"
            scope = "project"
            type = "number"
            default = 13

            [[setting]]
            key = "appearance.theme.mode"
            module = "appearance"
            scope = "user"
            type = "choice"
            default = "system"
            options = ["system", "dark", "light"]

            [[setting]]
            key = "mcp.tools.auto_approve"
            module = "mcp"
            scope = "project"
            type = "list"
            default = []

            [[setting]]
            key = "editor.indent.size"
            module = "editor"
            scope = "global"
            type = "number"
            default = 4

            [[setting]]
            key = "editor.indent.tabs"
            module = "editor"
            type = "choice"
            default = "spaces"
            options = ["spaces", "tabs"]
        "#,
        )
        .unwrap();
        let report = evaluate(
            parsed,
            KeymapCatalog {
                binding: Vec::new(),
            },
            RuntimeCatalog {
                settings: Vec::new(),
                keybinds: Vec::new(),
            },
            &BTreeMap::new(),
        );
        let bad: Vec<&str> = report
            .invalid_scopes()
            .into_iter()
            .map(|item| item.key.as_str())
            .collect();
        assert_eq!(
            bad,
            vec![
                "mcp.tools.auto_approve",
                "editor.indent.size",
                "editor.indent.tabs"
            ]
        );
    }

    #[test]
    fn design_settings_catalog_has_valid_scopes_for_all_entries() {
        let text = include_str!("../../../design/settings.toml");
        let parsed: SettingsCatalog = toml::from_str(text).expect("design/settings.toml parses");
        let report = evaluate(
            parsed,
            KeymapCatalog {
                binding: Vec::new(),
            },
            RuntimeCatalog {
                settings: Vec::new(),
                keybinds: Vec::new(),
            },
            &BTreeMap::new(),
        );
        assert!(
            report.invalid_scopes().is_empty(),
            "invalid scopes: {:?}",
            report.invalid_scopes()
        );
        let project_count = report
            .settings
            .iter()
            .filter(|s| s.scope.as_deref() == Some("project"))
            .count();
        assert!(project_count > 0);
    }
}
