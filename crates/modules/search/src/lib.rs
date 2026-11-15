//! Módulo `search` da Forja: o painel "Busca" (Ctrl+Shift+F, pelo módulo `find`).
//! Busca nos arquivos da bancada com `ignore` e `regex`, em thread, com resultados
//! em fluxo agrupados por arquivo. Substituição com prévia, confirmação e gravação
//! atômica, rechecando o disco antes de gravar.
//!
//! Contratos estão em `MODULE.md` e em `design/contracts.md`.

mod commands;
mod engine;
mod platform;
mod rg;
mod query;
mod replace;
mod state;
mod view;

use crate::state::{lock, Shared, State};
use crate::view::SearchView;
use forja_contracts::{names, BenchActivated, FileOpenRequested};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[forja_kernel::module]
static SEARCH: fn() -> Box<dyn Module> = || Box::new(SearchModule::new());

/// The module. Its state is shared by the panel and the commands.
pub struct SearchModule {
    shared: Shared,
}

impl SearchModule {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl Default for SearchModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for SearchModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("search", "search.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        // Contract events: this module publishes file.open_requested and follows the active bench.
        r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        let shared = Arc::clone(&self.shared);
        r.subscribe(move |_cx, event: &BenchActivated| {
            commands::on_bench_activated(&shared, &event.0);
        });

        let shared = Arc::clone(&self.shared);
        r.command("search.focus", "search.command.focus", move |_cx, _args| {
            commands::focus(&shared)
        });

        let shared = Arc::clone(&self.shared);
        r.command("search.run", "search.command.run", move |cx, args| {
            if let Some(bench_path) = args.str("bench_path") {
                commands::set_root(&shared, PathBuf::from(bench_path));
            } else {
                commands::sync_root(cx, &shared);
            }
            let mut form = lock(&shared).form.clone();
            if let Some(pattern) = args.str("pattern") {
                form.pattern = pattern.to_owned();
            }
            if let Some(value) = args.bool("regex") {
                form.regex = value;
            }
            if let Some(value) = args.bool("case_sensitive") {
                form.case_sensitive = value;
            }
            if let Some(value) = args.bool("whole_word") {
                form.whole_word = value;
            }
            if let Some(include) = args.str("include") {
                form.include = include.to_owned();
            }
            if let Some(exclude) = args.str("exclude") {
                form.exclude = exclude.to_owned();
            }
            commands::set_form(&shared, form);
            commands::start_search(cx, &shared)
        });

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("search.run", "search.method.run.description")
                .effect(MethodEffect::Read)
                .param(ParamSchema::required("pattern", ParamKind::String))
                .param(ParamSchema::optional("bench_path", ParamKind::String))
                .param(ParamSchema::optional("regex", ParamKind::Boolean))
                .param(ParamSchema::optional("case_sensitive", ParamKind::Boolean))
                .param(ParamSchema::optional("whole_word", ParamKind::Boolean))
                .param(ParamSchema::optional("include", ParamKind::String))
                .param(ParamSchema::optional("exclude", ParamKind::String))
                .result(ResultSchema::new("search.method.run.result")),
            move |cx, params| {
                let Some(pattern) = params.get("pattern").and_then(Value::as_str) else {
                    return Err(RpcError::invalid_params("pattern is required"));
                };
                if let Some(bench_path) = params.get("bench_path").and_then(Value::as_str) {
                    commands::set_root(&shared, PathBuf::from(bench_path));
                } else {
                    commands::sync_root(cx, &shared);
                }
                let mut form = lock(&shared).form.clone();
                form.pattern = pattern.to_owned();
                if let Some(value) = params.get("regex").and_then(Value::as_bool) {
                    form.regex = value;
                }
                if let Some(value) = params.get("case_sensitive").and_then(Value::as_bool) {
                    form.case_sensitive = value;
                }
                if let Some(value) = params.get("whole_word").and_then(Value::as_bool) {
                    form.whole_word = value;
                }
                if let Some(include) = params.get("include").and_then(Value::as_str) {
                    form.include = include.to_owned();
                }
                if let Some(exclude) = params.get("exclude").and_then(Value::as_str) {
                    form.exclude = exclude.to_owned();
                }
                commands::set_form(&shared, form);
                commands::start_search(cx, &shared).map_err(|error| RpcError::app(error.to_string()))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.rpc(
            Method::new("search.results", "search.method.results.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("search.method.results.result")),
            move |_cx, _params| Ok(commands::results_snapshot(&shared)),
        );

        let shared = Arc::clone(&self.shared);
        r.view(
            Slot::RightPanel,
            "search.panel",
            "search.panel.title",
            30,
            move || {
                Box::new(SearchView {
                    shared: Arc::clone(&shared),
                })
            },
        );

        for key in [
            "search.query.hint",
            "search.option.regex",
            "search.option.case",
            "search.option.word",
            "search.include.hint",
            "search.exclude.hint",
            "search.button.search",
            "search.button.stop",
            "search.status.idle",
            "search.status.running",
            "search.status.none",
            "search.status.results",
            "search.status.truncated",
            "search.status.more",
            "search.replace.title",
            "search.replace.hint",
            "search.button.preview",
            "search.button.apply",
            "search.button.confirm",
            "search.preview.summary",
            "search.preview.multiline",
            "search.preview.planning",
            "search.notice.no_bench",
            "search.notice.empty",
            "search.notice.thread",
            "search.notice.no_preview",
            "search.notice.replaced",
            "search.notice.changed",
            "search.notice.io",
            "search.notice.error",
            "search.error.invalid-regex",
        ] {
            r.uses_i18n(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};

    /// Stands in for the benches module: answers `bench.current` with one fixed bench.
    struct FakeBench {
        path: String,
    }

    impl Module for FakeBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "bench.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let path = self.path.clone();
            r.command(
                "bench.current",
                "bench.command.current",
                move |_cx, _args| {
                    Ok(serde_json::json!({
                        "project_id": "p",
                        "bench_id": "b",
                        "name": "main",
                        "path": path,
                        "branch": null
                    }))
                },
            );
        }
    }

    fn wait_until(runtime: &mut Runtime, done: impl Fn() -> bool) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
        loop {
            runtime.poll_tasks();
            if done() { return; }
            receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("task did not signal completion");
        }
    }

    #[test]
    fn search_streams_then_previews_and_applies_replace_through_tasks() {
        let dir = std::env::temp_dir().join(format!("forja-search-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::write(repo.join("notes.txt"), "needle one\nother\n").unwrap();
        std::fs::write(repo.join("src").join("lib.rs"), "fn needle()\n").unwrap();

        let module = SearchModule::new();
        let shared = Arc::clone(&module.shared);
        let mut runtime = Runtime::new(
            vec![
                Box::new(module),
                Box::new(FakeBench {
                    path: repo.display().to_string(),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("search and a bench stand-in register");

        {
            let mut cx = runtime.ctx("search");
            commands::sync_root(&mut cx, &shared);
        }
        commands::set_form(
            &shared,
            state::Form {
                pattern: "needle".into(),
                ..state::Form::default()
            },
        );
        {
            let mut cx = runtime.ctx("search");
            commands::start_search(&mut cx, &shared).unwrap();
        }
        wait_until(&mut runtime, || {
            let state = lock(&shared);
            !state.running && state.searched
        });
        {
            let state = lock(&shared);
            assert_eq!(state.files.len(), 2, "two files match");
            assert_eq!(state.matches, 2);
        }

        commands::set_form(
            &shared,
            state::Form {
                pattern: "needle".into(),
                replacement: "pin".into(),
                ..state::Form::default()
            },
        );
        {
            let mut cx = runtime.ctx("search");
            commands::preview(&mut cx, &shared).unwrap();
        }
        wait_until(&mut runtime, || lock(&shared).plan.is_some());
        assert_eq!(lock(&shared).plan.as_ref().map(|p| p.matches), Some(2));

        // The first press only asks for confirmation; nothing is written yet.
        {
            let mut cx = runtime.ctx("search");
            let first = commands::apply(&mut cx, &shared).unwrap();
            assert_eq!(first["confirm"], serde_json::json!(true));
        }
        assert_eq!(
            std::fs::read_to_string(repo.join("notes.txt")).unwrap(),
            "needle one\nother\n"
        );

        {
            let mut cx = runtime.ctx("search");
            commands::apply(&mut cx, &shared).unwrap();
        }
        wait_until(&mut runtime, || {
            let state = lock(&shared);
            !state.applying && state.plan.is_none() && !state.running
        });
        assert_eq!(
            std::fs::read_to_string(repo.join("notes.txt")).unwrap(),
            "pin one\nother\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("src").join("lib.rs")).unwrap(),
            "fn pin()\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_bench_activation_event_retargets_the_search_panel() {
        let dir = std::env::temp_dir().join(format!("forja-search-event-{}", std::process::id()));
        let module = SearchModule::new();
        let shared = Arc::clone(&module.shared);
        let mut runtime = Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("search registers its contract events");
        let root = dir.join("bench-a");
        runtime.emit(&BenchActivated(forja_contracts::BenchRef {
            project_id: "p".into(),
            bench_id: "a".into(),
            name: "a".into(),
            path: root.display().to_string(),
            branch: None,
        }));
        assert_eq!(lock(&shared).root.as_deref(), Some(root.as_path()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn contract_holds_and_focus_command_works() {
        let dir = std::env::temp_dir().join(format!("forja-search-lib-{}", std::process::id()));
        let mut runtime = Runtime::new(
            vec![Box::new(SearchModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("search registers cleanly");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let value = runtime.run_command("search.focus", Args::new()).unwrap();
        assert_eq!(value["focused"], serde_json::json!(true));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rpc_run_and_results_return_matches() {
        let dir = std::env::temp_dir().join(format!("forja-search-rpc-{}", std::process::id()));
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("hello.txt"), "forja smoke test\nother\n").unwrap();
        let module = SearchModule::new();
        let shared = Arc::clone(&module.shared);
        let mut runtime = Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("search registers cleanly");
        let started = runtime
            .call_method(
                "search.run",
                serde_json::Map::from_iter([
                    ("pattern".into(), serde_json::json!("forja")),
                    ("bench_path".into(), serde_json::json!(repo.display().to_string())),
                ]),
            )
            .expect("search.run rpc succeeds");
        assert_eq!(started["started"], serde_json::json!(true));
        wait_until(&mut runtime, || !lock(&shared).running);
        let results = runtime
            .call_method("search.results", serde_json::Map::new())
            .expect("search.results rpc succeeds");
        assert_eq!(results["matches"], serde_json::json!(1));
        assert_eq!(results["files"][0]["rel"], serde_json::json!("hello.txt"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn regex_parse_errors_are_humanized_without_raw_parser_output() {
        let dir = std::env::temp_dir().join(format!("forja-search-err-{}", std::process::id()));
        let module = SearchModule::new();
        let mut runtime = Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("search registers cleanly");
        let cx = runtime.ctx("search");
        let raw = "regex parse error:\n    C:\\forja\\999\n         ^\nerror: unrecognized escape sequence";
        let human = view::humanize_error(&cx, raw);
        assert_eq!(human.summary, "Expressão regular inválida.");
        assert!(!human.summary.contains("error:"));
        assert_eq!(human.detail.as_deref(), Some(raw));
        let _ = std::fs::remove_dir_all(dir);
    }
}
