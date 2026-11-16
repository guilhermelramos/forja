//! Módulo `find` da Forja: "Achar arquivo" (Ctrl+P), com índice da bancada por
//! `git ls-files` (ou caminhada de pasta), busca fuzzy, histórico de abertura
//! e `arquivo:linha`. Também contribui como fonte da Bigorna.
//!
//! O atalho Ctrl+Shift+F (`find.in_files`) pertence a este módulo pelo catálogo,
//! mas abre o painel de busca do módulo `search` pelo comando `search.focus`.
//!
//! Contratos estão em `MODULE.md` e em `design/contracts.md`.

mod commands;
mod fuzzy;
mod history;
mod index;
mod platform;
mod state;
mod view;

use crate::state::{Shared, State};
use crate::view::{AnvilView, FindView};
use forja_contracts::{names, BenchActivated, FileOpenRequested};
use forja_kernel::{
    Args, KernelError, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
    Slot,
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[forja_kernel::module]
static FIND: fn() -> Box<dyn Module> = || Box::new(FindModule::new());

/// The module. Its state is shared by the views and the commands.
pub struct FindModule {
    shared: Shared,
}

impl FindModule {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl Default for FindModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for FindModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("find", "find.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        r.setting(SettingSpec::new(
            "find.results.limit",
            SettingKind::Integer {
                min: Some(50),
                max: Some(1_000),
            },
            SettingValue::Integer(200),
            "setting-find-results-limit-label",
            "setting-find-results-limit-help",
        ));
        r.setting(SettingSpec::new(
            "find.search.respect_gitignore",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "setting-find-search-respect_gitignore-label",
            "setting-find-search-respect_gitignore-help",
        ));
        r.setting(SettingSpec::new(
            "find.search.include_hidden",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "setting-find-search-include_hidden-label",
            "setting-find-search-include_hidden-help",
        ));
        r.setting(SettingSpec::new(
            "find.recent.count",
            SettingKind::Integer {
                min: Some(5),
                max: Some(50),
            },
            SettingValue::Integer(20),
            "setting-find-recent-count-label",
            "setting-find-recent-count-help",
        ));
        r.setting(SettingSpec::new(
            "find.grep.engine",
            SettingKind::Choice {
                options: vec![
                    "auto".to_owned(),
                    "ripgrep".to_owned(),
                    "builtin".to_owned(),
                ],
            },
            SettingValue::Text("auto".to_owned()),
            "setting-find-grep-engine-label",
            "setting-find-grep-engine-help",
        ));
        r.setting(SettingSpec::new(
            "find.grep.context",
            SettingKind::Integer {
                min: Some(0),
                max: Some(10),
            },
            SettingValue::Integer(2),
            "setting-find-grep-context-label",
            "setting-find-grep-context-help",
        ));

        // Contract events: this module publishes file.open_requested and follows the active bench.
        r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &BenchActivated| {
            commands::on_bench_activated(&shared, &event.0);
            cx.request_repaint();
        });

        let shared = Arc::clone(&self.shared);
        r.command("find.file", "find.command.file", move |cx, _args| {
            commands::open_picker(cx, &shared)
        });

        r.command(
            "find.in_files",
            "find.command.in_files",
            move |cx, _args| match cx.run_command("search.focus", Args::new()) {
                Ok(value) => Ok(value),
                Err(KernelError::UnknownCommand(_)) => Ok(json!({ "focused": false })),
                Err(error) => Err(error),
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "find.record_open",
            "find.command.record_open",
            move |cx, args| {
                let path = args.required_str("path")?.to_owned();
                commands::record_open_command(cx, &shared, &path)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.view(
            Slot::TopBar,
            "find.trigger",
            "find.trigger",
            10,
            move || {
                Box::new(FindView {
                    shared: Arc::clone(&shared),
                })
            },
        );

        let shared = Arc::clone(&self.shared);
        r.view(
            Slot::AnvilSource,
            "find.anvil",
            "find.anvil.title",
            10,
            move || {
                Box::new(AnvilView {
                    shared: Arc::clone(&shared),
                    ranked_for: None,
                })
            },
        );

        for key in [
            "find.trigger",
            "find.shortcut",
            "find.picker.title",
            "find.picker.hint",
            "find.picker.indexing",
            "find.picker.searching",
            "find.picker.empty",
            "find.picker.no_bench",
            "find.picker.truncated",
            "find.picker.by_walk",
            "find.picker.searching",
            "find.anvil.hint",
            "find.notice.no_bench",
            "find.notice.error",
            "find.notice.save_failed",
        ] {
            r.uses_i18n(key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use crate::state::{lock, Which};
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::Path;
    use std::time::Duration;

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(FindModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("find registers cleanly")
    }

    struct TempGuard(std::path::PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = std::path::Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-find-lib-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    #[test]
    fn contract_holds_and_the_picker_command_opens_it() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let value = runtime.run_command("find.file", Args::new()).unwrap();
        assert_eq!(value["open"], json!(true));
    }

    #[test]
    fn record_open_needs_an_active_bench() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("path", json!("C:/x/y.rs"));
        assert!(runtime.run_command("find.record_open", args).is_err());
    }

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
                    Ok(json!({
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
            receiver.recv_timeout(Duration::from_secs(60)).expect("finder task did not signal completion");
        }
    }

    #[test]
    fn picker_indexes_the_bench_and_ranks_with_history_through_tasks() {
        let dir = temp_dir();
        let repo = dir.join("repo");
        for rel in ["src/main.rs", "src/lib.rs", "docs/notes.md"] {
            let path = repo.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        let module = FindModule::new();
        let shared = Arc::clone(&module.shared);
        let bench = FakeBench {
            path: repo.display().to_string(),
        };
        let mut runtime = Runtime::new(
            vec![Box::new(module), Box::new(bench)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("find and a bench stand-in register");

        // Opening the picker reads the bench and builds the index in a task.
        runtime.run_command("find.file", Args::new()).unwrap();
        wait_until(&mut runtime, || {
            let state = lock(&shared);
            state.index.is_some() && !state.picker.pending
        });
        assert_eq!(lock(&shared).index.as_ref().map(|i| i.files.len()), Some(3));

        // A file opened once is listed first when the query is empty.
        let mut args = Args::new();
        args.insert(
            "path",
            json!(repo.join("docs").join("notes.md").display().to_string()),
        );
        runtime.run_command("find.record_open", args).unwrap();
        {
            let cx = runtime.ctx("find");
            commands::request_rank(&cx, &shared, Which::Picker, String::new());
        }
        wait_until(&mut runtime, || !lock(&shared).picker.pending);
        assert_eq!(lock(&shared).picker.items.first().map(String::as_str), Some("docs/notes.md"));

        // A query ranks by name, and `:line` does not hide the file.
        {
            let cx = runtime.ctx("find");
            commands::request_rank(&cx, &shared, Which::Picker, "lib.rs:12".to_owned());
        }
        wait_until(&mut runtime, || !lock(&shared).picker.pending);
        assert_eq!(lock(&shared).picker.items.first().map(String::as_str), Some("src/lib.rs"));
    }

    #[test]
    fn a_bench_activation_event_retargets_the_finder() {
        let dir = temp_dir();
        let module = FindModule::new();
        let shared = Arc::clone(&module.shared);
        let mut runtime = Runtime::new(
            vec![Box::new(module)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("find registers its contract events");
        let root = dir.join("bench-a");
        runtime.emit(&BenchActivated(forja_contracts::BenchRef {
            project_id: "p".into(),
            bench_id: "a".into(),
            name: "a".into(),
            path: root.display().to_string(),
            branch: None,
        }));
        assert_eq!(lock(&shared).root.as_deref(), Some(root.as_path()));
    }

    #[test]
    fn in_files_degrades_without_the_search_module() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let value = runtime.run_command("find.in_files", Args::new()).unwrap();
        assert_eq!(value["focused"], json!(false));
    }
}
