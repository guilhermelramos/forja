//! Teclas do Bigorna. Os atalhos vêm do `keymap` no escopo `anvil`
//! (`anvil.close`, `anvil.run`, …, em `hosts.rs`). Quando um atalho não existe no
//! keymap, vale o padrão da janela.

use eframe::egui::{Key, Modifiers};
use forja_kernel::keymap::{KeyChord, Keymap, Scope};

/// Escopo dos atalhos do Bigorna.
pub(crate) const SCOPE: &str = "anvil";

/// Atalhos do Bigorna em `keymap.toml` de forma de tabela: `(id, tecla, padrão)`.
pub(crate) const BINDINGS: [(&str, &str); 7] = [
    ("anvil.close", "Escape"),
    ("anvil.run", "Enter"),
    ("anvil.previous", "Up"),
    ("anvil.next", "Down"),
    ("anvil.group.next", "Tab"),
    ("anvil.group.previous", "Shift+Tab"),
    ("anvil.run_background", "Ctrl+Enter"),
];

/// Título de cada comando do Bigorna (chave i18n), na mesma ordem de `BINDINGS`.
pub(crate) const COMMANDS: [(&str, &str); 7] = [
    ("anvil.close", "anvil.command.close"),
    ("anvil.run", "anvil.command.run"),
    ("anvil.previous", "anvil.command.previous"),
    ("anvil.next", "anvil.command.next"),
    ("anvil.group.next", "anvil.command.group-next"),
    ("anvil.group.previous", "anvil.command.group-previous"),
    ("anvil.run_background", "anvil.command.run-background"),
];

/// Corpo do `keymap` do host com os atalhos do Bigorna.
pub(crate) fn keymap_text() -> String {
    BINDINGS
        .iter()
        .map(|(id, key)| {
            format!("[[binding]]\nid = \"{id}\"\nkey = \"{key}\"\nscope = \"{SCOPE}\"\n\n")
        })
        .collect()
}

/// Tecla e modificadores de cada ação do Bigorna, já prontos para `consume_key`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AnvilKeys {
    pub close: (Modifiers, Key),
    pub run: (Modifiers, Key),
    pub previous: (Modifiers, Key),
    pub next: (Modifiers, Key),
    pub group_next: (Modifiers, Key),
    pub group_previous: (Modifiers, Key),
    pub run_background: (Modifiers, Key),
}

impl AnvilKeys {
    /// Lê os atalhos do keymap; ação sem atalho usa o padrão de `BINDINGS`.
    pub(crate) fn from_keymap(keymap: &Keymap) -> Self {
        let resolve = |id: &str| {
            chord_in(keymap, id)
                .or_else(|| {
                    BINDINGS
                        .iter()
                        .find(|(bound, _)| *bound == id)
                        .and_then(|(_, key)| KeyChord::parse(key).ok())
                        .and_then(|chord| to_egui(&chord))
                })
                .unwrap_or((Modifiers::NONE, Key::Enter))
        };
        Self {
            close: resolve("anvil.close"),
            run: resolve("anvil.run"),
            previous: resolve("anvil.previous"),
            next: resolve("anvil.next"),
            group_next: resolve("anvil.group.next"),
            group_previous: resolve("anvil.group.previous"),
            run_background: resolve("anvil.run_background"),
        }
    }
}

fn chord_in(keymap: &Keymap, id: &str) -> Option<(Modifiers, Key)> {
    keymap
        .bindings()
        .iter()
        .find(|resolved| {
            resolved.binding.command.as_str() == id
                && resolved.binding.scope == Scope::Custom(SCOPE.to_owned())
        })
        .and_then(|resolved| to_egui(&resolved.binding.chord))
}

/// Converte um atalho do kernel para `egui`. Tecla desconhecida devolve `None`.
pub(crate) fn to_egui(chord: &KeyChord) -> Option<(Modifiers, Key)> {
    let key = key_from_name(&chord.key)?;
    let modifiers = Modifiers {
        alt: chord.alt,
        ctrl: chord.ctrl,
        shift: chord.shift,
        mac_cmd: chord.meta,
        command: chord.ctrl,
    };
    Some((modifiers, key))
}

fn key_from_name(name: &str) -> Option<Key> {
    Some(match name {
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        other => Key::from_name(other)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::keymap::parse_bindings;

    fn keymap(text: &str) -> Keymap {
        let defaults = parse_bindings(text).expect("valid keymap text");
        Keymap::build(defaults, Vec::new()).0
    }

    #[test]
    fn named_keys_map_to_egui_keys() {
        assert_eq!(key_from_name("Escape"), Some(Key::Escape));
        assert_eq!(key_from_name("ArrowDown"), Some(Key::ArrowDown));
        assert_eq!(key_from_name("Tab"), Some(Key::Tab));
        assert_eq!(key_from_name("Nope"), None);
    }

    #[test]
    fn modifiers_follow_the_chord() {
        let chord = KeyChord::parse("Ctrl+Enter").expect("chord");
        let (modifiers, key) = to_egui(&chord).expect("maps");
        assert_eq!(key, Key::Enter);
        assert!(modifiers.ctrl && modifiers.command);
        assert!(!modifiers.shift && !modifiers.alt);
        let (shift_tab, _) = to_egui(&KeyChord::parse("Shift+Tab").unwrap()).unwrap();
        assert!(shift_tab.shift && !shift_tab.ctrl);
    }

    #[test]
    fn keymap_entries_drive_the_window_keys() {
        let keys = AnvilKeys::from_keymap(&keymap(&keymap_text()));
        assert_eq!(keys.close, (Modifiers::NONE, Key::Escape));
        assert_eq!(keys.next, (Modifiers::NONE, Key::ArrowDown));
        assert!(keys.run_background.0.ctrl);
        assert!(keys.group_previous.0.shift);
    }

    #[test]
    fn missing_bindings_fall_back_to_the_defaults() {
        let keys = AnvilKeys::from_keymap(&keymap(""));
        assert_eq!(keys.close, (Modifiers::NONE, Key::Escape));
        assert_eq!(keys.previous, (Modifiers::NONE, Key::ArrowUp));
        assert!(keys.run_background.0.ctrl);
    }
}
