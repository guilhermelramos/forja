//! Módulo `archive` da Forja: o Acervo de sessões dos ajudantes.
//!
//! Indexa transcrições com tantivy em `%APPDATA%\Forja\archive-index`, de forma
//! incremental e em segundo plano. Os caminhos das sessões e os argumentos de
//! retomar/bifurcar vêm dos perfis em `content/helpers`, nunca do código antigo.

mod index;
mod parse;
mod profile;
mod state;
mod view;

use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Args, Ctx, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec,
    SettingValue, Slot,
};
use index::Filters;
use parse::ArchiveSession;
use serde_json::{json, Map, Value};
use state::{ArchiveState, ScanOptions};

fn keep_transcripts(cx: &forja_kernel::Ctx<'_>) -> bool {
    transcript_policy(
        cx.setting("archive.transcripts.keep")
            .and_then(|value| value.as_bool()),
        cx.setting("helper.sessions.keep_transcripts")
            .and_then(|value| value.as_bool()),
    )
}

fn transcript_policy(archive: Option<bool>, helper: Option<bool>) -> bool {
    archive.unwrap_or(true) && helper.unwrap_or(true)
}
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use view::ArchiveView;

/// Ponteiro compartilhado para o estado do módulo.
pub(crate) type Shared = Arc<Core>;

/// Estado global do módulo.
pub struct Core {
    /// Índice e sessões.
    pub state: Mutex<ArchiveState>,
}

impl Core {
    fn new() -> Self {
        Self {
            state: Mutex::new(ArchiveState::default()),
        }
    }
}

#[forja_kernel::module]
static ARCHIVE: fn() -> Box<dyn Module> = || {
    Box::new(ArchiveModule {
        core: Arc::new(Core::new()),
    })
};

/// Módulo archive.
pub struct ArchiveModule {
    core: Shared,
}

impl Module for ArchiveModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("archive", "archive.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.core);
        rpc::register(r, &self.core);

        for key in [
            "archive.view.title",
            "archive.view.search",
            "archive.view.search-hint",
            "archive.view.search-help",
            "archive.view.search-button",
            "archive.view.refresh",
            "archive.view.all-helpers",
            "archive.view.project",
            "archive.view.project-hint",
            "archive.view.project-help",
            "archive.view.helper-filter",
            "archive.view.date-filter",
            "archive.view.date-all",
            "archive.view.date-7",
            "archive.view.date-30",
            "archive.view.date-365",
            "archive.view.sessions",
            "archive.view.files",
            "archive.view.results",
            "archive.view.indexing",
            "archive.view.no-sessions",
            "archive.view.empty",
            "archive.view.resume",
            "archive.view.fork",
            "archive.view.delete",
            "archive.view.delete-title",
            "archive.view.delete-body",
            "archive.view.delete-confirm",
            "archive.view.cancel",
            "archive.view.deleted",
            "archive.view.opened",
            "archive.error.invalid-query",
        ] {
            r.uses_i18n(key);
        }

        let view_core = Arc::clone(&self.core);
        let order = if std::env::var_os("FORJA_ARCHIVE_PREVIEW").is_some() {
            1
        } else {
            110
        };
        r.view(
            Slot::Rail,
            "archive.tab",
            "archive.view.title",
            order,
            move || Box::new(ArchiveView::new(Arc::clone(&view_core))),
        );
        r.view_glyph("archive.tab", forja_ui_kit::ARCHIVE.id());
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "archive.sessions.index",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-archive-sessions-index-label",
        "setting-archive-sessions-index-help",
    ));
    r.setting(SettingSpec::new(
        "archive.sessions.retention_days",
        SettingKind::Integer {
            min: Some(0),
            max: Some(3650),
        },
        SettingValue::Integer(365),
        "setting-archive-sessions-retention_days-label",
        "setting-archive-sessions-retention_days-help",
    ));
    r.setting(SettingSpec::new(
        "archive.sessions.max_gb",
        SettingKind::Integer {
            min: Some(1),
            max: Some(50),
        },
        SettingValue::Integer(10),
        "setting-archive-sessions-max_gb-label",
        "setting-archive-sessions-max_gb-help",
    ));
    r.setting(SettingSpec::new(
        "archive.transcripts.keep",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-archive-transcripts-keep-label",
        "setting-archive-transcripts-keep-help",
    ));
}

mod commands {
    use super::*;

    pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
        let search_core = Arc::clone(core);
        r.command(
            "archive.search",
            "archive.command.search",
            move |cx, args| {
                let query = args.str("query").unwrap_or_default().to_owned();
                let profile_id = args.str("profile_id").map(str::to_owned);
                let project = args.str("project").map(str::to_owned);
                let since_ms = args
                    .i64("since_days")
                    .filter(|days| *days > 0)
                    .map(|days| chrono::Utc::now().timestamp_millis() - days * 24 * 60 * 60 * 1000);
                let limit = args.i64("limit").unwrap_or(50).clamp(1, 500) as usize;
                let filters = Filters {
                    profile_id,
                    project,
                    since_ms,
                    until_ms: None,
                };
                let dir = state::index_dir(&cx.store().path());
                let hits = state::search_task(&dir, &query, &filters, limit)
                    .map_err(KernelError::Message)?;
                if let Ok(mut state) = search_core.state.lock() {
                    state.results = hits.clone();
                }
                Ok(json!({ "hits": hits }))
            },
        );

        let resume_core = Arc::clone(core);
        r.command(
            "archive.resume",
            "archive.command.resume",
            move |cx, args| {
                let profile_id = args.required_str("profile_id")?.to_owned();
                let session_id = args
                    .str("session_id")
                    .ok_or_else(|| KernelError::Message("missing session_id".to_owned()))?
                    .to_owned();
                let fork = args.bool("fork").unwrap_or(false);
                let bench = args
                    .str("bench_path")
                    .map(PathBuf::from)
                    .or_else(|| current_bench(cx));
                let path = args.str("path").map(PathBuf::from);
                resume(
                    cx,
                    &resume_core,
                    &profile_id,
                    &session_id,
                    bench,
                    path,
                    fork,
                )
            },
        );

        let delete_core = Arc::clone(core);
        r.command(
            "archive.delete",
            "archive.command.delete",
            move |cx, args| {
                let path = PathBuf::from(args.required_str("path")?);
                let confirm = args.bool("confirm").unwrap_or(false);
                if !confirm {
                    return Err(KernelError::Message(
                        "deleting a session requires confirmation".to_owned(),
                    ));
                }
                if path.exists() {
                    trash::delete(&path).map_err(|error| {
                        KernelError::Message(format!(
                            "não foi possível mover para a lixeira: {error}"
                        ))
                    })?;
                }
                let dir = state::index_dir(&cx.store().path());
                if let Ok(index) = index::ArchiveIndex::open(&dir) {
                    let _ = index.delete_path(&path);
                }
                if let Ok(mut state) = delete_core.state.lock() {
                    let text = path.display().to_string();
                    state.cache.forget(&path);
                    state.sessions.retain(|session| session.path != text);
                    state.results.retain(|hit| hit.path != text);
                }
                Ok(json!({ "deleted": true }))
            },
        );

        let list_core = Arc::clone(core);
        r.command("archive.list", "archive.command.list", move |cx, _| {
            let mut state = list_core.state.lock().map_err(poisoned)?;
            if state.profiles.is_empty() {
                let bench = current_bench(cx);
                state.ensure_profiles(bench.as_deref());
            }
            let sessions: Vec<Value> = state.sessions.iter().map(session_json).collect();
            Ok(json!({ "sessions": sessions }))
        });

