//! Grade de células do terminal: uma linha é um vetor de glifos.
//!
//! Larguras seguem o Leste Asiático (largos ocupam duas colunas) e a
//! apresentação de emoji. Nada aqui desenha: a view lê a grade.

use forja_kernel::ui_contract::egui::Color32;
use std::ops::{Deref, DerefMut};

/// Bits de atributo de um glifo.
pub mod attrs {
    pub const BOLD: u8 = 1;
    pub const DIM: u8 = 2;
    pub const ITALIC: u8 = 4;
    pub const UNDERLINE: u8 = 8;
    pub const REVERSE: u8 = 16;
    pub const CONCEAL: u8 = 32;
}

/// Origem de uma cor: 0–15 ANSI, 16–255 estendida, 256 padrão, 257 RGB.
pub const SOURCE_DEFAULT: u16 = 256;
pub const SOURCE_TRUE_COLOR: u16 = 257;

/// Uma célula da grade.
#[derive(Clone, Copy, PartialEq)]
pub struct Glyph {
    /// Caractere exibido.
    pub ch: char,
    /// Cor de frente pedida pelo programa.
    pub fg: Color32,
    /// Origem da cor de frente.
    pub fg_source: u16,
    /// Cor de fundo pedida pelo programa.
    pub bg: Color32,
    /// Origem da cor de fundo.
    pub bg_source: u16,
    /// Atributos (`attrs::*`).
    pub flags: u8,
    /// 2 = glifo largo, 0 = continuação, 1 = coluna comum.
    pub width: u8,
    /// O caractere pediu apresentação de emoji (U+FE0F).
    pub emoji: bool,
    /// Id de imagem embutida ocupando a célula, 0 para texto.
    pub image: u32,
    /// Índice do link OSC 8, 0 para nenhum.
    pub link: u32,
}

impl Default for Glyph {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color32::from_gray(230),
            fg_source: SOURCE_DEFAULT,
            bg: Color32::TRANSPARENT,
            bg_source: SOURCE_DEFAULT,
            flags: 0,
            width: 1,
            emoji: false,
            image: 0,
            link: 0,
        }
    }
}

/// Uma linha da tela ou do histórico.
#[derive(Clone)]
pub struct Row {
    cells: Vec<Glyph>,
}

impl Row {
    /// Linha toda em branco.
    pub fn blank(cols: usize) -> Self {
        Self::filled(cols, Glyph::default())
    }

    /// Linha com a mesma célula repetida.
    pub fn filled(cols: usize, cell: Glyph) -> Self {
        Self {
            cells: vec![cell; cols],
        }
    }

    /// Célula visível, escondendo a metade direita de um glifo largo cortado.
    pub fn display_cell(&self, col: usize, cols: usize) -> Glyph {
        let cell = self.cells.get(col).copied().unwrap_or_default();
        if cell.width == 2 && col + 1 >= cols {
            Glyph::default()
        } else {
            cell
        }
    }

    /// Restaura pares largos quando uma edição deixou órfãos.
    pub(crate) fn repair_wide(&mut self) {
        for col in 0..self.cells.len() {
            let invalid = match self.cells[col].width {
                0 => col == 0 || self.cells[col - 1].width != 2,
                2 => col + 1 >= self.cells.len() || self.cells[col + 1].width != 0,
                _ => false,
            };
            if invalid {
                self.cells[col] = Glyph::default();
            }
        }
    }

    /// Texto sem as continuações de glifos largos.
    pub fn text(&self) -> String {
        let mut text = String::new();
        for cell in &self.cells {
            if cell.width != 0 {
                text.push(cell.ch);
                if cell.emoji {
                    text.push('\u{fe0f}');
                }
            }
        }
        text
    }
}

impl Deref for Row {
    type Target = Vec<Glyph>;

    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

impl DerefMut for Row {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cells
    }
}

/// Largura em colunas: blocos largos e emoji de apresentação ocupam duas.
pub fn character_width(c: char) -> usize {
    if matches!(c as u32,
        0x1100..=0x115f | 0x231a..=0x231b | 0x2329..=0x232a | 0x23e9..=0x23ec | 0x23f0 | 0x23f3 |
        0x25fd..=0x25fe | 0x2614..=0x2615 | 0x2648..=0x2653 | 0x267f | 0x2693 | 0x26a1 |
        0x26aa..=0x26ab | 0x26bd..=0x26be | 0x26c4..=0x26c5 | 0x26ce | 0x26d4 | 0x26ea |
        0x26f2..=0x26f3 | 0x26f5 | 0x26fa | 0x26fd | 0x2705 | 0x270a..=0x270b | 0x2728 |
        0x274c | 0x274e | 0x2753..=0x2755 | 0x2757 | 0x2795..=0x2797 | 0x27b0 | 0x27bf |
        0x2b1b..=0x2b1c | 0x2b50 | 0x2b55 | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 |
        0xf900..=0xfaff | 0xfe10..=0xfe19 | 0xfe30..=0xfe6f | 0xff01..=0xff60 | 0xffe0..=0xffe6 |
        0x16fe0..=0x16fe4 | 0x17000..=0x18dff | 0x1aff0..=0x1b2ff | 0x1f004 | 0x1f0cf |
        0x1f18e | 0x1f191..=0x1f19a | 0x1f1e6..=0x1f1ff | 0x1f200..=0x1f251 |
        0x1f300..=0x1faff | 0x20000..=0x3fffd
    ) {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_glyph_hides_at_the_right_edge() {
        let mut row = Row::blank(4);
        row[2] = Glyph {
            ch: '\u{4e2d}',
            width: 2,
            ..Glyph::default()
        };
        assert_eq!(row.display_cell(2, 4).width, 2);
        assert_eq!(row.display_cell(2, 3).width, 1);
    }

    #[test]
    fn character_width_covers_cjk_and_emoji() {
        assert_eq!(character_width('a'), 1);
        assert_eq!(character_width('\u{4e2d}'), 2);
        assert_eq!(character_width('\u{1f600}'), 2);
    }
}
