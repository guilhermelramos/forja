//! Operações de host: arquivos, git e HTTP. Sempre na thread da extensão.

use crate::manifest::Capabilities;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuração imutável da thread de uma extensão.
#[derive(Clone)]
pub struct HostIo {
    pub dir: PathBuf,
    pub project: Option<PathBuf>,
    pub capabilities: Capabilities,
    pub network_allowed: bool,
    pub fs_outside_allowed: bool,
    pub store_path: PathBuf,
    pub settings_path: PathBuf,
}

impl HostIo {
    pub fn project_root(&self) -> Result<PathBuf, String> {
        self.project
            .clone()
            .ok_or_else(|| "[host] no project is open".to_owned())
    }

    /// Resolve um caminho relativo e confere os globs aprovados.
    pub fn fs_path(&self, path: &str, write: bool) -> Result<PathBuf, String> {
        if path.trim().is_empty() {
            return Err("[argument] empty path".to_owned());
        }
        let absolute = Path::new(path).is_absolute();
        if absolute && !self.fs_outside_allowed {
            return Err(format!(
                "[capability_denied] absolute path `{path}` needs extension.fs.allow_outside_project"
            ));
        }
        let normalized = path.replace('\\', "/");
        if normalized.split('/').any(|part| part == "..") {
            return Err(format!(
                "[capability_denied] path `{path}` leaves the project"
            ));
        }
        let root = self.project_root()?;
        let target = if absolute {
            PathBuf::from(path)
        } else {
            root.join(path)
        };
        let relative = target
            .strip_prefix(&root)
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| normalized.clone());
        if denied(&relative, write) {
            return Err(format!("[capability_denied] path `{path}` is always denied"));
        }
        let patterns = if write {
            &self.capabilities.fs_write
        } else {
            &self.capabilities.fs_read
        };
        if !patterns.iter().any(|pattern| glob_match(pattern, &relative)) {
            let capability = if write { "fs.write" } else { "fs.read" };
            return Err(format!(
                "[capability_denied] {capability}: \"{path}\" outside the approved globs"
            ));
        }
        Ok(target)
    }

    pub fn fs_list(&self, pattern: &str) -> Result<Vec<String>, String> {
        if !self.capabilities.fs_read.iter().any(|approved| approved == pattern)
            && !self
                .capabilities
                .fs_read
                .iter()
                .any(|approved| approved == "**/*")
        {
            return Err(format!(
                "[capability_denied] fs.read: glob \"{pattern}\" was not approved"
            ));
        }
        let root = self.project_root()?;
        let mut files = Vec::new();
        walk(&root, &root, 0, &mut files);
        files.sort();
        files.retain(|path| glob_match(pattern, path) && !denied(path, false));
        if files.len() > 5000 {
            files.truncate(5000);
        }
        Ok(files)
    }

    pub fn fs_read(&self, path: &str) -> Result<String, String> {
        let target = self.fs_path(path, false)?;
        let meta = std::fs::metadata(&target).map_err(|error| format!("[host] {error}"))?;
        if meta.len() > 1_048_576 {
            return Err("[argument] file is larger than 1 MB".to_owned());
        }
        std::fs::read_to_string(&target).map_err(|error| format!("[host] {error}"))
    }

    pub fn fs_write(&self, path: &str, text: &str) -> Result<(), String> {
        if text.len() > 1_048_576 {
            return Err("[argument] text is larger than 1 MB".to_owned());
        }
        let target = self.fs_path(path, true)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("[host] {error}"))?;
        }
        forja_kernel::store::write_atomic_bytes(&target, text.as_bytes())
            .map_err(|error| format!("[host] {error}"))
    }

    pub fn git(&self, args: &[&str]) -> Result<String, String> {
        if !self.capabilities.git_read {
            return Err("[capability_denied] git.read".to_owned());
        }
        let root = self.project_root()?;
        let output = forja_kernel::process::command("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|error| format!("[host] {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "[host] git {}: {}",
                args.first().copied().unwrap_or(""),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn git_status(&self) -> Result<Vec<(String, String)>, String> {
        let text = self.git(&["status", "--porcelain=v1", "-z"])?;
        let mut rows = Vec::new();
        let parts = text.split('\0');
        for item in parts {
            if item.len() < 3 {
                continue;
            }
            let code = &item[..2];
            let path = item[3..].to_owned();
            let state = if code.contains('U') || code.contains('A') && code.contains('A') {
                "conflict"
            } else if code.contains('?') {
                "untracked"
            } else if code.contains('D') {
                "deleted"
            } else if code.contains('A') {
                "new"
            } else {
                "modified"
            };
            rows.push((path, state.to_owned()));
        }
        Ok(rows)
    }

    pub fn git_branch(&self) -> Result<String, String> {
        Ok(self
            .git(&["rev-parse", "--abbrev-ref", "HEAD"])?
            .trim()
            .to_owned())
    }

    pub fn git_remote_url(&self, name: &str) -> Result<Option<String>, String> {
        match self.git(&["remote", "get-url", name]) {
            Ok(text) => Ok(Some(text.trim().to_owned())),
            Err(_) => Ok(None),
        }
    }

    pub fn git_log(&self, limit: usize, since: Option<&str>) -> Result<Vec<Value>, String> {
        let limit = limit.to_string();
        let mut args = vec!["log", "--format=%H%x1f%an%x1f%aI%x1f%s", "-n", &limit];
        let since_arg;
        if let Some(since) = since {
            since_arg = format!("--since={since}");
            args.push(&since_arg);
        }
        let text = self.git(&args)?;
        let rows = text
            .lines()
            .filter_map(|line| {
                let mut parts = line.split('\u{1f}');
                Some(serde_json::json!({
                    "hash": parts.next()?,
                    "author": parts.next()?,
                    "date": parts.next()?,
                    "subject": parts.next()?,
                }))
            })
            .collect();
        Ok(rows)
    }

    pub fn git_diff_stat(&self) -> Result<Value, String> {
        let text = self.git(&["diff", "--numstat"])?;
        let mut files = 0;
        let mut insertions = 0;
        let mut deletions = 0;
        for line in text.lines() {
            let mut parts = line.split('\t');
            let added = parts.next().unwrap_or("0").parse::<i64>().unwrap_or(0);
            let removed = parts.next().unwrap_or("0").parse::<i64>().unwrap_or(0);
            files += 1;
            insertions += added;
            deletions += removed;
        }
        Ok(serde_json::json!({
            "files": files,
            "insertions": insertions,
            "deletions": deletions,
        }))
    }

    pub fn http(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        headers: &[(String, String)],
        timeout_ms: u64,
    ) -> Result<Value, String> {
        if !self.capabilities.http.is_empty() && !self.network_allowed {
            return Err(
                "[capability_denied] extension.network.allow is off".to_owned(),
            );
        }
        let rest = url
            .strip_prefix("https://")
            .ok_or_else(|| "[argument] forja.http accepts https only".to_owned())?;
        let host = rest
            .split(['/', '?', '#'])
            .next()
            .and_then(|authority| authority.rsplit('@').next())
            .and_then(|authority| authority.split(':').next())
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "[argument] URL has no host".to_owned())?;
        if !self.capabilities.http.iter().any(|allowed| allowed == host) {
            return Err(format!(
                "[capability_denied] http: host \"{host}\" was not approved"
            ));
        }
        for (name, _) in headers {
            let lower = name.to_ascii_lowercase();
            if lower == "authorization" || lower == "cookie" {
                return Err(format!("[argument] header `{name}` is not allowed"));
            }
        }
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_millis(timeout_ms.clamp(100, 10_000))))
            .max_redirects(0)
            .build()
            .new_agent();
        let request = if method.eq_ignore_ascii_case("post") {
            let mut request = agent.post(url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            request.send(body.unwrap_or(""))
        } else {
            let mut request = agent.get(url);
            for (name, value) in headers {
                request = request.header(name, value);
            }
            request.call()
        };
        let response = request.map_err(|error| format!("[host] {error}"))?;
        let status = response.status().as_u16();
        let mut response_headers = serde_json::Map::new();
        for (name, value) in response.headers() {
            response_headers.insert(
                name.as_str().to_owned(),
                Value::String(value.to_str().unwrap_or_default().to_owned()),
            );
        }
        let mut body = response.into_body();
        let text = body
            .read_to_string()
            .map_err(|error| format!("[host] {error}"))?;
        if text.len() > 2 * 1024 * 1024 {
            return Err("[host] response is larger than 2 MB".to_owned());
        }
        Ok(serde_json::json!({
            "status": status,
            "headers": response_headers,
            "body": text,
        }))
    }
}

