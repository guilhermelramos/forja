//! Discovery, incremental cache and aggregation of local transcripts.

use crate::model::{day_key, UsageRecord, UsageSession, UsageSummary, UsageTotals};
use crate::parse;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roots {
    pub claude: Vec<PathBuf>,
    pub codex: Vec<PathBuf>,
    pub opencode: Vec<PathBuf>,
}

impl Roots {
    pub fn from_accounts(accounts: &[(String, String)]) -> Self {
        let home = user_home();
        let mut claude = BTreeSet::new();
        claude.insert(
            std::env::var_os("CLAUDE_CONFIG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".claude")),
        );
        let mut codex = BTreeSet::new();
        codex.insert(
            std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".codex")),
        );
        for (profile, dir) in accounts {
            let path = PathBuf::from(dir);
            if profile.contains("claude") {
                claude.insert(path);
            } else if profile.contains("codex") {
                codex.insert(path);
            }
        }
        let mut opencode = BTreeSet::new();
        if let Some(appdata) = std::env::var_os("APPDATA") {
            opencode.insert(PathBuf::from(appdata).join("opencode"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            opencode.insert(PathBuf::from(local).join("opencode"));
        }
        opencode.insert(home.join(".local").join("share").join("opencode"));
        opencode.insert(home.join(".config").join("opencode"));
        Self {
            claude: claude.into_iter().collect(),
            codex: codex.into_iter().collect(),
            opencode: opencode.into_iter().filter(|path| path.exists()).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanStats {
    pub discovered_files: usize,
    pub parsed_files: usize,
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified_ns: u128,
}

#[derive(Clone, Debug, Default)]
struct CachedFile {
    fingerprint: Fingerprint,
    records: Vec<UsageRecord>,
}

#[derive(Clone, Debug, Default)]
pub struct Cache {
    files: BTreeMap<PathBuf, CachedFile>,
}

impl Cache {
    pub fn scan(&mut self, roots: &Roots) -> ScanStats {
        let mut stats = ScanStats::default();
        let mut present = BTreeSet::new();
        for (agent, path) in discover(roots) {
            present.insert(path.clone());
            stats.discovered_files += 1;
            let Some(fingerprint) = fingerprint(&path) else {
                continue;
            };
            if self
                .files
                .get(&path)
                .is_some_and(|cached| cached.fingerprint == fingerprint)
            {
                continue;
            }
            match parse_file(agent, &path) {
                Ok(records) => {
                    self.files.insert(
                        path,
                        CachedFile {
                            fingerprint,
                            records,
                        },
                    );
                    stats.parsed_files += 1;
                }
                Err(error) => stats.errors.push(error),
            }
        }
        self.files.retain(|path, _| present.contains(path));
        stats
    }

    pub fn summary_at(&self, now_ms: i64) -> UsageSummary {
        summarize(
            self.files.values().flat_map(|file| file.records.iter()),
            now_ms,
        )
    }
}

pub fn summarize<'a>(records: impl Iterator<Item = &'a UsageRecord>, now_ms: i64) -> UsageSummary {
    let mut summary = UsageSummary::default();
    let today = day_key(now_ms);
    let week_start = day_key(now_ms - 7 * 24 * 60 * 60 * 1000);
    let month_start = day_key(now_ms - 30 * 24 * 60 * 60 * 1000);
    let mut grouped: BTreeMap<(String, String), UsageSession> = BTreeMap::new();
    for record in records {
        summary.total.add(&record.totals);
        summary
            .by_agent
            .entry(record.agent.clone())
            .or_default()
            .add(&record.totals);
        let model = if record.model.is_empty() {
            "unknown".to_owned()
        } else {
            record.model.clone()
        };
        summary
            .by_model
            .entry(model.clone())
            .or_default()
            .add(&record.totals);
        summary
            .by_project
            .entry(record.project.clone())
            .or_default()
            .add(&record.totals);
        if record.timestamp_ms > 0 {
            let day = day_key(record.timestamp_ms);
            summary
                .by_day
                .entry(day.clone())
                .or_default()
                .add(&record.totals);
            if day == today {
                summary.today.add(&record.totals);
            }
            if day >= week_start {
                summary.last_7_days.add(&record.totals);
            }
            if day >= month_start {
                summary.last_30_days.add(&record.totals);
            }
        }
        let session = grouped
            .entry((record.agent.clone(), record.session_id.clone()))
            .or_insert_with(|| UsageSession {
                agent: record.agent.clone(),
                session_id: record.session_id.clone(),
                project: record.project.clone(),
                model: model.clone(),
                title: record.title.clone(),
                started_ms: record.timestamp_ms,
                updated_ms: record.timestamp_ms,
                totals: UsageTotals::default(),
            });
        session.totals.add(&record.totals);
        if record.timestamp_ms > session.updated_ms {
            session.updated_ms = record.timestamp_ms;
        }
        if !record.project.is_empty() {
            session.project = record.project.clone();
        }
        if !record.model.is_empty() {
            session.model = model;
        }
        if !record.title.is_empty() {
            session.title = record.title.clone();
        }
    }
    summary.sessions = grouped.into_values().collect();
    summary
        .sessions
        .sort_by_key(|session| std::cmp::Reverse(session.updated_ms));
    summary
}

pub fn discover(roots: &Roots) -> Vec<(&'static str, PathBuf)> {
    let mut files = Vec::new();
    for root in &roots.claude {
        for path in walk(&root.join("projects"), MAX_DEPTH) {
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
            {
                files.push(("claude", path));
            }
        }
    }
    for root in &roots.codex {
        for path in walk(&root.join("sessions"), MAX_DEPTH) {
            if path
                .extension()
                .is_some_and(|extension| extension == "jsonl")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rollout-"))
            {
                files.push(("codex", path));
            }
        }
    }
    for root in &roots.opencode {
        for path in walk(&root.join("storage"), MAX_DEPTH) {
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                files.push(("opencode", path));
            }
        }
    }
    files
}

fn walk(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if depth == 0 {
        return files;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            files.extend(walk(&path, depth - 1));
        } else if kind.is_file() {
            files.push(path);
        }
    }
    files
}

fn parse_file(agent: &str, path: &Path) -> Result<Vec<UsageRecord>, String> {
    if agent == "opencode" {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        return Ok(parse::parse_opencode(&text, path));
    }
    let lines = parse::read_lines(path)?;
    let session = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("session");
    let project = path
        .ancestors()
        .find_map(|parent| parent.file_name().and_then(|name| name.to_str()))
        .unwrap_or("");
    Ok(match agent {
        "claude" => parse::parse_claude(&lines, session, project),
        "codex" => parse::parse_codex(&lines, session, project),
        _ => Vec::new(),
    })
}

fn fingerprint(path: &Path) -> Option<Fingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Fingerprint {
        len: metadata.len(),
        modified_ns: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0),
    })
}

