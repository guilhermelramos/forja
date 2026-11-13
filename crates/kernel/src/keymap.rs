//! Keyboard shortcut grammar, layers and conflict detection.
//!
//! Syntax is `Ctrl+Shift+K`: zero or more modifiers followed by one key.
//! Layers, lowest priority first:
//!
//! 1. module defaults embedded from each `keymap.toml`;
//! 2. packs in `~/.forja/keymaps/*.toml`, sorted by file name;
//! 3. the user file `~/.forja/keymap.toml`;
//! 4. the platform file `<data>/keymap-platform.toml`.
//!
//! A later layer replaces the binding of the same command and scope. Two
//! different commands with the same chord in the same scope are a conflict;
//! contract tests reject them before release.

use crate::args::CommandId;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Scope {
    Global,
    Bench,
    Editor,
    Terminal,
    Custom(String),
}

impl Scope {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Global => "global",
            Self::Bench => "bench",
            Self::Editor => "editor",
            Self::Terminal => "terminal",
            Self::Custom(value) => value,
        }
    }
}

impl FromStr for Scope {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(Self::Global);
        }
        Ok(match value.to_ascii_lowercase().as_str() {
            "global" => Self::Global,
            "bench" => Self::Bench,
            "editor" => Self::Editor,
            "terminal" => Self::Terminal,
            _ => Self::Custom(value.to_owned()),
        })
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parsed key chord. `key` is the canonical name (`K`, `Enter`, `F5`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyChord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    pub key: String,
}

impl KeyChord {
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("empty shortcut".to_owned());
        }
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (modifiers, key_part) = parts.split_at(parts.len() - 1);
        let key = canonical_key(key_part[0])?;
        let mut chord = Self {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
            key,
        };
        for modifier in modifiers {
            match modifier.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => set_once(&mut chord.ctrl, "Ctrl")?,
                "alt" | "option" => set_once(&mut chord.alt, "Alt")?,
                "shift" => set_once(&mut chord.shift, "Shift")?,
                "meta" | "win" | "super" | "cmd" => set_once(&mut chord.meta, "Meta")?,
                "" => return Err("empty modifier".to_owned()),
                other => return Err(format!("unknown modifier `{other}`")),
            }
        }
        Ok(chord)
    }

    pub fn matches(&self, ctrl: bool, alt: bool, shift: bool, meta: bool, key: &str) -> bool {
        self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
            && self.meta == meta
            && self.key.eq_ignore_ascii_case(key)
    }
}

fn set_once(slot: &mut bool, name: &str) -> Result<(), String> {
    if *slot {
        return Err(format!("modifier `{name}` appears twice"));
    }
    *slot = true;
    Ok(())
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.meta {
            parts.push("Meta");
        }
        parts.push(&self.key);
        f.write_str(&parts.join("+"))
    }
}

fn canonical_key(raw: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Err("shortcut has no key".to_owned());
    }
    if key.contains('+') {
        return Err(format!("invalid key `{key}`"));
    }
    if key.chars().count() == 1 {
        let c = key.chars().next().expect("one char");
        return Ok(c.to_ascii_uppercase().to_string());
    }
    let canonical = match key.to_ascii_lowercase().as_str() {
        "enter" | "return" => "Enter",
        "escape" | "esc" => "Escape",
        "space" | "spacebar" => "Space",
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
        "minus" => "Minus",
        "plus" => "Plus",
        "comma" => "Comma",
        "period" => "Period",
        "slash" => "Slash",
        "backslash" => "Backslash",
        "semicolon" => "Semicolon",
        "quote" => "Quote",
        "backtick" | "grave" => "Backtick",
        "bracketleft" => "BracketLeft",
        "bracketright" => "BracketRight",
        other if is_function_key(other) => return Ok(other.to_ascii_uppercase()),
        other => return Err(format!("unknown key `{other}`")),
    };
    Ok(canonical.to_owned())
}

