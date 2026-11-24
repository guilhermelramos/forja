//! Shared cross-module contracts: event payloads and command argument shapes.
//!
//! Modules never depend on each other's crates. When two modules must talk, the event
//! type or the command's JSON shape lives here, and both depend on `forja-contracts`.
//! Rules: additive changes only (new types or new optional fields); every type is
//! `Clone + Serialize + Deserialize`; ids are strings; paths are absolute.
//! Command ids and their JSON arguments are documented in `design/contracts.md`.
use serde::{Deserialize, Serialize};

pub mod requests;
pub mod viewers;
pub use requests::*;
pub use viewers::*;

/// Event names, used with `Registrar::event::<T>(NAME)`.
pub mod names {
    pub const PROJECT_ADDED: &str = "project.added";
    pub const PROJECT_REMOVED: &str = "project.removed";
    pub const PROJECT_ACTIVATED: &str = "project.activated";
    pub const BENCH_ACTIVATED: &str = "bench.activated";
    pub const BENCH_CREATED: &str = "bench.created";
    pub const BENCH_STORED: &str = "bench.stored";
    pub const BENCH_SLEEP_CHANGED: &str = "bench.sleep_changed";
    pub const STAGE_CHANGED: &str = "stage.changed";
    pub const TERMINAL_EXITED: &str = "terminal.exited";
    pub const TERMINAL_BELL: &str = "terminal.bell";
    pub const TERMINAL_COMMAND_FINISHED: &str = "terminal.command_finished";
    pub const TERMINAL_TITLE_CHANGED: &str = "terminal.title_changed";
    pub const TRUST_DECIDED: &str = "trust.decided";
    pub const FILE_OPEN_REQUESTED: &str = "file.open_requested";
    pub const GIT_STATUS_CHANGED: &str = "git.status_changed";
    pub const GIT_COMPARISON_CHANGED: &str = "git.comparison_changed";
    pub const HELPER_STATUS_CHANGED: &str = "helper.status_changed";
    pub const NOTIFY_REQUESTED: &str = "notify.requested";
    /// Resultado de qualquer operação de balcão (`counter_<nome>.*`). Payload: `CounterResult`.
    pub const COUNTER_RESULT: &str = "counter.result";
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchRef {
    pub project_id: String,
    pub bench_id: String,
    pub name: String,
    /// Absolute checkout path of the bench.
    pub path: String,
    pub branch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAdded(pub ProjectRef);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectActivated(pub ProjectRef);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRemoved {
    pub project_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchActivated(pub BenchRef);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchCreated(pub BenchRef);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchStored {
    pub bench_id: String,
    pub path: String,
}
/// Everything the board and the bench list need about one bench. `BenchRef`
/// stays the minimal shared reference; this richer card is additive and read
/// only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchCard {
    pub bench: BenchRef,
    pub is_main: bool,
    pub stage: String,
    pub archived: bool,
    pub pinned: bool,
    pub unread: bool,
    pub sleeping: bool,
    pub base_branch: Option<String>,
    /// Unix milliseconds.
    pub created_at: u64,
    /// Unix milliseconds.
    pub activated_at: u64,
    /// Key of the request the bench was forged from (`#123`, `LOCAL-1`), when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}
/// A bench was put to sleep or woken up. Sleeping is an organization state:
/// the worktree and the stage do not change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchSleepChanged {
    pub bench_id: String,
    pub path: String,
    pub sleeping: bool,
}
/// A stage transition that was persisted. `origin` is `manual`, `helper`,
/// `routine`, `muster`, `request`, `git` or `extension`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageChanged {
    pub bench_id: String,
    pub from: String,
    pub to: String,
    pub actor: String,
    pub origin: String,
    pub reason: String,
    /// Unix milliseconds.
    pub at: u64,
    /// Key of the request the bench came from, sent only when
    /// `bench.stage.sync_request` is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRef {
    pub terminal_id: String,
    /// Bench path the terminal belongs to (its working directory root).
    pub bench_path: String,
    pub title: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalExited {
    pub terminal: TerminalRef,
    pub exit_code: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalBell {
    pub terminal: TerminalRef,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCommandFinished {
    pub terminal: TerminalRef,
    pub exit_code: Option<i32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalTitleChanged {
    pub terminal: TerminalRef,
}

/// Kind of executable content that needs approval in the trust registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustKind {
    /// `forja.toml` setup script.
    Setup,
    /// `forja.toml` teardown script.
    Teardown,
    /// Project recipe under `.forja/recipes`.
    Recipe,
    /// Project command under `.forja/commands`.
    Command,
    /// Lua extension.
    Extension,
    /// Project helper profile under `.forja/helpers`.
    HelperProfile,
}

/// Answer event emitted after a pending trust request is decided.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustDecided {
    /// Correlates with the `request_id` returned by `trust.request`.
    pub request_id: String,
    /// Kind of content.
    pub kind: TrustKind,
    /// Canonical source path or folder.
    pub source_path: String,
    /// SHA-256 of the exact content that was shown.
    pub content_hash: String,
    /// Whether the content was approved.
    pub approved: bool,
    /// RFC 3339 timestamp of the decision.
    pub decided_at: String,
}

/// One stored trust decision, as listed by `trust.list`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustEntry {
    /// Kind of content.
    pub kind: TrustKind,
    /// Canonical source path or folder.
    pub source_path: String,
    /// SHA-256 that was approved or rejected.
    pub content_hash: String,
    /// Whether this decision approves the content.
    pub approved: bool,
    /// RFC 3339 timestamp of the decision.
    pub decided_at: String,
    /// Short human summary shown in Settings.
    pub summary: String,
    /// True for an approval that lives only until the app closes.
    pub session_only: bool,
}

/// Ask whichever module edits files to open `path` (absolute) at an optional position.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileOpenRequested {
    pub path: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitSummary {
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub staged: u32,
    pub unstaged: u32,
    pub untracked: u32,
    pub conflicts: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusChanged {
    pub bench_path: String,
    pub summary: GitSummary,
}

/// A background branch comparison finished for a bench.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitComparisonChanged {
    pub bench_path: String,
    pub base: Option<String>,
}

/// Coarse state of a helper (coding agent) as shown on benches, the board and toasts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelperStatus {
    #[default]
    Unknown,
    Idle,
    Working,
    NeedsYou,
    Finished,
    /// O ajudante falhou ao iniciar ou encerrou com erro.
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelperStatusChanged {
    pub bench_path: String,
    pub terminal_id: Option<String>,
    pub profile_id: String,
    pub status: HelperStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyLevel {
    #[default]
    Info,
    Success,
    Attention,
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyRequested {
    pub title: String,
    pub body: String,
    pub level: NotifyLevel,
    /// Bench the notification is about, so clicking it can focus that bench.
    pub bench_path: Option<String>,
}

/// Outbound HTTP/proxy policy returned by `network.proxy`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub mode: String,
    pub url: String,
    pub bypass: Vec<String>,
    pub auth: String,
    pub tls_verify: bool,
    pub timeout_s: u64,
    pub offline: bool,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            mode: "system".to_owned(),
            url: String::new(),
            bypass: vec!["localhost".to_owned(), "127.0.0.1".to_owned()],
            auth: "none".to_owned(),
            tls_verify: true,
            timeout_s: 30,
            offline: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&HelperStatus::NeedsYou).unwrap(),
            "\"needs_you\""
        );
    }

    #[test]
    fn trust_decision_serializes_snake_case() {
        let decided = TrustDecided {
            request_id: "r1".into(),
            kind: TrustKind::HelperProfile,
            source_path: "C:/p/.forja/helpers/x.toml".into(),
            content_hash: "abc".into(),
            approved: true,
            decided_at: "2026-11-12T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&decided).unwrap();
        assert_eq!(json["kind"], "helper_profile");
        assert_eq!(json["approved"], true);
    }

    #[test]
    fn bench_ref_roundtrips() {
        let bench = BenchRef {
            project_id: "p".into(),
            bench_id: "b".into(),
            name: "main".into(),
            path: "C:/x".into(),
            branch: Some("main".into()),
        };
        let json = serde_json::to_string(&BenchActivated(bench.clone())).unwrap();
        let back: BenchActivated = serde_json::from_str(&json).unwrap();
        assert_eq!(back.0, bench);
    }
}
