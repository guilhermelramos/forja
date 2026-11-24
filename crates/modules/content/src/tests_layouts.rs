use super::*;
use forja_kernel::ui_contract::View;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, Module, Registrar, Runtime, RuntimeOptions, SettingValue, Slot};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use super::tests::{runtime, temp_dir};
use crate::layouts::{LayoutSources, LAYOUT_DIR};
use crate::model::Layer;
use std::collections::BTreeMap;


    fn pump(runtime: &mut Runtime) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            runtime.poll_tasks();
            if runtime.tasks().pending() == 0 && runtime.tasks().inflight() == 0 {
                return;
            }
            assert!(Instant::now() < deadline, "tasks did not finish");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn listed_layouts(runtime: &mut Runtime) -> serde_json::Value {
        runtime
            .run_command("content.layouts.list", Args::new())
            .expect("layouts list")
    }

    #[test]
    fn layouts_are_loaded_written_and_removed_through_tasks() {
        let dir = temp_dir("layout-tasks");
        let mut runtime = runtime(&dir);
        let _ = listed_layouts(&mut runtime); // agenda a primeira leitura em Tasks
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        for id in ["oficina", "foco", "revisao", "mutirao"] {
            assert!(
                listed["layouts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| item["id"] == id && item["layer"] == "builtin"),
                "{listed}"
            );
        }

        let path = dir.join("user").join("layouts").join("oficina.toml");
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../content/layouts/oficina.toml"),
        )
        .unwrap();
        let mut args = Args::new();
        args.insert("id", serde_json::json!("oficina"));
        args.insert("layer", serde_json::json!("user"));
        args.insert("text", serde_json::json!(text));
        runtime
            .run_command("content.layouts.save", args)
            .expect("save layout");
        pump(&mut runtime);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        let listed = listed_layouts(&mut runtime);
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "oficina" && item["layer"] == "user"));

        std::fs::write(&path, "schema_version = 1\nid = \"oficina\"\nunknown = 1\n").unwrap();
        runtime
            .run_command("content.layouts.reload", Args::new())
            .expect("reload layouts");
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "oficina"));
        assert!(!listed["diagnostics"].as_array().unwrap().is_empty());

        let mut args = Args::new();
        args.insert("id", serde_json::json!("oficina"));
        args.insert("layer", serde_json::json!("user"));
        runtime
            .run_command("content.layouts.remove", args)
            .expect("remove layout");
        pump(&mut runtime);
        assert!(!path.exists());
        let listed = listed_layouts(&mut runtime);
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "oficina" && item["layer"] == "builtin"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn catalog_uses_the_registered_view_and_slot_snapshot() {
        struct SidebarStub;
        impl View for SidebarStub {
            fn ui(&mut self, _ui: &mut egui::Ui, _cx: &mut Ctx<'_>) {}
        }
        struct ViewsStub;
        impl Module for ViewsStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("test", "test.name", "0.1.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                r.view(
                    Slot::Sidebar,
                    "test.sidebar",
                    "test.name",
                    0,
                    || Box::new(SidebarStub) as Box<dyn View>,
                );
            }
        }
        let dir = temp_dir("view-snapshot");
        let user_layouts = dir.join("user").join("layouts");
        std::fs::create_dir_all(&user_layouts).unwrap();
        let text = |id: &str, view: &str| {
            format!(
                "schema_version = 1\nid = \"{id}\"\nname = {{ pt-BR = \"{id}\", en = \"{id}\" }}\n[root]\nkind = \"leaf\"\nid = \"only\"\ntabs = [\"{view}\"]\n"
            )
        };
        std::fs::write(user_layouts.join("dockable.toml"), text("dockable", "test.sidebar"))
            .unwrap();
        std::fs::write(
            user_layouts.join("fixed.toml"),
            text("fixed", "content.settings"),
        )
        .unwrap();
        let mut runtime = Runtime::new(
            vec![
                Box::new(ContentModule {
                    core: Arc::new(ContentCore::new()),
                }),
                Box::new(ViewsStub),
            ],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert_eq!(listed["known_views"]["test.sidebar"], "sidebar");
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "dockable"));
        assert!(!listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "fixed"));
        assert!(listed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["message"].as_str().unwrap().contains("non-dockable")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_saves_with_the_same_revision_only_one_wins() {
        let dir = temp_dir("concurrent-writes");
        let mut runtime = runtime(&dir);
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        let text = |name: &str| {
            format!(
                "schema_version = 1\nid = \"own\"\nname = {{ pt-BR = \"{name}\", en = \"{name}\" }}\n[root]\nkind = \"leaf\"\nid = \"only\"\n"
            )
        };
        fn save(runtime: &mut Runtime, text: String, expected: Option<&str>) {
            let mut args = Args::new();
            args.insert("id", serde_json::json!("own"));
            args.insert("layer", serde_json::json!("user"));
            args.insert("text", serde_json::json!(text));
            if let Some(hash) = expected {
                args.insert("expected_hash", serde_json::json!(hash));
            }
            runtime.run_command("content.layouts.save", args).expect("save");
        }
        save(&mut runtime, text("One"), None);
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        let before = listed["write_log"].as_array().unwrap().len();
        let hash = listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "own")
            .expect("saved layout")["hash"]
            .as_str()
            .unwrap()
            .to_owned();

        save(&mut runtime, text("Two"), Some(&hash));
        save(&mut runtime, text("Three"), Some(&hash));
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        let log = listed["write_log"].as_array().unwrap();
        let competing = &log[before..];
        let saved = competing.iter().filter(|item| item.as_str().unwrap().contains("salvo")).count();
        let failed = competing.iter().filter(|item| item.as_str().unwrap().contains("Falha")).count();
        assert_eq!(saved, 1, "one of the two competing saves must win: {competing:?}");
        assert_eq!(failed, 1, "the stale revision must be refused: {competing:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn two_benches_of_one_project_share_the_project_catalog() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = temp_dir("project-root");
        let project = dir.join("project");
        let layouts = project.join(".forja").join("layouts");
        std::fs::create_dir_all(&layouts).unwrap();
        std::fs::write(
            layouts.join("own.toml"),
            "schema_version = 1\nid = \"own\"\nname = { pt-BR = \"Own\", en = \"Own\" }\n[root]\nkind = \"leaf\"\nid = \"only\"\n",
        )
        .unwrap();
        let other_project = dir.join("project-b");
        let other_layouts = other_project.join(".forja").join("layouts");
        std::fs::create_dir_all(&other_layouts).unwrap();
        std::fs::write(
            other_layouts.join("other.toml"),
            "schema_version = 1\nid = \"other\"\nname = { pt-BR = \"Other\", en = \"Other\" }\n[root]\nkind = \"leaf\"\nid = \"only\"\n",
        )
        .unwrap();

        struct ProjectStub {
            roots: Vec<PathBuf>,
            current: Arc<AtomicUsize>,
        }
        impl Module for ProjectStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("project", "project.name", "0.1.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let roots = self
                    .roots
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>();
                let current = Arc::clone(&self.current);
                r.command("project.current", "project.name", move |_, _| {
                    let index = current.load(Ordering::SeqCst) % roots.len();
                    Ok(serde_json::json!({ "id": "p", "name": "P", "path": roots[index] }))
                });
                let current = Arc::clone(&self.current);
                r.command("project.switch", "project.name", move |_, _| {
                    current.fetch_add(1, Ordering::SeqCst);
                    Ok(serde_json::json!({ "ok": true }))
                });
            }
        }
        struct BenchStub {
            worktrees: Vec<PathBuf>,
            current: Arc<AtomicUsize>,
        }
        impl Module for BenchStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("bench", "bench.name", "0.1.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let worktrees = self.worktrees.clone();
                let current = Arc::clone(&self.current);
                r.command("bench.current", "bench.command.current", move |_, _| {
                    let index = current.load(Ordering::SeqCst) % worktrees.len();
                    Ok(serde_json::json!({ "path": worktrees[index].display().to_string() }))
                });
                let current = Arc::clone(&self.current);
                r.command("bench.switch", "bench.switch", move |_, _| {
                    current.fetch_add(1, Ordering::SeqCst);
                    Ok(serde_json::json!({ "ok": true }))
                });
            }
        }
        let project_current = Arc::new(AtomicUsize::new(0));
        let current = Arc::new(AtomicUsize::new(0));
        let mut runtime = Runtime::new(
            vec![
                Box::new(ContentModule {
                    core: Arc::new(ContentCore::new()),
                }),
                Box::new(ProjectStub {
                    roots: vec![project.clone(), other_project.clone()],
                    current: Arc::clone(&project_current),
                }),
                Box::new(BenchStub {
                    worktrees: vec![dir.join("bench-a"), dir.join("bench-b")],
                    current: Arc::clone(&current),
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        let first = listed_layouts(&mut runtime);
        let source = first["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "own")
            .expect("project layout")
            ["source"]
            .clone();
        assert_eq!(source, serde_json::json!(layouts.join("own.toml").display().to_string()));

        runtime
            .run_command("bench.switch", Args::new())
            .expect("switch bench");
        pump(&mut runtime);
        let second = listed_layouts(&mut runtime);
        assert_eq!(second["layouts"], first["layouts"], "benches share one project catalog");
        assert_eq!(second["layouts"].as_array().unwrap().len(), first["layouts"].as_array().unwrap().len());

        runtime
            .run_command("project.switch", Args::new())
            .expect("switch project");
        let switched = listed_layouts(&mut runtime);
        assert!(
            !switched["layouts"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == "own"),
            "the previous project snapshot must not be published"
        );
        pump(&mut runtime);
        let switched = listed_layouts(&mut runtime);
        assert!(switched["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "other" && item["layer"] == "project"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn real_watcher_reloads_creation_and_rename_and_stops_when_disabled() {
        let dir = temp_dir("real-watcher");
        let user_layouts = dir.join("user").join("layouts");
        std::fs::create_dir_all(&user_layouts).unwrap();
        let delays = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&delays);
        let core = Arc::new(ContentCore::new());
        let mut runtime = Runtime::new(
            vec![Box::new(ContentModule {
                core: Arc::clone(&core),
            })],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        runtime.set_wake_after(Arc::new(move |delay| recorded.lock().unwrap().push(delay)));
        let text = |id: &str| {
            format!(
                "schema_version = 1\nid = \"{id}\"\nname = {{ pt-BR = \"{id}\", en = \"{id}\" }}\n[root]\nkind = \"leaf\"\nid = \"only\"\n"
            )
        };
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        runtime.poll_ticks();
        pump(&mut runtime);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !core.layouts.with(|state| state.watch.is_some()).unwrap_or(false)
            && Instant::now() < deadline
        {
            runtime.poll_ticks();
            pump(&mut runtime);
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(core.layouts.with(|state| state.watch.is_some()).unwrap_or(false), "watcher must be installed");
        assert!(delays.lock().unwrap().is_empty(), "idle must not request frames");

        std::fs::write(user_layouts.join("own.toml"), text("own")).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        runtime.poll_ticks();
        assert_eq!(delays.lock().unwrap().len(), 1, "one deadline per event burst");
        assert!(!listed_layouts(&mut runtime)["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "own"));
        std::thread::sleep(Duration::from_millis(350));
        runtime.poll_ticks();
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "own"));

        std::fs::write(user_layouts.join("staged.toml"), text("second")).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        std::fs::rename(
            user_layouts.join("staged.toml"),
            user_layouts.join("second.toml"),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(750));
        runtime.poll_ticks();
        std::thread::sleep(Duration::from_millis(350));
        runtime.poll_ticks();
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert!(listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "second"), "atomic rename must reload");

        let _ = runtime
            .set_setting("content.reload.watch", SettingValue::Bool(false));
        std::fs::write(user_layouts.join("third.toml"), text("third")).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        runtime.poll_ticks();
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert!(!listed["layouts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "third"), "disabled watcher must not reload");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_default_layout_is_diagnosed() {
        let dir = temp_dir("default-diagnostic");
        let project = dir.join("project");
        std::fs::create_dir_all(project.join(".forja")).unwrap();
        struct ProjectStub(PathBuf);
        impl Module for ProjectStub {
            fn manifest(&self) -> ModuleManifest {
                ModuleManifest::new("project", "project.name", "0.1.0")
            }
            fn register(&mut self, r: &mut Registrar<'_>) {
                let root = self.0.display().to_string();
                r.command("project.current", "project.name", move |_, _| {
                    Ok(serde_json::json!({ "id": "p", "name": "P", "path": root }))
                });
            }
        }
        let mut runtime = Runtime::new(
            vec![
                Box::new(ContentModule {
                    core: Arc::new(ContentCore::new()),
                }),
                Box::new(ProjectStub(project.clone())),
            ],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert!(listed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["message"]
                .as_str()
                .unwrap()
                .contains("appearance.layout.default")));

        std::fs::write(
            project.join(".forja").join("ajustes.toml"),
            "[appearance.layout]\ndefault = \"foco\"\n",
        )
        .unwrap();
        runtime
            .run_command("content.layouts.reload", Args::new())
            .expect("reload");
        pump(&mut runtime);
        let listed = listed_layouts(&mut runtime);
        assert_eq!(listed["default_id"], "foco");
        assert!(!listed["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["message"]
                .as_str()
                .unwrap()
                .contains("appearance.layout.default")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn catalog_sources_are_built_from_the_stored_root_without_disk_probes() {
        // A UI monta as fontes com a raiz embutida já guardada. O caminho
        // injetado não existe: se a montagem consultasse o disco (is_dir/
        // exists/current_exe), ele seria descartado. Ele aparece intacto.
        let dir = temp_dir("sources-no-io");
        let injected = dir.join("builtin-inexistente");
        assert!(!injected.exists());
        let user = dir.join("user");
        let project = dir.join("project");
        let mut views = BTreeMap::new();
        views.insert("content.settings".to_owned(), "sidebar".to_owned());

        let sources = LayoutSources::from_resolved(
            Some(injected.clone()),
            user.clone(),
            Some(project.clone()),
            7,
            views.clone(),
        );
        assert_eq!(sources.builtin, Some(injected.clone()));
        assert_eq!(sources.views, views);
        assert!(sources.roots().contains(&(Layer::Builtin, injected.clone())));
        let watch = sources.watch_roots();
        assert!(watch.contains(&injected));
        assert!(watch.contains(&injected.join(LAYOUT_DIR)));

        // Sem raiz guardada, nenhuma fonte embutida é inventada.
        let none = LayoutSources::from_resolved(None, user, Some(project), 7, views);
        assert!(none.roots().iter().all(|(layer, _)| *layer != Layer::Builtin));
        assert!(!none.watch_roots().contains(&injected));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn builtin_root_is_resolved_once_into_the_state() {
        let dir = temp_dir("builtin-root-task");
        let core = Arc::new(ContentCore::new());
        let mut runtime = Runtime::new(
            vec![Box::new(ContentModule {
                core: Arc::clone(&core),
            })],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let _ = listed_layouts(&mut runtime);
        pump(&mut runtime);
        assert!(core.layouts.with(|state| state.builtin_root_pending).unwrap_or(false));
        assert_eq!(core.layouts.builtin_root(), crate::model::builtin_root_hint());
        let _ = std::fs::remove_dir_all(dir);
    }
