//! Módulo `content` da Forja.
//!
//! Conteúdo como arquivo: comandos, temas, modelos e receitas das camadas
//! embutida, usuária e de projeto. A leitura é validada, o `trust` decide o que
//! executa e `notify` recarrega depois de 300 ms sem mudanças.

mod commands;
pub mod layouts;
mod model;
mod state;
mod view;

use forja_kernel::settings::{SettingKind, SettingSpec, SettingValue};
use forja_kernel::{Module, ModuleManifest, Registrar, Slot};
use state::ContentCore;
use std::sync::Arc;
use view::{ContentAnvilView, ContentSettingsView};

/// Ponteiro compartilhado do módulo.
pub(crate) type Shared = Arc<ContentCore>;

#[forja_kernel::module]
static CONTENT: fn() -> Box<dyn Module> = || {
    Box::new(ContentModule {
        core: Arc::new(ContentCore::new()),
    })
};

/// Módulo content.
pub struct ContentModule {
    core: Shared,
}

impl Module for ContentModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("content", "content.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.setting(SettingSpec::new(
            "content.reload.watch",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "content.setting.reload-watch.label",
            "content.setting.reload-watch.help",
        ));
        r.setting(SettingSpec::new(
            "content.trust.review_on_change",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "content.setting.trust-review.label",
            "content.setting.trust-review.help",
        ));
        commands::register(r, &self.core);
        layouts::register(r, &self.core.layouts);
        for key in [
            "content.settings.title",
            "content.settings.reload",
            "content.settings.diagnostics",
            "content.settings.no-diagnostics",
            "content.settings.commands",
            "content.settings.themes",
            "content.settings.templates",
            "content.settings.recipes",
            "content.settings.layouts",
        ] {
            r.uses_i18n(key);
        }
        let anvil_core = Arc::clone(&self.core);
        r.view(
            Slot::AnvilSource,
            "content.commands",
            "content.anvil.title",
            30,
            move || Box::new(ContentAnvilView::new(Arc::clone(&anvil_core))),
        );
        let settings_core = Arc::clone(&self.core);
        r.view(
            Slot::SettingsSection,
            "content.settings",
            "content.settings.title",
            40,
            move || Box::new(ContentSettingsView::new(Arc::clone(&settings_core))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::ContentAnvilView;
    use forja_kernel::ui_contract::View;
    use forja_kernel::{contract, Args, Module, Registrar, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    struct BenchModule {
        path: PathBuf,
    }

    impl Module for BenchModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("bench", "bench.name", "0.1.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            let path = self.path.display().to_string();
            r.command("bench.current", "bench.command.current", move |_, _| {
                Ok(serde_json::json!({ "path": path }))
            });
        }
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-content-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(ContentModule {
                core: Arc::new(ContentCore::new()),
            })],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("content module registers")
    }

    #[test]
    fn commands_from_a_project_appear_in_the_contract() {
        let dir = temp_dir("commands");
        let bench = dir.join("bench");
        let commands = bench.join(".forja").join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        std::fs::write(
            commands.join("review.md"),
            "+++\ntitle = { pt-BR = \"Revisar\", en = \"Review\" }\nrun = \"helper\"\n+++\nRevise o diff.\n",
        )
        .unwrap();
        let mut runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let mut args = Map::new();
        args.insert("project_path".to_owned(), serde_json::json!(bench.display().to_string()));
        let listed = runtime
            .call_method("content.commands.list", args)
            .expect("list");
        let items = listed["commands"].as_array().expect("array");
        assert!(items
            .iter()
            .any(|item| item["id"] == "cmd.review" && item["layer"] == "project"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn templates_render_known_fields_and_keep_unknown_ones() {
        let dir = temp_dir("templates");
        let templates = dir.join("user").join("templates");
        std::fs::create_dir_all(&templates).unwrap();
        std::fs::write(
            templates.join("pr.md"),
            "+++\nname = \"PR\"\nuse = \"pr\"\n+++\n## {{summary}}\n{{tests}}\n",
        )
        .unwrap();
        let mut runtime = runtime(&dir);
        let mut args = Map::new();
        args.insert("id".to_owned(), serde_json::json!("pr"));
        args.insert(
            "fields".to_owned(),
            serde_json::json!({ "summary": "feito", "tests": "cargo test" }),
        );
        let rendered = runtime
            .call_method("content.templates.render", args)
            .expect("render");
        assert_eq!(rendered["text"], "## feito\ncargo test\n");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn risky_project_command_is_blocked_without_trust() {
        let dir = temp_dir("trust");
        let bench = dir.join("bench");
        let commands = bench.join(".forja").join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        std::fs::write(
            commands.join("deploy.md"),
            "+++\ntitle = \"Deploy\"\nrun = \"terminal\"\n+++\necho deploy\n",
        )
        .unwrap();
        let mut runtime = runtime(&dir);
        let mut args = Map::new();
        args.insert("project_path".to_owned(), serde_json::json!(bench.display().to_string()));
        let listed = runtime
            .call_method("content.commands.list", args)
            .expect("list");
        let item = listed["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"] == "cmd.deploy")
            .expect("command");
        assert_eq!(item["blocked"], true);
        assert_eq!(item["block_reason"], "trust");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn builtin_commands_are_listed_without_a_project() {
        let dir = temp_dir("builtin");
        let mut runtime = runtime(&dir);
        let listed = runtime
            .call_method("content.commands.list", Map::new())
            .expect("list");
        let items = listed["commands"].as_array().expect("array");
        assert!(items
            .iter()
            .any(|item| item["id"] == "cmd.review-bench" && item["layer"] == "builtin"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn anvil_source_offers_a_markdown_command() {
        let dir = temp_dir("anvil");
        let bench = dir.join("bench");
        let commands = bench.join(".forja").join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        std::fs::write(
            commands.join("review.md"),
            "+++\ntitle = { pt-BR = \"Revisar\", en = \"Review\" }\nrun = \"helper\"\n+++\nRevise.\n",
        )
        .unwrap();
        let core = Arc::new(ContentCore::new());
        let modules: Vec<Box<dyn Module>> = vec![
            Box::new(ContentModule { core: Arc::clone(&core) }),
            Box::new(BenchModule { path: bench.clone() }),
        ];
        let mut runtime = Runtime::new(
            modules,
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let mut view = ContentAnvilView::new(core);
        let mut cx = runtime.ctx("content");
        let hits = view.anvil_hits("revisar", &mut cx);
        assert!(hits.iter().any(|hit| hit.command == "content.run"));
        assert!(hits.iter().any(|hit| hit.title == "Revisar"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_edit_keeps_the_previous_command() {
        let dir = temp_dir("broken");
        let bench = dir.join("bench");
        let commands = bench.join(".forja").join("commands");
        std::fs::create_dir_all(&commands).unwrap();
        let path = commands.join("review.md");
        std::fs::write(
            &path,
            "+++\ntitle = \"Review\"\nrun = \"helper\"\n+++\nRevise.\n",
        )
        .unwrap();
        let mut runtime = Runtime::new(
            vec![
                Box::new(ContentModule {
                    core: Arc::new(ContentCore::new()),
                }),
                Box::new(BenchModule { path: bench.clone() }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.join("data")),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime");
        let mut args = Map::new();
        args.insert("project_path".to_owned(), serde_json::json!(bench.display().to_string()));
        runtime
            .call_method("content.commands.list", args.clone())
            .expect("first list");
        std::fs::write(&path, "+++\ntitle = \"Review\"\n+++\n").unwrap();
        runtime
            .call_method("content.reload", Map::new())
            .expect("reload");
        let listed = runtime.call_method("content.commands.list", args).expect("list");
        assert!(
            listed["commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["id"] == "cmd.review"),
            "{listed}"
        );
        assert!(
            !listed["diagnostics"].as_array().unwrap().is_empty(),
            "{listed}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn theme_toml_from_a_project_is_listed() {
        let dir = temp_dir("themes");
        let bench = dir.join("bench");
        let themes = bench.join(".forja").join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(
            themes.join("own.toml"),
            "id = \"own\"\nname = \"Own\"\nbackground = \"#000000\"\n[ansi]\nblack = \"#000000\"\n",
        )
        .unwrap();
        let mut runtime = runtime(&dir);
        let mut args = Map::new();
        args.insert("project_path".to_owned(), serde_json::json!(bench.display().to_string()));
        let listed = runtime
            .call_method("content.themes.list", args)
            .expect("themes");
        assert!(listed["themes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|theme| theme["id"] == "own" && theme["kind"] == "terminal"));
        let _ = std::fs::remove_dir_all(dir);
    }

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
        let current = Arc::new(AtomicUsize::new(0));
        let mut runtime = Runtime::new(
            vec![
                Box::new(ContentModule {
                    core: Arc::new(ContentCore::new()),
                }),
                Box::new(ProjectStub(project.clone())),
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
        let _ = std::fs::remove_dir_all(dir);
    }
}
