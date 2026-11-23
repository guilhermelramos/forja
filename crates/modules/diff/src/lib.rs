//! Módulo `diff` da Forja.
//!
//! Aba "Diferenças" alimentada por `git.changes`/`git.diff`, com visão lado a
//! lado, unificada e combinada, árvore de arquivos, recolhimento de trechos
//! iguais, diff de imagem e notas de revisão enviadas ao terminal do ajudante.
//!
//! Contratos em `MODULE.md` e em `design/contracts.md`.

mod commands;
mod images;
mod model;
mod state;
mod view;

use crate::state::Shared;
use forja_contracts::{BenchActivated, GitComparisonChanged, GitStatusChanged, names};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Args, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::Value;

#[forja_kernel::module]
static DIFF: fn() -> Box<dyn Module> = || Box::new(DiffModule::new());

pub struct DiffModule {
    state: Shared,
}

impl DiffModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for DiffModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for DiffModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("diff", "diff.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<GitStatusChanged>(names::GIT_STATUS_CHANGED);
        r.contract_event::<GitComparisonChanged>(names::GIT_COMPARISON_CHANGED);
        let bench_state = self.state.clone();
        r.subscribe(move |_cx, event: &BenchActivated| {
            bench_state.set_bench(Some(event.0.path.clone()));
            bench_state.set_stale(true);
        });
        let status_state = self.state.clone();
        r.subscribe(move |_cx, event: &GitStatusChanged| {
            if status_state.bench().as_deref() == Some(event.bench_path.as_str()) {
                status_state.set_stale(true);
            }
        });
        let comparison_state = self.state.clone();
        r.subscribe(move |cx, event: &GitComparisonChanged| {
            if comparison_state.bench().as_deref() == Some(event.bench_path.as_str())
                && comparison_state
                    .comparison()
                    .as_deref()
                    .and_then(commands::comparison_base)
                    == event.base.as_deref()
            {
                let _ = commands::refresh(cx, &comparison_state, &Args::new());
            }
        });

        let open = self.state.clone();
        r.command("diff.open", "diff.command.open", move |cx, args| {
            commands::open(cx, &open, &args)
        });

        let refresh = self.state.clone();
        r.command("diff.refresh", "diff.command.refresh", move |cx, args| {
            commands::refresh(cx, &refresh, &args)
        });

        let select = self.state.clone();
        r.command("diff.select", "diff.command.select", move |cx, args| {
            let bench = commands::bench_path(cx, &select, &args)?;
            let file = args.required_str("file")?.to_owned();
            let staged = args.bool("staged").unwrap_or(false);
            let comparison = select.comparison();
            commands::select(cx, &select, &bench, &file, staged, comparison.as_deref())
        });

        let send = self.state.clone();
        r.command(
            "diff.send_notes",
            "diff.command.send_notes",
            move |cx, args| commands::send_notes(cx, &send, &args),
        );

        register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "diff.tab",
            "diff.tab.title",
            bench_tab_order(14),
            move || Box::new(view::DiffView::new(view_state.clone())),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    let settings = [
        (
            "diff.view.mode",
            SettingKind::Choice {
                options: vec!["split".to_owned(), "unified".to_owned()],
            },
            SettingValue::Text("split".to_owned()),
        ),
        (
            "diff.context.lines",
            SettingKind::Integer {
                min: Some(0),
                max: Some(20),
            },
            SettingValue::Integer(3),
        ),
        (
            "diff.whitespace.ignore",
            SettingKind::Bool,
            SettingValue::Bool(false),
        ),
        (
            "diff.view.collapse_unchanged",
            SettingKind::Bool,
            SettingValue::Bool(true),
        ),
        (
            "diff.notes.inline",
            SettingKind::Bool,
            SettingValue::Bool(true),
        ),
        (
            "diff.notes.send_mode",
            SettingKind::Choice {
                options: vec!["append".to_owned(), "replace".to_owned(), "ask".to_owned()],
            },
            SettingValue::Text("append".to_owned()),
        ),
    ];
    for (key, kind, default) in settings {
        let slug = key.replace('.', "-");
        r.setting(SettingSpec::new(
            key,
            kind,
            default,
            format!("setting-{slug}-label"),
            format!("setting-{slug}-help"),
        ));
    }
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let changes = shared.clone();
    r.rpc(
        Method::new("diff.changes", "diff.method.changes.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("path", ParamKind::String).alias("caminho"))
            .result(ResultSchema::new("diff.method.changes.result")),
        move |cx, params| {
            let args = args_from(&params);
            commands::refresh(cx, &changes, &args).map_err(rpc_error)
        },
    );

    let file = shared.clone();
    r.rpc(
        Method::new("diff.file", "diff.method.file.description")
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::required("file", ParamKind::String).alias("arquivo"))
            .param(ParamSchema::optional("path", ParamKind::String).alias("caminho"))
            .param(ParamSchema::optional("staged", ParamKind::Boolean))
            .result(ResultSchema::new("diff.method.file.result")),
        move |cx, params| {
            let args = args_from(&params);
            let bench = commands::bench_path(cx, &file, &args).map_err(rpc_error)?;
            let path = params
                .get("file")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcError::invalid_params("missing `file`"))?;
            let staged = params
                .get("staged")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let comparison = file.comparison();
            commands::select(cx, &file, &bench, path, staged, comparison.as_deref())
                .map_err(rpc_error)
        },
    );

    let notes = shared.clone();
    r.rpc(
        Method::new("diff.notes.send", "diff.method.notes.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::optional("terminal_id", ParamKind::String))
            .result(ResultSchema::new("diff.method.notes.result")),
        move |cx, params| {
            let args = args_from(&params);
            commands::send_notes(cx, &notes, &args).map_err(rpc_error)
        },
    );
}

