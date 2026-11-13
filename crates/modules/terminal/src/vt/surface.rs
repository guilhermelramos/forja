//! Estado da tela: grade, histórico, buffers alternativos e protocolos.
//!
//! O parser em `parser.rs` traduz bytes; este arquivo mantém a grade e as
//! operações de rolagem. A view consome somente leitura, exceto pela seleção,
//! que mora no terminal.

use super::grid::{Glyph, Row, SOURCE_DEFAULT};
use super::image::InlineImage;
use forja_kernel::ui_contract::egui::Color32;
use std::collections::VecDeque;

/// Evento de protocolo detectado durante a leitura.
#[derive(Clone, Debug, PartialEq)]
pub enum Signal {
    /// BEL.
    Bell,
    /// OSC 133;A — prompt desenhado.
    Prompt,
    /// OSC 133;D — comando terminou.
    CommandFinished(Option<i32>),
    /// OSC 52 — texto para a área de transferência.
    Clipboard(String),
}

/// Limites de histórico do catálogo (`terminal.scroll.history_lines`).
pub const MIN_HISTORY: usize = 1_000;
/// Teto de linhas de histórico.
pub const MAX_HISTORY: usize = 200_000;
/// Valor inicial do catálogo.
pub const DEFAULT_HISTORY: usize = 10_000;

/// Buffer de tela e histórico.
pub struct Surface {
    pub(crate) lines: VecDeque<Row>,
    pub(crate) history: VecDeque<Row>,
    pub(crate) history_start: usize,
    pub(crate) history_limit: usize,
    pub(crate) row: usize,
    pub(crate) col: usize,
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) top: usize,
    pub(crate) bottom: usize,
    pub(crate) fg: Color32,
    pub(crate) fg_source: u16,
    pub(crate) bg: Color32,
    pub(crate) bg_source: u16,
    pub(crate) flags: u8,
    pub(crate) autowrap: bool,
    pub(crate) last_printed: Option<char>,
    pub(crate) saved: (usize, usize),
    pub(crate) alternate: Option<VecDeque<Row>>,
    pub(crate) alternate_cursor: (usize, usize),
    pub(crate) alternate_margins: (usize, usize),
    /// Links OSC 8; o índice 0 é sempre vazio.
    pub links: Vec<String>,
    pub(crate) hyperlink: u32,
    /// Imagens embutidas desta sessão.
    pub images: Vec<InlineImage>,
    pub(crate) inline_images: bool,
    pub(crate) kitty_data: Vec<u8>,
    /// Incrementa a cada `feed`; a view usa para invalidar busca.
    pub revision: u64,
    /// Incrementa ao trocar de buffer ou limpar tudo.
    pub generation: u64,
    /// Título pedido por OSC 0/2.
    pub title: String,
    /// Respostas a consultas do programa (DSR, DA).
    pub responses: Vec<u8>,
    /// Eventos de protocolo pendentes.
    pub signals: Vec<Signal>,
    pub cursor_visible: bool,
    pub application_cursor: bool,
    pub bracketed_paste: bool,
    pub mouse_tracking: bool,
    pub mouse_sgr: bool,
    pub(crate) shell_marks: bool,
    pub(crate) pending: Vec<u8>,
    pub(crate) parser: u8,
    pub(crate) escape: String,
}

impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}

impl Surface {
    /// Consome bytes do processo (parser VT).
    pub fn feed(&mut self, bytes: &[u8]) {
        super::parser::feed(self, bytes);
    }

    /// Cria uma tela 100x30 com histórico de 10 mil linhas.
    pub fn new() -> Self {
        Self {
            lines: (0..30).map(|_| Row::blank(100)).collect(),
            history: VecDeque::new(),
            history_start: 0,
            history_limit: DEFAULT_HISTORY,
            row: 0,
            col: 0,
            cols: 100,
            rows: 30,
            top: 0,
            bottom: 29,
            fg: Color32::from_gray(230),
            fg_source: SOURCE_DEFAULT,
            bg: Color32::TRANSPARENT,
            bg_source: SOURCE_DEFAULT,
            flags: 0,
            autowrap: true,
            last_printed: None,
            saved: (0, 0),
            alternate: None,
            alternate_cursor: (0, 0),
            alternate_margins: (0, 29),
            links: vec![String::new()],
            hyperlink: 0,
            images: Vec::new(),
            inline_images: true,
            kitty_data: Vec::new(),
            revision: 0,
            generation: 0,
            title: String::new(),
            responses: Vec::new(),
            signals: Vec::new(),
            cursor_visible: true,
            application_cursor: false,
            bracketed_paste: false,
            mouse_tracking: false,
            mouse_sgr: false,
            shell_marks: true,
            pending: Vec::new(),
            parser: 0,
            escape: String::new(),
        }
    }

