//! Editable price table for models whose transcript carries no cost.
//!
//! Every entry lives in `content/pricing/*.toml` (or the user/project layer),
//! states its source and the date it was checked, and is expressed in USD per
//! one million tokens. A model without an entry is reported as "no price"; the
//! module never guesses one.

use crate::model::UsageTotals;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MILLION: f64 = 1_000_000.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pricing {
    entries: Vec<PriceEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PriceEntry {
    pub id: String,
    pub patterns: Vec<String>,
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,
    pub source: String,
    pub checked: String,
}

impl Pricing {
    /// Reads the built-in, user and project layers, in that order. A later
    /// layer replaces an entry with the same `id`.
    pub fn load(user_dir: &Path, project: Option<&Path>) -> (Self, Vec<String>) {
        let mut roots = Vec::new();
        if let Some(root) = builtin_root() {
            roots.push(root);
        }
        roots.push(user_dir.to_path_buf());
        if let Some(project) = project {
            roots.push(project.join(".forja"));
        }
        load_roots(&roots)
    }

    /// Test and fixture constructor; production reads the content layers.
    #[cfg(test)]
    pub(crate) fn for_tests(entries: Vec<PriceEntry>) -> Self {
        Self { entries }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Built-in table only; used by the deterministic screenshot fixture.
    /// Parsed once so the fixture does not read files on every frame.
    pub fn builtin() -> Self {
        static BUILTIN: std::sync::OnceLock<Pricing> = std::sync::OnceLock::new();
        BUILTIN
            .get_or_init(|| {
                builtin_root()
                    .map(|root| load_roots(&[root]).0)
                    .unwrap_or_default()
            })
            .clone()
    }

    pub fn lookup(&self, model: &str) -> Option<&PriceEntry> {
        let model = normalize_model(model);
        self.entries
            .iter()
            .filter_map(|entry| {
                entry
                    .patterns
                    .iter()
                    .filter_map(|pattern| match_len(&model, pattern).map(|len| (len, entry)))
                    .max_by_key(|(len, _)| *len)
            })
            .max_by_key(|(len, _)| *len)
            .map(|(_, entry)| entry)
    }

    /// Cost of the token totals for `model`. `None` means at least one token
    /// category has no price, so the caller must not estimate.
    pub fn cost_for(&self, model: &str, totals: &UsageTotals) -> Option<f64> {
        let normalized = normalize_model(model);
        if normalized.is_empty() || normalized == "unknown" {
            return None;
        }
        // OpenCode Zen's free models report `-free`/`:free`; their price is
        // genuinely zero, not an unknown.
        if normalized.ends_with("-free") || normalized.ends_with(":free") {
            return Some(0.0);
        }
        let entry = self.lookup(model)?;
        let mut cost = per_million(totals.input_tokens, entry.input)?;
        cost += per_million(totals.output_tokens, entry.output)?;
        cost += per_million(totals.cache_read_tokens, entry.cache_read)?;
        cost += per_million(totals.cache_write_tokens, entry.cache_write)?;
        Some(cost)
    }
}

fn per_million(tokens: u64, rate: Option<f64>) -> Option<f64> {
    if tokens == 0 {
        return Some(0.0);
    }
    rate.map(|rate| tokens as f64 * rate / MILLION)
}

#[derive(Debug, Deserialize)]
struct PricingFile {
    #[serde(default)]
    model: Vec<PriceFileEntry>,
}

#[derive(Debug, Deserialize)]
struct PriceFileEntry {
    id: String,
    #[serde(default, rename = "match")]
    match_: Option<String>,
    #[serde(default)]
    models: Vec<String>,
    #[serde(default)]
    input: Option<f64>,
    #[serde(default)]
    output: Option<f64>,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    checked: String,
}

fn load_roots(roots: &[PathBuf]) -> (Pricing, Vec<String>) {
    let mut entries: BTreeMap<String, PriceEntry> = BTreeMap::new();
    let mut errors = Vec::new();
    for root in roots {
        for path in files_in(&root.join("pricing")) {
            match std::fs::read_to_string(&path) {
                Ok(text) => parse_file(&path, &text, &mut entries, &mut errors),
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
    }
    (
        Pricing {
            entries: entries.into_values().collect(),
        },
        errors,
    )
}

fn parse_file(
    path: &Path,
    text: &str,
    entries: &mut BTreeMap<String, PriceEntry>,
    errors: &mut Vec<String>,
) {
    let document: PricingFile = match toml::from_str(text) {
        Ok(document) => document,
        Err(error) => {
            errors.push(format!("{}: {error}", path.display()));
            return;
        }
    };
    if document.model.is_empty() {
        errors.push(format!("{}: no [[model]] entries", path.display()));
        return;
    }
    for raw in document.model {
        let id = raw.id.trim().to_ascii_lowercase();
        if id.is_empty() {
            errors.push(format!("{}: an entry has no id", path.display()));
            continue;
        }
        let mut patterns: Vec<String> = raw
            .models
            .iter()
            .map(|item| normalize_pattern(item))
            .collect();
        if let Some(single) = raw.match_ {
            patterns.push(normalize_pattern(&single));
        }
        patterns.retain(|pattern| !pattern.is_empty());
        patterns.sort();
        patterns.dedup();
        if patterns.is_empty() {
            errors.push(format!(
                "{}: `{id}` needs `match` or `models`",
                path.display()
            ));
            continue;
        }
        if raw.source.trim().is_empty() {
            errors.push(format!(
                "{}: `{id}` needs the official `source` of its prices",
                path.display()
            ));
            continue;
        }
        if chrono::NaiveDate::parse_from_str(raw.checked.trim(), "%Y-%m-%d").is_err() {
            errors.push(format!(
                "{}: `{id}` needs `checked = \"YYYY-MM-DD\"`",
                path.display()
            ));
            continue;
        }
        let rates = [
            ("input", raw.input),
            ("output", raw.output),
            ("cache_read", raw.cache_read),
            ("cache_write", raw.cache_write),
        ];
        if raw.input.is_none() && raw.output.is_none() {
            errors.push(format!(
                "{}: `{id}` needs at least input and output prices",
                path.display()
            ));
            continue;
        }
        if rates
            .iter()
            .any(|(_, rate)| rate.is_some_and(|value| !value.is_finite() || value < 0.0))
        {
            errors.push(format!(
                "{}: `{id}` has a negative or invalid price",
                path.display()
            ));
            continue;
        }
        entries.insert(
            id.clone(),
            PriceEntry {
                id,
                patterns,
                input: raw.input,
                output: raw.output,
                cache_read: raw.cache_read,
                cache_write: raw.cache_write,
                source: raw.source.trim().to_owned(),
                checked: raw.checked.trim().to_owned(),
            },
        );
    }
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
        })
        .collect();
    files.sort();
    files
}

/// Path of the embedded content: the repository `content/` folder or the one
/// next to the executable. Same contract as the `content` module.
fn builtin_root() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("FORJA_BUILTIN_CONTENT_DIR") {
        return Some(PathBuf::from(value));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../content");
    if manifest.is_dir() {
        return Some(manifest);
    }
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    [parent.join("content"), parent.join("..").join("content")]
        .into_iter()
        .find(|candidate| candidate.is_dir())
}

fn normalize_model(model: &str) -> String {
    let tail = model.rsplit('/').next().unwrap_or(model);
    tail.trim().to_ascii_lowercase().replace(['.', '_'], "-")
}

fn normalize_pattern(pattern: &str) -> String {
    pattern.trim().to_ascii_lowercase().replace(['.', '_'], "-")
}

fn match_len(model: &str, pattern: &str) -> Option<usize> {
    if let Some(prefix) = pattern.strip_suffix('*') {
        return model.starts_with(prefix).then_some(prefix.len() + 1);
    }
    if model == pattern {
        return Some(pattern.len());
    }
    if !model.starts_with(pattern) {
        return None;
    }
    let rest = model.get(pattern.len()..)?;
    rest.chars()
        .next()
        .is_some_and(|next| !next.is_ascii_alphanumeric())
        .then_some(pattern.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-usage-pricing-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pricing(dir: &Path, name: &str, body: &str) {
        let pricing = dir.join("pricing");
        fs::create_dir_all(&pricing).unwrap();
        fs::write(pricing.join(name), body).unwrap();
    }

    const TABLE: &str = r#"
[[model]]
id = "sonnet"
models = ["claude-sonnet-4-5"]
input = 3.0
output = 15.0
cache_read = 0.3
cache_write = 3.75
source = "https://example.test/pricing"
checked = "2026-11-14"

[[model]]
id = "gpt-5"
match = "gpt-5"
input = 1.25
output = 10.0
cache_read = 0.125
cache_write = 0.0
source = "https://example.test/pricing"
checked = "2026-11-14"

[[model]]
id = "gpt-5-5"
match = "gpt-5-5"
input = 5.0
output = 30.0
cache_read = 0.5
cache_write = 0.0
source = "https://example.test/pricing"
checked = "2026-11-14"
"#;

    fn totals(input: u64, output: u64, read: u64, write: u64) -> UsageTotals {
        UsageTotals {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: read,
            cache_write_tokens: write,
            total_tokens: input + output,
            cost_usd: 0.0,
            unpriced_tokens: 0,
        }
    }

    #[test]
    fn computes_cost_from_the_table_and_reports_unknown_models() {
        let dir = temp_dir("calc");
        write_pricing(&dir, "models.toml", TABLE);
        let (pricing, errors) = load_roots(std::slice::from_ref(&dir));
        assert!(errors.is_empty(), "{errors:?}");
        let cost = pricing
            .cost_for(
                "claude-sonnet-4-5-20250929",
                &totals(1_000_000, 100_000, 200_000, 50_000),
            )
            .expect("price exists");
        let expected = 3.0 + 1.5 + 0.06 + 0.1875;
        assert!((cost - expected).abs() < 1e-9, "{cost} != {expected}");
        assert!(pricing
            .cost_for("unknown-model", &totals(10, 0, 0, 0))
            .is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn provider_prefix_and_longest_match_win() {
        let dir = temp_dir("match");
        write_pricing(&dir, "models.toml", TABLE);
        let (pricing, _) = load_roots(std::slice::from_ref(&dir));
        let cost = pricing
            .cost_for("openai/gpt-5.5", &totals(1_000_000, 0, 0, 0))
            .expect("gpt-5.5 is priced");
        assert!((cost - 5.0).abs() < 1e-9, "{cost}");
        let base = pricing
            .cost_for("gpt-5-codex", &totals(1_000_000, 0, 0, 0))
            .expect("gpt-5 family is priced");
        assert!((base - 1.25).abs() < 1e-9, "{base}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_cache_price_keeps_the_cost_unknown_when_cache_was_used() {
        let dir = temp_dir("cache");
        write_pricing(
            &dir,
            "models.toml",
            r#"
[[model]]
id = "partial"
match = "partial"
input = 1.0
output = 2.0
source = "https://example.test/pricing"
checked = "2026-11-14"
"#,
        );
        let (pricing, _) = load_roots(std::slice::from_ref(&dir));
        assert!(pricing.cost_for("partial", &totals(10, 2, 0, 0)).is_some());
        assert!(pricing.cost_for("partial", &totals(10, 2, 5, 0)).is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn free_models_cost_zero_without_a_table_entry() {
        let (pricing, _) = load_roots(&[]);
        assert_eq!(
            pricing.cost_for("opencode/deepseek-v4.1-flash-free", &totals(10, 2, 0, 0)),
            Some(0.0)
        );
    }

    #[test]
    fn entries_without_source_or_checked_date_are_rejected() {
        let dir = temp_dir("invalid");
        write_pricing(
            &dir,
            "models.toml",
            r#"
[[model]]
id = "no-source"
match = "no-source"
input = 1.0
output = 2.0
checked = "2026-11-14"

[[model]]
id = "no-date"
match = "no-date"
input = 1.0
output = 2.0
source = "https://example.test/pricing"
"#,
        );
        let (pricing, errors) = load_roots(std::slice::from_ref(&dir));
        assert!(pricing.is_empty());
        assert_eq!(errors.len(), 2, "{errors:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_later_layer_replaces_the_same_id() {
        let builtin = temp_dir("layer-builtin");
        let user = temp_dir("layer-user");
        write_pricing(&builtin, "models.toml", TABLE);
        write_pricing(
            &user,
            "models.toml",
            r#"
[[model]]
id = "sonnet"
match = "claude-sonnet-4-5"
input = 1.0
output = 2.0
cache_read = 0.1
cache_write = 0.1
source = "https://example.test/user-pricing"
checked = "2026-11-14"
"#,
        );
        let (pricing, errors) = load_roots(&[builtin.clone(), user.clone()]);
        assert!(errors.is_empty(), "{errors:?}");
        let cost = pricing
            .cost_for("claude-sonnet-4-5", &totals(1_000_000, 0, 0, 0))
            .expect("price");
        assert!((cost - 1.0).abs() < 1e-9, "{cost}");
        let _ = fs::remove_dir_all(builtin);
        let _ = fs::remove_dir_all(user);
    }
}
