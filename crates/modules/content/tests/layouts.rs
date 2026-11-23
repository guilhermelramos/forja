//! Testes do catálogo de arranjos: camadas, escrita, debounce e geração.

use forja_module_content::layouts::{
    read_catalog, read_limited, remove_layout, write_layout, Catalog, CatalogCache, LayoutSources,
    LayoutStore, WatchRoots, DEBOUNCE,
};
use notify::event::{CreateKind, Event, EventKind};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("forja-layouts-{label}-{nanos}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn layout_text(id: &str) -> String {
    format!(
        "schema_version = 1\nid = \"{id}\"\nname = {{ pt-BR = \"{id}\", en = \"{id}\" }}\n[chrome]\nrail_hidden = []\n[root]\nkind = \"leaf\"\nid = \"only\"\ntabs = [\"editor.tab\"]\n"
    )
}

fn write_file(root: &Path, layer: &str, id: &str, text: &str) -> PathBuf {
    let directory = match layer {
        "project" => root.join("project").join(".forja").join("layouts"),
        other => root.join(other).join("layouts"),
    };
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{id}.toml"));
    std::fs::write(&path, text).unwrap();
    path
}

fn sources(root: &Path, generation: u64, project: bool) -> LayoutSources {
    LayoutSources {
        builtin: Some(root.join("builtin")),
        user: root.join("user"),
        project: project.then(|| root.join("project")),
        generation,
    }
}

