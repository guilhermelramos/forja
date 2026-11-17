//! Sample benches for `forja-app --demo`.
//!
//! The shell prepares the worktrees and calls [`demo_seed`] before the runtime
//! starts. The bench file format stays here, not in the shell.

use crate::model::{BenchFile, BenchRecord, SCHEMA_VERSION};
use forja_kernel::Store;

/// One sample worktree the shell already created on disk.
#[derive(Clone, Debug)]
pub struct DemoBench {
    pub project_id: String,
    pub name: String,
    pub path: String,
    pub branch: String,
    pub stage: String,
    pub pinned: bool,
    pub sleeping: bool,
    /// Request key the bench was forged from, shown on the card.
    pub request: Option<String>,
}

/// Writes the main bench of every project plus one record per demo bench.
///
/// `current` is the bench that opens first. Only the first project root becomes
/// the visible starting point when `current` is `None`.
pub fn demo_seed(
    store: &Store,
    mains: &[(String, String, String)],
    benches: &[DemoBench],
    current: Option<&str>,
) -> Result<(), String> {
    let mut records: Vec<BenchRecord> = Vec::with_capacity(mains.len() + benches.len());
    for (index, (project_id, name, path)) in mains.iter().enumerate() {
        records.push(BenchRecord {
            bench_id: crate::model::main_bench_id(project_id),
            project_id: project_id.clone(),
            name: name.clone(),
            path: path.clone(),
            branch: None,
            is_main: true,
            stage: crate::stages::DRAFT.to_owned(),
            archived: false,
            pinned: false,
            unread: false,
            sleeping: false,
            created_at: index as u64,
            activated_at: 0,
            base_branch: None,
            parent_bench_id: None,
            created_head: None,
            request_command: None,
            open_tabs: Vec::new(),
            request_key: None,
        });
    }
    for (index, bench) in benches.iter().enumerate() {
        records.push(BenchRecord {
            bench_id: format!("b-demo-{index}"),
            project_id: bench.project_id.clone(),
            name: bench.name.clone(),
            path: bench.path.clone(),
            branch: Some(bench.branch.clone()),
            is_main: false,
            stage: bench.stage.clone(),
            archived: false,
            pinned: bench.pinned,
            unread: index == 0,
            sleeping: bench.sleeping,
            created_at: 1_700_000_000 + index as u64,
            activated_at: 0,
            base_branch: Some("main".to_owned()),
            parent_bench_id: None,
            created_head: None,
            request_command: None,
            open_tabs: Vec::new(),
            request_key: bench.request.clone(),
        });
    }
    let file = BenchFile {
        schema_version: SCHEMA_VERSION,
        benches: records,
        current: current.map(str::to_owned),
        approvals: Vec::new(),
    };
    store
        .save("bench", &file)
        .map_err(|error| format!("demo: could not write benches: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_mains_and_demo_benches() {
        let dir = std::env::temp_dir().join(format!("forja-bench-demo-{}", std::process::id()));
        let store = Store::new(&dir);
        demo_seed(
            &store,
            &[("p-one".to_owned(), "oficina".to_owned(), "C:/demo/oficina".to_owned())],
            &[DemoBench {
                project_id: "p-one".to_owned(),
                name: "relatorio".to_owned(),
                path: "C:/demo/relatorio".to_owned(),
                branch: "feature/relatorio".to_owned(),
                stage: crate::stages::REVIEW.to_owned(),
                pinned: true,
                sleeping: false,
                request: Some("LOCAL-1".to_owned()),
            }],
            Some("b-demo-0"),
        )
        .unwrap();
        let file: BenchFile = store.load("bench").into_option().expect("benches");
        assert_eq!(file.benches.len(), 2);
        assert!(file.benches.iter().any(|bench| bench.is_main));
        assert_eq!(file.current.as_deref(), Some("b-demo-0"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
