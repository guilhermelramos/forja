//! Custo do mutirão lido do contrato `usage.spend`.
//!
//! O contrato devolve o gasto por pasta de trabalho (bancada) e período. O
//! Mutirão usa a bancada do mutirão e a de cada atribuição para mostrar custo
//! por ajudante e por tarefa sem pedir um valor digitado à mão.

use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Intervalo mínimo entre duas leituras do contrato.
const REFRESH: Duration = Duration::from_secs(3);

/// Resposta de `usage.spend` que a página usa.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpendInfo {
    /// Custo em dólares.
    pub usd: f64,
    /// Tokens somados.
    pub tokens: u64,
    /// Tokens sem preço conhecido.
    pub unpriced_tokens: u64,
    /// Todos os tokens tinham preço.
    pub priced: bool,
    /// Pastas de log que entraram na soma.
    pub matched_projects: usize,
    /// A varredura do módulo Uso ainda está rodando.
    pub pending: bool,
}

impl SpendInfo {
    /// Há dados de uso para esta bancada, mesmo que o custo seja zero.
    pub fn available(&self) -> bool {
        self.matched_projects > 0 || self.usd > 0.0
    }

    /// Soma duas leituras para o total do mutirão.
    fn add(&mut self, other: &Self) {
        self.usd += other.usd;
        self.tokens = self.tokens.saturating_add(other.tokens);
        self.unpriced_tokens = self.unpriced_tokens.saturating_add(other.unpriced_tokens);
        self.priced &= other.priced;
        self.matched_projects += other.matched_projects;
        self.pending |= other.pending;
    }
}

/// Cache curto por bancada normalizada.
#[derive(Debug, Default)]
pub struct SpendBook {
    by_bench: HashMap<String, SpendInfo>,
    contract: bool,
    last_sync: Option<Instant>,
}

impl SpendBook {
    /// Lê o contrato para cada bancada, no máximo a cada três segundos.
    pub fn sync(&mut self, cx: &mut Ctx<'_>, benches: &[String], force: bool) {
        let due = force || self.last_sync.is_none_or(|last| last.elapsed() >= REFRESH);
        if !due {
            return;
        }
        self.last_sync = Some(Instant::now());
        if cx.registry().command("usage.spend").is_none() {
            self.contract = false;
            self.by_bench.clear();
            return;
        }
        self.contract = true;
        for bench in benches {
            let key = normalize(bench);
            if key.is_empty() {
                continue;
            }
            let mut args = Args::new();
            args.insert("project", json!(bench));
            args.insert("period", json!("all"));
            let Ok(value) = cx.run_command("usage.spend", args) else {
                continue;
            };
            if let Some(info) = parse(&value) {
                self.by_bench.insert(key, info);
            }
        }
    }

    /// O comando `usage.spend` está registrado.
    pub fn contract_available(&self) -> bool {
        self.contract
    }

    /// Leitura de uma bancada.
    pub fn get(&self, bench: &str) -> Option<&SpendInfo> {
        self.by_bench.get(&normalize(bench))
    }

    /// Soma as bancadas informadas, ignorando as que não têm dados.
    pub fn total(&self, benches: &[String]) -> Option<SpendInfo> {
        let mut total = SpendInfo {
            priced: true,
            ..SpendInfo::default()
        };
        let mut any = false;
        for bench in benches {
            let Some(info) = self.get(bench) else {
                continue;
            };
            if !info.available() {
                continue;
            }
            any = true;
            total.add(info);
        }
        any.then_some(total)
    }

    /// Alguma das bancadas ainda espera a varredura do Uso.
    pub fn any_pending(&self, benches: &[String]) -> bool {
        benches
            .iter()
            .filter_map(|bench| self.get(bench))
            .any(|info| info.pending)
    }
}

