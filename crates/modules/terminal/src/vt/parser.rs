//! Parser VT: CSI, OSC, APC, BEL, alt screen, regiões de rolagem e imagens.
//!
//! O parser é um autômato simples que acumula um escape em texto. Entrada
//! inválida nunca derruba a sessão: o estado volta ao chão e o texto seguinte é
//! impresso normalmente.

use super::grid::{character_width, Glyph};
use super::image::decode_base64;
use super::sgr::{apply_modes, apply_sgr};
use super::surface::{Signal, Surface};

const GROUND: u8 = 0;
const ESCAPE: u8 = 1;
const CSI: u8 = 2;
const OSC: u8 = 3;
const OSC_ST: u8 = 4;
const ESC_INTERMEDIATE: u8 = 5;
const APC: u8 = 6;
const APC_ST: u8 = 7;

const MAX_OSC: usize = 1_400_000;
const MAX_APC: usize = 16_000_000;

/// Consome um bloco de bytes do processo.
pub fn feed(surface: &mut Surface, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    surface.revision = surface.revision.wrapping_add(1);
    surface.pending.extend_from_slice(bytes);
    let mut consumed = 0;
    let mut text = String::new();
    while consumed < surface.pending.len() {
        match std::str::from_utf8(&surface.pending[consumed..]) {
            Ok(valid) => {
                text.push_str(valid);
                consumed = surface.pending.len();
            }
            Err(error) => {
                let end = consumed + error.valid_up_to();
                text.push_str(&String::from_utf8_lossy(&surface.pending[consumed..end]));
                consumed = end;
                if let Some(length) = error.error_len() {
                    text.push('\u{fffd}');
                    consumed += length;
                } else {
                    break;
                }
            }
        }
    }
    surface.pending.drain(..consumed);
    for ch in text.chars() {
        step(surface, ch);
    }
}

fn step(surface: &mut Surface, ch: char) {
    match surface.parser {
        ESCAPE => {
            escape_step(surface, ch);
            return;
        }
        ESC_INTERMEDIATE => {
            if !('\u{20}'..='\u{2f}').contains(&ch) {
                surface.parser = GROUND;
            }
            return;
        }
        CSI => {
            if ('@'..='~').contains(&ch) {
                csi(surface, ch);
                surface.parser = GROUND;
            } else if surface.escape.len() < 256 {
                surface.escape.push(ch);
            } else {
                surface.parser = GROUND;
            }
            return;
        }
        OSC => {
            if ch == '\u{7}' {
                finish_osc(surface);
                surface.parser = GROUND;
            } else if ch == '\u{1b}' {
                surface.parser = OSC_ST;
            } else if surface.escape.len() < MAX_OSC {
                surface.escape.push(ch);
            }
            return;
        }
        OSC_ST => {
            if ch == '\\' {
                finish_osc(surface);
                surface.parser = GROUND;
            } else {
                surface.parser = OSC;
                if surface.escape.len() < MAX_OSC {
                    surface.escape.push(ch);
                }
            }
            return;
        }
        APC => {
            if ch == '\u{1b}' {
                surface.parser = APC_ST;
            } else if surface.escape.len() < MAX_APC {
                surface.escape.push(ch);
            }
            return;
        }
        APC_ST => {
            if ch == '\\' {
                finish_apc(surface);
                surface.parser = GROUND;
            } else {
                surface.parser = APC;
                if surface.escape.len() < MAX_APC {
                    surface.escape.push(ch);
                }
            }
            return;
        }
        _ => {}
    }

    match ch {
        '\u{7}' => surface.signal(Signal::Bell),
        '\u{1b}' => surface.parser = ESCAPE,
        '\r' => surface.col = 0,
        '\n' => surface.newline(),
        '\u{8}' => surface.col = surface.col.saturating_sub(1),
        '\t' => surface.col = ((surface.col / 8 + 1) * 8).min(surface.cols - 1),
        ch if !ch.is_control() => print(surface, ch),
        _ => {}
    }
}

