//! Bench records, stage state and the module JSON file.

use forja_contracts::{BenchCard, BenchRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchRecord {
    pub bench_id: String,
    pub project_id: String,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub is_main: bool,
    #[serde(default = "default_stage")]
    pub stage: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub unread: bool,
    #[serde(default)]
    pub sleeping: bool,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub activated_at: u64,
    #[serde(default)]
    pub base_branch: Option<String>,
    #[serde(default)]
    pub parent_bench_id: Option<String>,
    /// HEAD when the bench was created, used to know whether Forja created
    /// commits in it (the `forging -> draft` guard).
    #[serde(default)]
    pub created_head: Option<String>,
    /// Command from `[request] command` in `forja.toml`, already expanded when
    /// the bench came from a request. Shown by the bench detail view.
    #[serde(default)]
    pub request_command: Option<String>,
    /// Terminal tabs open when the user last left this bench (see `tabs`).
    #[serde(default)]
    pub open_tabs: Vec<String>,
    /// Key of the request this bench was forged from, when it came from one.
    #[serde(default)]
    pub request_key: Option<String>,
    /// Rich request summary (title, description, links) preserved from `from_request`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_ref: Option<forja_contracts::RequestRef>,
}

fn default_stage() -> String {
    "draft".to_owned()
}

impl BenchRecord {
    pub fn to_ref(&self) -> BenchRef {
        BenchRef {
            project_id: self.project_id.clone(),
            bench_id: self.bench_id.clone(),
            name: self.name.clone(),
            path: self.path.clone(),
            branch: self.branch.clone(),
        }
    }

    pub fn to_card(&self) -> BenchCard {
        BenchCard {
            bench: self.to_ref(),
            is_main: self.is_main,
            stage: self.stage.clone(),
            archived: self.archived,
            pinned: self.pinned,
            unread: self.unread,
            sleeping: self.sleeping,
            base_branch: self.base_branch.clone(),
            created_at: self.created_at,
            activated_at: self.activated_at,
            request: self.request_key.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptApproval {
    pub project_id: String,
    #[serde(default)]
    pub setup_hash: Option<String>,
    #[serde(default)]
    pub teardown_hash: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BenchFile {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub benches: Vec<BenchRecord>,
    #[serde(default)]
    pub current: Option<String>,
    #[serde(default)]
    pub approvals: Vec<ScriptApproval>,
}

fn schema_version() -> u32 {
    SCHEMA_VERSION
}

impl BenchFile {
    pub fn main_for(&self, project_id: &str) -> Option<&BenchRecord> {
        self.benches
            .iter()
            .find(|bench| bench.project_id == project_id && bench.is_main)
    }

    pub fn find(&self, bench_id: &str) -> Option<&BenchRecord> {
        self.benches.iter().find(|bench| bench.bench_id == bench_id)
    }

    pub fn find_mut(&mut self, bench_id: &str) -> Option<&mut BenchRecord> {
        self.benches
            .iter_mut()
            .find(|bench| bench.bench_id == bench_id)
    }

    pub fn approval(&self, project_id: &str) -> Option<&ScriptApproval> {
        self.approvals
            .iter()
            .find(|approval| approval.project_id == project_id)
    }
}

/// Sorted view used by `bench.list` and the next/previous shortcuts.
/// Pinned benches come first, then the most recently activated.
pub fn ordered(benches: &[BenchRecord], include_archived: bool) -> Vec<&BenchRecord> {
    let mut selected: Vec<&BenchRecord> = benches
        .iter()
        .filter(|bench| include_archived || !bench.archived)
        .collect();
    selected.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.activated_at.cmp(&a.activated_at))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    selected
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hex = String::with_capacity(12);
    for byte in digest.iter().take(6) {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub fn bench_id(project_id: &str, path: &Path, salt: u64) -> String {
    let seed = format!("{project_id}|{}|{salt}", path.to_string_lossy());
    format!("b-{}", short_hash(&seed))
}

pub fn main_bench_id(project_id: &str) -> String {
    format!("main-{project_id}")
}

pub fn slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for character in value.trim().to_lowercase().chars().map(fold_accent) {
        if character.is_alphanumeric() {
            slug.push(character);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "bancada".to_owned()
    } else {
        slug
    }
}

fn fold_accent(character: char) -> char {
    match character {
        '\u{00e1}' | '\u{00e0}' | '\u{00e2}' | '\u{00e3}' | '\u{00e4}' => 'a',
        '\u{00e9}' | '\u{00e8}' | '\u{00ea}' | '\u{00eb}' => 'e',
        '\u{00ed}' | '\u{00ec}' | '\u{00ee}' | '\u{00ef}' => 'i',
        '\u{00f3}' | '\u{00f2}' | '\u{00f4}' | '\u{00f5}' | '\u{00f6}' => 'o',
        '\u{00fa}' | '\u{00f9}' | '\u{00fb}' | '\u{00fc}' => 'u',
        '\u{00e7}' => 'c',
        other => other,
    }
}

/// Applies `bench.worktree.name_template`. Unknown placeholders are left
/// untouched so a typo is visible instead of disappearing.
pub fn bench_name(template: &str, request: Option<&str>, value: &str, number: u32) -> String {
    let slug_value = slug(value);
    let req = request.unwrap_or("").trim();
    let req_slug = slug(req);
    let pedido = if !req_slug.is_empty()
        && (slug_value.starts_with(&format!("pedido-{req_slug}"))
            || slug_value == req_slug
            || slug_value.starts_with(&format!("{req_slug}-")))
    {
        ""
    } else {
        req
    };
    let mut name = template
        .replace("{pedido}", pedido)
        .replace("{slug}", &slug_value)
        .replace("{tipo}", "bench")
        .replace("{numero}", &number.to_string());
    while name.contains("--") {
        name = name.replace("--", "-");
    }
    let name = name.trim_matches(|c: char| c == '-' || c == '_' || c == ' ' || c == '/');
    if name.is_empty() {
        slug_value
    } else {
        slugify_path(name)
    }
}

/// Folder-safe form of an arbitrary template result.
pub fn slugify_path(value: &str) -> String {
    let mut result = String::new();
    for character in value.to_lowercase().chars().map(fold_accent) {
        if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
            result.push(character);
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    let result = result.trim_matches('-').to_owned();
    if result.is_empty() {
        "bancada".to_owned()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_folder_safe() {
        assert_eq!(slug("Pedido 123: Título Curto!"), "pedido-123-titulo-curto");
        assert_eq!(slug("---"), "bancada");
    }

    #[test]
    fn template_keeps_request_and_slug() {
        assert_eq!(
            bench_name("{pedido}-{slug}", Some("123"), "Arrumar login", 1),
            "123-arrumar-login"
        );
        assert_eq!(
            bench_name("{pedido}-{slug}", Some("LOCAL-1"), "pedido-local-1-arrumar-login", 1),
            "pedido-local-1-arrumar-login"
        );
        assert_eq!(
            bench_name("{pedido}-{slug}", None, "Arrumar login", 1),
            "arrumar-login"
        );
        assert_eq!(bench_name("x", None, "a", 1), "x");
    }

    #[test]
    fn ordering_puts_pinned_and_recent_first_and_hides_archived() {
        let mut a = record("a", 1, false, false);
        let b = record("b", 3, false, false);
        let c = record("c", 2, false, true);
        a.pinned = true;
        let benches = [a, b, c];
        let list: Vec<&str> = ordered(&benches, false)
            .iter()
            .map(|bench| bench.name.as_str())
            .collect();
        assert_eq!(list, vec!["a", "b"]);
    }

    fn record(name: &str, activated: u64, pinned: bool, archived: bool) -> BenchRecord {
        BenchRecord {
            bench_id: name.into(),
            project_id: "p".into(),
            name: name.into(),
            path: format!("C:/{name}"),
            branch: None,
            is_main: false,
            stage: "draft".into(),
            archived,
            pinned,
            unread: false,
            sleeping: false,
            created_at: 0,
            activated_at: activated,
            base_branch: None,
            parent_bench_id: None,
            created_head: None,
            request_command: None,
            open_tabs: Vec::new(),
            request_key: None,
            request_ref: None,
        }
    }
}
