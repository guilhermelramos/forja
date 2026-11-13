//! Imagens embutidas: iTerm2 (OSC 1337) e Kitty (APC `G`).
//!
//! O programa envia PNG/GIF/JPEG em base64. Guardamos os bytes e as células
//! ocupadas; a view decide se desenha (configuração de GPU) e cria a textura.

use std::sync::atomic::{AtomicU32, Ordering};

/// Próximo identificador de imagem.
static NEXT_IMAGE: AtomicU32 = AtomicU32::new(1);

/// Uma imagem posicionada na grade.
#[derive(Clone, Debug, PartialEq)]
pub struct InlineImage {
    /// Identificador único dentro da sessão.
    pub id: u32,
    /// Bytes codificados (PNG, GIF ou JPEG).
    pub data: Vec<u8>,
    /// Colunas ocupadas.
    pub cols: usize,
    /// Linhas ocupadas.
    pub rows: usize,
    /// Largura em pixels da imagem original.
    pub pixel_width: u32,
    /// Altura em pixels da imagem original.
    pub pixel_height: u32,
}

impl InlineImage {
    /// Reserva um id novo.
    pub fn next_id() -> u32 {
        NEXT_IMAGE.fetch_add(1, Ordering::Relaxed).max(1)
    }
}

/// Decodifica base64 com limite de tamanho. Devolve `None` em entrada inválida.
pub fn decode_base64(encoded: &str, limit: usize) -> Option<Vec<u8>> {
    if encoded.len() > limit || !encoded.len().is_multiple_of(4) {
        return None;
    }
    let mut result = Vec::with_capacity(encoded.len() / 4 * 3);
    let bytes = encoded.as_bytes();
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let mut value = 0u32;
        let mut padding = 0;
        for (offset, byte) in chunk.iter().enumerate() {
            let digit = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if offset >= 2 => {
                    padding += 1;
                    0
                }
                _ => return None,
            };
            if padding > 0 && *byte != b'=' {
                return None;
            }
            value = (value << 6) | u32::from(digit);
        }
        if padding > 0 && index + 1 != bytes.len() / 4 {
            return None;
        }
        result.push((value >> 16) as u8);
        if padding < 2 {
            result.push((value >> 8) as u8);
        }
        if padding == 0 {
            result.push(value as u8);
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_valid_base64_and_rejects_junk() {
        assert_eq!(decode_base64("aGVsbG8=", 100), Some(b"hello".to_vec()));
        assert_eq!(decode_base64("?", 100), None);
        assert_eq!(decode_base64("%%%%", 100), None);
        assert_eq!(decode_base64("aGVsbG8=", 2), None);
    }
}
