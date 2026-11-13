//! Front matter de um `SKILL.md`: o bloco entre duas linhas `---` no começo do arquivo.
//!
//! Lê `name` e `description`, os campos exigidos. As outras chaves são ignoradas. Aceita valores
//! entre aspas e os blocos `>` e `|` (com `-` opcional). Não é um parser de YAML completo: o que
//! estiver fora desse subconjunto vira erro de linha, e a skill aparece na lista com o motivo.

use std::collections::BTreeMap;

/// Campos exigidos de uma skill.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Front {
    pub name: String,
    pub description: String,
}

/// Por que o front matter não serve. A tela traduz pela chave de [`FrontError::key`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrontError {
    /// O arquivo não começa com `---`.
    Missing,
    /// Falta a linha `---` que fecha o bloco.
    Unterminated,
    /// Uma linha não é `chave: valor`. Guarda o número da linha no arquivo.
    BadLine(usize),
    NameMissing,
    NameInvalid,
    DescriptionMissing,
}

impl FrontError {
    pub fn key(&self) -> &'static str {
        match self {
            Self::Missing => "skill-error-frontmatter-missing",
            Self::Unterminated => "skill-error-frontmatter-unterminated",
            Self::BadLine(_) => "skill-error-frontmatter-line",
            Self::NameMissing => "skill-error-name-missing",
            Self::NameInvalid => "skill-error-name-invalid",
            Self::DescriptionMissing => "skill-error-description-missing",
        }
    }
}

/// Lê o front matter de um `SKILL.md` inteiro (só o começo é usado).
pub fn parse(text: &str) -> Result<Front, FrontError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    if lines.next().map(str::trim_end) != Some("---") {
        return Err(FrontError::Missing);
    }
    let mut block = Vec::new();
    let mut closed = false;
    for line in lines {
        if line.trim_end() == "---" {
            closed = true;
            break;
        }
        block.push(line);
    }
    if !closed {
        return Err(FrontError::Unterminated);
    }
    let values = fields(&block)?;

    let name = values.get("name").map(|value| value.trim()).unwrap_or("");
    if name.is_empty() {
        return Err(FrontError::NameMissing);
    }
    if !valid_name(name) {
        return Err(FrontError::NameInvalid);
    }
    let description = values.get("description").map(|value| value.trim()).unwrap_or("");
    if description.is_empty() {
        return Err(FrontError::DescriptionMissing);
    }
    Ok(Front {
        name: name.to_owned(),
        description: description.to_owned(),
    })
}

/// Nome de skill: 1 a 64 caracteres, só minúsculas, dígitos e hífens; sem hífen no começo, no
/// fim ou duplo. A regra vem da especificação do formato de skills.
pub fn valid_name(name: &str) -> bool {
    let length = name.chars().count();
    (1..=64).contains(&length)
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// Pares `chave: valor` do bloco. Linhas vazias e comentários `#` são ignorados.
fn fields(block: &[&str]) -> Result<BTreeMap<String, String>, FrontError> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < block.len() {
        let line = block[index];
        index += 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(FrontError::BadLine(index + 1));
        }
        let Some((key, raw)) = line.split_once(':') else {
            return Err(FrontError::BadLine(index + 1));
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(FrontError::BadLine(index + 1));
        }
        let raw = raw.trim();
        let value = if matches!(raw, ">" | ">-" | "|" | "|-") {
            let folded = raw.starts_with('>');
            let mut parts: Vec<String> = Vec::new();
            while index < block.len()
                && (block[index].trim().is_empty()
                    || block[index].starts_with(' ')
                    || block[index].starts_with('\t'))
            {
                parts.push(block[index].trim().to_owned());
                index += 1;
            }
            while parts.last().is_some_and(|part| part.is_empty()) {
                parts.pop();
            }
            if folded {
                parts.join(" ")
            } else {
                parts.join("\n")
            }
        } else {
            unquote(raw)
        };
        values.insert(key.to_owned(), value);
    }
    Ok(values)
}

/// Tira aspas simples ou duplas de um valor. Sem aspas, devolve o texto como está.
fn unquote(raw: &str) -> String {
    if raw.len() >= 2 && raw.starts_with('"') && raw.ends_with('"') {
        return raw[1..raw.len() - 1].replace("\\\"", "\"").replace("\\\\", "\\");
    }
    if raw.len() >= 2 && raw.starts_with('\'') && raw.ends_with('\'') {
        return raw[1..raw.len() - 1].replace("''", "'");
    }
    raw.to_owned()
}

#[cfg(test)]
mod tests;
