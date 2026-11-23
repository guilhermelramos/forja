//! Discovery, incremental cache and aggregation of local transcripts.

use crate::model::{
    day_key, UsagePeriod, UsageRecord, UsageSession, UsageSlice, UsageSummary, UsageTotals, DAY_MS,
};
use crate::parse;
use crate::pricing::Pricing;
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
    /// Resolves local transcript folders, isolating test and demo runs from the
    /// user's real `~/.claude`, `~/.codex` and OpenCode directories.
    pub fn for_run(accounts: &[(String, String)], data_dir: &Path) -> Self {
        let test_usage_dir = env_path("FORJA_TEST_USAGE_DIR");
        let isolated = cfg!(test)
            || env_path("FORJA_TEST_DRIVER").is_some()
            || env_path("FORJA_SCREENSHOT").is_some();
        Self::resolve(
            accounts,
            data_dir,
            env_path("FORJA_DEMO").is_some(),
            isolated,
            test_usage_dir,
        )
    }

    fn resolve(
        accounts: &[(String, String)],
        data_dir: &Path,
        demo: bool,
        isolated: bool,
        test_usage_dir: Option<PathBuf>,
    ) -> Self {
        // `--demo` reads only the sample folder the shell points at, so the
        // person's own sessions never show up in the demonstration.
        if demo {
            return Self {
                claude: env_path("CLAUDE_CONFIG_DIR").into_iter().collect(),
                codex: env_path("CODEX_HOME").into_iter().collect(),
                opencode: Vec::new(),
            };
        }
        if let Some(base) = test_usage_dir {
            return Self::isolated_under(&base);
        }
        if isolated {
            return Self::isolated_under(&data_dir.join("usage"));
        }
        Self::from_accounts(accounts)
    }

    fn isolated_under(base: &Path) -> Self {
        Self {
            claude: vec![base.to_path_buf(), base.join("claude")],
            codex: vec![base.to_path_buf(), base.join("codex")],
            opencode: vec![base.to_path_buf(), base.join("opencode")],
        }
    }

    pub fn from_accounts(accounts: &[(String, String)]) -> Self {
        let home = user_home();
        if env_path("FORJA_DEMO").is_some() {
            return Self {
                claude: env_path("CLAUDE_CONFIG_DIR").into_iter().collect(),
                codex: env_path("CODEX_HOME").into_iter().collect(),
                opencode: Vec::new(),
            };
        }
        let mut claude = BTreeSet::new();
        claude.insert(
            env_path("CLAUDE_CONFIG_DIR").unwrap_or_else(|| home.join(".claude")),
        );
        let mut codex = BTreeSet::new();
        codex.insert(env_path("CODEX_HOME").unwrap_or_else(|| home.join(".codex")));
        for (profile, dir) in accounts {
            let path = PathBuf::from(dir);
            if profile.contains("claude") {
                claude.insert(path);
            } else if profile.contains("codex") {
                codex.insert(path);
            }
        }
        let mut opencode = BTreeSet::new();
        if let Some(appdata) = env_path("APPDATA") {
            opencode.insert(appdata.join("opencode"));
        }
        if let Some(local) = env_path("LOCALAPPDATA") {
            opencode.insert(local.join("opencode"));
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

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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

    pub fn summary_at(&self, now_ms: i64, pricing: &Pricing) -> UsageSummary {
        summarize(
            self.files.values().flat_map(|file| file.records.iter()),
            now_ms,
            pricing,
        )
    }
}

pub fn summarize<'a>(
    records: impl Iterator<Item = &'a UsageRecord>,
    now_ms: i64,
    pricing: &Pricing,
) -> UsageSummary {
    let mut summary = UsageSummary::default();
    let today = day_key(now_ms);
    let week_start = day_key(now_ms - 6 * DAY_MS);
    let month_start = day_key(now_ms - 29 * DAY_MS);
    let mut slices: BTreeMap<String, UsageSlice> = UsagePeriod::ALL
        .iter()
        .map(|period| (period.id().to_owned(), UsageSlice::default()))
        .collect();
    let mut grouped: BTreeMap<(String, String), UsageSession> = BTreeMap::new();
    for record in records {
        let mut totals = record.totals;
        match record.cost_usd {
            Some(cost) if cost.is_finite() && cost >= 0.0 => totals.cost_usd = cost,
            Some(_) => totals.unpriced_tokens = totals.total_tokens,
            None => match pricing.cost_for(&record.model, &record.totals) {
                Some(cost) => totals.cost_usd = cost,
                None => totals.unpriced_tokens = totals.total_tokens,
            },
        }
        let model = if record.model.is_empty() {
            "unknown".to_owned()
        } else {
            record.model.clone()
        };
        summary.total.add(&totals);
        summary
            .by_agent
            .entry(record.agent.clone())
            .or_default()
            .add(&totals);
        summary
            .by_model
            .entry(model.clone())
            .or_default()
            .add(&totals);
        summary
            .by_project
            .entry(record.project.clone())
            .or_default()
            .add(&totals);
        let day = (record.timestamp_ms > 0).then(|| day_key(record.timestamp_ms));
        if let Some(day) = &day {
            summary.by_day.entry(day.clone()).or_default().add(&totals);
        }
        for period in UsagePeriod::ALL {
            let included = match &day {
                Some(day) => period.includes(day, &today, &week_start, &month_start),
                None => period == UsagePeriod::All,
            };
            if included {
                if let Some(slice) = slices.get_mut(period.id()) {
                    add_to_slice(
                        slice,
                        day.as_deref(),
                        &record.agent,
                        &model,
                        &record.project,
                        &totals,
                    );
                }
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
        session.totals.add(&totals);
        if record.timestamp_ms > session.updated_ms {
            session.updated_ms = record.timestamp_ms;
        }
        if record.timestamp_ms > 0
            && (session.started_ms == 0 || record.timestamp_ms < session.started_ms)
        {
            session.started_ms = record.timestamp_ms;
        }
        if !record.project.is_empty() {
            session.project = record.project.clone();
        }
        if !record.model.is_empty() {
            session.model = model.clone();
        }
        if !record.title.is_empty() {
            session.title = record.title.clone();
        }
    }
    summary.today = slice_totals(&slices, UsagePeriod::Today);
    summary.last_7_days = slice_totals(&slices, UsagePeriod::Last7);
    summary.last_30_days = slice_totals(&slices, UsagePeriod::Last30);
    summary.by_agent = summary_period(&slices, UsagePeriod::All).by_agent.clone();
    summary.by_model = summary_period(&slices, UsagePeriod::All).by_model.clone();
    summary.by_project = summary_period(&slices, UsagePeriod::All).by_project.clone();
    summary.by_day = summary_period(&slices, UsagePeriod::All).by_day.clone();
    summary.periods = slices;
    summary.sessions = grouped.into_values().collect();
    summary
        .sessions
        .sort_by_key(|session| std::cmp::Reverse(session.updated_ms));
    summary
}

fn add_to_slice(
    slice: &mut UsageSlice,
    day: Option<&str>,
    agent: &str,
    model: &str,
    project: &str,
    totals: &UsageTotals,
) {
    slice.totals.add(totals);
    slice
        .by_agent
        .entry(agent.to_owned())
        .or_default()
        .add(totals);
    slice
        .by_model
        .entry(model.to_owned())
        .or_default()
        .add(totals);
    slice
        .by_project
        .entry(project.to_owned())
        .or_default()
        .add(totals);
    if let Some(day) = day {
        slice.by_day.entry(day.to_owned()).or_default().add(totals);
    }
}

fn slice_totals(slices: &BTreeMap<String, UsageSlice>, period: UsagePeriod) -> UsageTotals {
    summary_period(slices, period).totals
}

fn summary_period(slices: &BTreeMap<String, UsageSlice>, period: UsagePeriod) -> &UsageSlice {
    slices
        .get(period.id())
        .expect("every period is initialized")
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
    use crate::pricing::{PriceEntry, Pricing};

    fn record(agent: &str, session: &str, project: &str, day: i64, tokens: u64) -> UsageRecord {
        UsageRecord {
            agent: agent.into(),
            session_id: session.into(),
            project: project.into(),
            model: "model-a".into(),
            title: "Tarefa".into(),
            timestamp_ms: 1_762_000_000_000 + day * DAY_MS,
            totals: UsageTotals {
                input_tokens: tokens,
                output_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                total_tokens: tokens,
                cost_usd: 0.0,
                unpriced_tokens: 0,
            },
            cost_usd: None,
        }
    }

    fn pricing() -> Pricing {
        Pricing::for_tests(vec![PriceEntry {
            id: "model-a".into(),
            patterns: vec!["model-a".into()],
            input: Some(10.0),
            output: Some(0.0),
            cache_read: Some(0.0),
            cache_write: Some(0.0),
            source: "https://example.test/pricing".into(),
            checked: "2026-11-14".into(),
        }])
    }

    #[test]
    fn summary_groups_today_week_month_periods_and_sessions() {
        let now = 1_762_000_000_000 + 10 * DAY_MS;
        let records = [
            record("claude", "a", "C:/app", 10, 100),
            record("claude", "a", "C:/app", 9, 50),
            record("codex", "b", "C:/other", 2, 25),
            record("codex", "c", "C:/other", -20, 5),
        ];
        let summary = summarize(records.iter(), now, &pricing());
        assert_eq!(summary.today.total_tokens, 100);
        assert_eq!(summary.last_7_days.total_tokens, 150);
        assert_eq!(summary.last_30_days.total_tokens, 175);
        assert_eq!(summary.total.total_tokens, 180);
        assert_eq!(summary.by_agent["claude"].total_tokens, 150);
        assert_eq!(summary.by_project["C:/other"].total_tokens, 30);
        assert_eq!(summary.periods["today"].totals.total_tokens, 100);
        assert_eq!(
            summary.periods["last7"].by_agent["claude"].total_tokens,
            150
        );
        assert_eq!(summary.periods["all"].by_model["model-a"].total_tokens, 180);
        assert_eq!(summary.sessions.len(), 3);
        assert_eq!(summary.sessions[0].session_id, "a");
        assert_eq!(summary.sessions[0].totals.total_tokens, 150);
        assert!((summary.total.cost_usd - 0.0018).abs() < 1e-9);
    }

    #[test]
    fn reported_cost_wins_over_the_price_table() {
        let now = 1_762_000_000_000 + 10 * DAY_MS;
        let mut first = record("claude", "a", "C:/app", 10, 100);
        first.cost_usd = Some(0.42);
        let second = record("claude", "b", "C:/app", 10, 100);
        let summary = summarize([first, second].iter(), now, &pricing());
        assert!((summary.total.cost_usd - 0.421).abs() < 1e-9);
        assert_eq!(summary.total.unpriced_tokens, 0);
    }

    #[test]
    fn records_without_a_price_are_counted_as_unpriced() {
        let now = 1_762_000_000_000 + 10 * DAY_MS;
        let mut unknown = record("codex", "x", "C:/other", 10, 100);
        unknown.model = "mystery".into();
        let summary = summarize([unknown].iter(), now, &pricing());
        assert_eq!(summary.today.unpriced_tokens, 100);
        assert_eq!(summary.today.cost_usd, 0.0);
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
        let summary = cache.summary_at(1_762_000_000_000, &Pricing::default());
        assert_eq!(summary.total.total_tokens, 202);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn isolated_runs_never_scan_host_home_and_respect_test_usage_dir() {
        let data_dir = Path::new("C:/isolated-data");
        let accounts = vec![
            ("claude-personal".to_owned(), "C:/real-user/.claude".to_owned()),
            ("codex-work".to_owned(), "C:/real-user/.codex".to_owned()),
        ];

        let default_isolated = Roots::resolve(&accounts, data_dir, false, true, None);
        assert_eq!(
            default_isolated.claude,
            vec![data_dir.join("usage"), data_dir.join("usage").join("claude")]
        );
        assert_eq!(
            default_isolated.codex,
            vec![data_dir.join("usage"), data_dir.join("usage").join("codex")]
        );
        assert_eq!(
            default_isolated.opencode,
            vec![data_dir.join("usage"), data_dir.join("usage").join("opencode")]
        );

        let override_dir = PathBuf::from("C:/custom-usage");
        let overridden =
            Roots::resolve(&accounts, data_dir, false, true, Some(override_dir.clone()));
        assert_eq!(
            overridden.claude,
            vec![override_dir.clone(), override_dir.join("claude")]
        );
        assert_eq!(
            overridden.codex,
            vec![override_dir.clone(), override_dir.join("codex")]
        );
        assert_eq!(
            overridden.opencode,
            vec![override_dir.clone(), override_dir.join("opencode")]
        );
    }
}