fn escape_step(surface: &mut Surface, ch: char) {
    match ch {
        '[' => {
            surface.parser = CSI;
            surface.escape.clear();
        }
        ']' => {
            surface.parser = OSC;
            surface.escape.clear();
        }
        '_' => {
            surface.parser = APC;
            surface.escape.clear();
        }
        '7' => {
            surface.saved = (surface.row, surface.col);
            surface.parser = GROUND;
        }
        '8' => {
            surface.row = surface.saved.0.min(surface.rows - 1);
            surface.col = surface.saved.1.min(surface.cols - 1);
            surface.parser = GROUND;
        }
        'M' => {
            if surface.row == surface.top {
                surface.scroll_down(surface.top, surface.bottom, 1);
            } else {
                surface.row = surface.row.saturating_sub(1);
            }
            surface.parser = GROUND;
        }
        'D' => {
            surface.newline();
            surface.parser = GROUND;
        }
        'E' => {
            surface.col = 0;
            surface.newline();
            surface.parser = GROUND;
        }
        'c' => {
            surface.reset();
            surface.parser = GROUND;
        }
        '\u{20}'..='\u{2f}' => surface.parser = ESC_INTERMEDIATE,
        _ => surface.parser = GROUND,
    }
}

fn print(surface: &mut Surface, ch: char) {
    if ch == '\u{fe0f}' && surface.col > 0 {
        let mut previous = surface.col.min(surface.cols) - 1;
        if surface.lines[surface.row][previous].width == 0 && previous > 0 {
            previous -= 1;
        }
        surface.lines[surface.row][previous].emoji = true;
        if surface.lines[surface.row][previous].width == 1 && previous + 1 < surface.cols {
            surface.clear_glyph(surface.row, previous + 1);
            surface.lines[surface.row][previous].width = 2;
            let cell = surface.lines[surface.row][previous];
            surface.set_wide(surface.row, previous, cell);
            surface.col = previous + 2;
        }
        return;
    }
    // Seletores de variação e ZWJ não ocupam coluna; composição completa fica
    // para uma camada de shaping futura.
    if matches!(ch as u32, 0x200d | 0xfe00..=0xfe0f | 0xe0100..=0xe01ef) {
        return;
    }
    let mut width = character_width(ch).min(surface.cols);
    if surface.col + width > surface.cols {
        if surface.autowrap {
            surface.col = 0;
            surface.newline();
        } else {
            surface.col = surface.cols - 1;
            width = 1;
        }
    }
    surface.clear_glyph(surface.row, surface.col);
    if width == 2 {
        surface.clear_glyph(surface.row, surface.col + 1);
    }
    let ch = if width < character_width(ch) {
        '\u{fffd}'
    } else {
        ch
    };
    surface.last_printed = Some(ch);
    let cell = Glyph {
        ch,
        fg: surface.fg,
        fg_source: surface.fg_source,
        bg: surface.bg,
        bg_source: surface.bg_source,
        flags: surface.flags,
        width: width as u8,
        emoji: false,
        image: 0,
        link: surface.hyperlink,
    };
    let (row, col) = (surface.row, surface.col);
    surface.lines[row][col] = cell;
    if width == 2 {
        surface.lines[row][col + 1] = Glyph {
            ch: ' ',
            width: 0,
            ..cell
        };
    }
    surface.col += width;
}

fn move_vertical(surface: &mut Surface, count: usize, down: bool) {
    let inside = (surface.top..=surface.bottom).contains(&surface.row);
    let (top, bottom) = if inside {
        (surface.top, surface.bottom)
    } else {
        (0, surface.rows - 1)
    };
    surface.row = if down {
        surface.row.saturating_add(count).min(bottom)
    } else {
        surface.row.saturating_sub(count).max(top)
    };
}

fn finish_osc(surface: &mut Surface) {
    let escape = std::mem::take(&mut surface.escape);
    let Some((code, payload)) = escape.split_once(';') else {
        return;
    };
    match code {
        "0" | "2" => {
            surface.title = payload.chars().filter(|ch| !ch.is_control()).collect();
        }
        "8" => {
            if let Some((_, url)) = payload.split_once(';') {
                if url.is_empty() {
                    surface.hyperlink = 0;
                } else if surface.links.len() < 4096
                    && url.len() <= 8192
                    && !url.chars().any(char::is_control)
                {
                    surface.hyperlink = match surface.links.iter().position(|item| item == url) {
                        Some(index) => index as u32,
                        None => {
                            surface.links.push(url.to_owned());
                            (surface.links.len() - 1) as u32
                        }
                    };
                }
            }
        }
        "52" => {
            if let Some((_, encoded)) = payload.split_once(';') {
                if let Some(bytes) = decode_base64(encoded, 1_400_000) {
                    if let Ok(text) = String::from_utf8(bytes) {
                        surface.signal(Signal::Clipboard(text));
                    }
                }
            }
        }
        "133" if surface.shell_marks => {
            let mut parts = payload.split(';');
            match parts.next() {
                Some("A") => surface.signal(Signal::Prompt),
                Some("D") => surface.signal(Signal::CommandFinished(
                    parts.next().and_then(|value| value.parse().ok()),
                )),
                _ => {}
            }
        }
        "1337" => finish_iterm(surface, payload),
        _ => {}
    }
}

