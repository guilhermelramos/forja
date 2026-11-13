//! Busca literal no histórico e na tela do terminal.

use crate::vt::Surface;

/// Uma ocorrência, em linha absoluta e colunas da grade.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    /// Linha absoluta (0 = primeira do histórico).
    pub row: usize,
    /// Primeira coluna.
    pub start: usize,
    /// Coluna seguinte à última.
    pub end: usize,
}

/// Encontra `query` nas linhas disponíveis. Vazio devolve lista vazia.
pub fn find(surface: &Surface, query: &str) -> Vec<Hit> {
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (row, line) in surface
        .history
        .iter()
        .chain(surface.lines.iter())
        .enumerate()
    {
        let text = line.text();
        for (byte, matched) in text.match_indices(query) {
            let start = text[..byte].chars().count();
            hits.push(Hit {
                row,
                start,
                end: start + matched.chars().count(),
            });
            if hits.len() >= 20_000 {
                return hits;
            }
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_multiple_hits_per_line() {
        let mut surface = Surface::new();
        surface.feed(b"alpha alpha\r\nbeta alpha");
        let hits = find(&surface, "alpha");
        assert_eq!(
            hits,
            vec![
                Hit { row: 0, start: 0, end: 5 },
                Hit { row: 0, start: 6, end: 11 },
                Hit { row: 1, start: 5, end: 10 },
            ]
        );
    }

    #[test]
    fn empty_query_and_wide_text_are_safe() {
        let mut surface = Surface::new();
        surface.feed("ação 中文".as_bytes());
        assert!(find(&surface, "").is_empty());
        let hits = find(&surface, "中文");
        assert_eq!(hits[0].start, 5);
        assert_eq!(hits[0].end, 7);
    }
}
