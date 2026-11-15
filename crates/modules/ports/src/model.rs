//! Listening TCP ports and the bench a process probably belongs to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortInfo {
    /// Local port.
    pub port: u16,
    /// Owning process id.
    pub pid: u32,
    /// Image name from `tasklist`; `unknown` when the process vanished.
    pub process: String,
    /// Local address without brackets.
    pub address: String,
    /// Process current directory, when Windows lets us read it.
    #[serde(default)]
    pub cwd: String,
    /// Bench whose path contains the cwd.
    #[serde(default)]
    pub bench_id: String,
    /// Bench display name.
    #[serde(default)]
    pub bench_name: String,
    /// Whether this process runs inside a Forja terminal/helper process tree.
    #[serde(default)]
    pub from_tree: bool,
    /// Whether this process is an OS or Windows service process.
    #[serde(default)]
    pub is_system: bool,
}

impl PortInfo {
    /// Local URL suitable for opening HTTP(S) services in a browser.
    pub fn local_url(&self) -> String {
        let scheme = match self.port {
            443 | 8443 => "https",
            _ => "http",
        };
        let host = match self.address.as_str() {
            "::1" => "[::1]".to_owned(),
            "::" | "0.0.0.0" => "localhost".to_owned(),
            other => other.to_owned(),
        };
        format!("{scheme}://{host}:{}", self.port)
    }

    /// True when the listener belongs to a Forja bench/project or spawned tree.
    pub fn is_forja_port(&self) -> bool {
        !self.bench_name.is_empty() || !self.bench_id.is_empty() || self.from_tree
    }

    /// Best-effort heuristic for whether this listening port serves HTTP(S).
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

    /// Effective bench label when associated with a bench.
    pub fn bench_label(&self) -> Option<&str> {
        if !self.bench_name.is_empty() {
            Some(self.bench_name.as_str())
        } else if !self.bench_id.is_empty() {
            Some(self.bench_id.as_str())
        } else {
            None
        }
    }

    /// True if all whitespace-separated tokens of `query` match this port.
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

        trimmed.split_whitespace().all(|token| {
            let needle = token.to_ascii_lowercase();
            port_str.contains(&needle)
                || colon_port.contains(&needle)
                || pid_str.contains(&needle)
                || process.contains(&needle)
                || bench_name.contains(&needle)
                || bench_id.contains(&needle)
                || address.contains(&needle)
                || url.contains(&needle)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PortGroupKey {
    /// Listening processes belonging to a named Forja bench or project.
    Bench(String),
    /// Processes spawned in Forja's process tree whose specific bench could not be inferred.
    Forja,
    /// Non-system processes outside Forja benches.
    Other,
    /// OS / Windows service processes.
    System,
}

#[derive(Clone, Debug)]
pub struct PortGroup<'a> {
    pub key: PortGroupKey,
    pub ports: Vec<&'a PortInfo>,
}

/// Filters and groups ports for UI display.
///
/// When `show_all` is false, only Forja bench/tree ports are returned.
/// Benches appear first (alphabetically), followed by `Forja`, `Other`, and `System`.
pub fn group_ports<'a>(ports: &'a [PortInfo], show_all: bool, filter: &str) -> Vec<PortGroup<'a>> {
    let mut groups: BTreeMap<PortGroupKey, Vec<&'a PortInfo>> = BTreeMap::new();
    for port in ports {
        if !show_all && !port.is_forja_port() {
            continue;
        }
        if !port.matches_filter(filter) {
            continue;
        }
        let key = if let Some(label) = port.bench_label() {
            PortGroupKey::Bench(label.to_owned())
        } else if port.from_tree {
            PortGroupKey::Forja
        } else if port.is_system {
            PortGroupKey::System
        } else {
            PortGroupKey::Other
        };
        groups.entry(key).or_default().push(port);
    }

    groups
        .into_iter()
        .map(|(key, mut items)| {
            items.sort_by_key(|port| (port.port, port.pid));
            PortGroup { key, ports: items }
        })
        .collect()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub ports: Vec<PortInfo>,
    pub scanned_at_ms: u64,
    pub error: Option<String>,
}

