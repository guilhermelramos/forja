//! Leitura assíncrona de regras do projeto e montagem do contexto inicial de lançamento.

use crate::profile::Profile;
use crate::Shared;
use forja_contracts::RequestRef;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

const MAX_FILE_CHARS: usize = 8_000;
const MAX_TOTAL_CHARS: usize = 16_000;

/// Chave do cache em memória de arquivos de regras lidos.
pub(crate) type RulesCacheKey = (String, Vec<String>);

/// Trecho de um arquivo de regras do projeto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleSnippet {
    /// Caminho relativo declarado no projeto/perfil.
    pub path: String,
    /// Conteúdo lido e sanitizado.
    pub content: String,
}

/// Resumo da bancada usado quando `helper.launch.initial_context = "bench"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchContext {
    pub name: String,
    pub branch: Option<String>,
    pub path: String,
    pub request_key: Option<String>,
    pub request_ref: Option<RequestRef>,
}

/// Chave estável para indexar mapas por caminho de bancada.
pub(crate) fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

/// Chave de cache para um par `(bancada, candidatos)`.
pub(crate) fn cache_key(bench_dir: &Path, candidates: &[String]) -> RulesCacheKey {
    (path_key(bench_dir), candidates.to_vec())
}

/// Rejeita caminhos absolutos ou com `..` ao ler arquivos de regras.
fn clean_rel_path(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return None;
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push_str("\n...");
    out
}

/// Lê os arquivos de regras existentes na pasta da bancada (sem bloquear a thread da UI).
pub fn read_rules_files(bench_dir: &Path, candidates: &[String]) -> Vec<RuleSnippet> {
    let mut rules = Vec::new();
    let mut remaining = MAX_TOTAL_CHARS;
    let Ok(root) = std::fs::canonicalize(bench_dir) else {
        return rules;
    };
    for raw in candidates {
        if remaining == 0 {
            break;
        }
        let Some(rel) = clean_rel_path(raw) else {
            continue;
        };
        let full = bench_dir.join(&rel);
        // A junção ou link simbólico pode apontar para fora da bancada; a
        // validação lexical sozinha não detecta isso.
        let Ok(effective) = std::fs::canonicalize(&full) else {
            continue;
        };
        if !effective.starts_with(&root) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&effective) else {
            continue;
        };
        let trimmed = content.trim();
        if trimmed.is_empty() {
            continue;
        }
        let budget = remaining.min(MAX_FILE_CHARS);
        let clipped = truncate_chars(trimmed, budget);
        let used = clipped.chars().count();
        remaining = remaining.saturating_sub(used);
        rules.push(RuleSnippet {
            path: raw.trim().to_owned(),
            content: clipped,
        });
    }
    rules
}

/// Formata as regras lidas para entrar no contexto inicial do ajudante.
pub fn format_rules_block(rules: &[RuleSnippet]) -> Option<String> {
    if rules.is_empty() {
        return None;
    }
    let mut parts = Vec::with_capacity(rules.len());
    for rule in rules {
        parts.push(format!("## Project rules ({})\n{}", rule.path, rule.content));
    }
    Some(parts.join("\n\n"))
}

/// Formata título, descrição e links do pedido para o contexto inicial.
pub fn format_request_context(req: &RequestRef) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Request [{} {}]: {}",
        req.counter, req.key, req.title
    ));
    if let Some(description) = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Description:\n{description}"));
    }
    let mut links = Vec::new();
    if let Some(url) = req
        .url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        links.push(url.to_owned());
    }
    for link in &req.links {
        let trimmed = link.trim();
        if !trimmed.is_empty() && !links.iter().any(|existing| existing == trimmed) {
            links.push(trimmed.to_owned());
        }
    }
    if !links.is_empty() {
        lines.push("Links:".to_owned());
        for link in links {
            lines.push(format!("- {link}"));
        }
    }
    lines.join("\n\n")
}

/// Formata resumo da bancada quando `helper.launch.initial_context = "bench"`.
pub fn format_bench_context(bench: &BenchContext, req: Option<&RequestRef>) -> String {
    let mut lines = vec![format!("Bench: {}", bench.name)];
    if let Some(branch) = bench
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Branch: {branch}"));
    }
    lines.push(format!("Path: {}", bench.path));
    if let Some(req) = req {
        lines.push(format!("Request: {} — {}", req.key, req.title));
    } else if let Some(key) = bench
        .request_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        lines.push(format!("Request: {key}"));
    }
    lines.join("\n")
}

