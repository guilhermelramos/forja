//! Renderização da grade no egui.
//!
//! Uma linha vira um `LayoutJob` com trechos de mesmo formato. Glifos largos
//! são desenhados por cima em duas colunas, e imagens embutidas viram texturas
//! quando a configuração de GPU permite.

use crate::settings::TermSettings;
use crate::terminal::Terminal;
use crate::theme::Palette;
use crate::vt::{attrs, Glyph, Row};
use forja_kernel::ui_contract::egui::{
    self, Color32, FontData, FontFamily, FontId, Pos2, Rect, TextFormat,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Margem interna da grade.
const PAD_X: f32 = 8.0;
/// Margem vertical.
const PAD_Y: f32 = 6.0;

/// Medidas da grade medidas no quadro atual.
#[derive(Clone, Copy, Debug)]
pub struct GridMetrics {
    /// Canto superior esquerdo da primeira célula.
    pub origin: Pos2,
    /// Largura de uma coluna.
    pub char_width: f32,
    /// Altura de uma linha.
    pub line_height: f32,
    /// Colunas visíveis.
    pub cols: usize,
}

impl GridMetrics {
    /// Célula absoluta sob um ponto.
    pub fn hit(&self, terminal: &Terminal, position: Pos2) -> (usize, usize) {
        let (start, _) = terminal.visible_range();
        let row = (((position.y - self.origin.y) / self.line_height).floor().max(0.0) as usize)
            .min(terminal.visible_rows.saturating_sub(1));
        let x = ((position.x - self.origin.x) / self.char_width).max(0.0);
        let mut col = (x.round() as usize).min(self.cols);
        let relative = start.saturating_sub(terminal.surface.display().history_start) + row;
        if let Some(line) = terminal.surface.display().line_at(relative) {
            if col < self.cols && line.get(col).is_some_and(|cell| cell.width == 0) {
                col = if x < col as f32 { col - 1 } else { col + 1 };
            }
        }
        (start + row, col)
    }
}

/// Resultado de desenhar a grade.
pub struct GridResponse {
    /// Área interativa.
    pub response: egui::Response,
    /// Medidas usadas no quadro.
    pub metrics: GridMetrics,
}

/// Desenha a grade e devolve a resposta de interação.
#[allow(clippy::too_many_arguments)]
pub fn draw_grid(
    ui: &mut egui::Ui,
    terminal: &mut Terminal,
    settings: &TermSettings,
    palette: &Palette,
    textures: &mut HashMap<u32, egui::TextureHandle>,
    body: Rect,
    focused: bool,
) -> GridResponse {
    let font_id = terminal_font(ui.ctx(), &settings.font_family, settings.font_size());
    let char_width = ui.fonts_mut(|fonts| fonts.glyph_width(&font_id, 'M')).max(1.0);
    let line_height = settings.font_size() * 1.24;
    let cols = ((body.width() - PAD_X * 2.0) / char_width)
        .floor()
        .clamp(1.0, 400.0) as usize;
    let rows = ((body.height() - PAD_Y * 2.0) / line_height)
        .floor()
        .clamp(1.0, 200.0) as usize;
    terminal.resize(cols, rows);
    terminal.refresh_search();

    let response = ui.interact(
        body,
        ui.id().with(("forja-terminal-body", terminal.id)),
        egui::Sense::click_and_drag(),
    );

    let metrics = GridMetrics {
        origin: body.min + egui::vec2(PAD_X, PAD_Y),
        char_width,
        line_height,
        cols,
    };
    let painter = ui.painter().with_clip_rect(body);
    painter.rect_filled(body, 0.0, palette.background);
    if focused {
        painter.rect_stroke(
            body.shrink(0.5),
            egui::CornerRadius::same(2),
            egui::Stroke::new(1.0_f32, palette.cursor.gamma_multiply(0.45)),
            egui::StrokeKind::Inside,
        );
    }

    if settings.gpu_mode != "off" {
        ensure_textures(ui.ctx(), terminal, textures);
    }

    let (start, end) = terminal.visible_range();
    let history_start = terminal.surface.display().history_start;
    let mut wide_present = false;
    for (offset, absolute) in (start..end).enumerate() {
        let Some(line) = terminal.surface.display().line_at(absolute.saturating_sub(history_start)) else {
            continue;
        };
        if line.iter().any(|cell| cell.width == 2) {
            wide_present = true;
        }
        let y = metrics.origin.y + offset as f32 * line_height;
        let job = line_job(
            line,
            cols,
            palette,
            font_id.clone(),
            line_height,
        );
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        let origin = egui::pos2(metrics.origin.x, y);
        painter.galley(origin, galley.clone(), Color32::WHITE);
        paint_wide_glyphs(ui, line, &galley.job, origin, &metrics, palette);
        for (col, cell) in line.iter().take(cols).enumerate() {
            if cell.flags & attrs::OVERLINE != 0 {
                let (fg, _) = colors(*cell, palette);
                let left = origin + egui::vec2(col as f32 * char_width, 1.0);
                painter.line_segment([left, left + egui::vec2(char_width, 0.0)], egui::Stroke::new(1.0_f32, fg));
            }
        }
        if settings.gpu_mode != "off" {
            paint_images(
                ui,
                terminal,
                absolute,
                origin,
                &metrics,
                textures,
            );
        }
        paint_search(ui, terminal, absolute, origin, &metrics, palette);
        paint_selection(ui, terminal, absolute, origin, &metrics, palette);
        paint_cursor(ui, terminal, absolute, origin, &metrics, settings, palette, focused);
    }
    if wide_present {
        ensure_wide_fonts(ui.ctx());
    }
    GridResponse { response, metrics }
}

fn line_job(
    line: &Row,
    cols: usize,
    palette: &Palette,
    font_id: FontId,
    line_height: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let mut run = String::new();
    let mut previous: Option<TextFormat> = None;
    for col in 0..cols {
        let cell = line.display_cell(col, cols);
        let (fg, bg) = colors(cell, palette);
        let format = TextFormat {
            font_id: font_id.clone(),
            color: fg,
            background: bg,
            strikethrough: if cell.flags & attrs::STRIKE != 0 { egui::Stroke::new(1.0_f32, fg) } else { egui::Stroke::NONE },
            italics: cell.flags & attrs::ITALIC != 0,
            underline: if cell.flags & attrs::UNDERLINE != 0 {
                egui::Stroke::new(1.0_f32, fg)
            } else {
                egui::Stroke::NONE
            },
            line_height: Some(line_height),
            ..Default::default()
        };
        if previous.as_ref().is_some_and(|old| *old != format) {
            if let Some(old) = previous.take() {
                job.append(&run, 0.0, old);
                run.clear();
            }
        }
        previous = Some(format);
        if cell.width == 1 && !cell.emoji {
            run.push(cell.ch);
        } else if cell.emoji && cell.width == 1 {
            run.push(cell.ch);
            run.push('\u{fe0f}');
        } else {
            run.push(' ');
        }
    }
    if let Some(format) = previous {
        job.append(&run, 0.0, format);
    }
    job.wrap.max_width = f32::INFINITY;
    job
}

fn colors(cell: Glyph, palette: &Palette) -> (Color32, Color32) {
    let mut fg = palette.resolve(cell.fg_source, cell.fg, false);
    let mut bg = palette.resolve(cell.bg_source, cell.bg, true);
    if cell.flags & attrs::REVERSE != 0 {
        std::mem::swap(&mut fg, &mut bg);
        if fg == Color32::TRANSPARENT {
            fg = palette.background;
        }
    }
    if cell.flags & attrs::DIM != 0 {
        fg = fg.gamma_multiply(0.6);
    }
    if cell.flags & attrs::CONCEAL != 0 {
        fg = if bg == Color32::TRANSPARENT {
            palette.background
        } else {
            bg
        };
    }
    (fg, bg)
}

fn paint_wide_glyphs(
    ui: &egui::Ui,
    line: &Row,
    job: &egui::text::LayoutJob,
    origin: Pos2,
    metrics: &GridMetrics,
    palette: &Palette,
) {
    let mut offset = 0;
    let cols = job.text.chars().count();
    for (col, cell) in line.iter().take(cols).enumerate() {
        if cell.width == 2 && col + 1 < cols {
            if let Some(section) = job.sections.iter().find(|section| section.byte_range.contains(&offset)) {
                let (fg, _) = colors(*cell, palette);
                let mut format = section.format.clone();
                format.background = Color32::TRANSPARENT;
                format.underline = egui::Stroke::NONE;
                format.color = fg;
                let mut glyph = egui::text::LayoutJob::default();
                let text = if cell.emoji {
                    format!("{}\u{fe0f}", cell.ch)
                } else {
                    cell.ch.to_string()
                };
                glyph.append(&text, 0.0, format);
                let galley = ui.fonts_mut(|fonts| fonts.layout_job(glyph));
                let rect = Rect::from_min_size(
                    origin + egui::vec2(col as f32 * metrics.char_width, 0.0),
                    egui::vec2(metrics.char_width * 2.0, metrics.line_height),
                );
                let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
                let position = rect.min
                    + egui::vec2(((rect.width() - galley.size().x) / 2.0).max(0.0), 0.0);
                painter.galley(position, galley, Color32::WHITE);
            }
        }
        offset += if cell.width != 1 { 1 } else { cell.ch.len_utf8() };
    }
}

fn paint_images(
    ui: &egui::Ui,
    terminal: &Terminal,
    absolute: usize,
    origin: Pos2,
    metrics: &GridMetrics,
    textures: &HashMap<u32, egui::TextureHandle>,
) {
    let Some(line) = terminal
        .surface.display()
        .line_at(absolute.saturating_sub(terminal.surface.display().history_start))
    else {
        return;
    };
    let mut seen: Vec<u32> = Vec::new();
    for cell in line.iter() {
        if cell.image == 0 || seen.contains(&cell.image) {
            continue;
        }
        let id = cell.image;
        seen.push(id);
        let Some(image) = terminal.surface.display().images.iter().find(|image| image.id == id) else {
            continue;
        };
        let Some(col) = line.iter().position(|cell| cell.image == id) else {
            continue;
        };
        let mut above = 0usize;
        while above + 1 < image.rows {
            let Some(previous) = terminal
                .surface.display()
                .line_at(absolute.saturating_sub(above + 1).saturating_sub(terminal.surface.display().history_start))
            else {
                break;
            };
            if previous.get(col).is_none_or(|cell| cell.image != id) {
                break;
            }
            above += 1;
        }
        let Some(texture) = textures.get(&id) else {
            continue;
        };
        let rect = Rect::from_min_size(
            origin
                + egui::vec2(
                    col as f32 * metrics.char_width,
                    -(above as f32) * metrics.line_height,
                ),
            egui::vec2(
                image.cols as f32 * metrics.char_width,
                image.rows as f32 * metrics.line_height,
            ),
        );
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

fn paint_search(
    ui: &egui::Ui,
    terminal: &Terminal,
    absolute: usize,
    origin: Pos2,
    metrics: &GridMetrics,
    palette: &Palette,
) {
    if !terminal.search.open {
        return;
    }
    let current = terminal.current_hit();
    for hit in &terminal.search.hits {
        if hit.row != absolute {
            continue;
        }
        let rect = Rect::from_min_max(
            origin + egui::vec2(hit.start as f32 * metrics.char_width, 0.0),
            origin + egui::vec2(hit.end as f32 * metrics.char_width, metrics.line_height),
        );
        let active = current.is_some_and(|current| current == *hit);
        ui.painter().rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(
                255,
                193,
                77,
                if active { 130 } else { 70 },
            ),
        );
    }
    let _ = palette;
}

fn paint_selection(
    ui: &egui::Ui,
    terminal: &Terminal,
    absolute: usize,
    origin: Pos2,
    metrics: &GridMetrics,
    palette: &Palette,
) {
    let Some(range) = terminal.selection.and_then(|selection| {
        selection.range(
            absolute,
            terminal.surface.display().cols,
            terminal.surface.display().generation,
        )
    }) else {
        return;
    };
    let rect = Rect::from_min_max(
        origin + egui::vec2(range.start as f32 * metrics.char_width, 0.0),
        origin + egui::vec2(range.end as f32 * metrics.char_width, metrics.line_height),
    );
    ui.painter()
        .rect_filled(rect, 0.0, palette.selection.gamma_multiply(0.65));
}

#[allow(clippy::too_many_arguments)]
fn paint_cursor(
    ui: &egui::Ui,
    terminal: &Terminal,
    absolute: usize,
    origin: Pos2,
    metrics: &GridMetrics,
    settings: &TermSettings,
    palette: &Palette,
    focused: bool,
) {
    if !terminal.surface.display().cursor_visible {
        return;
    }
    let cursor_row = terminal.surface.display().history_start + terminal.surface.display().row;
    if absolute != cursor_row {
        return;
    }
    let col = terminal.surface.display().col.min(metrics.cols.saturating_sub(1));
    let base = origin + egui::vec2(col as f32 * metrics.char_width, 0.0);
    let shape = terminal.surface.display().cursor_shape;
    let (visible, wake) = cursor_blink(shape, focused, ui.input(|input| input.time));
    if let Some(wake) = wake { ui.ctx().request_repaint_after(wake); }
    if !visible { return; }
    let style = match shape { Some(3 | 4) => "underline", Some(5 | 6) => "bar", Some(_) => "block", None => settings.cursor_style.as_str() };
    let (offset, size) = match style {
        "bar" => (egui::Vec2::ZERO, egui::vec2(2.0, metrics.line_height)),
        "underline" => (
            egui::vec2(0.0, metrics.line_height - 2.0),
            egui::vec2(metrics.char_width, 2.0),
        ),
        _ => (
            egui::Vec2::ZERO,
            egui::vec2(metrics.char_width, metrics.line_height),
        ),
    };
    let rect = Rect::from_min_size(base + offset, size);
    if focused { ui.painter().rect_filled(rect, 0.0, palette.cursor.gamma_multiply(0.65)); }
    else { ui.painter().rect_stroke(Rect::from_min_size(base, egui::vec2(metrics.char_width, metrics.line_height)), 0.0, egui::Stroke::new(1.0_f32, palette.cursor), egui::StrokeKind::Inside); }
}

fn ensure_textures(
    ctx: &egui::Context,
    terminal: &Terminal,
    textures: &mut HashMap<u32, egui::TextureHandle>,
) {
    let ids: Vec<u32> = terminal.surface.display().images.iter().map(|image| image.id).collect();
    textures.retain(|id, _| ids.contains(id));
    for image in &terminal.surface.display().images {
        if textures.contains_key(&image.id) {
            continue;
        }
        let Ok(decoded) = image::load_from_memory(&image.data) else {
            continue;
        };
        let rgba = decoded.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let texture = ctx.load_texture(
            format!("forja-terminal-image-{}", image.id),
            color,
            egui::TextureOptions::LINEAR,
        );
        textures.insert(image.id, texture);
    }
}

/// Instala uma família monoespaçada pedida, uma vez por sessão.
pub fn terminal_font(ctx: &egui::Context, family: &str, size: f32) -> FontId {
    let (key, files) = match family {
        "cascadia_code" => (
            "forja-terminal-cascadia-code",
            &["CascadiaCode.ttf", "CascadiaCode-VariableFont_wght.ttf"][..],
        ),
        "cascadia_mono" => ("forja-terminal-cascadia-mono", &["CascadiaMono.ttf"][..]),
        "consolas" => ("forja-terminal-consolas", &["consola.ttf"][..]),
        _ => return FontId::monospace(size),
    };
    let flag = egui::Id::new(("forja-terminal-font", key));
    if !ctx.data(|data| data.get_temp::<bool>(flag).unwrap_or(false)) {
        ctx.data_mut(|data| data.insert_temp(flag, true));
        if let Some(bytes) = read_windows_font(files) {
            let mut definitions = ctx.fonts_mut(|fonts| fonts.definitions().clone());
            definitions
                .font_data
                .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
            definitions.families.insert(
                FontFamily::Name(key.into()),
                vec![key.to_owned()],
            );
            ctx.set_fonts(definitions);
            ctx.request_repaint();
        }
    }
    if ctx.fonts_mut(|fonts| fonts.definitions().families.contains_key(&FontFamily::Name(key.into()))) {
        FontId::new(size, FontFamily::Name(key.into()))
    } else {
        FontId::monospace(size)
    }
}

fn read_windows_font(files: &[&str]) -> Option<Vec<u8>> {
    let root = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("Fonts");
    for file in files {
        if let Ok(bytes) = std::fs::read(root.join(file)) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
    }
    None
}

/// Instala fallback para CJK e emoji, uma vez por contexto.
pub fn ensure_wide_fonts(ctx: &egui::Context) {
    static FONTS: OnceLock<Vec<(&'static str, Arc<FontData>)>> = OnceLock::new();
    let flag = egui::Id::new("forja-terminal-wide-fonts");
    if ctx.data(|data| data.get_temp::<bool>(flag).unwrap_or(false)) {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(flag, true));
    let fonts = FONTS.get_or_init(|| {
        let root = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
            .join("Fonts");
        [
            ("forja-terminal-cjk", "msyh.ttc"),
            ("forja-terminal-japanese", "msgothic.ttc"),
            ("forja-terminal-korean", "malgun.ttf"),
            ("forja-terminal-emoji", "seguiemj.ttf"),
        ]
        .into_iter()
        .filter_map(|(name, file)| {
            std::fs::read(root.join(file))
                .ok()
                .map(|bytes| (name, Arc::new(FontData::from_owned(bytes))))
        })
        .collect()
    });
    if fonts.is_empty() {
        return;
    }
    let mut definitions = ctx.fonts_mut(|fonts| fonts.definitions().clone());
    for (name, data) in fonts {
        definitions.font_data.insert((*name).to_owned(), Arc::clone(data));
        definitions
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push((*name).to_owned());
    }
    ctx.set_fonts(definitions);
    ctx.request_repaint();
}

fn cursor_blink(shape: Option<u8>, focused: bool, time: f64) -> (bool, Option<std::time::Duration>) {
    if !focused || !shape.is_some_and(|shape| matches!(shape, 0 | 1 | 3 | 5)) { return (true, None); }
    let phase = time.rem_euclid(1.0);
    (phase < 0.5, Some(std::time::Duration::from_secs_f64(if phase < 0.5 { 0.5 - phase } else { 1.0 - phase })))
}
#[cfg(test)]
mod protocol_render_tests {
    use super::*;
    #[test]
    fn cursor_blink_schedules_only_explicit_focused_program_modes() {
        for shape in 0..=6 {
            assert_eq!(cursor_blink(Some(shape), false, 0.75), (true,None));
            let (visible,wake) = cursor_blink(Some(shape),true,0.75);
            if matches!(shape,0|1|3|5) { assert!(!visible); assert_eq!(wake.unwrap().as_millis(),250); }
            else { assert!(visible); assert!(wake.is_none()); }
        }
        assert_eq!(cursor_blink(None,true,0.75),(true,None));
    }
    #[test]
    fn sgr_text_format_renders_strike_italic_underline_and_dim() {
        let palette = Palette { background:Color32::BLACK,foreground:Color32::WHITE,cursor:Color32::WHITE,selection:Color32::BLUE,selection_foreground:Color32::WHITE,ansi:[Color32::WHITE;16] };
        let mut surface = crate::vt::Surface::new();
        surface.feed(b"\x1b[2;3;4;9mX");
        let job = line_job(surface.line_at(0).unwrap(),1,&palette,FontId::monospace(14.0),18.0);
        let format = &job.sections[0].format;
        assert!(format.italics); assert!(format.strikethrough.width > 0.0);assert!(format.underline.width > 0.0);
        assert_ne!(format.color,Color32::WHITE);
    }
}