/// iTerm2 `OSC 1337;File=...:BASE64`.
fn finish_iterm(surface: &mut Surface, payload: &str) {
    if !surface.inline_images {
        return;
    }
    let Some(body) = payload.strip_prefix("File=") else {
        return;
    };
    let Some((arguments, encoded)) = body.split_once(':') else {
        return;
    };
    let argument = |name: &str| {
        arguments.split(';').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then_some(value)
        })
    };
    if argument("inline") != Some("1") {
        return;
    }
    let Some(data) = decode_base64(encoded, 12_000_000) else {
        return;
    };
    let cols = Surface::parse_size(argument("width"));
    let rows = Surface::parse_size(argument("height"));
    surface.place_image(data, cols, rows, true);
}

fn finish_apc(surface: &mut Surface) {
    let apc = std::mem::take(&mut surface.escape);
    if !surface.inline_images {
        return;
    }
    let Some(body) = apc.strip_prefix('G') else {
        return;
    };
    let (params, payload) = body.split_once(';').unwrap_or((body, ""));
    let parameter = |name: &str| {
        params.split(',').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then_some(value)
        })
    };
    if parameter("a") != Some("T") {
        return;
    }
    if parameter("f").is_some_and(|format| format != "100") {
        return;
    }
    let Some(chunk) = decode_base64(payload, 16_000_000) else {
        return;
    };
    let more = parameter("m") == Some("1");
    let mut data = std::mem::take(&mut surface.kitty_data);
    data.extend_from_slice(&chunk);
    if data.len() > 8_000_000 {
        return;
    }
    if more {
        surface.kitty_data = data;
        return;
    }
    let cols = parameter("c").and_then(|value| value.parse().ok());
    let rows = parameter("r").and_then(|value| value.parse().ok());
    surface.place_image(data, cols, rows, false);
}