/// One bench path used to guess where a process is running.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchRoot {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Picks the longest bench path that contains `cwd`.
pub fn bench_for<'a>(cwd: &str, benches: &'a [BenchRoot]) -> Option<&'a BenchRoot> {
    if cwd.trim().is_empty() {
        return None;
    }
    let normalized = cwd.replace('\\', "/").trim_end_matches('/').to_ascii_lowercase();
    benches
        .iter()
        .filter(|bench| {
            let path = bench
                .path
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_ascii_lowercase();
            !path.is_empty() && (normalized == path || normalized.starts_with(&format!("{path}/")))
        })
        .max_by_key(|bench| bench.path.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench(id: &str, path: &str) -> BenchRoot {
        BenchRoot {
            id: id.into(),
            name: id.into(),
            path: path.into(),
        }
    }

    #[test]
    fn the_longest_matching_bench_wins() {
        let benches = [
            bench("repo", "C:/code/app"),
            bench("bench", "C:/code/app/.worktrees/login"),
        ];
        assert_eq!(
            bench_for("C:\\code\\app\\.worktrees\\login\\src", &benches)
                .map(|bench| bench.id.as_str()),
            Some("bench")
        );
        assert_eq!(
            bench_for("C:/code/app/other", &benches).map(|bench| bench.id.as_str()),
            Some("repo")
        );
        assert!(bench_for("C:/elsewhere", &benches).is_none());
        assert!(bench_for("", &benches).is_none());
    }

    #[test]
    fn local_url_uses_localhost_for_unspecified_addresses() {
        let mut port = PortInfo {
            port: 3000,
            address: "0.0.0.0".into(),
            ..PortInfo::default()
        };
        assert_eq!(port.local_url(), "http://localhost:3000");
        port.address = "::1".into();
        assert_eq!(port.local_url(), "http://[::1]:3000");
        port.address = "127.0.0.1".into();
        assert_eq!(port.local_url(), "http://127.0.0.1:3000");
        port.port = 8443;
        assert_eq!(port.local_url(), "https://127.0.0.1:8443");
    }

    #[test]
    fn http_detection_excludes_databases_and_windows_rpc() {
        let http = PortInfo {
            port: 3000,
            process: "node.exe".into(),
            ..PortInfo::default()
        };
        assert!(http.is_http());

        let db = PortInfo {
            port: 5432,
            process: "postgres.exe".into(),
            ..PortInfo::default()
        };
        assert!(!db.is_http());

        let rpc = PortInfo {
            port: 135,
            process: "svchost.exe".into(),
            is_system: true,
            ..PortInfo::default()
        };
        assert!(!rpc.is_http());

        let inspect = PortInfo {
            port: 9229,
            process: "node.exe".into(),
            ..PortInfo::default()
        };
        assert!(!inspect.is_http());
    }

    #[test]
    fn group_ports_hides_system_and_other_by_default_and_groups_by_bench() {
        let ports = vec![
            PortInfo {
                port: 135,
                pid: 1596,
                process: "svchost.exe".into(),
                address: "0.0.0.0".into(),
                is_system: true,
                ..PortInfo::default()
            },
            PortInfo {
                port: 8080,
                pid: 2001,
                process: "node.exe".into(),
                address: "127.0.0.1".into(),
                bench_id: "b2".into(),
                bench_name: "zeta".into(),
                ..PortInfo::default()
            },
            PortInfo {
                port: 3000,
                pid: 2002,
                process: "vite.exe".into(),
                address: "127.0.0.1".into(),
                bench_id: "b1".into(),
                bench_name: "alfa".into(),
                ..PortInfo::default()
            },
            PortInfo {
                port: 4000,
                pid: 2003,
                process: "python.exe".into(),
                address: "127.0.0.1".into(),
                from_tree: true,
                ..PortInfo::default()
            },
            PortInfo {
                port: 6768,
                pid: 3000,
                process: "custom-app.exe".into(),
                address: "127.0.0.1".into(),
                ..PortInfo::default()
            },
        ];

        let default_groups = group_ports(&ports, false, "");
        assert_eq!(default_groups.len(), 3);
        assert_eq!(default_groups[0].key, PortGroupKey::Bench("alfa".into()));
        assert_eq!(default_groups[1].key, PortGroupKey::Bench("zeta".into()));
        assert_eq!(default_groups[2].key, PortGroupKey::Forja);

        let all_groups = group_ports(&ports, true, "");
        assert_eq!(all_groups.len(), 5);
        assert_eq!(all_groups[3].key, PortGroupKey::Other);
        assert_eq!(all_groups[4].key, PortGroupKey::System);

        let filtered = group_ports(&ports, true, "zeta 8080");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].ports[0].port, 8080);
    }
}
