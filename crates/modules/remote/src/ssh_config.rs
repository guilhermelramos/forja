//! Lê a configuração do cliente OpenSSH (`~/.ssh/config`) e transforma os aliases concretos em
//! destinos importáveis.
//!
//! Regras (a ordem de OpenSSH: o primeiro valor de cada palavra-chave vence):
//! - Os blocos são lidos na ordem do arquivo; opções antes do primeiro `Host` valem para todos.
//! - Um bloco vale para um alias quando um padrão positivo casa e nenhum padrão `!negado` casa.
//!   Padrões usam `*` e `?` e não diferenciam maiúsculas.
//! - `Include` é expandido no lugar (caminhos relativos vão para `~/.ssh`; globs no último
//!   componente). Blocos `Match` nunca valem.
//! - Palavras-chave não diferenciam maiúsculas; valores podem vir com espaço ou `=` e entre aspas.
//!   Só comentários de linha inteira (`#`) são reconhecidos, como no OpenSSH.

use crate::target::{SshTarget, DEFAULT_SSH_PORT};
use std::path::{Path, PathBuf};

const MAX_INCLUDE_DEPTH: usize = 16;

/// Acesso a arquivos usado pelo parser, para que os testes usem uma árvore em memória.
pub trait ConfigSource {
    fn read(&self, path: &Path) -> Option<String>;
    /// Arquivos cujo último componente casa com `pattern` (que pode ter `*` e `?`).
    fn glob(&self, pattern: &Path) -> Vec<PathBuf>;
}

/// Lê do sistema de arquivos.
pub struct FsSource;

impl ConfigSource for FsSource {
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn glob(&self, pattern: &Path) -> Vec<PathBuf> {
        let (Some(parent), Some(name)) = (pattern.parent(), pattern.file_name()) else {
            return Vec::new();
        };
        let name = name.to_string_lossy();
        if !name.contains(['*', '?']) {
            return if pattern.is_file() {
                vec![pattern.to_path_buf()]
            } else {
                Vec::new()
            };
        }
        let Ok(entries) = std::fs::read_dir(parent) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|n| wildcard_match(&name, &n.to_string_lossy()))
                    && path.is_file()
            })
            .collect();
        found.sort();
        found
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Block {
    patterns: Vec<String>,
    options: Vec<(String, String)>,
}

/// Um alias concreto com as opções efetivas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostEntry {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub proxy_jump: Option<String>,
}

/// Lê `text` (o arquivo principal, em `ssh_dir/config`) e devolve cada alias concreto, na ordem
/// em que aparece.
pub fn parse_hosts(text: &str, ssh_dir: &Path, home: &Path, source: &dyn ConfigSource) -> Vec<HostEntry> {
    let mut parser = Parser {
        blocks: Vec::new(),
        ssh_dir,
        home,
        source,
    };
    parser.parse_text(text, vec!["*".into()], 0);
    let blocks = parser.blocks;

    let mut aliases: Vec<String> = Vec::new();
    for pattern in blocks.iter().flat_map(|b| b.patterns.iter()) {
        if is_concrete(pattern) && !aliases.iter().any(|a| a.eq_ignore_ascii_case(pattern)) {
            aliases.push(pattern.clone());
        }
    }
    aliases
        .into_iter()
        .map(|alias| {
            let options = effective_options(&blocks, &alias);
            let get = |key: &str| {
                options
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .filter(|v| !v.is_empty() && v != "none")
            };
            HostEntry {
                hostname: get("hostname"),
                user: get("user"),
                port: get("port").and_then(|p| p.parse().ok()),
                identity_file: get("identityfile"),
                proxy_jump: get("proxyjump"),
                alias,
            }
        })
        .collect()
}

/// Converte os aliases em destinos. O alias vira `host`, para o `ssh` continuar resolvendo o
/// que não foi copiado; o que foi copiado fica explícito, para o destino ser editável.
pub fn import_targets(entries: &[HostEntry]) -> Vec<SshTarget> {
    let mut ids: Vec<String> = Vec::new();
    entries
        .iter()
        .map(|entry| {
            let id = unique_id(&slug(&entry.alias), &ids);
            ids.push(id.clone());
            SshTarget {
                id,
                label: entry.alias.clone(),
                host: entry.alias.clone(),
                user: entry.user.clone().unwrap_or_default(),
                port: entry.port.unwrap_or(DEFAULT_SSH_PORT),
                identity_file: entry.identity_file.clone().unwrap_or_default(),
                proxy_jump: entry.proxy_jump.clone().unwrap_or_default(),
                forwards: Vec::new(),
            }
        })
        .collect()
}

