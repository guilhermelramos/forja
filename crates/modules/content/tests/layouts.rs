//! Testes do catálogo de arranjos: camadas, escrita, debounce e geração.

use forja_module_content::layouts::{
    read_catalog, remove_layout, write_layout, Catalog, LayoutSources, LayoutStore, DEBOUNCE,
};
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
    let second = read_catalog(&sources, &first.entries);
    assert_eq!(second.entries["own"].spec, first.entries["own"].spec);
    assert!(!second.diagnostics.is_empty());
    assert!(second.diagnostics[0].message.contains("version"));

    std::fs::write(
        root.join("user").join("layouts").join("bad.toml"),
        layout_text("wrong-id"),
    )
    .unwrap();
    let third = read_catalog(&sources, &second.entries);
    assert!(!third.entries.contains_key("bad"));
    assert!(third.diagnostics.iter().any(|item| item.message.contains("declares id")));
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
