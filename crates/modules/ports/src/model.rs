//! Listening TCP ports and the bench a process probably belongs to.

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
}

impl PortInfo {
    pub fn local_url(&self) -> String {
        let host = match self.address.as_str() {
            "::1" => "[::1]".to_owned(),
            "::" | "0.0.0.0" => "localhost".to_owned(),
            other => other.to_owned(),
        };
        format!("http://{host}:{}", self.port)
    }
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
    let normalized = cwd.replace('\\', "/").to_ascii_lowercase();
    benches
        .iter()
        .filter(|bench| {
            let path = bench.path.replace('\\', "/").to_ascii_lowercase();
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
    }
}
