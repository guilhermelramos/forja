//! Builds the `GitStatusChanged` event from the status report that the git
//! module already computed. Keeping this here avoids re-reading git anywhere
//! else: the Board and the Files tree only consume the published event.

use crate::engine::StatusReport;
use forja_contracts::GitStatusChanged;
use forja_kernel::Ctx;

/// Changed paths, relative to the bench root, normalised to forward slashes.
pub(crate) fn changed_files(report: &StatusReport) -> Vec<String> {
    report
        .files
        .iter()
        .map(|file| file.path.replace('\\', "/"))
        .collect()
}

/// Publishes one status event for the current bench.
pub(crate) fn emit(cx: &mut Ctx<'_>, bench_path: &str, report: &StatusReport) {
    cx.emit(&GitStatusChanged {
        bench_path: bench_path.to_owned(),
        summary: report.summary.clone(),
        files: changed_files(report),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{GitFile, StatusReport};

    #[test]
    fn changed_paths_are_relative_and_normalised() {
        let report = StatusReport {
            files: vec![
                GitFile {
                    path: "src/login.ts".to_owned(),
                    index: '.',
                    worktree: 'M',
                    staged: false,
                    unstaged: true,
                    untracked: false,
                    conflict: false,
                },
                GitFile {
                    path: "docs\\guia.md".to_owned(),
                    index: '?',
                    worktree: '?',
                    staged: false,
                    unstaged: false,
                    untracked: true,
                    conflict: false,
                },
            ],
            ..StatusReport::default()
        };
        assert_eq!(
            changed_files(&report),
            vec!["src/login.ts".to_owned(), "docs/guia.md".to_owned()]
        );
    }
}