/// Combina regras, contexto inicial (`request` | `bench` | `none`) e prompt explícito.
pub fn compose_initial_prompt(
    context_mode: &str,
    bench: Option<&BenchContext>,
    request_ref: Option<&RequestRef>,
    rules: Option<&[RuleSnippet]>,
    explicit_prompt: Option<&str>,
) -> Option<String> {
    let mut sections = Vec::new();
    if let Some(rules) = rules.and_then(format_rules_block) {
        sections.push(rules);
    }
    match context_mode.trim() {
        "request" => {
            if let Some(req) = request_ref {
                sections.push(format_request_context(req));
            }
        }
        "bench" => {
            if let Some(bench) = bench {
                sections.push(format_bench_context(bench, request_ref));
            }
        }
        _ => {}
    }
    if let Some(prompt) = explicit_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        sections.push(prompt.to_owned());
    }
    (!sections.is_empty()).then(|| sections.join("\n\n"))
}

/// Consulta os metadados e o `RequestRef` da bancada no módulo `bench`, se existir.
pub(crate) fn fetch_bench_context(cx: &mut Ctx<'_>, bench_path: &Path) -> Option<BenchContext> {
    if cx.registry().command("bench.request").is_some() {
        let mut args = Args::new();
        args.insert("bench_path", json!(bench_path.display().to_string()));
        if let Ok(value) = cx.run_command("bench.request", args) {
            if !value.is_null() {
                return Some(BenchContext {
                    name: value
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    branch: value
                        .get("branch")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    path: value
                        .get("bench_path")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    request_key: value
                        .get("request_key")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    request_ref: value
                        .get("request_ref")
                        .cloned()
                        .and_then(|raw| serde_json::from_value::<RequestRef>(raw).ok()),
                });
            }
        }
    }
    let current = cx.run_command("bench.current", Args::new()).ok()?;
    let path = current.get("path").and_then(Value::as_str)?;
    if !crate::status::same_path(path, &bench_path.display().to_string()) {
        return None;
    }
    Some(BenchContext {
        name: current
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        branch: current
            .get("branch")
            .and_then(Value::as_str)
            .map(str::to_owned),
        path: path.to_owned(),
        request_key: None,
        request_ref: None,
    })
}

/// Pré-carrega em segundo plano os arquivos de regras da bancada, sem bloquear a UI.
pub(crate) fn schedule_rules_preload(
    cx: &mut Ctx<'_>,
    core: &Shared,
    bench_dir: PathBuf,
    candidates: Vec<String>,
) {
    if !cx
        .setting("helper.launch.include_rules")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
    {
        return;
    }
    let key = cache_key(&bench_dir, &candidates);
    let should_start = core
        .with_state(|state| {
            if state.launch.rules_cache.contains_key(&key) || state.launch.rules_loading.contains(&key) {
                false
            } else {
                state.launch.rules_loading.insert(key.clone());
                true
            }
        })
        .unwrap_or(false);
    if !should_start {
        return;
    }
    let owned = Arc::clone(core);
    let candidates_bg = candidates.clone();
    cx.spawn(
        move || read_rules_files(&bench_dir, &candidates_bg),
        move |_cx, rules| {
            let _ = owned.with_state(|state| {
                state.launch.rules_loading.remove(&key);
                state.launch.rules_cache.insert(key, rules);
            });
        },
    );
}

use std::sync::Arc;

