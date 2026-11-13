//! Seleção linear de texto sobre a grade do terminal.

/// Seleção entre duas células absolutas.
#[derive(Clone, Copy, Debug)]
pub struct Selection {
    /// Início arrastado.
    pub anchor: (usize, usize),
    /// Extremidade atual.
    pub head: (usize, usize),
    /// Geração da superfície em que a seleção nasceu.
    pub generation: u64,
}

impl Selection {
    /// Intervalo de colunas nesta linha, quando a linha pertence à seleção.
    pub fn range(self, row: usize, len: usize, generation: u64) -> Option<std::ops::Range<usize>> {
        if self.generation != generation {
            return None;
        }
        let (start, end) = if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        if start == end || row < start.0 || row > end.0 {
            return None;
        }
        Some(
            (if row == start.0 { start.1.min(len) } else { 0 })
                ..(if row == end.0 { end.1.min(len) } else { len }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_ranges_cover_first_and_last_lines() {
        let selection = Selection {
            anchor: (10, 3),
            head: (12, 4),
            generation: 1,
        };
        assert_eq!(selection.range(9, 80, 1), None);
        assert_eq!(selection.range(10, 80, 1), Some(3..80));
        assert_eq!(selection.range(11, 80, 1), Some(0..80));
        assert_eq!(selection.range(12, 80, 1), Some(0..4));
        assert_eq!(selection.range(10, 80, 2), None);
    }
}
