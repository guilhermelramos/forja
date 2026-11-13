//! Runs netstat/tasklist, reads process cwd and guesses the bench.

use crate::cwd;
use crate::model::{bench_for, BenchRoot, Snapshot};
use crate::parser;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn scan(benches: &[BenchRoot]) -> Snapshot {
    let netstat = run("netstat", &["-ano"]);
    let tasklist = run("tasklist", &["/FO", "CSV", "/NH"]);
    match netstat {
        Ok(text) => {
            let tasklist = tasklist.unwrap_or_default();
            scan_text(&text, &tasklist, benches)
        }
        Err(error) => Snapshot {
            error: Some(error),
            scanned_at_ms: now_ms(),
            ..Snapshot::default()
        },
    }
}

pub fn scan_text(netstat: &str, tasklist: &str, benches: &[BenchRoot]) -> Snapshot {
    let mut ports = parser::parse_ports(netstat, tasklist);
    let own = std::process::id();
    for port in &mut ports {
        if port.pid == own || port.pid == 0 || port.pid == 4 {
            continue;
        }
        if let Some(cwd) = cwd::process_cwd(port.pid) {
            port.cwd = cwd.display().to_string();
            if let Some(bench) = bench_for(&port.cwd, benches) {
                port.bench_id = bench.id.clone();
                port.bench_name = bench.name.clone();
            }
        }
    }
    Snapshot {
        ports,
        scanned_at_ms: now_ms(),
        error: None,
    }
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

/// Sample data for screenshots; never used in normal runs.
pub fn sample() -> Snapshot {
    let mut ports: Vec<_> = [
        (
            3000u16,
            8124u32,
            "node.exe",
            "0.0.0.0",
            "forja-web",
            "C:/work/forja/web",
        ),
        (
            5173,
            12120,
            "node.exe",
            "127.0.0.1",
            "forja-docs",
            "C:/work/forja/docs",
        ),
        (
            8080,
            4488,
            "forja-api.exe",
            "0.0.0.0",
            "forja-api",
            "C:/work/forja/api",
        ),
        (
            9229,
            8124,
            "node.exe",
            "127.0.0.1",
            "forja-web",
            "C:/work/forja/web",
        ),
        (5432, 6120, "postgres.exe", "0.0.0.0", "", ""),
    ]
    .into_iter()
    .map(
        |(port, pid, process, address, bench_name, cwd)| crate::model::PortInfo {
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
    fn sample_has_sorted_ports() {
        let sample = sample();
        assert!(sample.ports.len() >= 4);
        assert!(sample
            .ports
            .windows(2)
            .all(|pair| pair[0].port <= pair[1].port));
    }
}
