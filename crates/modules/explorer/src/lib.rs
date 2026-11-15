//! Módulo `explorer` da Forja: o painel "Arquivos" (árvore da bancada atual,
//! operações de arquivo com desfazer/refazer e cache atualizado pelo sistema).
//!
//! Contratos publicados e consumidos estão em `MODULE.md` e em `design/contracts.md`.

mod commands;
mod dialogs;
mod ops;
mod platform;
mod state;
mod tree;
mod view;
mod watch;

use crate::state::{lock, Shared, State};
use crate::view::ExplorerView;
use forja_contracts::{
    names, BenchActivated, FileOpenRequested, ProjectActivated, ProjectRemoved,
};
use forja_kernel::{
    Args, Ctx, KernelError, Module, ModuleManifest, Registrar, Result as KResult, SettingKind,
    SettingSpec, SettingValue, Slot,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[forja_kernel::module]
static EXPLORER: fn() -> Box<dyn Module> = || Box::new(ExplorerModule::new());

/// The module. Its state is shared by the view and the commands.
pub struct ExplorerModule {
    shared: Shared,
}

impl ExplorerModule {
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(State::default())),
        }
    }
}

impl Default for ExplorerModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ExplorerModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("explorer", "explorer.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        r.setting(SettingSpec::new(
            "explorer.tree.show_ignored",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "setting-explorer-tree-show_ignored-label",
            "setting-explorer-tree-show_ignored-help",
        ));
        r.setting(SettingSpec::new(
            "explorer.tree.follow_symlinks",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "setting-explorer-tree-follow_symlinks-label",
            "setting-explorer-tree-follow_symlinks-help",
        ));

        // Contract events. This module publishes file.open_requested and follows the active
        // bench. Both declarations merge with the declarations of the other modules.
        r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<ProjectActivated>(names::PROJECT_ACTIVATED);
        r.contract_event::<ProjectRemoved>(names::PROJECT_REMOVED);
        let shared = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &BenchActivated| {
            commands::on_bench_activated(cx, &shared, &event.0);
        });
        let shared_proj = Arc::clone(&self.shared);
        r.subscribe(move |cx, event: &ProjectActivated| {
            commands::on_project_activated(cx, &shared_proj, &event.0);
        });
        let shared_removed = Arc::clone(&self.shared);
        r.subscribe(move |cx, _event: &ProjectRemoved| {
            commands::on_project_removed(cx, &shared_removed);
        });

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.new_file",
            "explorer.command.new_file",
            move |cx, args| {
                let dir = folder_arg(cx, &shared, &args)?;
                commands::new_entry(
                    cx,
                    &shared,
                    dir,
                    args.str("name").map(str::to_owned),
                    false,
                    args.str("template").map(str::to_owned),
                )
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.new_folder",
            "explorer.command.new_folder",
            move |cx, args| {
                let dir = folder_arg(cx, &shared, &args)?;
                commands::new_entry(
                    cx,
                    &shared,
                    dir,
                    args.str("name").map(str::to_owned),
                    true,
                    None,
                )
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.rename",
            "explorer.command.rename",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::rename(cx, &shared, path, args.str("name").map(str::to_owned))
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.duplicate",
            "explorer.command.duplicate",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::duplicate(cx, &shared, path)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.trash",
            "explorer.command.trash",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::trash(cx, &shared, path)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.undo",
            "explorer.command.undo",
            move |cx, _args| commands::undo(cx, &shared),
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.redo",
            "explorer.command.redo",
            move |cx, _args| commands::redo(cx, &shared),
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.copy_absolute_path",
            "explorer.command.copy_absolute_path",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::copy_path(cx, &shared, &path, false)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.copy_relative_path",
            "explorer.command.copy_relative_path",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::copy_path(cx, &shared, &path, true)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.reveal",
            "explorer.command.reveal",
            move |cx, args| {
                let path = target_arg(cx, &shared, &args)?;
                commands::reveal(cx, &shared, path)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.open_terminal",
            "explorer.command.open_terminal",
            move |cx, args| {
                let dir = folder_arg(cx, &shared, &args)?;
                commands::open_terminal(cx, &shared, &dir)
            },
        );

        let shared = Arc::clone(&self.shared);
        r.command(
            "explorer.refresh",
            "explorer.command.refresh",
            move |_cx, _args| commands::refresh(&shared),
        );

        let shared = Arc::clone(&self.shared);
        r.view(
            Slot::RightPanel,
            "explorer.panel",
            "explorer.panel.title",
            10,
            move || {
                Box::new(ExplorerView {
                    shared: Arc::clone(&shared),
                })
            },
        );

        for key in [
            "explorer.panel.empty",
            "explorer.panel.loading",
            "explorer.panel.empty_folder",
            "explorer.menu.open",
            "explorer.dialog.new_file.title",
            "explorer.dialog.new_folder.title",
            "explorer.dialog.rename.title",
            "explorer.dialog.name.label",
            "explorer.dialog.template.label",
            "explorer.dialog.template.none",
            "explorer.dialog.confirm",
            "explorer.dialog.cancel",
            "explorer.dialog.trash.title",
            "explorer.dialog.trash.body",
            "explorer.dialog.trash.yes",
            "explorer.notice.created",
            "explorer.notice.renamed",
            "explorer.notice.duplicated",
            "explorer.notice.trashed",
            "explorer.notice.undone",
            "explorer.notice.redone",
            "explorer.notice.nothing_to_undo",
            "explorer.notice.nothing_to_redo",
            "explorer.notice.busy",
            "explorer.notice.copied",
            "explorer.notice.no_terminal",
            "explorer.notice.no_bench",
            "explorer.notice.name_empty",
            "explorer.notice.name_invalid",
            "explorer.notice.error",
            "explorer.notice.dismiss",
            "explorer.error.not.found",
            "explorer.error.permission",
            "explorer.error.exists",
            "explorer.copy_suffix",
        ] {
            r.uses_i18n(key);
        }
    }
}

