//! Data contracts, bench server filtering, and background doc discovery for `forja://inicio`.

use crate::history::History;
use forja_kernel::{Args, Ctx};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const HOME_URL: &str = "forja://inicio";
const MAX_DOCS: usize = 32;
const MAX_DOC_DEPTH: usize = 4;

/// Returns `true` when `url` targets Forja's built-in browser home page.
pub fn is_home_url(url: &str) -> bool {
    let normalized = url.trim().trim_end_matches('/');
    normalized.eq_ignore_ascii_case(HOME_URL)
}

/// Port descriptor matching the `ports.list` contract schema.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct PortEntry {
    pub port: u16,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub process: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub bench_id: String,
    #[serde(default)]
    pub bench_name: String,
    #[serde(default)]
    pub from_tree: bool,
    #[serde(default)]
    pub is_system: bool,
}

impl PortEntry {
    /// Same bench predicate used by the `ports` module.
    pub fn is_forja_port(&self) -> bool {
        !self.bench_name.is_empty() || !self.bench_id.is_empty() || self.from_tree
    }

    /// Same local HTTP URL rule used by the `ports` module.
    pub fn local_url(&self) -> String {
        let scheme = match self.port {
            443 | 8443 => "https",
            _ => "http",
        };
        let host = match self.address.as_str() {
            "::1" => "[::1]",
            "::" | "0.0.0.0" | "" => "localhost",
            other => other,
        };
        format!("{scheme}://{host}:{}", self.port)
    }

    /// Same best-effort HTTP(S) heuristic used by the `ports` module.
    pub fn is_http(&self) -> bool {
        if matches!(
            self.port,
            135 | 139
                | 445
                | 1433
                | 1521
                | 3306
                | 3389
                | 5040
                | 5357
                | 5432
                | 5672
                | 6379
                | 7680
                | 9229
                | 27017
        ) {
            return false;
        }
        if matches!(
            self.port,
            80 | 443
                | 1313
                | 3000..=3999
                | 4000..=4999
                | 5000..=5999
                | 6006
                | 7000..=7999
                | 8000..=8999
                | 9000..=9099
        ) {
            return !self.is_system || matches!(self.port, 80 | 443 | 8080 | 8443);
        }
        if self.is_system {
            return false;
        }
        let stem = self
            .process
            .to_ascii_lowercase()
            .trim_end_matches(".exe")
            .to_owned();
        matches!(
            stem.as_str(),
            "node"
                | "bun"
                | "deno"
                | "python"
                | "uvicorn"
                | "gunicorn"
                | "ruby"
                | "puma"
                | "php"
                | "caddy"
                | "nginx"
                | "httpd"
                | "hugo"
                | "vite"
                | "trunk"
                | "webpack"
                | "esbuild"
                | "next-server"
        )
    }

    /// Bench display label, if known.
    pub fn bench_label(&self) -> Option<&str> {
        if !self.bench_name.is_empty() {
            Some(self.bench_name.as_str())
        } else if !self.bench_id.is_empty() {
            Some(self.bench_id.as_str())
        } else {
            None
        }
    }

    /// Same token filter semantics as `ports::model::PortInfo::matches_filter`.
    pub fn matches_filter(&self, query: &str) -> bool {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return true;
        }
        let port_str = self.port.to_string();
        let colon_port = format!(":{port_str}");
        let pid_str = self.pid.to_string();
        let url = self.local_url().to_ascii_lowercase();
        let process = self.process.to_ascii_lowercase();
        let bench_name = self.bench_name.to_ascii_lowercase();
        let bench_id = self.bench_id.to_ascii_lowercase();
        let address = self.address.to_ascii_lowercase();
        let cwd = self.cwd.to_ascii_lowercase();
        trimmed.split_whitespace().all(|token| {
            let token = token.to_ascii_lowercase();
            port_str.contains(&token)
                || colon_port.contains(&token)
                || pid_str.contains(&token)
                || url.contains(&token)
                || process.contains(&token)
                || bench_name.contains(&token)
                || bench_id.contains(&token)
                || address.contains(&token)
                || cwd.contains(&token)
        })
    }
}

/// Active bench or project context discovered through existing commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveRoot {
    pub name: String,
    pub path: PathBuf,
    pub bench_id: String,
    pub is_bench: bool,
}

impl ActiveRoot {
    pub fn matches_port(&self, port: &PortEntry) -> bool {
        if self.is_bench && !self.bench_id.is_empty() && port.bench_id == self.bench_id {
            return true;
        }
        if !self.name.is_empty() && port.bench_name.eq_ignore_ascii_case(&self.name) {
            return true;
        }
        false
    }
}

