//! Detecção de links no texto do terminal.
//!
//! Prioriza links OSC 8; sem eles, reconhece URLs e caminhos de arquivo por
//! token. Caminhos aceitam `arquivo:linha:coluna` e `file://` com escapes.

use crate::vt::{Row, Surface};
use std::path::{Path, PathBuf};

/// Destino de um clique com Ctrl.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// URL http(s).
    Url(String),
    /// Arquivo local, com posição opcional.
    File {
        /// Caminho absoluto ou relativo ao diretório do terminal.
        path: PathBuf,
        /// Linha pedida pelo texto.
        line: Option<u32>,
        /// Coluna pedida pelo texto.
        column: Option<u32>,
    },
}

impl Link {
    /// Texto exibido na confirmação.
    pub fn display(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::File { path, line, column } => {
                let mut text = path.display().to_string();
                if let Some(line) = line {
                    text.push(':');
                    text.push_str(&line.to_string());
                }
                if let Some(column) = column {
                    text.push(':');
                    text.push_str(&column.to_string());
                }
                text
            }
        }
    }
}

/// Encontra o link sob a célula `(row, col)`.
pub fn detect(surface: &Surface, row: usize, col: usize) -> Option<Link> {
    let line = surface.line_at(row)?;
    if let Some(glyph) = line.get(col) {
        if glyph.link != 0 {
            if let Some(url) = surface.links.get(glyph.link as usize) {
                if !url.is_empty() {
                    return parse_url(url);
                }
            }
        }
    }
    let text = line.text();
    let mut offset = 0;
    for token in text.split_inclusive(char::is_whitespace) {
        let length = token.chars().count();
        if (offset..offset + length).contains(&col) {
            return parse_token(token.trim());
        }
        offset += length;
    }
    None
}

fn parse_url(url: &str) -> Option<Link> {
    let trimmed = url.trim_end_matches([',', ';']);
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(Link::Url(trimmed.to_owned()))
    } else if trimmed.starts_with("file://") {
        let (path, line, column) = parse_file_link(trimmed);
        Some(Link::File { path, line, column })
    } else {
        None
    }
}

fn parse_token(token: &str) -> Option<Link> {
    let token = token
        .trim_matches(['(', ')', '[', ']', '<', '>', '"', '\''])
        .trim_end_matches([',', ';']);
    if token.is_empty() {
        return None;
    }
    if token.starts_with("http://") || token.starts_with("https://") {
        return Some(Link::Url(token.to_owned()));
    }
    let looks_like_path = token.starts_with("file://")
        || token.contains('/')
        || token.contains('\\');
    if !looks_like_path {
        return None;
    }
    let (path, line, column) = parse_file_link(token);
    Some(Link::File { path, line, column })
}

/// Separa `path:line:column` e decodifica `file://`.
pub fn parse_file_link(link: &str) -> (PathBuf, Option<u32>, Option<u32>) {
    let mut text = link
        .strip_prefix("file:///")
        .or_else(|| link.strip_prefix("file://"))
        .unwrap_or(link)
        .to_owned();
    if link.starts_with("file://") {
        text = percent_decode(&text);
    }
    let mut line = None;
    let mut column = None;
    if let Some((base, suffix)) = text.rsplit_once(':') {
        if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            if let Some((base2, suffix2)) = base.rsplit_once(':') {
                if !suffix2.is_empty() && suffix2.bytes().all(|byte| byte.is_ascii_digit()) {
                    column = suffix.parse().ok();
                    line = suffix2.parse().ok();
                    text = base2.to_owned();
                } else {
                    line = suffix.parse().ok();
                    text = base.to_owned();
                }
            } else {
                line = suffix.parse().ok();
                text = base.to_owned();
            }
        }
    }
    (PathBuf::from(text), line, column)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    result.push(byte);
                    index += 3;
                    continue;
                }
            }
        }
        result.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(result).unwrap_or_else(|_| text.to_owned())
}

/// Intervalo de colunas da palavra em `(row, col)`.
pub fn word_bounds(row: &Row, col: usize, separators: &str) -> (usize, usize) {
    if row.is_empty() {
        return (0, 0);
    }
    let col = col.min(row.len() - 1);
    let is_separator =
        |index: usize| row[index].ch.is_whitespace() || separators.contains(row[index].ch);
    let mut start = col;
    let mut end = (col + 1).min(row.len());
    if !is_separator(col) {
        while start > 0 && !is_separator(start - 1) {
            start -= 1;
        }
        while end < row.len() && !is_separator(end) {
            end += 1;
        }
    }
    (start, end)
}

/// Caminho absoluto a partir do diretório do terminal.
pub fn resolve(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_links_decode_spaces_and_positions() {
        let (path, line, column) =
            parse_file_link("file:///C:/my%20project/main.rs:12:3");
        assert_eq!(path, PathBuf::from("C:/my project/main.rs"));
        assert_eq!(line, Some(12));
        assert_eq!(column, Some(3));
        let (path, line, column) = parse_file_link("src/main.rs:12");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(line, Some(12));
        assert_eq!(column, None);
        let (path, line, column) = parse_file_link("C:/work/main.rs");
        assert_eq!(path, PathBuf::from("C:/work/main.rs"));
        assert_eq!((line, column), (None, None));
    }

    #[test]
    fn detect_finds_urls_and_paths_by_token() {
        let mut surface = Surface::new();
        surface.feed(b"see https://example.test/docs and src/main.rs:9\r\n");
        assert_eq!(
            detect(&surface, 0, 6),
            Some(Link::Url("https://example.test/docs".into()))
        );
        assert!(matches!(
            detect(&surface, 0, 39),
            Some(Link::File { .. })
        ));
    }

    #[test]
    fn osc8_wins_over_plain_text() {
        let mut surface = Surface::new();
        surface.feed(b"\x1b]8;;https://forja.test\x1b\\forja\x1b]8;;\x07");
        assert_eq!(
            detect(&surface, 0, 1),
            Some(Link::Url("https://forja.test".into()))
        );
    }

    #[test]
    fn word_bounds_obey_separators() {
        let mut surface = Surface::new();
        surface.feed(b"abc/def ghi");
        assert_eq!(word_bounds(&surface.lines[0], 4, "/"), (4, 7));
        assert_eq!(word_bounds(&surface.lines[0], 4, ""), (0, 7));
    }

}
