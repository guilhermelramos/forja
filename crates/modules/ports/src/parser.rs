//! `netstat -ano` and `tasklist /FO CSV /NH` parsing.
//!
//! A listening row has an unspecified peer (`0.0.0.0:0`, `[::]:0` or `0.0.0.0:0`),
//! so the localized state label ("LISTENING", "OUVINDO", ...) is never read.

use crate::model::PortInfo;
use std::collections::BTreeMap;

/// Builds the final list: tasklist names, port+pid deduplication, port order.
pub fn parse_ports(netstat: &str, tasklist: &str) -> Vec<PortInfo> {
    let names = parse_tasklist_csv(tasklist);
    let mut ports = parse_netstat_listeners(netstat);
    ports.sort_by(|a, b| {
        (a.port, a.pid, a.address.as_str()).cmp(&(b.port, b.pid, b.address.as_str()))
    });
    ports.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);
    for port in &mut ports {
        port.process = names
            .get(&port.pid)
            .cloned()
            .unwrap_or_else(|| "unknown".to_owned());
    }
    ports
}

pub fn parse_netstat_listeners(text: &str) -> Vec<PortInfo> {
    let mut ports = Vec::new();
    for raw in text.lines() {
        let fields: Vec<&str> = raw.split_whitespace().collect();
        if fields.len() < 5 || !fields[0].eq_ignore_ascii_case("TCP") {
            continue;
        }
        if !matches!(split_host_port(fields[2]), Some(("0.0.0.0" | "::", 0))) {
            continue;
        }
        let Some((address, port)) = split_host_port(fields[1]) else {
            continue;
        };
        let Some(pid) = fields.last().and_then(|pid| pid.parse::<u32>().ok()) else {
            continue;
        };
        ports.push(PortInfo {
            port,
            pid,
            process: String::new(),
            address: address.to_owned(),
            ..PortInfo::default()
        });
    }
    ports
}

pub fn parse_tasklist_csv(text: &str) -> BTreeMap<u32, String> {
    let mut names = BTreeMap::new();
    for raw in text.lines() {
        let fields = csv_fields(raw);
        if fields.len() < 2 {
            continue;
        }
        let name = fields[0].trim();
        let Ok(pid) = fields[1].trim().parse::<u32>() else {
            continue;
        };
        if !name.is_empty() {
            names.insert(pid, name.to_owned());
        }
    }
    names
}

fn split_host_port(value: &str) -> Option<(&str, u16)> {
    let (host, port) = value.rsplit_once(':')?;
    let port = port.parse::<u16>().ok()?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Some((host, port))
}

fn csv_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENGLISH: &str = "Active Connections\r\n\r\n  Proto  Local Address          Foreign Address        State           PID\r\n  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1234\r\n  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       4321\r\n  TCP    127.0.0.1:5000         0.0.0.0:0              LISTENING       4321\r\n  TCP    127.0.0.1:5000         127.0.0.1:62000        ESTABLISHED     4321\r\n  TCP    [::]:135               [::]:0                 LISTENING       1234\r\n  TCP    [::1]:5000             [::]:0                 LISTENING       4321\r\n  UDP    0.0.0.0:1900           *:*                                    9999\r\n";

    const PORTUGUESE: &str = "Conexões ativas\r\n\r\n  Proto  Endereço local         Endereço externo       Estado          PID\r\n  TCP    0.0.0.0:135            0.0.0.0:0              OUVINDO         1234\r\n  TCP    0.0.0.0:3000           0.0.0.0:0              OUVINDO         4321\r\n  TCP    [::]:135               [::]:0                 OUVINDO         1234\r\n  TCP    192.168.1.10:139       0.0.0.0:0              OUVINDO         777\r\n";

    const TASKLIST: &str = "\"svc.exe\",\"1234\",\"Services\",\"0\",\"1.234 K\"\r\n\"node.exe\",\"4321\",\"Console\",\"1\",\"90.000 K\"\r\n\"name, with comma.exe\",\"777\",\"Console\",\"1\",\"1 K\"\r\n";

    #[test]
    fn english_and_portuguese_listeners_are_detected_by_the_peer() {
        let english = parse_netstat_listeners(ENGLISH);
        assert_eq!(english.len(), 5);
        assert!(english
            .iter()
            .any(|port| port.port == 5000 && port.pid == 4321 && port.address == "127.0.0.1"));
        assert!(english
            .iter()
            .any(|port| port.port == 135 && port.address == "::"));

        let portuguese = parse_netstat_listeners(PORTUGUESE);
        assert_eq!(portuguese.len(), 4);
        assert!(portuguese
            .iter()
            .any(|port| port.port == 139 && port.pid == 777));
    }

    #[test]
    fn tasklist_csv_handles_quotes_and_commas() {
        let names = parse_tasklist_csv(TASKLIST);
        assert_eq!(names.get(&1234).map(String::as_str), Some("svc.exe"));
        assert_eq!(names.get(&4321).map(String::as_str), Some("node.exe"));
        assert_eq!(
            names.get(&777).map(String::as_str),
            Some("name, with comma.exe")
        );
    }

    #[test]
    fn merge_sorts_and_names_processes() {
        let ports = parse_ports(ENGLISH, TASKLIST);
        assert_eq!(ports.len(), 3);
        assert_eq!(ports[0].port, 135);
        assert_eq!(ports[0].process, "svc.exe");
        assert!(ports.windows(2).all(|pair| pair[0].port <= pair[1].port));
    }
}