        let reindex_core = Arc::clone(core);
        r.command(
            "archive.reindex",
            "archive.command.reindex",
            move |cx, _| {
                let started = start_scan(cx, &reindex_core)?;
                Ok(json!({ "started": started }))
            },
        );
    }

    fn resume(
        cx: &mut Ctx<'_>,
        core: &Shared,
        profile_id: &str,
        session_id: &str,
        bench: Option<PathBuf>,
        path: Option<PathBuf>,
        fork: bool,
    ) -> Result<Value, KernelError> {
        let mut state = core.state.lock().map_err(poisoned)?;
        if state.profiles.is_empty() {
            state.ensure_profiles(bench.as_deref());
        }
        let profile = state
            .profile(profile_id)
            .cloned()
            .ok_or_else(|| KernelError::Message(format!("perfil `{profile_id}` não encontrado")))?;
        let session = state.find(profile_id, session_id).cloned().or_else(|| {
            let path = path.clone()?;
            let format = profile.format()?;
            parse::parse_file(format, profile_id, &path).ok().flatten()
        });
        let argv = if fork {
            profile.fork_argv(session_id)
        } else {
            profile.resume_argv(session_id)
        };
        if argv.is_empty() {
            return Err(KernelError::Message(format!(
                "o perfil `{profile_id}` não declara argumentos de {}",
                if fork { "bifurcação" } else { "retomada" }
            )));
        }
        let cwd = bench
            .filter(|path| path.is_dir())
            .or_else(|| {
                session
                    .as_ref()
                    .map(|session| PathBuf::from(&session.project))
                    .filter(|path| path.is_dir())
            })
            .or_else(|| current_bench(cx))
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let args_text = argv
            .iter()
            .map(|arg| quote_arg(arg))
            .collect::<Vec<_>>()
            .join(" ");
        let command_line = format!("{} {}", quote_arg(&profile.start.command), args_text)
            .trim()
            .to_owned();
        let mut command = Args::new();
        command.insert("cwd", json!(cwd.display().to_string()));
        command.insert("command", json!(profile.start.command.clone()));
        command.insert("args", json!(args_text));
        command.insert("title", json!(profile.name.clone()));
        let opened = cx.run_command("terminal.open", command).map_err(|_| {
            KernelError::Message(
                "terminal module is not available; archive.resume needs terminal.open".to_owned(),
            )
        })?;
        let terminal_id = opened
            .get("terminal_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                KernelError::Message("terminal.open returned no terminal_id".to_owned())
            })?
            .to_owned();
        Ok(json!({
            "terminal_id": terminal_id,
            "profile_id": profile.id,
            "session_id": session_id,
            "fork": fork,
            "command_line": command_line
        }))
    }

    pub(super) fn start_scan(cx: &mut Ctx<'_>, core: &Shared) -> Result<bool, KernelError> {
        let (profiles, cache, index_dir) = {
            let mut state = core.state.lock().map_err(poisoned)?;
            if state.scanning {
                return Ok(false);
            }
            if state.profiles.is_empty() {
                let bench = current_bench(cx);
                state.ensure_profiles(bench.as_deref());
            }
            state.scanning = true;
            (
                state.profiles.clone(),
                std::mem::take(&mut state.cache),
                state::index_dir(&cx.store().path()),
            )
        };
        let options = ScanOptions {
            keep_text: crate::keep_transcripts(cx),
            retention_days: cx
                .setting("archive.sessions.retention_days")
                .and_then(|value| value.as_i64())
                .unwrap_or(365),
            max_bytes: (cx
                .setting("archive.sessions.max_gb")
                .and_then(|value| value.as_i64())
                .unwrap_or(10)
                .max(1) as u64)
                * 1024
                * 1024
                * 1024,
        };
        let owned = Arc::clone(core);
        cx.spawn(
            move || state::scan_task(profiles, index_dir, cache, options),
            move |_cx, result| {
                if let Ok(mut state) = owned.state.lock() {
                    match result {
                        Ok((cache, sessions, report)) => state.finish(cache, sessions, report),
                        Err(error) => {
                            let cache = std::mem::take(&mut state.cache);
                            state.fail(cache, error);
                        }
                    }
                }
            },
        );
        Ok(true)
    }

    fn current_bench(cx: &mut Ctx<'_>) -> Option<PathBuf> {
        cx.run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| value.get("path").and_then(Value::as_str).map(PathBuf::from))
    }
}

fn session_json(session: &ArchiveSession) -> Value {
    json!({
        "session_id": session.session_id,
        "profile_id": session.helper_id,
        "project": session.project,
        "path": session.path,
        "started_ms": session.started_ms,
        "updated_ms": session.updated_ms,
        "title": session.title
    })
}

fn poisoned<T>(_: T) -> KernelError {
    KernelError::Message("archive state is poisoned".to_owned())
}

/// Cita um argumento para a linha de comando do Windows.
pub fn quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return value.to_owned();
    }
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            result.push_str(&"\\".repeat(slashes));
        }
        slashes = 0;
        result.push(ch);
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}

mod rpc {
    use super::*;