/// Documentation link discovered under the active project or bench.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocLink {
    pub label: String,
    pub abs_path: PathBuf,
    pub viewer_supported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentShortcut {
    pub title: String,
    pub url: String,
}

/// Parses `ports.list` JSON payload into `PortEntry` items.
pub fn parse_ports_response(value: &Value) -> Vec<PortEntry> {
    value
        .get("ports")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| serde_json::from_value::<PortEntry>(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Filters and orders bench HTTP servers using the Ports bench filter semantics.
pub fn filter_bench_servers<'a>(
    ports: &'a [PortEntry],
    query: &str,
    only_bench: Option<&ActiveRoot>,
    prioritize_bench: Option<&ActiveRoot>,
) -> Vec<&'a PortEntry> {
    let mut matched: Vec<&'a PortEntry> = ports
        .iter()
        .filter(|port| port.is_forja_port() && port.is_http())
        .filter(|port| port.matches_filter(query))
        .filter(|port| {
            only_bench
                .map(|bench| bench.matches_port(port))
                .unwrap_or(true)
        })
        .collect();
    matched.sort_by_key(|port| {
        let active_rank = prioritize_bench
            .map(|bench| !bench.matches_port(port))
            .unwrap_or(true);
        (
            active_rank,
            port.bench_label().unwrap_or("").to_ascii_lowercase(),
            port.port,
            port.pid,
        )
    });
    matched
}

/// Extracts up to `limit` deduplicated external/local recent shortcuts from history.
pub fn recent_shortcuts(history: &History, limit: usize) -> Vec<RecentShortcut> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in &history.entries {
        let trimmed = entry.url.trim();
        if trimmed.is_empty() || trimmed.starts_with("forja:") || trimmed.starts_with("about:") {
            continue;
        }
        if seen.insert(trimmed.to_ascii_lowercase()) {
            out.push(RecentShortcut {
                title: entry.title.trim().to_owned(),
                url: trimmed.to_owned(),
            });
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// Discovers `README*` and `docs/` documentation files under `root`.
/// Must be called from a background task (`cx.spawn`), never from the UI thread.
pub fn discover_docs(root: &Path) -> Vec<DocLink> {
    if !root.is_dir() {
        return Vec::new();
    }
    let mut readmes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if is_readme_name(name) {
                readmes.push(DocLink {
                    label: name.to_owned(),
                    viewer_supported: is_viewer_supported(&path),
                    abs_path: path,
                });
            }
        }
    }
    readmes.sort_by(|a, b| {
        (!a.viewer_supported, a.label.to_ascii_lowercase())
            .cmp(&(!b.viewer_supported, b.label.to_ascii_lowercase()))
    });

    let mut docs = Vec::new();
    let docs_dir = root.join("docs");
    if docs_dir.is_dir() {
        collect_docs_dir(root, &docs_dir, 0, &mut docs);
    }
    docs.sort_by(|a, b| {
        (!a.viewer_supported, a.label.to_ascii_lowercase())
            .cmp(&(!b.viewer_supported, b.label.to_ascii_lowercase()))
    });

    readmes.extend(docs);
    readmes.truncate(MAX_DOCS);
    readmes
}

fn collect_docs_dir(root: &Path, dir: &Path, depth: usize, out: &mut Vec<DocLink>) {
    if depth >= MAX_DOC_DEPTH || out.len() >= MAX_DOCS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if out.len() >= MAX_DOCS {
            break;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if matches!(name, "target" | "node_modules" | "dist" | "build") {
                continue;
            }
            collect_docs_dir(root, &path, depth + 1, out);
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(DocLink {
                label: rel,
                viewer_supported: is_viewer_supported(&path),
                abs_path: path,
            });
        }
    }
}

fn is_readme_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "readme" || lower.starts_with("readme.")
}

fn is_viewer_supported(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "md" | "markdown"
            | "csv"
            | "tsv"
            | "ipynb"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "webp"
            | "ico"
            | "pdf"
    )
}