/// Retorna regras em cache para `(bench_dir, profile)` ou inicia leitura assíncrona em tarefa do kernel.
pub(crate) fn resolve_rules_for_launch(
    cx: &mut Ctx<'_>,
    core: &Shared,
    bench_dir: &Path,
    profile: &Profile,
) -> Option<Vec<RuleSnippet>> {
    if !cx
        .setting("helper.launch.include_rules")
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
    {
        return Some(Vec::new());
    }
    let candidates = profile.rule_files();
    let key = cache_key(bench_dir, &candidates);
    core.with_state(|state| state.launch.rules_cache.get(&key).cloned())
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Layer;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-context-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    #[test]
    fn read_rules_reads_profile_and_standard_files_and_rejects_traversal() {
        let dir = temp_dir("rules");
        std::fs::write(dir.0.join("AGENTS.md"), "Always run cargo test.").unwrap();
        std::fs::write(dir.0.join("CLAUDE.md"), "Use Rust 2021.").unwrap();
        std::fs::write(dir.0.join("CUSTOM.md"), "Custom profile rule.").unwrap();

        let profile = Profile::parse(
            r#"
id = "rule-agent"
name = "Rule Agent"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["x"]
[start]
command = "x"
[rules]
files = ["../secret.txt", "CUSTOM.md"]
"#,
            PathBuf::from("rule-agent.toml"),
            Layer::User,
        )
        .unwrap();
        let candidates = profile.rule_files();
        let loaded = read_rules_files(&dir.0, &candidates);
        let paths: Vec<&str> = loaded.iter().map(|rule| rule.path.as_str()).collect();
        assert_eq!(paths, vec!["CUSTOM.md", "AGENTS.md", "CLAUDE.md"]);
        assert!(loaded[0].content.contains("Custom profile rule."));
        assert!(loaded[1].content.contains("Always run cargo test."));
    }

    #[cfg(windows)]
    #[test]
    fn read_rules_rejects_junction_that_escapes_the_bench() {
        let dir = temp_dir("junction");
        let outside = dir.0.join("outside");
        let bench = dir.0.join("bench");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(&bench).unwrap();
        std::fs::write(outside.join("AGENTS.md"), "EXTERNAL RULE").unwrap();
        std::fs::write(bench.join("AGENTS.md"), "BENCH RULE").unwrap();

        let link = bench.join("escape");
        let output = forja_kernel::process::command("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(&outside)
            .output()
            .expect("mklink runs");
        assert!(output.status.success(), "mklink failed: {:?}", output);

        let loaded = read_rules_files(&bench, &["escape/AGENTS.md".to_owned()]);
        assert!(loaded.is_empty(), "junction to outside must be rejected");

        let normal = read_rules_files(&bench, &["AGENTS.md".to_owned()]);
        assert_eq!(normal.len(), 1);
        assert!(normal[0].content.contains("BENCH RULE"));
    }

    #[test]
    fn compose_initial_prompt_handles_request_bench_none_and_rules() {
        let req = RequestRef {
            counter: "github".into(),
            key: "#42".into(),
            title: "Fix startup race".into(),
            url: Some("https://github.com/org/repo/issues/42".into()),
            description: Some("Repro steps inside issue body.".into()),
            links: vec![
                "https://github.com/org/repo/issues/42".into(),
                "https://docs.example.test/spec".into(),
            ],
        };
        let bench = BenchContext {
            name: "pedido-42".into(),
            branch: Some("forja/pedido-42".into()),
            path: "C:/work/pedido-42".into(),
            request_key: Some("#42".into()),
            request_ref: Some(req.clone()),
        };
        let rules = vec![RuleSnippet {
            path: "AGENTS.md".into(),
            content: "Keep files small.".into(),
        }];

        let request_prompt =
            compose_initial_prompt("request", Some(&bench), Some(&req), Some(&rules), Some("Go"))
                .unwrap();
        assert!(request_prompt.contains("AGENTS.md"));
        assert!(request_prompt.contains("Keep files small."));
        assert!(request_prompt.contains("Fix startup race"));
        assert!(request_prompt.contains("Repro steps inside issue body."));
        assert!(request_prompt.contains("https://github.com/org/repo/issues/42"));
        assert!(request_prompt.contains("https://docs.example.test/spec"));
        assert!(request_prompt.ends_with("Go"));

        let bench_prompt =
            compose_initial_prompt("bench", Some(&bench), Some(&req), None, None).unwrap();
        assert!(bench_prompt.contains("Bench: pedido-42"));
        assert!(bench_prompt.contains("Branch: forja/pedido-42"));
        assert!(!bench_prompt.contains("Repro steps inside issue body."));

        let none_prompt =
            compose_initial_prompt("none", Some(&bench), Some(&req), None, Some("Only me"))
                .unwrap();
        assert_eq!(none_prompt, "Only me");
        assert!(compose_initial_prompt("none", Some(&bench), Some(&req), None, None).is_none());
    }

    #[test]
    fn request_context_accepts_current_and_legacy_request_refs() {
        let current: RequestRef = serde_json::from_str(
            r##"{"counter":"github","key":"#7","title":"Fix [spec](https://docs.example.test/spec)","url":null,"description":"Body with [spec](https://docs.example.test/spec)","links":["https://docs.example.test/spec"]}"##,
        )
        .unwrap();
        let legacy: RequestRef = serde_json::from_str(
            r##"{"counter":"github","key":"#8","title":"Old request","url":"https://github.com/o/r/issues/8"}"##,
        )
        .unwrap();

        let full = compose_initial_prompt("request", None, Some(&current), None, Some("Go"))
            .expect("current request context");
        assert!(full.contains("Fix [spec](https://docs.example.test/spec)"));
        assert!(full.contains("Body with [spec](https://docs.example.test/spec)"));
        assert!(full.contains("https://docs.example.test/spec"));
        assert!(full.ends_with("Go"));

        let old = compose_initial_prompt("request", None, Some(&legacy), None, None)
            .expect("legacy request context");
        assert!(old.contains("Old request"));
        assert!(old.contains("https://github.com/o/r/issues/8"));
        assert!(!old.contains("Description:"));
    }
}