fn is_function_key(value: &str) -> bool {
    let Some(number) = value.strip_prefix('f') else {
        return false;
    };
    matches!(number.parse::<u8>(), Ok(1..=24))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyBinding {
    pub command: CommandId,
    pub chord: KeyChord,
    pub scope: Scope,
    /// Optional i18n key describing the action in the keymap UI.
    pub title_key: Option<String>,
}

impl KeyBinding {
    pub fn new(command: impl Into<CommandId>, chord: KeyChord, scope: Scope) -> Self {
        Self {
            command: command.into(),
            chord,
            scope,
            title_key: None,
        }
    }

    pub fn with_title(mut self, title_key: impl Into<String>) -> Self {
        self.title_key = Some(title_key.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyConflict {
    pub scope: Scope,
    pub chord: KeyChord,
    pub first: CommandId,
    pub second: CommandId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBinding {
    pub binding: KeyBinding,
    pub source: BindingSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingSource {
    Module,
    Pack,
    Project,
    User,
    Platform,
}

/// One override file applied on top of module defaults.
#[derive(Clone, Debug)]
pub struct KeymapLayer {
    pub label: String,
    pub source: BindingSource,
    pub text: String,
}

/// Effective bindings after applying the layers.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    bindings: Vec<ResolvedBinding>,
    conflicts: Vec<KeyConflict>,
}

impl Keymap {
    /// `layers` is ordered from lowest to highest priority. The caller builds
    /// it as packs, user file, platform file after the module defaults.
    pub fn build(defaults: Vec<KeyBinding>, layers: Vec<KeymapLayer>) -> (Self, Vec<String>) {
        Self::build_with_policy(defaults, layers, false)
    }

    /// `block_conflicts` implements `keymap.conflict.policy = "block"`: a layer
    /// binding that steals a chord from another command in the same scope is
    /// ignored and reported, instead of shadowing the current binding.
    pub fn build_with_policy(
        defaults: Vec<KeyBinding>,
        layers: Vec<KeymapLayer>,
        block_conflicts: bool,
    ) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let mut effective: BTreeMap<(Scope, CommandId), ResolvedBinding> = BTreeMap::new();
        for binding in defaults {
            effective.insert(
                (binding.scope.clone(), binding.command.clone()),
                ResolvedBinding {
                    binding,
                    source: BindingSource::Module,
                },
            );
        }
        for layer in layers {
            match parse_bindings(&layer.text) {
                Ok(bindings) => {
                    for binding in bindings {
                        if layer.source == BindingSource::Project
                            && !binding.command.as_str().starts_with("cmd.")
                            && !binding.command.as_str().starts_with("ext.")
                        {
                            warnings.push(format!(
                                "{}: a project pack cannot bind `{}`; binding ignored",
                                layer.label, binding.command.as_str()
                            ));
                            continue;
                        }
                        if block_conflicts {
                            let taken = effective.values().find(|resolved| {
                                resolved.binding.scope == binding.scope
                                    && resolved.binding.chord == binding.chord
                                    && resolved.binding.command != binding.command
                            });
                            if let Some(taken) = taken {
                                warnings.push(format!(
                                    "{}: `{}` in scope `{}` is already used by `{}`; binding ignored",
                                    layer.label,
                                    binding.chord,
                                    binding.scope,
                                    taken.binding.command
                                ));
                                continue;
                            }
                        }
                        effective.insert(
                            (binding.scope.clone(), binding.command.clone()),
                            ResolvedBinding {
                                binding,
                                source: layer.source,
                            },
                        );
                    }
                }
                Err(error) => warnings.push(format!("{}: {error}; file ignored", layer.label)),
            }
        }
        let bindings: Vec<ResolvedBinding> = effective.into_values().collect();
        let mut by_chord: BTreeMap<(Scope, KeyChord), CommandId> = BTreeMap::new();
        let mut conflicts = Vec::new();
        for resolved in &bindings {
            let key = (
                resolved.binding.scope.clone(),
                resolved.binding.chord.clone(),
            );
            match by_chord.get(&key) {
                Some(existing) if existing != &resolved.binding.command => {
                    conflicts.push(KeyConflict {
                        scope: resolved.binding.scope.clone(),
                        chord: resolved.binding.chord.clone(),
                        first: existing.clone(),
                        second: resolved.binding.command.clone(),
                    });
                }
                Some(_) => {}
                None => {
                    by_chord.insert(key, resolved.binding.command.clone());
                }
            }
        }
        (
            Self {
                bindings,
                conflicts,
            },
            warnings,
        )
    }

    pub fn bindings(&self) -> &[ResolvedBinding] {
        &self.bindings
    }

    pub fn conflicts(&self) -> &[KeyConflict] {
        &self.conflicts
    }

    /// First effective binding whose chord matches, regardless of scope. The
    /// shell applies scopes as features arrive; scope still guards conflicts.
    pub fn find(
        &self,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
        key: &str,
    ) -> Option<&ResolvedBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.binding.chord.matches(ctrl, alt, shift, meta, key))
    }
}

#[derive(Deserialize)]
struct KeymapFile {
    #[serde(default)]
    binding: Vec<RawBinding>,
}

#[derive(Deserialize)]
struct RawBinding {
    #[serde(alias = "id")]
    command: String,
    #[serde(alias = "key")]
    chord: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    title_key: Option<String>,
}

/// Parses one `keymap.toml` document.
pub fn parse_bindings(text: &str) -> Result<Vec<KeyBinding>, String> {
    let file: KeymapFile = toml::from_str(text).map_err(|error| error.to_string())?;
    file.binding
        .into_iter()
        .enumerate()
        .map(|(index, raw)| {
            let chord = KeyChord::parse(&raw.chord)
                .map_err(|error| format!("binding #{}: {error}", index + 1))?;
            let scope = raw
                .scope
                .parse::<Scope>()
                .map_err(|error| format!("binding #{}: {error}", index + 1))?;
            let mut binding = KeyBinding::new(raw.command, chord, scope);
            binding.title_key = raw.title_key;
            Ok(binding)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_parses_canonical_and_named_keys() {
        let chord = KeyChord::parse("ctrl + shift + k").unwrap();
        assert!(chord.ctrl && chord.shift && !chord.alt && !chord.meta);
        assert_eq!(chord.key, "K");
        assert_eq!(chord.to_string(), "Ctrl+Shift+K");
        assert_eq!(KeyChord::parse("F5").unwrap().key, "F5");
        assert_eq!(KeyChord::parse("esc").unwrap().key, "Escape");
        assert_eq!(KeyChord::parse("Meta+Enter").unwrap().key, "Enter");
    }

    #[test]
    fn grammar_rejects_unknown_and_duplicate_parts() {
        assert!(KeyChord::parse("").is_err());
        assert!(KeyChord::parse("Ctrl+Ctrl+K").is_err());
        assert!(KeyChord::parse("Hyper+K").is_err());
        assert!(KeyChord::parse("Ctrl+Nope").is_err());
        assert!(KeyChord::parse("Ctrl+").is_err());
        assert!(KeyChord::parse("F25").is_err());
    }

    #[test]
    fn parses_keymap_file_with_default_scope() {
        let bindings = parse_bindings(
            r#"
[[binding]]
command = "hello.greet"
chord = "Ctrl+Alt+H"
"#,
        )
        .unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].scope, Scope::Global);
    }

    #[test]
    fn user_layer_replaces_command_binding() {
        let defaults = vec![KeyBinding::new(
            "hello.greet",
            KeyChord::parse("Ctrl+Alt+H").unwrap(),
            Scope::Bench,
        )];
        let user = r#"
[[binding]]
command = "hello.greet"
chord = "Ctrl+Shift+H"
scope = "bench"
"#;
        let layers = vec![KeymapLayer {
            label: "keymap.toml".to_owned(),
            source: BindingSource::User,
            text: user.to_owned(),
        }];
        let (keymap, warnings) = Keymap::build(defaults, layers);
        assert!(warnings.is_empty());
        assert_eq!(keymap.bindings().len(), 1);
        assert_eq!(keymap.bindings()[0].source, BindingSource::User);
        assert_eq!(
            keymap.bindings()[0].binding.chord.to_string(),
            "Ctrl+Shift+H"
        );
    }

    #[test]
    fn block_policy_ignores_conflicting_layer_binding() {
        let defaults = vec![KeyBinding::new(
            "a.one",
            KeyChord::parse("Ctrl+K").unwrap(),
            Scope::Global,
        )];
        let layer = KeymapLayer {
            label: "atalhos.toml".to_owned(),
            source: BindingSource::User,
            text: "[[binding]]\ncommand = \"b.two\"\nkey = \"Ctrl+K\"\nscope = \"global\"\n"
                .to_owned(),
        };
        let (keymap, warnings) =
            Keymap::build_with_policy(defaults.clone(), vec![layer.clone()], true);
        assert_eq!(keymap.bindings().len(), 1);
        assert_eq!(
            keymap.bindings()[0].binding.command,
            CommandId::new("a.one")
        );
        assert!(warnings[0].contains("already used by `a.one`"));

        let (keymap, warnings) = Keymap::build(defaults, vec![layer]);
        assert_eq!(keymap.bindings().len(), 2);
        assert_eq!(keymap.conflicts().len(), 1);
        assert!(warnings.is_empty());
    }

    #[test]
    fn conflicts_are_reported_within_a_scope_only() {
        let defaults = vec![
            KeyBinding::new("a.one", KeyChord::parse("Ctrl+K").unwrap(), Scope::Global),
            KeyBinding::new("b.two", KeyChord::parse("Ctrl+K").unwrap(), Scope::Global),
            KeyBinding::new("c.three", KeyChord::parse("Ctrl+K").unwrap(), Scope::Bench),
        ];
        let (keymap, _) = Keymap::build(defaults, Vec::new());
        assert_eq!(keymap.conflicts().len(), 1);
        assert_eq!(keymap.conflicts()[0].scope, Scope::Global);
    }
}
