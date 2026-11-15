//! Runs netstat/tasklist, inspects process CWD & ancestry, and guesses the bench.

use crate::cwd::{self, ProcessInspection};
use crate::model::{bench_for, BenchRoot, Snapshot};
use crate::parser;
use std::collections::{HashMap, HashSet};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_ANCESTOR_DEPTH: usize = 16;

/// Context passed into a port scan so CWD and process-tree ancestry map to Forja benches.
#[derive(Clone, Debug)]
pub struct ScanContext {
    pub benches: Vec<BenchRoot>,
    pub fallback_bench: Option<BenchRoot>,
    pub own_pid: u32,
}

impl Default for ScanContext {
    fn default() -> Self {
        Self {
            benches: Vec::new(),
            fallback_bench: None,
            own_pid: std::process::id(),
        }
    }
}

pub fn scan_with_context(ctx: &ScanContext) -> Snapshot {
    let netstat = run("netstat", &["-ano"]);
    let tasklist = run("tasklist", &["/FO", "CSV", "/NH"]);
    match netstat {
        Ok(text) => {
            let tasklist = tasklist.unwrap_or_default();
            scan_text_with_inspector(&text, &tasklist, ctx, cwd::inspect_process)
        }
        Err(error) => Snapshot {
            error: Some(error),
            scanned_at_ms: now_ms(),
            ..Snapshot::default()
        },
    }
}

#[cfg(test)]
pub fn scan_text(netstat: &str, tasklist: &str, benches: &[BenchRoot]) -> Snapshot {
    let ctx = ScanContext {
        benches: benches.to_vec(),
        ..ScanContext::default()
    };
    scan_text_with_inspector(netstat, tasklist, &ctx, cwd::inspect_process)
}

pub fn scan_text_with_inspector<F>(
    netstat: &str,
    tasklist: &str,
    ctx: &ScanContext,
    mut inspector: F,
) -> Snapshot
where
    F: FnMut(u32) -> ProcessInspection,
{
    let canonical_benches: Vec<BenchRoot> = ctx
        .benches
        .iter()
        .map(|bench| BenchRoot {
            id: bench.id.clone(),
            name: bench.name.clone(),
            path: normalize_dir_for_match(&bench.path),
        })
        .collect();

    let mut ports = parser::parse_ports(netstat, tasklist);
    let mut cache: HashMap<u32, ProcessInspection> = HashMap::new();

    let mut inspect_cached = |pid: u32| -> ProcessInspection {
        if let Some(cached) = cache.get(&pid) {
            return cached.clone();
        }
        let result = inspector(pid);
        cache.insert(pid, result.clone());
        result
    };

    for port in &mut ports {
        if port.pid == 0 || port.pid == 4 || port.pid == ctx.own_pid {
            continue;
        }
        if parser::is_browser_helper_name(&port.process) {
            continue;
        }

        let mut current = Some(port.pid);
        let mut visited = HashSet::new();
        for depth in 0..MAX_ANCESTOR_DEPTH {
            let Some(pid) = current else {
                break;
            };
            if pid == 0 || pid == 4 || !visited.insert(pid) {
                break;
            }
            if pid == ctx.own_pid {
                if depth > 0 {
                    port.from_tree = true;
                }
                break;
            }

            let info = inspect_cached(pid);
            if let Some(ref dir) = info.cwd {
                let dir_str = normalize_dir_for_match(&dir.display().to_string());
                if port.cwd.is_empty() && depth == 0 {
                    port.cwd = dir_str.clone();
                }
                if port.bench_id.is_empty() {
                    if let Some(bench) = bench_for(&dir_str, &canonical_benches) {
                        port.bench_id = bench.id.clone();
                        port.bench_name = bench.name.clone();
                        if port.cwd.is_empty() {
                            port.cwd = dir_str;
                        }
                    }
                }
            }
            current = info.parent_pid;
        }

        if port.from_tree && port.bench_id.is_empty() {
            if let Some(ref fallback) = ctx.fallback_bench {
                port.bench_id = fallback.id.clone();
                port.bench_name = fallback.name.clone();
            }
        }
        if port.is_forja_port() {
            port.is_system = false;
        }
    }

    Snapshot {
        ports,
        scanned_at_ms: now_ms(),
        error: None,
    }
}

fn normalize_dir_for_match(raw: &str) -> String {
    let cleaned = forja_kernel::paths::clean_path_str(raw.trim());
    if cleaned.is_empty() {
        return cleaned;
    }
    std::fs::canonicalize(&cleaned)
        .ok()
        .map(|path| forja_kernel::paths::clean_path_str(&path.display().to_string()))
        .unwrap_or(cleaned)
}