/// The path a command names: the `path` argument, else the selected item, else the bench root.
fn target_arg(cx: &Ctx<'_>, shared: &Shared, args: &Args) -> KResult<PathBuf> {
    if let Some(path) = args.str("path") {
        return Ok(PathBuf::from(path));
    }
    let state = lock(shared);
    state
        .selected
        .clone()
        .or_else(|| state.tree.root().map(Path::to_path_buf))
        .ok_or_else(|| KernelError::message(cx.t("explorer.notice.no_bench")))
}

/// The folder a command works in: the `path` argument, else the bench root.
fn folder_arg(cx: &Ctx<'_>, shared: &Shared, args: &Args) -> KResult<PathBuf> {
    if let Some(path) = args.str("path") {
        return Ok(PathBuf::from(path));
    }
    lock(shared)
        .tree
        .root()
        .map(Path::to_path_buf)
        .ok_or_else(|| KernelError::message(cx.t("explorer.notice.no_bench")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use crate::state::lock;
    use forja_kernel::Registrar;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::time::Duration;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-explorer-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &Path) -> Runtime {
        Runtime::new(
            vec![Box::new(ExplorerModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("explorer registers cleanly")
    }

    /// Waits until `expected` task completions have run on this thread. Each disk
    /// operation yields exactly one completion, which records its history step.
    fn settle(runtime: &mut Runtime, completed: &mut usize, expected: usize) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
        while *completed < expected {
            *completed += runtime.poll_tasks();
            if *completed < expected {
                receiver.recv_timeout(Duration::from_secs(60)).expect("disk task did not signal completion");
            }
        }
    }

    fn wait_until(runtime: &mut Runtime, mut done: impl FnMut(&mut Runtime) -> bool) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
        loop {
            runtime.poll_tasks();
            if done(runtime) { return; }
            receiver.recv_timeout(Duration::from_secs(60)).expect("explorer task did not signal completion");
        }
    }

    fn args(path: &Path) -> Args {
        let mut args = Args::new();
        args.insert("path", serde_json::json!(path.display().to_string()));
        args
    }

    #[test]
    fn contract_holds_and_file_operations_undo_and_redo() {
        let dir = temp_dir("ops");
        let folder = dir.join("bench");
        std::fs::create_dir_all(&folder).unwrap();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let mut completed = 0usize;

        // Create a file with an explicit name (1 disk operation).
        let mut create = args(&folder);
        create.insert("name", serde_json::json!("notes.md"));
        runtime.run_command("explorer.new_file", create).unwrap();
        let created = folder.join("notes.md");
        settle(&mut runtime, &mut completed, 1);
        assert!(created.is_file());

        // Rename it (2).
        let mut rename = args(&created);
        rename.insert("name", serde_json::json!("plan.md"));
        runtime.run_command("explorer.rename", rename).unwrap();
        let renamed = folder.join("plan.md");
        settle(&mut runtime, &mut completed, 2);
        assert!(renamed.is_file() && !created.exists());

        // Undo the rename (3), redo it (4).
        runtime.run_command("explorer.undo", Args::new()).unwrap();
        settle(&mut runtime, &mut completed, 3);
        assert!(created.is_file() && !renamed.exists());
        runtime.run_command("explorer.redo", Args::new()).unwrap();
        settle(&mut runtime, &mut completed, 4);
        assert!(renamed.is_file() && !created.exists());

        // Undo the rename (5) and the creation (6): the folder is empty again.
        runtime.run_command("explorer.undo", Args::new()).unwrap();
        settle(&mut runtime, &mut completed, 5);
        runtime.run_command("explorer.undo", Args::new()).unwrap();
        settle(&mut runtime, &mut completed, 6);
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);

        // Nothing left to undo: a clear answer and no disk change.
        let value = runtime.run_command("explorer.undo", Args::new()).unwrap();
        assert_eq!(value["done"], serde_json::json!(false));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_creates_a_copy_next_to_the_original() {
        let dir = temp_dir("dup");
        let file = dir.join("data.txt");
        std::fs::write(&file, "payload").unwrap();
        let mut runtime = runtime(&dir);
        let mut completed = 0usize;
        runtime
            .run_command("explorer.duplicate", args(&file))
            .unwrap();
        settle(&mut runtime, &mut completed, 1);
        // The test runs in pt-BR, so the copy suffix comes from the Portuguese bundle.
        let copy = dir.join("data cópia.txt");
        assert_eq!(std::fs::read_to_string(&copy).unwrap(), "payload");
        let _ = std::fs::remove_dir_all(dir);
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

    #[test]
    fn the_panel_follows_the_active_bench_and_sees_new_files() {
        let dir = temp_dir("bench");
        let repo = dir.join("repo");
        std::fs::create_dir_all(repo.join("src")).unwrap();
        std::fs::create_dir_all(repo.join("target")).unwrap();
        std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(repo.join("README.md"), "hi").unwrap();

        let module = ExplorerModule::new();
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
        .expect("explorer and a bench stand-in register");

        {
            let mut cx = runtime.ctx("explorer");
            commands::sync(&mut cx, &shared);
        }
        let root = lock(&shared)
            .tree
            .root()
            .map(Path::to_path_buf)
            .expect("bench root");
        assert_eq!(root, repo);
        {
            let cx = runtime.ctx("explorer");
            commands::request_children(&cx, &shared, &root);
        }
        wait_until_listed(&mut runtime, &shared, &root, "README.md");
        let listing = lock(&shared).tree.children(&root).unwrap();
        let names: Vec<&str> = listing.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["src", ".gitignore", "README.md"],
            "folders first; target/ is ignored"
        );

        // The watcher is attached only after its background startup completes.
        wait_until(&mut runtime, |_| lock(&shared).watcher.is_some());
        std::fs::write(repo.join("fresh.txt"), "new").unwrap();
        wait_until(&mut runtime, |runtime| {
            let cached = lock(&shared).tree.children(&root)
                .is_some_and(|list| list.iter().any(|entry| entry.name == "fresh.txt"));
            if !cached && lock(&shared).tree.children(&root).is_none() {
                let cx = runtime.ctx("explorer");
                commands::request_children(&cx, &shared, &root);
            }
            cached
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    fn bench_ref(path: &Path) -> forja_contracts::BenchRef {
        forja_contracts::BenchRef {
            project_id: "p".into(),
            bench_id: "b".into(),
            name: "main".into(),
            path: path.display().to_string(),
            branch: None,
        }
    }

    /// Stands in for an editor: subscribes to `file.open_requested`.
    struct FakeEditor;

    impl Module for FakeEditor {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("editor", "editor.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.contract_event::<FileOpenRequested>(names::FILE_OPEN_REQUESTED);
            r.subscribe(|_cx, _event: &FileOpenRequested| {});
        }
    }

    #[test]
    fn bench_activation_events_move_the_panel_and_editor_listeners_are_detected() {
        let dir = temp_dir("events");
        let first = dir.join("first");
        let second = dir.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let module = ExplorerModule::new();
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
        .expect("explorer registers its contract events");

        // The panel follows bench.activated, which the benches module publishes.
        runtime.emit(&BenchActivated(bench_ref(&first)));
        assert_eq!(lock(&shared).tree.root(), Some(first.as_path()));
        runtime.emit(&BenchActivated(bench_ref(&second)));
        assert_eq!(lock(&shared).tree.root(), Some(second.as_path()));

        // Without an editor, file opens use the fallbacks (no subscriber for the event).
        {
            let cx = runtime.ctx("explorer");
            assert!(!commands::has_file_listener(&cx));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_editor_subscribed_to_file_open_requests_is_a_listener() {
        let dir = temp_dir("listener");
        let mut runtime = Runtime::new(
            vec![Box::new(ExplorerModule::new()), Box::new(FakeEditor)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("explorer and an editor stand-in register");
        {
            let cx = runtime.ctx("explorer");
            assert!(commands::has_file_listener(&cx));
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    fn wait_until_listed(runtime: &mut Runtime, shared: &Shared, dir: &Path, name: &str) {
        wait_until(runtime, |_| {
            lock(shared).tree.children(dir)
                .is_some_and(|list| list.iter().any(|entry| entry.name == name))
        });
    }

    #[test]
    fn copy_relative_path_falls_back_to_absolute_without_a_bench() {
        let dir = temp_dir("copy");
        let file = dir.join("src").join("lib.rs");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
        let mut runtime = runtime(&dir);
        let value = runtime
            .run_command("explorer.copy_relative_path", args(&file))
            .unwrap();
        assert_eq!(value["text"], serde_json::json!(file.display().to_string()));
        let _ = std::fs::remove_dir_all(dir);
    }
}