#[test]
fn three_layers_override_by_id_and_keep_their_origin() {
    let root = temp_dir("layers");
    write_file(&root, "builtin", "oficina", &layout_text("oficina"));
    write_file(&root, "builtin", "only-builtin", &layout_text("only-builtin"));
    write_file(&root, "user", "only-user", &layout_text("only-user"));
    write_file(&root, "project", "only-project", &layout_text("only-project"));
    write_file(&root, "user", "oficina", &layout_text("oficina"));
    write_file(&root, "project", "oficina", &layout_text("oficina"));

    let catalog = read_catalog(&sources(&root, 7, true), &Default::default());
    assert_eq!(catalog.generation, 7);
    assert_eq!(catalog.entries["oficina"].layer.label(), "project");
    assert_eq!(catalog.entries["only-builtin"].layer.label(), "builtin");
    assert_eq!(catalog.entries["only-user"].layer.label(), "user");
    assert_eq!(catalog.entries["only-project"].layer.label(), "project");
    assert!(catalog.diagnostics.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_invalid_file_keeps_the_last_valid_version() {
    let root = temp_dir("invalid");
    let path = write_file(&root, "user", "own", &layout_text("own"));
    let sources = sources(&root, 1, false);
    let first = read_catalog(&sources, &Default::default());
    assert!(first.entries.contains_key("own"));

    std::fs::write(&path, "schema_version = 99\nid = \"own\"\n").unwrap();
    let second = read_catalog(&sources, &CatalogCache {
        project: first.project.clone(),
        entries: first.entries.clone(),
        valid: first.valid.clone(),
    });
    assert_eq!(second.entries["own"].spec, first.entries["own"].spec);
    assert!(!second.diagnostics.is_empty());
    assert!(second.diagnostics[0].message.contains("version"));

    std::fs::write(
        root.join("user").join("layouts").join("bad.toml"),
        layout_text("wrong-id"),
    )
    .unwrap();
    let third = read_catalog(&sources, &CatalogCache {
        project: second.project.clone(),
        entries: second.entries.clone(),
        valid: second.valid.clone(),
    });
    assert!(!third.entries.contains_key("bad"));
    assert!(third.diagnostics.iter().any(|item| item.message.contains("declares id")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn oversized_layout_files_are_refused_at_the_descriptor() {
    let root = temp_dir("size-limit");
    let path = root.join("huge.toml");
    let mut text = layout_text("huge");
    text.push_str(&"x".repeat(forja_kernel::layout_spec::MAX_INPUT_BYTES));
    std::fs::write(&path, &text).unwrap();
    let limit = forja_kernel::layout_spec::MAX_INPUT_BYTES as u64;
    let error = read_limited(&path, limit).unwrap_err();
    assert!(error.contains("exceeds"), "{error}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn valid_versions_are_kept_per_file_layer_and_project() {
    let root = temp_dir("cache-layers");
    let project_a = root.join("project-a");
    let sources_a = LayoutSources {
        builtin: None,
        user: root.join("user"),
        project: Some(project_a.clone()),
        generation: 1,
    };
    let user_path = write_file(&root, "user", "own", &layout_text("own"));
    std::fs::create_dir_all(project_a.join(".forja").join("layouts")).unwrap();
    let project_path = project_a.join(".forja").join("layouts").join("own.toml");
    std::fs::write(&project_path, layout_text("own")).unwrap();

    let first = read_catalog(&sources_a, &Default::default());
    assert_eq!(first.entries["own"].layer.label(), "project");
    let cache = CatalogCache {
        project: first.project.clone(),
        entries: first.entries.clone(),
        valid: first.valid.clone(),
    };

    std::fs::write(&project_path, "schema_version = 99\nid = \"own\"\n").unwrap();
    let second = read_catalog(&sources_a, &cache);
    assert_eq!(second.entries["own"].layer.label(), "project");
    assert!(!second.diagnostics.is_empty());
    assert!(second
        .valid
        .keys()
        .any(|key| key.1.label() == "user" && key.2 == user_path));
    let cache = CatalogCache {
        project: second.project.clone(),
        entries: second.entries.clone(),
        valid: second.valid.clone(),
    };

    std::fs::remove_file(&project_path).unwrap();
    std::fs::write(&user_path, "schema_version = 99\nid = \"own\"\n").unwrap();
    let third = read_catalog(&sources_a, &cache);
    assert_eq!(third.entries["own"].layer.label(), "user", "hidden layer was retained");
    let cache = CatalogCache {
        project: third.project.clone(),
        entries: third.entries.clone(),
        valid: third.valid.clone(),
    };

    let project_b = root.join("project-b");
    std::fs::create_dir_all(project_b.join(".forja").join("layouts")).unwrap();
    std::fs::write(
        project_b.join(".forja").join("layouts").join("own.toml"),
        "schema_version = 99\nid = \"own\"\n",
    )
    .unwrap();
    let sources_b = LayoutSources {
        builtin: None,
        user: root.join("user"),
        project: Some(project_b),
        generation: 2,
    };
    let other = read_catalog(&sources_b, &cache);
    assert!(!other.entries.contains_key("own"), "project B cannot borrow project A cache");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn writing_creates_the_folder_is_atomic_and_respects_the_expected_hash() {
    let root = temp_dir("write");
    let folder = root.join("layouts");
    assert!(!folder.exists());
    let text = layout_text("own");
    let hash = write_layout(&root, "own", &text, None).unwrap();
    assert!(folder.is_dir());
    assert_eq!(std::fs::read_to_string(folder.join("own.toml")).unwrap(), text);
    assert!(write_layout(&root, "own", "bad", Some(&hash)).is_err());
    assert_eq!(std::fs::read_to_string(folder.join("own.toml")).unwrap(), text);
    assert!(write_layout(&root, "own", &text, None).is_ok());
    assert!(remove_layout(&root, "own", Some("stale")).is_err());
    assert!(remove_layout(&root, "own", None).is_ok());
    assert!(!folder.join("own.toml").exists());
    assert!(write_layout(&root, "../escape", "x", None).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn a_junction_cannot_redirect_the_layout_folder() {
    let root = temp_dir("junction");
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let link = root.join("layouts");
    let output = forja_kernel::process::command("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.display().to_string(),
            &outside.display().to_string(),
        ])
        .output()
        .expect("mklink");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        write_layout(&root, "own", &layout_text("own"), None).is_err(),
        "a junction must not redirect the layouts folder"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn debounce_coalesces_events_until_the_last_one() {
    let store = LayoutStore::new();
    let base = Instant::now();
    store.mark_dirty(base);
    assert!(!store.due(base + DEBOUNCE - Duration::from_millis(1)));
    assert!(store.due(base + DEBOUNCE));
    store.mark_dirty(base + Duration::from_millis(200));
    assert!(!store.due(base + Duration::from_millis(450)));
    assert!(store.due(base + Duration::from_millis(500)));
}

#[test]
fn a_stale_generation_or_project_is_never_applied() {
    let store = LayoutStore::new();
    let mut catalog = Catalog {
        generation: 0,
        project: None,
        entries: Default::default(),
        diagnostics: Vec::new(),
        default_id: None,
        revision: 0,
        valid: Default::default(),
    };
    assert!(store.apply(catalog.clone()));
    catalog.generation = 1;
    assert!(!store.apply(catalog.clone()));
    catalog.generation = 0;
    catalog.project = Some(PathBuf::from("elsewhere"));
    assert!(!store.apply(catalog));
    let _ = store.with(|state| {
        assert_eq!(state.catalog.revision, 1);
        assert!(state.ready);
    });
}

#[test]
fn a_disk_signal_invalidates_an_inflight_catalog() {
    let store = LayoutStore::new();
    let catalog = Catalog {
        generation: 0,
        project: None,
        entries: Default::default(),
        diagnostics: Vec::new(),
        default_id: None,
        revision: 0,
        valid: Default::default(),
    };
    assert!(store.apply(catalog.clone()));
    store.signal(Instant::now());
    assert!(!store.apply(catalog), "a read from before the event must be discarded");
    let _ = store.with(|state| {
        assert_eq!(state.generation, 1);
        assert!(!state.ready);
    });
}

#[test]
fn only_layout_files_and_folder_creation_are_relevant() {
    let root = temp_dir("watch-filter");
    let user = root.join("user");
    let project = root.join("project");
    std::fs::create_dir_all(user.join("layouts")).unwrap();
    let roots = WatchRoots::from_context(user.clone(), Some(project.clone()), None);

    let layout_event = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![user.join("layouts").join("own.toml")],
        attrs: Default::default(),
    };
    assert!(roots.relevant(&layout_event));
    let project_event = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![project.join(".forja").join("layouts").join("own.toml")],
        attrs: Default::default(),
    };
    assert!(roots.relevant(&project_event));
    let folder_event = Event {
        kind: EventKind::Create(CreateKind::Folder),
        paths: vec![project.join(".forja")],
        attrs: Default::default(),
    };
    assert!(roots.relevant(&folder_event));
    let unrelated = Event {
        kind: EventKind::Create(CreateKind::File),
        paths: vec![user.join("notes.txt")],
        attrs: Default::default(),
    };
    assert!(!roots.relevant(&unrelated));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn watch_roots_include_the_parents_of_the_layouts_folder() {
    let root = temp_dir("watch");
    std::fs::create_dir_all(root.join("user").join("layouts")).unwrap();
    std::fs::create_dir_all(root.join("project").join(".forja").join("layouts")).unwrap();
    let roots = sources(&root, 1, true).watch_roots();
    assert!(roots.iter().any(|path| path == &root.join("user")));
    assert!(roots.iter().any(|path| path == &root.join("user").join("layouts")));
    assert!(roots
        .iter()
        .any(|path| path == &root.join("project").join(".forja").join("layouts")));
    let _ = std::fs::remove_dir_all(root);
}
