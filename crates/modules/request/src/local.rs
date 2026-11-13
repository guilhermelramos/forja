//! Pedidos locais: criados na própria Forja, guardados no store do módulo.
//!
//! Leitura e escrita de disco rodam em `Ctx::spawn`; daqui saem só funções puras e
//! as duas operações de arquivo, chamadas de fora da thread da interface.

use forja_contracts::{RequestItem, RequestState};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Id do balcão dos pedidos locais.
pub const COUNTER: &str = "local";

/// Arquivo do store: `request.json` na pasta de dados.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFile {
    #[serde(default)]
    pub items: Vec<RequestItem>,
}

/// Próxima chave livre: `prefixo` + número. O número começa em `start` e nunca repete.
pub fn next_key(items: &[RequestItem], prefix: &str, start: u32) -> String {
    let highest = items
        .iter()
        .filter_map(|item| item.key.strip_prefix(prefix)?.parse::<u32>().ok())
        .max();
    let number = highest.map_or(start, |high| high.saturating_add(1).max(start));
    format!("{prefix}{number}")
}

/// Pedido local novo, aberto e atribuído a quem criou.
pub fn new_item(
    items: &[RequestItem],
    prefix: &str,
    start: u32,
    title: &str,
    body: Option<&str>,
    updated_at: &str,
) -> RequestItem {
    let mut item = RequestItem::new(COUNTER, next_key(items, prefix, start), title.trim());
    item.state = RequestState::Open;
    item.mine = true;
    item.body = body.map(str::trim).filter(|body| !body.is_empty()).map(str::to_owned);
    item.updated_at = Some(updated_at.to_owned());
    item
}

/// Lê o arquivo do store. Sem arquivo, a lista começa vazia.
pub fn load(path: &Path) -> Result<LocalFile, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LocalFile::default()),
        Err(error) => Err(error.to_string()),
    }
}

/// Grava o arquivo do store de forma atômica. Chamar só em `spawn`.
pub fn save(path: &Path, file: &LocalFile) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(file).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(path, &bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_continue_from_the_highest_number_and_respect_the_start() {
        assert_eq!(next_key(&[], "LOCAL-", 1), "LOCAL-1");
        assert_eq!(next_key(&[], "LOCAL-", 40), "LOCAL-40");
        let items = vec![
            RequestItem::new("local", "LOCAL-3", "a"),
            RequestItem::new("local", "LOCAL-9", "b"),
        ];
        assert_eq!(next_key(&items, "LOCAL-", 1), "LOCAL-10");
        assert_eq!(next_key(&items, "LOCAL-", 20), "LOCAL-20");
    }

    #[test]
    fn new_items_are_open_mine_and_trimmed() {
        let item = new_item(&[], "LOCAL-", 1, "  Revisar o cache  ", Some("  "), "2026-11-12T10:00:00Z");
        assert_eq!(item.counter, COUNTER);
        assert_eq!(item.key, "LOCAL-1");
        assert_eq!(item.title, "Revisar o cache");
        assert!(item.mine && item.is_open());
        assert_eq!(item.body, None);
    }

    #[test]
    fn save_then_load_round_trips_in_a_temp_folder() {
        let dir = std::env::temp_dir().join(format!("forja-request-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("request.json");
        assert_eq!(load(&path).unwrap(), LocalFile::default());
        let file = LocalFile {
            items: vec![new_item(&[], "LOCAL-", 1, "Teste", None, "2026-11-12T10:00:00Z")],
        };
        save(&path, &file).unwrap();
        assert_eq!(load(&path).unwrap(), file);
        let _ = std::fs::remove_dir_all(dir);
    }
}
