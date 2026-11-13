//! Tradução de teclas egui para sequências de terminal.

use forja_kernel::ui_contract::egui;

/// Bytes de uma tecla pressionada, respeitando o modo de cursor do aplicativo.
pub fn key_bytes(key: egui::Key, modifiers: egui::Modifiers, application: bool) -> Vec<u8> {
    use egui::Key::*;
    let modifier = 1
        + usize::from(modifiers.shift)
        + 2 * usize::from(modifiers.alt)
        + 4 * usize::from(modifiers.ctrl);
    let letter = match key {
        ArrowUp => Some('A'),
        ArrowDown => Some('B'),
        ArrowRight => Some('C'),
        ArrowLeft => Some('D'),
        Home => Some('H'),
        End => Some('F'),
        _ => None,
    };
    if let Some(letter) = letter {
        return if modifier > 1 {
            format!("\x1b[1;{modifier}{letter}")
        } else if application {
            format!("\x1bO{letter}")
        } else {
            format!("\x1b[{letter}")
        }
        .into_bytes();
    }
    if modifiers.ctrl {
        let name = format!("{key:?}");
        if name.len() == 1 && name.as_bytes()[0].is_ascii_uppercase() {
            let control = name.as_bytes()[0] - b'A' + 1;
            return if modifiers.alt {
                vec![27, control]
            } else {
                vec![control]
            };
        }
    }
    let sequence = match key {
        Enter => "\r",
        Backspace if modifiers.ctrl => "\x08",
        Backspace => "\x7f",
        Tab if modifiers.shift => "\x1b[Z",
        Tab => "\t",
        Escape => "\x1b",
        Insert => "\x1b[2~",
        Delete => "\x1b[3~",
        PageUp => "\x1b[5~",
        PageDown => "\x1b[6~",
        F1 => "\x1bOP",
        F2 => "\x1bOQ",
        F3 => "\x1bOR",
        F4 => "\x1bOS",
        F5 => "\x1b[15~",
        F6 => "\x1b[17~",
        F7 => "\x1b[18~",
        F8 => "\x1b[19~",
        F9 => "\x1b[20~",
        F10 => "\x1b[21~",
        F11 => "\x1b[23~",
        F12 => "\x1b[24~",
        _ => "",
    };
    let mut bytes = Vec::new();
    if modifiers.alt && !sequence.is_empty() {
        bytes.push(27);
    }
    bytes.extend_from_slice(sequence.as_bytes());
    bytes
}

/// Bytes de colagem, com o modo seguro do shell quando ligado.
pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\n").replace('\n', "\r");
    if bracketed {
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.into_bytes()
    }
}

/// Relatório de roda do mouse em modo SGR ou legado.
pub fn wheel_bytes(up: bool, col: usize, row: usize, tracking: bool, sgr: bool) -> Vec<u8> {
    if !tracking {
        return if up {
            b"\x1b[A".to_vec()
        } else {
            b"\x1b[B".to_vec()
        };
    }
    let code = if up { 64 } else { 65 };
    if sgr {
        format!("\x1b[<{code};{};{}M", col + 1, row + 1).into_bytes()
    } else {
        vec![
            27,
            b'[',
            b'M',
            code + 32,
            (col.min(222) + 33) as u8,
            (row.min(222) + 33) as u8,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(ctrl: bool, alt: bool, shift: bool) -> egui::Modifiers {
        egui::Modifiers {
            ctrl,
            alt,
            shift,
            ..Default::default()
        }
    }

    #[test]
    fn arrows_follow_cursor_mode_and_modifiers() {
        assert_eq!(
            key_bytes(egui::Key::ArrowUp, modifiers(false, false, false), false),
            b"\x1b[A"
        );
        assert_eq!(
            key_bytes(egui::Key::ArrowUp, modifiers(false, false, false), true),
            b"\x1bOA"
        );
        assert_eq!(
            key_bytes(egui::Key::ArrowLeft, modifiers(true, false, false), false),
            b"\x1b[1;5D"
        );
    }

    #[test]
    fn control_letters_and_special_keys() {
        assert_eq!(
            key_bytes(egui::Key::C, modifiers(true, false, false), false),
            vec![3]
        );
        assert_eq!(
            key_bytes(egui::Key::Enter, modifiers(false, false, false), false),
            b"\r"
        );
        assert_eq!(
            key_bytes(egui::Key::Tab, modifiers(false, false, true), false),
            b"\x1b[Z"
        );
        assert_eq!(
            key_bytes(egui::Key::F5, modifiers(false, false, false), false),
            b"\x1b[15~"
        );
    }

    #[test]
    fn paste_normalizes_newlines_and_wraps() {
        assert_eq!(paste_bytes("a\nb", false), b"a\rb");
        assert_eq!(paste_bytes("a\nb", true), b"\x1b[200~a\rb\x1b[201~");
    }

    #[test]
    fn wheel_reports_match_enabled_modes() {
        assert_eq!(wheel_bytes(true, 3, 4, true, true), b"\x1b[<64;4;5M");
        assert_eq!(
            wheel_bytes(false, 0, 0, true, false),
            vec![27, b'[', b'M', 97, 33, 33]
        );
        assert_eq!(wheel_bytes(true, 0, 0, false, false), b"\x1b[A");
    }
}