fn unique_id(base: &str, taken: &[String]) -> String {
    let base = format!("ssh-{base}");
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken.contains(candidate))
        .unwrap_or(base)
}

fn slug(alias: &str) -> String {
    let slug: String = alias
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "host".into()
    } else {
        trimmed.into()
    }
}

struct Parser<'a> {
    blocks: Vec<Block>,
    ssh_dir: &'a Path,
    home: &'a Path,
    source: &'a dyn ConfigSource,
}

impl Parser<'_> {
    fn parse_text(&mut self, text: &str, initial: Vec<String>, depth: usize) {
        let mut current = Block {
            patterns: initial,
            options: Vec::new(),
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = split_keyword(line) else {
                continue;
            };
            match key.to_ascii_lowercase().as_str() {
                "host" => {
                    self.blocks.push(std::mem::take(&mut current));
                    current.patterns = split_words(value);
                }
                "match" => {
                    // Blocos Match não são avaliados: as opções deles nunca valem.
                    self.blocks.push(std::mem::take(&mut current));
                    current.patterns = Vec::new();
                }
                "include" => {
                    if depth >= MAX_INCLUDE_DEPTH {
                        continue;
                    }
                    let outer = current.patterns.clone();
                    self.blocks.push(std::mem::take(&mut current));
                    current.patterns = outer.clone();
                    for pattern in split_words(value) {
                        for file in self.include_files(&pattern) {
                            if let Some(text) = self.source.read(&file) {
                                self.parse_text(&text, outer.clone(), depth + 1);
                            }
                        }
                    }
                }
                other => {
                    let value = unquote(value);
                    let value = self.expand_home(&value);
                    current.options.push((other.to_string(), value));
                }
            }
        }
        self.blocks.push(current);
    }

    fn include_files(&self, pattern: &str) -> Vec<PathBuf> {
        let expanded = self.expand_home(&unquote(pattern));
        let path = Path::new(&expanded);
        let full = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.ssh_dir.join(path)
        };
        self.source.glob(&full)
    }

    fn expand_home(&self, value: &str) -> String {
        for prefix in ["~/", "~\\"] {
            if let Some(rest) = value.strip_prefix(prefix) {
                return self.home.join(rest).to_string_lossy().into_owned();
            }
        }
        value.to_string()
    }
}

fn effective_options(blocks: &[Block], alias: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    for block in blocks.iter().filter(|b| block_matches(b, alias)) {
        for (key, value) in &block.options {
            if !result.iter().any(|(k, _)| k == key) {
                result.push((key.clone(), value.clone()));
            }
        }
    }
    result
}

fn block_matches(block: &Block, alias: &str) -> bool {
    let mut positive = false;
    for pattern in &block.patterns {
        if let Some(negated) = pattern.strip_prefix('!') {
            if wildcard_match(negated, alias) {
                return false;
            }
        } else if wildcard_match(pattern, alias) {
            positive = true;
        }
    }
    positive
}

fn is_concrete(pattern: &str) -> bool {
    !pattern.is_empty() && !pattern.contains(['*', '?', '!'])
}

/// Casamento de padrão sem diferenciar maiúsculas, com `*` e `?`.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Separa `Palavra valor` ou `Palavra=valor`. `None` quando a palavra-chave está vazia.
fn split_keyword(line: &str) -> Option<(&str, &str)> {
    let end = line.find(|c: char| c.is_whitespace() || c == '=')?;
    let key = &line[..end];
    if key.is_empty() {
        return None;
    }
    let rest = line[end..].trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest).trim();
    Some((key, rest))
}

/// Divide por espaço, mantendo trechos entre aspas duplas juntos.
fn split_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_word = false;
    for c in value.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has_word = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has_word {
                    words.push(std::mem::take(&mut current));
                    has_word = false;
                }
            }
            c => {
                current.push(c);
                has_word = true;
            }
        }
    }
    if has_word {
        words.push(current);
    }
    words
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

#[cfg(test)]
mod tests;
