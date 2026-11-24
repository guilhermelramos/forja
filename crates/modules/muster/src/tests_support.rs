//! Suporte compartilhado pelos testes do Mutirão: stubs e helpers.

use super::*;
pub(crate) use forja_contracts::{
    names, BenchCreateFailed, BenchCreated, BenchRef, BenchStoreOutcome, TerminalExited, TerminalRef,
};
pub(crate) use forja_kernel::{contract, Runtime, RuntimeOptions};
pub(crate) use serde_json::{json, Map};
pub(crate) use std::path::PathBuf;
pub(crate) use std::sync::Arc;

    pub(crate) struct TempGuard(pub(crate) PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    pub(crate) fn temp_dir() -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-muster-module-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    pub(crate) fn runtime_with(dir: &std::path::Path, shared: Shared) -> Runtime {
        Runtime::new(
            vec![Box::new(MusterModule { shared })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster module registers")
    }

    pub(crate) fn runtime(dir: &std::path::Path) -> Runtime {
        runtime_with(dir, state::new_shared())
    }

    pub(crate) struct StubShell {
        pub(crate) calls: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Module for StubShell {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("shell", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let calls = Arc::clone(&self.calls);
            r.command("shell.rail.open", "muster.command.open", move |_, args| {
                if let Some(view) = args.str("view_id") {
                    calls.lock().unwrap().push(view.to_owned());
                }
                Ok(json!({ "opening": true }))
            });
        }
    }

    #[test]
    fn muster_open_queues_the_rail_page() {
        let dir = temp_dir();
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubShell {
                    calls: Arc::clone(&calls),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.0.clone()),
                user_dir: Some(dir.0.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster and shell register");
        runtime
            .run_command("muster.open", forja_kernel::Args::new())
            .expect("open");
        assert_eq!(calls.lock().unwrap().as_slice(), ["muster.page"]);
    }

    pub(crate) struct StubHelper {
        pub(crate) profiles: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) paths: Arc<std::sync::Mutex<Vec<String>>>,
    }

    impl Module for StubHelper {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("helper", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let profiles = Arc::clone(&self.profiles);
            let paths = Arc::clone(&self.paths);
            r.command("helper.launch", "muster.command.start", move |_, args| {
                let profile = args.str("profile_id").unwrap_or("auto").to_owned();
                profiles.lock().unwrap().push(profile.clone());
                paths
                    .lock()
                    .unwrap()
                    .push(args.str("bench_path").unwrap_or("").to_owned());
                Ok(json!({
                    "profile_id": profile,
                    "terminal_id": "9",
                    "mode": "tui"
                }))
            });
        }
    }

    pub(crate) struct StubBench {
        pub(crate) current: String,
        pub(crate) created: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) stored: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) benches: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) dirty_calls: Arc<std::sync::Mutex<usize>>,
        pub(crate) dirty: Arc<std::sync::Mutex<std::collections::BTreeMap<String, bool>>>,
    }

    impl Module for StubBench {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.contract_event::<BenchStoreOutcome>(names::BENCH_STORE_OUTCOME);
            let current = self.current.clone();
            r.command("bench.current", "muster.command.status", move |_, _| {
                Ok(json!({
                    "project_id": "p1",
                    "bench_id": "bench-current",
                    "name": "atual",
                    "path": current
                }))
            });

            let created = Arc::clone(&self.created);
            let benches = Arc::clone(&self.benches);
            r.command("bench.create", "muster.command.start", move |_, args| {
                let name = args.required_str("name")?.to_owned();
                created.lock().unwrap().push(name.clone());
                benches.lock().unwrap().push("bench-new".to_owned());
                Ok(json!({
                    "project_id": "p1",
                    "bench_id": "bench-new",
                    "name": name,
                    "path": format!("C:/bancadas/{name}")
                }))
            });

            let stored = Arc::clone(&self.stored);
            let benches = Arc::clone(&self.benches);
            let dirty = Arc::clone(&self.dirty);
            r.command("bench.store", "muster.command.start", move |cx, args| {
                let id = args.required_str("bench_id")?.to_owned();
                let is_dirty = dirty.lock().unwrap().get(&id).copied().unwrap_or(false);
                if is_dirty {
                    // O serviço real barra aqui, dentro da tarefa que apaga.
                    cx.emit(&BenchStoreOutcome {
                        bench_id: id,
                        outcome: "kept_dirty".to_owned(),
                    });
                } else {
                    stored.lock().unwrap().push(id.clone());
                    benches.lock().unwrap().retain(|item| item != &id);
                    cx.emit(&BenchStoreOutcome {
                        bench_id: id,
                        outcome: "stored".to_owned(),
                    });
                }
                Ok(json!({ "pending": true }))
            });

            // Armadilha: se a limpeza chamar is_dirty na thread da UI, o teste falha.
            let dirty_calls = Arc::clone(&self.dirty_calls);
            r.command("bench.is_dirty", "muster.command.status", move |_, args| {
                let _ = args.required_str("bench_id")?;
                *dirty_calls.lock().unwrap() += 1;
                Ok(json!({ "dirty": false }))
            });
        }
    }

    pub(crate) struct StubUsage {
        pub(crate) usd: f64,
    }

    impl Module for StubUsage {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("usage", "muster.name", "0.0.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let usd = self.usd;
            r.command("usage.spend", "muster.command.spend", move |_, _| {
                Ok(json!({
                    "period": "all",
                    "project": "C:/bench",
                    "matched_projects": 1,
                    "usd": usd,
                    "tokens": 100,
                    "unpriced_tokens": 0,
                    "priced": true,
                    "pending": false
                }))
            });
        }
    }

    /// Lê os ids do catálogo do ajudante direto do fonte dele: o módulo
    /// `helper` não publica API com essa lista e não pode virar dependência do
    /// Mutirão. Se um lado mudar, este teste quebra.
    pub(crate) fn helper_profile_ids() -> Vec<String> {
        let source = include_str!("../../helper/src/lib.rs");
        let start = source
            .find("\"helper.default.profile\"")
            .expect("helper.default.profile");
        let block = &source[start..];
        let open = block.find("options: [").expect("helper options");
        let rest = &block[open + "options: [".len()..];
        let close = rest.find(']').expect("helper options end");
        rest[..close]
            .split(',')
            .map(str::trim)
            .filter_map(|item| item.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn bench_runtime(dir: &std::path::Path) -> (Runtime, BenchStubs) {
        let stubs = BenchStubs::default();
        let runtime = Runtime::new(
            vec![
                Box::new(MusterModule::new()),
                Box::new(StubHelper {
                    profiles: Arc::clone(&stubs.profiles),
                    paths: Arc::clone(&stubs.paths),
                }),
                Box::new(StubBench {
                    current: "C:/atual".to_owned(),
                    created: Arc::clone(&stubs.created),
                    stored: Arc::clone(&stubs.stored),
                    benches: Arc::clone(&stubs.benches),
                    dirty_calls: Arc::clone(&stubs.dirty_calls),
                    dirty: Arc::clone(&stubs.dirty),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("muster, helper and bench register");
        (runtime, stubs)
    }

    /// Drena as tarefas em segundo plano até não haver trabalho pendente.
    pub(crate) fn settle(runtime: &mut Runtime) {
        let (sender, receiver) = std::sync::mpsc::channel();
        runtime.set_wake(Arc::new(move || {
            let _ = sender.send(());
        }));
        loop {
            runtime.poll_tasks();
            if runtime.tasks().inflight() == 0 {
                runtime.poll_tasks();
                return;
            }
            let _ = receiver.recv_timeout(std::time::Duration::from_secs(10));
        }
    }

    #[derive(Default)]
    pub(crate) struct BenchStubs {
        pub(crate) profiles: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) paths: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) created: Arc<std::sync::Mutex<Vec<String>>>,
        pub(crate) stored: Arc<std::sync::Mutex<Vec<String>>>,
        /// Bancadas existentes no serviço falso, incluindo as que não são do mutirão.
        pub(crate) benches: Arc<std::sync::Mutex<Vec<String>>>,
        /// Número de chamadas a `bench.is_dirty`; a limpeza não deve chamar nada disso.
        pub(crate) dirty_calls: Arc<std::sync::Mutex<usize>>,
        pub(crate) dirty: Arc<std::sync::Mutex<std::collections::BTreeMap<String, bool>>>,
    }

    pub(crate) fn new_run_and_task(runtime: &mut Runtime) -> (String, String) {
        let run = runtime
            .call_method(
                "muster.run",
                Map::from_iter([("title".to_owned(), json!("Despacho"))]),
            )
            .unwrap();
        let task = runtime
            .call_method(
                "muster.task",
                Map::from_iter([
                    ("run".to_owned(), run["id"].clone()),
                    ("title".to_owned(), json!("Tarefa")),
                ]),
            )
            .unwrap();
        (
            run["id"].as_str().unwrap().to_owned(),
            task["id"].as_str().unwrap().to_owned(),
        )
    }

    pub(crate) fn finish_new_bench_dispatch(runtime: &mut Runtime, task_id: &str) {
        let ready = BenchCreated(BenchRef {
            project_id: "p1".to_owned(),
            bench_id: "bench-new".to_owned(),
            name: format!("muster-{task_id}"),
            path: "C:/bancadas/pronta".to_owned(),
            branch: None,
        });
        runtime.emit(&ready);
        settle(runtime);
    }