    pub(super) fn register(r: &mut Registrar<'_>, core: &Shared) {
        let search_core = Arc::clone(core);
        r.rpc(
            Method::new("archive.search", "archive.method.search.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::optional("query", ParamKind::String).alias("q"))
                .param(ParamSchema::optional("profile_id", ParamKind::String).alias("profile"))
                .param(ParamSchema::optional("project", ParamKind::String))
                .param(ParamSchema::optional("since_days", ParamKind::Integer))
                .param(ParamSchema::optional("limit", ParamKind::Integer))
                .result(
                    ResultSchema::new("archive.method.search.result").schema(json!({
                        "type": "object",
                        "properties": { "hits": { "type": "array" } },
                        "required": ["hits"]
                    })),
                ),
            move |cx, params| run(cx, &search_core, "archive.search", params),
        );

        let resume_core = Arc::clone(core);
        r.rpc(
            Method::new("archive.resume", "archive.method.resume.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("profile_id", ParamKind::String).alias("profile"))
                .param(ParamSchema::required("session_id", ParamKind::String))
                .param(ParamSchema::optional("bench_path", ParamKind::String))
                .param(ParamSchema::optional("path", ParamKind::String))
                .param(ParamSchema::optional("fork", ParamKind::Boolean))
                .result(
                    ResultSchema::new("archive.method.resume.result").schema(json!({
                        "type": "object",
                        "properties": {
                            "terminal_id": { "type": "string" },
                            "profile_id": { "type": "string" },
                            "session_id": { "type": "string" },
                            "fork": { "type": "boolean" },
                            "command_line": { "type": "string" }
                        },
                        "required": ["terminal_id", "command_line"]
                    })),
                ),
            move |cx, params| run(cx, &resume_core, "archive.resume", params),
        );

        let delete_core = Arc::clone(core);
        r.rpc(
            Method::new("archive.delete", "archive.method.delete.description")
                .effect(MethodEffect::Write)
                .param(ParamSchema::required("path", ParamKind::String))
                .param(ParamSchema::optional("confirm", ParamKind::Boolean))
                .result(
                    ResultSchema::new("archive.method.delete.result").schema(json!({
                        "type": "object",
                        "properties": { "deleted": { "type": "boolean" } },
                        "required": ["deleted"]
                    })),
                ),
            move |cx, params| run(cx, &delete_core, "archive.delete", params),
        );

        let list_core = Arc::clone(core);
        r.rpc(
            Method::new("archive.list", "archive.method.list.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(
                    ResultSchema::new("archive.method.list.result").schema(json!({
                        "type": "object",
                        "properties": { "sessions": { "type": "array" } },
                        "required": ["sessions"]
                    })),
                ),
            move |cx, params| run(cx, &list_core, "archive.list", params),
        );

        let reindex_core = Arc::clone(core);
        r.rpc(
            Method::new("archive.reindex", "archive.method.reindex.description")
                .effect(MethodEffect::Write)
                .result(
                    ResultSchema::new("archive.method.reindex.result").schema(json!({
                        "type": "object",
                        "properties": { "started": { "type": "boolean" } },
                        "required": ["started"]
                    })),
                ),
            move |cx, params| run(cx, &reindex_core, "archive.reindex", params),
        );
    }

    fn run(
        cx: &mut Ctx<'_>,
        core: &Shared,
        command: &str,
        params: Map<String, Value>,
    ) -> Result<Value, RpcError> {
        let _ = core;
        cx.run_command(command, Args::from_map(params))
            .map_err(|error| RpcError::app(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn either_transcript_preference_can_disable_text() {
        assert!(super::transcript_policy(None, None));
        assert!(!super::transcript_policy(Some(false), Some(true)));
        assert!(!super::transcript_policy(Some(true), Some(false)));
    }
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    struct TempGuard(PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = PathBuf;
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-archive-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(ArchiveModule {
                core: Arc::new(Core::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("archive module registers")
    }

    #[test]
    fn contract_search_and_resume_need_terminal() {
        let dir = temp_dir("rpc");
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let search = runtime
            .call_method(
                "archive.search",
                Map::from_iter([("query".to_owned(), json!("parser"))]),
            )
            .expect("search empty index");
        assert_eq!(search["hits"], json!([]));
        let error = runtime
            .call_method(
                "archive.resume",
                Map::from_iter([
                    ("profile_id".to_owned(), json!("claude-code")),
                    ("session_id".to_owned(), json!("s1")),
                    ("fork".to_owned(), json!(true)),
                ]),
            )
            .expect_err("terminal module is absent");
        assert!(error.message.contains("terminal"), "{}", error.message);
        let missing = runtime
            .call_method(
                "archive.delete",
                Map::from_iter([("path".to_owned(), json!("C:/missing.jsonl"))]),
            )
            .expect_err("confirmation is required");
        assert!(missing.message.contains("confirmation"));
    }

    #[test]
    fn command_line_quotes_spaces() {
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg("two words"), "\"two words\"");
    }

    #[test]
    fn archive_errors_are_humanized_in_pt_br_and_en() {
        let dir = temp_dir("humanize");
        let mut rt = runtime(&dir);
        {
            let cx = rt.ctx("archive");
            let io_err = view::humanize_error(
                &cx,
                "An IO error occurred: 'Access is denied. (os error 5)'",
            );
            assert!(!io_err.summary.contains("os error"));
            assert!(!io_err.summary.contains("IO error"));
            assert!(io_err
                .detail
                .as_deref()
                .is_some_and(|d| d.contains("os error 5")));

            let query_err = view::humanize_error(&cx, "busca inválida: syntax error");
            assert!(query_err.summary.contains("sintaxe inválida"));
            assert!(query_err.detail.is_some());
        }
        rt.set_locale(forja_kernel::Locale::En);
        let cx = rt.ctx("archive");
        let io_err_en = view::humanize_error(
            &cx,
            "An IO error occurred: 'Access is denied. (os error 5)'",
        );
        assert!(!io_err_en.summary.contains("os error"));
        assert!(!io_err_en.summary.contains("IO error"));
    }
}