    /// Redimensiona a grade sem reescrever o histórico.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(1);
        let rows = rows.max(1);
        while self.lines.len() > rows {
            if self.row >= rows {
                if let Some(line) = self.lines.pop_front() {
                    self.push_history(line);
                }
                self.row -= 1;
            } else {
                self.lines.pop_back();
            }
        }
        self.cols = cols;
        self.rows = rows;
        for line in &mut self.lines {
            line.resize(cols, Glyph::default());
            line.repair_wide();
        }
        self.lines.resize_with(rows, || Row::blank(cols));
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols - 1);
        self.top = 0;
        self.bottom = rows - 1;
    }

    /// Define o teto do histórico; o catálogo limita entre 1000 e 200000.
    pub fn set_history_limit(&mut self, rows: usize) {
        self.history_limit = rows.clamp(MIN_HISTORY, MAX_HISTORY);
        while self.history.len() > self.history_limit {
            self.history.pop_front();
            self.history_start += 1;
        }
    }

    /// Liga ou desliga a decodificação de imagens embutidas.
    pub fn set_inline_images(&mut self, enabled: bool) {
        self.inline_images = enabled;
        if !enabled {
            self.images.clear();
            self.kitty_data.clear();
        }
    }

    /// Liga a interpretação de OSC 133 (`terminal.shell.marks`).
    pub fn set_shell_marks(&mut self, enabled: bool) {
        self.shell_marks = enabled;
    }

    /// Verdadeiro quando o buffer alternativo está ativo.
    pub fn is_alternate(&self) -> bool {
        self.alternate.is_some()
    }

    /// Total de linhas disponíveis para rolagem.
    pub fn total_rows(&self) -> usize {
        self.history.len() + self.lines.len()
    }

    /// Linha por índice relativo (0 = primeira do histórico).
    pub fn line_at(&self, index: usize) -> Option<&Row> {
        if index < self.history.len() {
            self.history.get(index)
        } else {
            self.lines.get(index - self.history.len())
        }
    }

    /// Texto de todo o histórico e da tela.
    pub fn text(&self) -> String {
        self.history
            .iter()
            .chain(self.lines.iter())
            .map(|line| line.text().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Últimas `count` linhas com texto, sem espaços nem linhas vazias no fim.
    pub fn tail(&self, count: usize) -> String {
        let total = self.total_rows();
        let mut end = total;
        while end > 0
            && self
                .line_at(end - 1)
                .is_none_or(|line| line.text().trim_end().is_empty())
        {
            end -= 1;
        }
        let start = end.saturating_sub(count.max(1));
        (start..end)
            .filter_map(|index| self.line_at(index))
            .map(|line| line.text().trim_end().to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn blank_cell(&self) -> Glyph {
        Glyph {
            ch: ' ',
            fg: self.fg,
            fg_source: self.fg_source,
            bg: self.bg,
            bg_source: self.bg_source,
            flags: self.flags,
            width: 1,
            emoji: false,
            image: 0,
            link: self.hyperlink,
        }
    }

    pub(crate) fn push_history(&mut self, line: Row) {
        if self.alternate.is_none() {
            self.history.push_back(line);
            if self.history.len() > self.history_limit {
                self.history.pop_front();
                self.history_start += 1;
            }
        }
    }

    pub(crate) fn scroll_up(&mut self, top: usize, bottom: usize, count: usize) {
        for _ in 0..count.min(bottom.saturating_sub(top) + 1) {
            if let Some(line) = self.lines.remove(top) {
                if top == 0 && bottom == self.rows - 1 {
                    self.push_history(line);
                }
            }
            self.lines
                .insert(bottom, Row::filled(self.cols, self.blank_cell()));
        }
    }

    pub(crate) fn scroll_down(&mut self, top: usize, bottom: usize, count: usize) {
        for _ in 0..count.min(bottom.saturating_sub(top) + 1) {
            if bottom < self.lines.len() {
                self.lines.remove(bottom);
            }
            self.lines
                .insert(top.min(self.lines.len()), Row::filled(self.cols, self.blank_cell()));
        }
    }

    pub(crate) fn newline(&mut self) {
        if self.row == self.bottom {
            self.scroll_up(self.top, self.bottom, 1);
        } else {
            self.row = (self.row + 1).min(self.rows - 1);
        }
    }

    pub(crate) fn clear_glyph(&mut self, row: usize, col: usize) {
        let blank = self.blank_cell();
        let line = &mut self.lines[row];
        if line[col].width == 0 && col > 0 {
            line[col - 1] = blank;
        }
        if line[col].width == 2 && col + 1 < line.len() {
            line[col + 1] = blank;
        }
        line[col] = blank;
    }

    pub(crate) fn signal(&mut self, event: Signal) {
        if self.signals.len() < 1024 {
            self.signals.push(event);
        }
    }

    /// Reinicia o estado do terminal preservando o que é configuração.
    pub(crate) fn reset(&mut self) {
        let (cols, rows) = (self.cols, self.rows);
        let limit = self.history_limit;
        let inline = self.inline_images;
        let marks = self.shell_marks;
        *self = Self::new();
        self.history_limit = limit;
        self.inline_images = inline;
        self.shell_marks = marks;
        self.resize(cols, rows);
    }

    pub(crate) fn parse_size(value: Option<&str>) -> Option<usize> {
        let value = value?;
        if value == "auto" || value.ends_with('%') || value.ends_with("px") {
            return None;
        }
        value.parse().ok()
    }

    /// Posiciona uma imagem decodificada a partir da célula atual.
    pub(crate) fn place_image(
        &mut self,
        data: Vec<u8>,
        cols: Option<usize>,
        rows: Option<usize>,
        advance: bool,
    ) {
        if !self.inline_images || data.is_empty() || data.len() > 8_000_000 {
            return;
        }
        let Ok(decoded) = image::load_from_memory(&data) else {
            return;
        };
        let (pixel_width, pixel_height) = (decoded.width(), decoded.height());
        if pixel_width == 0 || pixel_height == 0 {
            return;
        }
        let max_cols = self.cols.saturating_sub(self.col).max(1);
        let max_rows = self.rows.saturating_sub(self.row).max(1);
        let cols = cols
            .unwrap_or_else(|| ((pixel_width as f32 / 8.0).ceil() as usize).max(1))
            .clamp(1, max_cols);
        let rows = match rows {
            Some(rows) => rows,
            None => {
                let ratio = pixel_height as f32 / pixel_width as f32;
                ((cols as f32 * 8.0 * ratio / 16.0).ceil() as usize).max(1)
            }
        }
        .clamp(1, max_rows);
        let id = InlineImage::next_id();
        let (start_row, start_col) = (self.row, self.col);
        for row in 0..rows {
            let blank = self.blank_cell();
            for col in 0..cols {
                let cell = Glyph {
                    ch: ' ',
                    bg: Color32::TRANSPARENT,
                    image: id,
                    ..blank
                };
                self.lines[start_row + row][start_col + col] = cell;
            }
        }
        if self.images.len() >= 64 {
            self.images.remove(0);
        }
        self.images.push(InlineImage {
            id,
            data,
            cols,
            rows,
            pixel_width,
            pixel_height,
        });
        if advance {
            self.col = 0;
            for _ in 0..rows {
                self.newline();
            }
        }
    }

    /// Marca uma célula como parte de um glifo largo em continuação.
    pub(crate) fn set_wide(&mut self, row: usize, col: usize, cell: Glyph) {
        self.lines[row][col] = cell;
        if col + 1 < self.cols {
            self.lines[row][col + 1] = Glyph {
                ch: ' ',
                width: 0,
                ..cell
            };
        }
    }

}