/// Checks whether a live PID belongs to the OS or a Windows service.
pub fn is_system_pid(pid: u32) -> bool {
    if pid == 0 || pid == 4 {
        return true;
    }
    let filter = format!("PID eq {pid}");
    let Ok(output) = run("tasklist", &["/FI", &filter, "/FO", "CSV", "/NH"]) else {
        return false;
    };
    let entries = parser::parse_tasklist_entries(&output);
    entries
        .get(&pid)
        .is_some_and(|entry| parser::is_system_task(pid, entry))
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// Sample data for screenshots and fixture tests; never used in normal runs.
pub fn sample() -> Snapshot {
    let mut ports: Vec<_> = [
        (
            135u16,
            1596u32,
            "svchost.exe",
            "0.0.0.0",
            "",
            "",
            false,
            true,
        ),
        (445, 4, "System", "0.0.0.0", "", "", false, true),
        (
            3000,
            8124,
            "node.exe",
            "0.0.0.0",
            "forja-web",
            "C:/work/forja/web",
            true,
            false,
        ),
        (
            5173,
            12120,
            "node.exe",
            "127.0.0.1",
            "forja-docs",
            "C:/work/forja/docs",
            true,
            false,
        ),
        (
            5432,
            6120,
            "postgres.exe",
            "0.0.0.0",
            "",
            "",
            false,
            false,
        ),
        (
            8080,
            4488,
            "forja-api.exe",
            "0.0.0.0",
            "forja-api",
            "C:/work/forja/api",
            true,
            false,
        ),
        (
            9229,
            8124,
            "node.exe",
            "127.0.0.1",
            "forja-web",
            "C:/work/forja/web",
            true,
            false,
        ),
    ]
    .into_iter()
    .map(
        |(port, pid, process, address, bench_name, cwd, from_tree, is_system)| {
            crate::model::PortInfo {
                port,
                pid,
                process: process.into(),
                address: address.into(),
                cwd: cwd.into(),
                bench_id: if bench_name.is_empty() {
                    String::new()
                } else {
                    bench_name.replace("forja-", "")
                },
                bench_name: bench_name.into(),
                from_tree,
                is_system,
            }
        },
    )
    .collect();
    ports.sort_by_key(|port| port.port);
    Snapshot {
        ports,
        scanned_at_ms: now_ms(),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scan_text_keeps_ports_without_readable_cwd() {
        let netstat = "  TCP    0.0.0.0:3000    0.0.0.0:0    LISTENING    4321\r\n";
        let tasklist = "\"node.exe\",\"4321\",\"Console\",\"1\",\"1 K\"\r\n";
        let snapshot = scan_text(netstat, tasklist, &[]);
        assert_eq!(snapshot.ports.len(), 1);
        assert_eq!(snapshot.ports[0].process, "node.exe");
        assert!(snapshot.error.is_none());
    }

    #[test]
    fn scan_maps_ancestor_cwd_and_forja_process_tree() {
        let netstat = concat!(
            "  TCP    127.0.0.1:3000    0.0.0.0:0    LISTENING    200\r\n",
            "  TCP    127.0.0.1:4000    0.0.0.0:0    LISTENING    300\r\n",
            "  TCP    127.0.0.1:135     0.0.0.0:0    LISTENING    900\r\n",
        );
        let tasklist = concat!(
            "\"vite.exe\",\"200\",\"Console\",\"1\",\"1 K\"\r\n",
            "\"python.exe\",\"300\",\"Console\",\"1\",\"1 K\"\r\n",
            "\"msedgewebview2.exe\",\"900\",\"Console\",\"1\",\"1 K\"\r\n",
        );
        let ctx = ScanContext {
            benches: vec![BenchRoot {
                id: "b1".into(),
                name: "forja-exemplo".into(),
                path: "C:/work/forja-exemplo".into(),
            }],
            fallback_bench: None,
            own_pid: 10,
        };
        let snapshot = scan_text_with_inspector(netstat, tasklist, &ctx, |pid| match pid {
            // Child changed cwd to temp, but parent shell 100 is inside bench and spawned by Forja.
            200 => ProcessInspection {
                cwd: Some(PathBuf::from("C:/Windows/Temp")),
                parent_pid: Some(100),
            },
            100 => ProcessInspection {
                cwd: Some(PathBuf::from("C:/work/forja-exemplo")),
                parent_pid: Some(10),
            },
            // Child has no readable CWD, spawned under Forja tree without bench match.
            300 => ProcessInspection {
                cwd: None,
                parent_pid: Some(10),
            },
            // Embedded browser helper under Forja tree must NOT become a bench port.
            900 => ProcessInspection {
                cwd: None,
                parent_pid: Some(10),
            },
            _ => ProcessInspection::default(),
        });

        let p3000 = snapshot.ports.iter().find(|p| p.port == 3000).unwrap();
        assert_eq!(p3000.bench_name, "forja-exemplo");
        assert!(p3000.from_tree);
        assert!(p3000.is_forja_port());

        let p4000 = snapshot.ports.iter().find(|p| p.port == 4000).unwrap();
        assert!(p4000.from_tree);
        assert!(p4000.bench_name.is_empty());
        assert!(p4000.is_forja_port());

        let p135 = snapshot.ports.iter().find(|p| p.port == 135).unwrap();
        assert!(!p135.from_tree);
        assert!(!p135.is_forja_port());
    }

    #[test]
    fn sample_has_sorted_ports() {
        let sample = sample();
        assert!(sample.ports.len() >= 4);
        assert!(sample
            .ports
            .windows(2)
            .all(|pair| pair[0].port <= pair[1].port));
    }
}
