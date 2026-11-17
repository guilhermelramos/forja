//! Sample helper runs for `forja-app --demo`.
//!
//! Only stored state: no process is started and no helper is detected. The runs
//! live in their own file (`helper-demo.json`) so the module's real file is
//! never mixed with demo data.

use crate::status::Run;
use forja_contracts::HelperStatus;
use forja_kernel::Store;
use serde::{Deserialize, Serialize};

/// Store id of the demo file; it is never written outside `--demo`.
const MODULE: &str = "helper-demo";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DemoRun {
    terminal_id: String,
    profile_id: String,
    bench_path: String,
    started_at: String,
    status: HelperStatus,
    last_event: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct DemoFile {
    #[serde(default)]
    runs: Vec<DemoRun>,
}

/// Writes one helper working and one waiting for the coordinator.
///
/// `bench_paths` are the demo benches, so the page shows where each helper is.
pub fn demo_seed(store: &Store, bench_paths: &[String]) -> Result<(), String> {
    let now = chrono::Utc::now();
    let entry = |index: usize, minutes: i64, status: HelperStatus, event: &str| DemoRun {
        terminal_id: format!("demo-terminal-{index}"),
        profile_id: "demo-helper".to_owned(),
        bench_path: bench_paths
            .get(index)
            .or_else(|| bench_paths.first())
            .cloned()
            .unwrap_or_default(),
        started_at: (now - chrono::Duration::minutes(minutes)).to_rfc3339(),
        status,
        last_event: event.to_owned(),
    };
    let file = DemoFile {
        runs: vec![
            entry(1, 26, HelperStatus::Working, "tool"),
            entry(2, 12, HelperStatus::NeedsYou, "bell"),
        ],
    };
    store
        .save(MODULE, &file)
        .map_err(|error| format!("demo: could not write helper runs: {error}"))
}

/// Runs the shell seeded for this start, if any. Called once when the module
/// loads its state.
pub(crate) fn load(store: &Store) -> Vec<Run> {
    let file: DemoFile = match store.load(MODULE) {
        forja_kernel::LoadResult::Loaded(file) => file,
        _ => return Vec::new(),
    };
    file.runs
        .into_iter()
        .map(|run| Run {
            terminal_id: run.terminal_id,
            profile_id: run.profile_id,
            bench_path: run.bench_path,
            started_at: run.started_at,
            hook_token: String::new(),
            status: run.status,
            last_event: Some(run.last_event),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_one_working_and_one_waiting_run() {
        let dir = std::env::temp_dir().join(format!("forja-helper-demo-{}", std::process::id()));
        let store = Store::new(&dir);
        demo_seed(&store, &["C:/demo/relatorio".to_owned()]).unwrap();
        let runs = load(&store);
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|run| run.status == HelperStatus::Working));
        assert!(runs.iter().any(|run| run.status == HelperStatus::NeedsYou));
        let _ = std::fs::remove_dir_all(dir);
    }
}
