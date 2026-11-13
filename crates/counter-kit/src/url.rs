//! Codificação de componentes de URL (RFC 3986). Sem ela, rótulos com espaço ou `+`
//! mudariam o sentido da consulta.

/// Codifica tudo que não é letra, dígito, `-`, `_`, `.` ou `~`.
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode_component;

    #[test]
    fn reserved_characters_are_escaped() {
        assert_eq!(encode_component("bug fix+1"), "bug%20fix%2B1");
        assert_eq!(encode_component("acme/tools"), "acme%2Ftools");
        assert_eq!(encode_component("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(encode_component("ação"), "a%C3%A7%C3%A3o");
    }
}