fn args_from(params: &serde_json::Map<String, Value>) -> Args {
    Args::from_map(params.clone())
}

fn rpc_error(error: forja_kernel::KernelError) -> RpcError {
    RpcError::app(error.to_string())
}

fn bench_tab_order(default: i32) -> i32 {
    match std::env::var("FORJA_BENCH_TAB") {
        Ok(module) if module == "diff" => 1,
        _ => default,
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "diff.name",
        "diff.tab.title",
        "diff.command.open",
        "diff.command.refresh",
        "diff.command.select",
        "diff.command.send_notes",
        "diff.mode.split",
        "diff.mode.unified",
        "diff.mode.combined",
        "diff.toolbar.whitespace",
        "diff.toolbar.collapse",
        "diff.toolbar.refresh",
        "diff.toolbar.previous-change",
        "diff.toolbar.next-change",
        "diff.filter.hint",
        "diff.empty",
        "diff.select-file",
        "diff.staged",
        "diff.binary",
        "diff.collapsed",
        "diff.notes.add",
        "diff.notes.hint",
        "diff.notes.confirm",
        "diff.notes.copy",
        "diff.notes.clear",
        "diff.notes.send",
        "diff.notes.count",
        "diff.notes.empty",
        "diff.notes.no-terminal",
        "diff.notes.heading",
        "diff.notes.intro.append",
        "diff.notes.intro.replace",
        "diff.notes.intro.ask",
        "diff.notes.sent",
        "diff.image.loading",
        "diff.image.missing",
        "diff.error.no-bench",
        "diff.comparison.heading",
        "diff.error.comparison",
        "diff.method.changes.description",
        "diff.method.changes.result",
        "diff.method.file.description",
        "diff.method.file.result",
        "diff.method.notes.description",
        "diff.method.notes.result",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Module, ModuleManifest, Runtime, RuntimeOptions, contract};
    use serde_json::{Map, json};
    use std::path::Path;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-diff-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct FakeBench {
        path: String,
    }

    impl Module for FakeBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "diff.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let path = self.path.clone();
            r.command("bench.current", "diff.command.open", move |_cx, _args| {
                Ok(json!({
                    "project_id": "p",
                    "bench_id": "b",
                    "name": "main",
                    "path": path,
                    "branch": null
                }))
            });
        }
    }

    struct FakeGit;

    impl Module for FakeGit {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("git", "diff.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.command("git.changes", "diff.command.refresh", |_cx, _args| {
                Ok(json!({
                    "summary": {},
                    "files": [
                        {"path": "src/a.rs", "index": "M", "worktree": ".", "staged": true, "unstaged": false, "untracked": false, "conflict": false},
                        {"path": "src/b.rs", "index": ".", "worktree": "M", "staged": false, "unstaged": true, "untracked": false, "conflict": false}
                    ],
                    "upstream": null,
                    "detached": false
                }))
            });
            r.command("git.diff", "diff.command.select", |_cx, args| {
                assert_eq!(args.required_str("file")?, "src/a.rs");
                Ok(json!({
                    "path": "src/a.rs",
                    "staged": true,
                    "binary": false,
                    "hunks": [{
                        "old_start": 1,
                        "old_lines": 1,
                        "new_start": 1,
                        "new_lines": 1,
                        "header": "fn main",
                        "lines": [
                            {"kind": "del", "text": "-a"},
                            {"kind": "add", "text": "+b"}
                        ]
                    }]
                }))
            });
        }
    }

    fn runtime(dir: &Path, bench: &str) -> Runtime {
        Runtime::new(
            vec![
                Box::new(DiffModule::new()),
                Box::new(FakeBench {
                    path: bench.to_owned(),
                }),
                Box::new(FakeGit),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("diff registers cleanly")
    }

    #[test]
    fn contract_holds_and_settings_match_the_catalog() {
        let dir = temp_dir();
        let runtime = runtime(&dir, "C:/bench");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .filter(|spec| spec.key.starts_with("diff."))
            .map(|spec| spec.key.as_str())
            .collect();
        assert_eq!(keys.len(), 6);
        assert!(keys.contains(&"diff.notes.send_mode"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn open_reads_changes_and_the_first_file() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir, "C:/bench");
        let value = runtime.run_command("diff.open", Args::new()).unwrap();
        assert_eq!(value["files"], 2);
        let value = runtime
            .call_method(
                "diff.file",
                Map::from_iter([("file".to_owned(), json!("src/a.rs"))]),
            )
            .unwrap();
        assert_eq!(value["hunks"], 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn notes_send_to_a_terminal_and_are_formatted() {
        let dir = temp_dir();
        let module = DiffModule::new();
        let shared = module.state.clone();
        shared.set_bench(Some("C:/bench".to_owned()));
        shared.add_note("src/a.rs", 12, "usar o e-mail");
        let mut runtime = Runtime::new(
            vec![Box::new(module), Box::new(FakeTerminal)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        let value = runtime.run_command("diff.send_notes", Args::new()).unwrap();
        assert_eq!(value["sent"], true);
        assert_eq!(value["terminal_id"], "t-1");
        let _ = std::fs::remove_dir_all(dir);
    }

    struct FakeTerminal;

    impl Module for FakeTerminal {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("terminal", "diff.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.command("terminal.list", "diff.command.open", |_cx, _args| {
                Ok(json!({"terminals": [{"terminal_id": "t-1", "bench_path": "C:/bench", "title": "helper"}]}))
            });
            r.command("terminal.send", "diff.command.send_notes", |_cx, args| {
                assert_eq!(args.required_str("terminal_id")?, "t-1");
                assert!(args.required_str("text")?.contains("usar o e-mail"));
                Ok(json!({}))
            });
        }
    }
}