fn csi(surface: &mut Surface, final_char: char) {
    if surface.escape.starts_with('>') {
        if final_char == 'c' {
            surface.responses.extend_from_slice(b"\x1b[>0;1;0c");
        }
        return;
    }
    if final_char == 'q' && surface.escape.ends_with(' ') {
        if let Ok(shape @ 0..=6) = surface.escape.trim().parse::<u8>() { surface.cursor_shape = Some(shape); }
        return;
    }
    if final_char == 'p' && surface.escape.starts_with('?') && surface.escape.ends_with('$') {
        let mode = surface.escape[1..surface.escape.len()-1].parse::<usize>().unwrap_or(0);
        let state = match mode { 2026 => if surface.synchronized_output { 1 } else { 2 }, 1004 => if surface.focus_reporting { 1 } else { 2 }, _ => 0 };
        surface.responses.extend_from_slice(format!("\x1b[?{mode};{state}$y").as_bytes());
        return;
    }
    if surface.escape.starts_with('=') || surface.escape.chars().any(|ch| (' '..='/').contains(&ch)) {
        return;
    }
    let private = surface.escape.starts_with('?');
    let values: Vec<usize> = surface
        .escape
        .trim_start_matches('?')
        .split([';', ':'])
        .filter(|part| !part.is_empty() || !surface.escape.contains(':'))
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let first = *values.first().unwrap_or(&0);
    let count = first.max(1);
    let blank = surface.blank_cell();
    match final_char {
        'H' | 'f' => {
            surface.row = count.saturating_sub(1).min(surface.rows - 1);
            surface.col = values
                .get(1)
                .copied()
                .unwrap_or(1)
                .max(1)
                .saturating_sub(1)
                .min(surface.cols - 1);
        }
        'A' => move_vertical(surface, count, false),
        'B' => move_vertical(surface, count, true),
        'C' => surface.col = surface.col.saturating_add(count).min(surface.cols - 1),
        'D' => surface.col = surface.col.saturating_sub(count),
        'E' => {
            move_vertical(surface, count, true);
            surface.col = 0;
        }
        'F' => {
            move_vertical(surface, count, false);
            surface.col = 0;
        }
        'G' | '`' => surface.col = count.saturating_sub(1).min(surface.cols - 1),
        'd' => surface.row = count.saturating_sub(1).min(surface.rows - 1),
        'J' => match first {
            2 | 3 => {
                for row in &mut surface.lines {
                    row.fill(blank);
                }
                if first == 3 {
                    surface.history.clear();
                    surface.history_start = 0;
                    surface.generation = surface.generation.wrapping_add(1);
                }
            }
            0 => {
                for col in surface.col.min(surface.cols - 1)..surface.cols {
                    surface.lines[surface.row][col] = blank;
                }
                for row in surface.row + 1..surface.rows {
                    surface.lines[row].fill(blank);
                }
            }
            1 => {
                for row in 0..surface.row {
                    surface.lines[row].fill(blank);
                }
                for col in 0..=surface.col.min(surface.cols - 1) {
                    surface.lines[surface.row][col] = blank;
                }
            }
            _ => {}
        },
        'K' => {
            let (from, to) = match first {
                1 => (0, surface.col.min(surface.cols - 1) + 1),
                2 => (0, surface.cols),
                _ => (surface.col.min(surface.cols - 1), surface.cols),
            };
            surface.lines[surface.row][from..to].fill(blank);
        }
        'r' if !private => {
            let bottom = values.get(1).copied().unwrap_or(surface.rows);
            let bottom = if bottom == 0 { surface.rows } else { bottom };
            if count < bottom && bottom <= surface.rows {
                surface.top = count - 1;
                surface.bottom = bottom - 1;
                surface.row = 0;
                surface.col = 0;
            }
        }
        'L' if (surface.top..=surface.bottom).contains(&surface.row) => {
            surface.scroll_down(surface.row, surface.bottom, count)
        }
        'M' if (surface.top..=surface.bottom).contains(&surface.row) => {
            surface.scroll_up(surface.row, surface.bottom, count)
        }
        'S' => surface.scroll_up(surface.top, surface.bottom, count),
        'T' => surface.scroll_down(surface.top, surface.bottom, count),
        '@' => {
            let col = surface.col.min(surface.cols - 1);
            let shift = count.min(surface.cols - col);
            let line = &mut surface.lines[surface.row];
            line.copy_within(col..surface.cols - shift, col + shift);
            line[col..col + shift].fill(blank);
        }
        'X' => {
            let col = surface.col.min(surface.cols - 1);
            surface.lines[surface.row][col..col.saturating_add(count).min(surface.cols)]
                .fill(blank);
        }
        'P' => {
            let col = surface.col.min(surface.cols - 1);
            let shift = count.min(surface.cols - col);
            let line = &mut surface.lines[surface.row];
            line.copy_within(col + shift..surface.cols, col);
            line[surface.cols - shift..].fill(blank);
        }
        'b' => {
            if let Some(ch) = surface.last_printed {
                for _ in 0..count.min(surface.cols) {
                    print(surface, ch);
                }
            }
        }
        'm' => {
            // Underline subparameters are not separate SGR attributes.
            let sgr: Vec<usize> = surface.escape.split(';').flat_map(|part| {
                if let Some(style) = part.strip_prefix("4:") { vec![if style == "0" { 24 } else { 4 }] }
                else { part.split(':').filter(|value| !value.is_empty() || !surface.escape.contains(':')).map(|value| value.parse().unwrap_or(0)).collect() }
            }).collect();
            apply_sgr(surface, &sgr);
        },
        'n' if first == 6 => surface.responses.extend_from_slice(
            format!("\x1b[{};{}R", surface.row + 1, surface.col + 1).as_bytes(),
        ),
        'c' => surface.responses.extend_from_slice(b"\x1b[?1;2c"),
        's' => surface.saved = (surface.row, surface.col),
        'u' => {
            surface.row = surface.saved.0.min(surface.rows - 1);
            surface.col = surface.saved.1.min(surface.cols - 1);
        }
        'h' | 'l' if private => apply_modes(surface, &values, final_char == 'h'),
        _ => {}
    }
    if final_char == 'J' {
        for line in &mut surface.lines {
            line.repair_wide();
        }
    } else if matches!(final_char, 'K' | '@' | 'X' | 'P') {
        surface.lines[surface.row].repair_wide();
    }
}



#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