fn user_home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UsageTotals;

    fn record(agent: &str, session: &str, project: &str, day: i64, tokens: u64) -> UsageRecord {
        UsageRecord {
            agent: agent.into(),
            session_id: session.into(),
            project: project.into(),
            model: "model-a".into(),
            title: "Tarefa".into(),
            timestamp_ms: 1_762_000_000_000 + day * 86_400_000,
            totals: UsageTotals {
                input_tokens: tokens,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: tokens,
            },
        }
    }

    #[test]
    fn summary_groups_today_week_month_and_sessions() {
        let now = 1_762_000_000_000 + 10 * 86_400_000;
        let records = [
            record("claude", "a", "C:/app", 10, 100),
            record("claude", "a", "C:/app", 9, 50),
            record("codex", "b", "C:/other", 2, 25),
            record("codex", "c", "C:/other", -20, 5),
        ];
        let summary = summarize(records.iter(), now);
        assert_eq!(summary.today.total_tokens, 100);
        assert_eq!(summary.last_7_days.total_tokens, 150);
        assert_eq!(summary.last_30_days.total_tokens, 180);
        assert_eq!(summary.total.total_tokens, 180);
        assert_eq!(summary.by_agent["claude"].total_tokens, 150);
        assert_eq!(summary.by_project["C:/other"].total_tokens, 30);
        assert_eq!(summary.sessions.len(), 3);
        assert_eq!(summary.sessions[0].session_id, "a");
        assert_eq!(summary.sessions[0].totals.total_tokens, 150);
    }

    #[test]
    fn scan_reuses_unchanged_files_and_reparses_changed_ones() {
        let dir = std::env::temp_dir().join(format!(
            "forja-usage-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let project = dir.join("claude").join("projects").join("app");
        std::fs::create_dir_all(&project).unwrap();
        let file = project.join("s1.jsonl");
        std::fs::write(
            &file,
            "{\"type\":\"assistant\",\"sessionId\":\"s1\",\"timestamp\":\"2026-11-10T10:00:00Z\",\"message\":{\"model\":\"m\",\"usage\":{\"input_tokens\":10,\"output_tokens\":2}}}\n",
        )
        .unwrap();
        let roots = Roots {
            claude: vec![dir.join("claude")],
            ..Roots::default()
        };
        let mut cache = Cache::default();
        let first = cache.scan(&roots);
        assert_eq!(first.discovered_files, 1);
        assert_eq!(first.parsed_files, 1);
        let second = cache.scan(&roots);
        assert_eq!(second.parsed_files, 0);
        std::fs::write(
            &file,
            "{\"type\":\"assistant\",\"sessionId\":\"s1\",\"timestamp\":\"2026-11-10T10:00:00Z\",\"message\":{\"model\":\"m\",\"usage\":{\"input_tokens\":200,\"output_tokens\":2}}}\n",
        )
        .unwrap();
        let third = cache.scan(&roots);
        assert_eq!(third.parsed_files, 1);
        let summary = cache.summary_at(1_762_000_000_000);
        assert_eq!(summary.total.total_tokens, 202);
        let _ = std::fs::remove_dir_all(dir);
    }
}
