    use super::*;
    use crate::view::ContentAnvilView;
    use forja_kernel::ui_contract::View;
    use forja_kernel::{contract, Module, Registrar, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;

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

    pub(super) fn temp_dir(label: &str) -> PathBuf {
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

    pub(super) fn runtime(dir: &std::path::Path) -> Runtime {
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

