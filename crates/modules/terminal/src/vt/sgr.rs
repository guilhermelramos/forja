//! SGR e modos privados do parser VT.

use super::grid::{attrs, SOURCE_DEFAULT};
use super::surface::Surface;
use forja_kernel::ui_contract::egui::Color32;

pub(super) fn apply_sgr(surface: &mut Surface, values: &[usize]) {
    let mut index = 0;
    while index < values.len() {
        match values[index] {
            0 => {
                surface.fg = Color32::from_gray(230);
                surface.bg = Color32::TRANSPARENT;
                surface.flags = 0;
                surface.fg_source = SOURCE_DEFAULT;
                surface.bg_source = SOURCE_DEFAULT;
            }
            39 => {
                surface.fg = Color32::from_gray(230);
                surface.fg_source = SOURCE_DEFAULT;
            }
            49 => {
                surface.bg = Color32::TRANSPARENT;
                surface.bg_source = SOURCE_DEFAULT;
            }
            1 => surface.flags |= attrs::BOLD,
            2 => surface.flags |= attrs::DIM,
            3 => surface.flags |= attrs::ITALIC,
            4 => surface.flags |= attrs::UNDERLINE,
            7 => surface.flags |= attrs::REVERSE,
            9 => surface.flags |= attrs::STRIKE,
            53 => surface.flags |= attrs::OVERLINE,
            29 => surface.flags &= !attrs::STRIKE,
            55 => surface.flags &= !attrs::OVERLINE,
            8 => surface.flags |= attrs::CONCEAL,
            22 => surface.flags &= !(attrs::BOLD | attrs::DIM),
            23 => surface.flags &= !attrs::ITALIC,
            24 => surface.flags &= !attrs::UNDERLINE,
            27 => surface.flags &= !attrs::REVERSE,
            28 => surface.flags &= !attrs::CONCEAL,
            40..=47 => {
                surface.bg_source = (values[index] - 40) as u16;
                surface.bg = Color32::TRANSPARENT;
            }
            100..=107 => {
                surface.bg_source = (values[index] - 100 + 8) as u16;
                surface.bg = Color32::TRANSPARENT;
            }
            30..=37 => {
                surface.fg_source = (values[index] - 30) as u16;
                surface.fg = Color32::TRANSPARENT;
            }
            90..=97 => {
                surface.fg_source = (values[index] - 90 + 8) as u16;
                surface.fg = Color32::TRANSPARENT;
            }
            38 | 48 if values.get(index + 1) == Some(&2) && values.len() > index + 4 => {
                let color = Color32::from_rgb(
                    values[index + 2].min(255) as u8,
                    values[index + 3].min(255) as u8,
                    values[index + 4].min(255) as u8,
                );
                if values[index] == 38 {
                    surface.fg = color;
                    surface.fg_source = super::grid::SOURCE_TRUE_COLOR;
                } else {
                    surface.bg = color;
                    surface.bg_source = super::grid::SOURCE_TRUE_COLOR;
                }
                index += 4;
            }
            38 | 48 if values.get(index + 1) == Some(&5) && values.len() > index + 2 => {
                let color_index = values[index + 2].min(255) as u16;
                if values[index] == 38 {
                    surface.fg = Color32::TRANSPARENT;
                    surface.fg_source = color_index;
                } else {
                    surface.bg = Color32::TRANSPARENT;
                    surface.bg_source = color_index;
                }
                index += 2;
            }
            _ => {}
        }
        index += 1;
    }
}

pub(super) fn apply_modes(surface: &mut Surface, values: &[usize], enabled: bool) {
    for code in values {
        match code {
            2026 => surface.synchronize(enabled),
            1004 => surface.focus_reporting = enabled,
            25 => surface.cursor_visible = enabled,
            1 => surface.application_cursor = enabled,
            7 => surface.autowrap = enabled,
            2004 => surface.bracketed_paste = enabled,
            1000 | 1002 | 1003 => surface.mouse_tracking = enabled,
            1006 => surface.mouse_sgr = enabled,
            1048 => {
                if enabled {
                    surface.saved = (surface.row, surface.col);
                } else {
                    surface.row = surface.saved.0.min(surface.rows - 1);
                    surface.col = surface.saved.1.min(surface.cols - 1);
                }
            }
            47 | 1047 | 1049 => {
                if enabled && surface.alternate.is_some() {
                    continue;
                }
                surface.generation = surface.generation.wrapping_add(1);
                if enabled && surface.alternate.is_none() {
                    surface.alternate = Some(std::mem::replace(
                        &mut surface.lines,
                        (0..surface.rows).map(|_| super::grid::Row::blank(surface.cols)).collect(),
                    ));
                    surface.alternate_cursor = (surface.row, surface.col);
                    surface.alternate_margins = (surface.top, surface.bottom);
                    surface.top = 0;
                    surface.bottom = surface.rows - 1;
                    surface.row = 0;
                    surface.col = 0;
                } else if let Some(lines) = surface.alternate.take() {
                    surface.lines = lines;
                    surface.lines.resize_with(surface.rows, || {
                        super::grid::Row::blank(surface.cols)
                    });
                    let blank = surface.blank_cell();
                    for line in &mut surface.lines {
                        line.resize(surface.cols, blank);
                    }
                    surface.row = surface.alternate_cursor.0.min(surface.rows - 1);
                    surface.col = surface.alternate_cursor.1.min(surface.cols - 1);
                    surface.top = surface.alternate_margins.0.min(surface.rows - 1);
                    surface.bottom = surface.alternate_margins.1.min(surface.rows - 1);
                }
            }
            _ => {}
        }
    }
}