fn walk(root: &Path, dir: &Path, depth: usize, files: &mut Vec<String>) {
    if depth > 16 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, depth + 1, files);
        } else if let Ok(relative) = path.strip_prefix(root) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn denied(path: &str, write: bool) -> bool {
    let lower = path.to_ascii_lowercase();
    let file = lower.rsplit('/').next().unwrap_or(&lower);
    if lower.starts_with(".git/") || lower.contains("/.git/") {
        return true;
    }
    if file == ".env" || file.starts_with(".env.") {
        return true;
    }
    if file.ends_with(".pem") || file.ends_with(".key") {
        return true;
    }
    if write && (lower.starts_with(".forja/") || lower.contains("/.forja/")) {
        return true;
    }
    false
}

/// Correspondência de glob com `**`, `*` e `?`.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn inner(pattern: &[u8], text: &[u8]) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }
        match pattern[0] {
            b'*' => {
                if pattern.get(1) == Some(&b'*') {
                    let rest = pattern.get(2..).unwrap_or_default();
                    let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                    return (0..=text.len()).any(|index| inner(rest, &text[index..]));
                }
                let mut index = 0;
                loop {
                    if inner(&pattern[1..], &text[index..]) {
                        return true;
                    }
                    if index >= text.len() || text[index] == b'/' {
                        return false;
                    }
                    index += 1;
                }
            }
            b'?' => !text.is_empty() && inner(&pattern[1..], &text[1..]),
            byte => !text.is_empty() && text[0] == byte && inner(&pattern[1..], &text[1..]),
        }
    }
    inner(pattern.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(capabilities: Capabilities) -> HostIo {
        HostIo {
            dir: PathBuf::from("C:/ext"),
            project: Some(PathBuf::from("C:/project")),
            capabilities,
            network_allowed: false,
            fs_outside_allowed: false,
            store_path: PathBuf::from("C:/data/store.json"),
            settings_path: PathBuf::from("C:/data/settings.json"),
        }
    }

    #[test]
    fn fs_read_is_limited_to_approved_globs() {
        let io = io(Capabilities {
            fs_read: vec!["**/*.rs".to_owned()],
            ..Capabilities::default()
        });
        assert!(io.fs_path("src/main.rs", false).is_ok());
        assert!(io.fs_path("src/main.ts", false).is_err());
        assert!(io.fs_path("../secret.rs", false).is_err());
        assert!(io.fs_path(".git/config", false).is_err());
        assert!(io.fs_path(".env", false).is_err());
    }

    #[test]
    fn fs_write_always_denies_the_forja_folder() {
        let io = io(Capabilities {
            fs_write: vec!["**/*.md".to_owned()],
            ..Capabilities::default()
        });
        assert!(io.fs_path("notes.md", true).is_ok());
        assert!(io.fs_path(".forja/notes.md", true).is_err());
    }

    #[test]
    fn http_rejects_unknown_hosts_and_secret_headers() {
        let mut config = io(Capabilities {
            http: vec!["api.example.com".to_owned()],
            ..Capabilities::default()
        });
        config.network_allowed = true;
        let error = config
            .http("get", "https://other.example.com/x", None, &[], 1000)
            .unwrap_err();
        assert!(error.contains("capability_denied"));
        let error = config
            .http(
                "get",
                "https://api.example.com/x",
                None,
                &[("Authorization".to_owned(), "token".to_owned())],
                1000,
            )
            .unwrap_err();
        assert!(error.contains("not allowed"));
    }

    #[test]
    fn glob_matches_recursive_paths() {
        assert!(glob_match("**/*.rs", "src/main.rs"));
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(!glob_match("src/*.rs", "src/deep/main.rs"));
    }
}