/// Lê a resposta do contrato.
pub fn parse(value: &Value) -> Option<SpendInfo> {
    Some(SpendInfo {
        usd: value.get("usd")?.as_f64()?,
        tokens: value.get("tokens").and_then(Value::as_u64).unwrap_or(0),
        unpriced_tokens: value
            .get("unpriced_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        priced: value.get("priced").and_then(Value::as_bool).unwrap_or(true),
        matched_projects: value
            .get("matched_projects")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize,
        pending: value
            .get("pending")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Caminho comparável: sem barra final, com `/` e minúsculas.
pub fn normalize(path: &str) -> String {
    path.trim()
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/")
        .to_ascii_lowercase()
}

/// Remove caminhos repetidos sem mexer na ordem original.
pub fn distinct_benches(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut kept: Vec<(String, String)> = Vec::new();
    for path in paths {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            continue;
        }
        let norm = normalize(trimmed);
        if kept.iter().any(|(existing, _)| *existing == norm) {
            continue;
        }
        kept.push((norm, trimmed.to_owned()));
    }
    kept.into_iter().map(|(_, original)| original).collect()
}

/// Bancadas do mutirão sem as que já estão dentro de outra: `usage.spend`
/// soma as subpastas, então a bancada pai já conta tudo.
pub fn unique_benches(paths: impl IntoIterator<Item = String>) -> Vec<String> {
    let distinct = distinct_benches(paths);
    let mut kept: Vec<(String, String)> = Vec::new();
    for original in distinct {
        let norm = normalize(&original);
        if norm.is_empty() {
            continue;
        }
        let prefix = format!("{norm}/");
        if kept
            .iter()
            .any(|(existing, _)| norm == *existing || norm.starts_with(&format!("{existing}/")))
        {
            continue;
        }
        kept.retain(|(existing, _)| !existing.starts_with(&prefix));
        kept.push((norm, original));
    }
    kept.into_iter().map(|(_, original)| original).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(usd: f64, matched: usize) -> SpendInfo {
        SpendInfo {
            usd,
            tokens: 10,
            unpriced_tokens: 0,
            priced: true,
            matched_projects: matched,
            pending: false,
        }
    }

    #[test]
    fn parses_the_usage_spend_contract() {
        let value = json!({
            "period": "all",
            "project": "C:/work/forja",
            "matched_projects": 2,
            "usd": 1.25,
            "tokens": 120,
            "unpriced_tokens": 10,
            "priced": false,
            "pending": true
        });
        let parsed = parse(&value).expect("contract");
        assert!((parsed.usd - 1.25).abs() < f64::EPSILON);
        assert_eq!(parsed.tokens, 120);
        assert_eq!(parsed.unpriced_tokens, 10);
        assert!(!parsed.priced);
        assert_eq!(parsed.matched_projects, 2);
        assert!(parsed.pending);
        assert!(parsed.available());
    }

    #[test]
    fn a_bench_without_logs_is_not_available() {
        let parsed = parse(&json!({
            "usd": 0.0,
            "tokens": 0,
            "matched_projects": 0,
            "pending": false
        }))
        .expect("contract");
        assert!(!parsed.available());
    }

    #[test]
    fn nested_benches_collapse_to_the_parent() {
        let benches = unique_benches([
            "C:/work/forja".to_owned(),
            "c:\\WORK\\forja\\sub".to_owned(),
            "C:/work/other".to_owned(),
        ]);
        assert_eq!(benches, ["C:/work/forja", "C:/work/other"]);
    }

    #[test]
    fn a_parent_added_later_replaces_its_children() {
        let benches = unique_benches(["C:/work/forja/sub".to_owned(), "C:/work/forja".to_owned()]);
        assert_eq!(benches, ["C:/work/forja"]);
    }

    #[test]
    fn distinct_benches_keep_the_first_spelling() {
        let benches = distinct_benches([
            "C:\\Work\\Forja\\".to_owned(),
            "c:/work/forja".to_owned(),
            " ".to_owned(),
        ]);
        assert_eq!(benches, ["C:\\Work\\Forja\\"]);
    }

    #[test]
    fn total_ignores_benches_without_data_and_marks_unpriced() {
        let mut book = SpendBook {
            contract: true,
            ..SpendBook::default()
        };
        book.by_bench
            .insert("c:/work/forja".to_owned(), info(1.5, 2));
        book.by_bench.insert(
            "c:/work/other".to_owned(),
            SpendInfo {
                unpriced_tokens: 30,
                priced: false,
                ..info(0.25, 1)
            },
        );
        book.by_bench
            .insert("c:/work/empty".to_owned(), SpendInfo::default());
        let benches = vec![
            "C:/work/forja".to_owned(),
            "C:/work/other".to_owned(),
            "C:/work/empty".to_owned(),
        ];
        let total = book.total(&benches).expect("total");
        assert!((total.usd - 1.75).abs() < f64::EPSILON);
        assert_eq!(total.matched_projects, 3);
        assert_eq!(total.unpriced_tokens, 30);
        assert!(!total.priced);
    }
}