pub fn resolve_active_root(cx: &mut Ctx<'_>) -> Option<ActiveRoot> {
    if let Ok(value) = cx.run_command("bench.current", Args::new()) {
        if let (Some(name), Some(path)) = (
            value.get("name").and_then(Value::as_str),
            value.get("path").and_then(Value::as_str),
        ) {
            if !path.is_empty() {
                return Some(ActiveRoot {
                    name: name.to_owned(),
                    path: PathBuf::from(path),
                    bench_id: value
                        .get("bench_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    is_bench: true,
                });
            }
        }
    }
    if let Ok(value) = cx.run_command("project.current", Args::new()) {
        if let (Some(name), Some(path)) = (
            value.get("name").and_then(Value::as_str),
            value.get("path").and_then(Value::as_str),
        ) {
            if !path.is_empty() {
                return Some(ActiveRoot {
                    name: name.to_owned(),
                    path: PathBuf::from(path),
                    bench_id: String::new(),
                    is_bench: false,
                });
            }
        }
    }
    None
}

/// Opens a project doc in Visualizador via `viewers.open` + `shell.tab.open`, with `file.open` fallback.
pub fn open_doc_in_viewer(cx: &mut Ctx<'_>, path: &Path) -> bool {
    let mut args = Args::new();
    args.insert("path", json!(path.display().to_string()));
    if let Ok(value) = cx.run_command("viewers.open", args) {
        if value.get("opened").and_then(Value::as_bool) == Some(true) {
            let mut tab_args = Args::new();
            tab_args.insert("view_id", json!("viewers.tab"));
            let _ = cx.run_command("shell.tab.open", tab_args);
            return true;
        }
    }
    let mut file_args = Args::new();
    file_args.insert("path", json!(path.display().to_string()));
    cx.run_command("file.open", file_args).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Entry;
    use forja_kernel::{Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    struct TempGuard(PathBuf);
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-browser-home-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    #[test]
    fn is_home_url_matches_default_case_and_trailing_slash() {
        assert!(is_home_url("forja://inicio"));
        assert!(is_home_url("  FORJA://INICIO/ "));
        assert!(!is_home_url("forja://mermaid"));
        assert!(!is_home_url("http://localhost:3000"));
    }

    #[test]
    fn bench_server_filter_reuses_ports_bench_contract() {
        let payload = json!({
            "ports": [
                { "port": 135, "pid": 1, "process": "svchost.exe", "address": "0.0.0.0", "bench_id": "", "bench_name": "", "from_tree": false, "is_system": true },
                { "port": 3000, "pid": 2, "process": "node.exe", "address": "0.0.0.0", "bench_id": "web", "bench_name": "forja-web", "from_tree": true, "is_system": false },
                { "port": 5173, "pid": 3, "process": "node.exe", "address": "127.0.0.1", "bench_id": "docs", "bench_name": "forja-docs", "from_tree": true, "is_system": false },
                { "port": 5432, "pid": 4, "process": "postgres.exe", "address": "0.0.0.0", "bench_id": "", "bench_name": "", "from_tree": false, "is_system": false },
                { "port": 9229, "pid": 5, "process": "node.exe", "address": "127.0.0.1", "bench_id": "web", "bench_name": "forja-web", "from_tree": true, "is_system": false }
            ]
        });
        let entries = parse_ports_response(&payload);
        assert_eq!(entries.len(), 5);

        let all_benches = filter_bench_servers(&entries, "", None, None);
        assert_eq!(all_benches.len(), 2);
        assert_eq!(all_benches[0].local_url(), "http://127.0.0.1:5173");
        assert_eq!(all_benches[1].local_url(), "http://localhost:3000");

        let web_root = ActiveRoot {
            name: "forja-web".to_owned(),
            path: PathBuf::from("C:/work/forja/web"),
            bench_id: "web".to_owned(),
            is_bench: true,
        };
        let filtered_active = filter_bench_servers(&entries, "", Some(&web_root), Some(&web_root));
        assert_eq!(filtered_active.len(), 1);
        assert_eq!(filtered_active[0].port, 3000);

        let filtered_query = filter_bench_servers(&entries, "5173 docs", None, None);
        assert_eq!(filtered_query.len(), 1);
        assert_eq!(filtered_query[0].bench_name, "forja-docs");
    }

    #[test]
    fn discover_docs_finds_root_readme_and_docs_directory() {
        let dir = temp_dir();
        std::fs::write(dir.0.join("README.md"), "# Projeto").unwrap();
        std::fs::write(dir.0.join("Cargo.toml"), "[package]").unwrap();
        std::fs::create_dir_all(dir.0.join("docs/sub")).unwrap();
        std::fs::write(dir.0.join("docs/guia.md"), "# Guia").unwrap();
        std::fs::write(dir.0.join("docs/sub/arquitetura.md"), "# Arq").unwrap();
        std::fs::create_dir_all(dir.0.join("docs/target")).unwrap();
        std::fs::write(dir.0.join("docs/target/skip.md"), "skip").unwrap();

        let links = discover_docs(&dir.0);
        let labels: Vec<&str> = links.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["README.md", "docs/guia.md", "docs/sub/arquitetura.md"]
        );
        assert!(links.iter().all(|d| d.viewer_supported));
    }

    #[test]
    fn recent_shortcuts_dedupes_and_skips_internal_schemes() {
        let mut history = History::default();
        history.add("forja://inicio", "Forja");
        history.add("about:blank", "Blank");
        history.add("http://localhost:3000", "Old Title");
        history.add("http://localhost:5173", "Docs");
        history.add("http://localhost:3000", "Web App");

        let shortcuts = recent_shortcuts(&history, 8);
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(shortcuts[0].url, "http://localhost:3000");
        assert_eq!(shortcuts[0].title, "Web App");
        assert_eq!(shortcuts[1].url, "http://localhost:5173");
    }

    struct StubViewers(Arc<Mutex<Option<String>>>);
    impl Module for StubViewers {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("viewers", "viewers.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("viewers-name = V", "viewers-name = V");
            let opened_doc = Arc::clone(&self.0);
            r.command("viewers.open", "viewers.name", move |_cx, args| {
                let path = args.required_str("path")?.to_owned();
                *opened_doc.lock().unwrap() = Some(path);
                Ok(json!({ "opened": true, "viewer": "markdown" }))
            });
        }
    }

    struct StubShell(Arc<AtomicBool>);
    impl Module for StubShell {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("shell", "shell.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("shell-name = S", "shell-name = S");
            let switched = Arc::clone(&self.0);
            r.command("shell.tab.open", "shell.name", move |_cx, args| {
                if args.required_str("view_id")? == "viewers.tab" {
                    switched.store(true, Ordering::SeqCst);
                }
                Ok(json!({ "opening": true }))
            });
        }
    }

    struct StubPorts;
    impl Module for StubPorts {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("ports", "ports.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("ports-name = P", "ports-name = P");
            r.rpc(
                forja_kernel::forja_protocol::Method::new("ports.list", "ports.name"),
                |_cx, _params| {
                    Ok(json!({
                        "ports": [{
                            "port": 5173,
                            "pid": 100,
                            "process": "vite.exe",
                            "address": "127.0.0.1",
                            "bench_id": "docs",
                            "bench_name": "forja-docs",
                            "from_tree": true,
                            "is_system": false
                        }],
                        "scanned_at_ms": 1,
                        "busy": false,
                        "error": null
                    }))
                },
            );
        }
    }

    struct StubCaller;
    impl Module for StubCaller {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("caller", "caller.name", "0.1.0")
        }
        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("caller-name = C", "caller-name = C");
            r.command("caller.open_doc", "caller.name", |cx, args| {
                let path = args.required_str("path")?;
                let ok = open_doc_in_viewer(cx, Path::new(path));
                Ok(json!({ "ok": ok }))
            });
            r.command("caller.call_ports", "caller.name", |cx, _args| {
                let value = cx
                    .call_method("ports.list", Map::new())
                    .map_err(|e| forja_kernel::KernelError::Message(e.to_string()))?;
                let entries = parse_ports_response(&value);
                Ok(json!({ "count": entries.len(), "first_url": entries[0].local_url() }))
            });
        }
    }

    #[test]
    fn open_doc_in_viewer_and_ports_rpc_contract_work() {
        let dir = temp_dir();
        let opened_doc = Arc::new(Mutex::new(None));
        let tab_switched = Arc::new(AtomicBool::new(false));
        let mut runtime = Runtime::new(
            vec![
                Box::new(StubViewers(Arc::clone(&opened_doc))),
                Box::new(StubShell(Arc::clone(&tab_switched))),
                Box::new(StubPorts),
                Box::new(StubCaller),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();

        let readme = dir.0.join("README.md");
        std::fs::write(&readme, "# Readme").unwrap();
        let mut args = Args::new();
        args.insert("path", json!(readme.display().to_string()));
        let res = runtime.run_command("caller.open_doc", args).unwrap();
        assert_eq!(res["ok"], true);
        assert_eq!(
            opened_doc.lock().unwrap().as_deref(),
            Some(readme.display().to_string().as_str())
        );
        assert!(tab_switched.load(Ordering::SeqCst));

        let ports_res = runtime.run_command("caller.call_ports", Args::new()).unwrap();
        assert_eq!(ports_res["count"], 1);
        assert_eq!(ports_res["first_url"], "http://127.0.0.1:5173");
        let _ = Entry {
            url: String::new(),
            title: String::new(),
            visited_at: 0,
        };
    }
}
