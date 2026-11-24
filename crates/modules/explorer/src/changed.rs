//! Changed-file marks for the Files tree. Paths come from the git status event
//! the git module publishes, so the panel never reads git or the disk itself.

use crate::state::{lock, Shared};
use forja_contracts::GitStatusChanged;
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;
use std::collections::BTreeSet;
use std::path::Path;

fn key(path: &str) -> String {
    path.trim().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Accepts a status only for the bench whose files the tree is showing.
pub(crate) fn apply(shared: &Shared, event: &GitStatusChanged) {
    let mut state = lock(shared);
    let Some(root) = state.tree.root().map(Path::to_path_buf) else {
        return;
    };
    if key(&root.to_string_lossy()) != key(&event.bench_path) {
        return;
    }
    state.changed_paths = event
        .files
        .iter()
        .map(|path| key(path))
        .filter(|path| !path.is_empty())
        .collect();
}

/// True for a changed file, or for a folder containing one.
pub(crate) fn is_changed(root: &Path, entry: &Path, changed: &BTreeSet<String>) -> bool {
    let Ok(relative) = entry.strip_prefix(root) else {
        return false;
    };
    let path = key(&relative.to_string_lossy());
    changed.iter().any(|changed| {
        changed == &path || changed.starts_with(&format!("{path}/"))
    })
}

/// One quiet tag per changed row. The glyph is language-neutral; the tooltip
/// carries the translated explanation so a screen reader can name it.
pub(crate) fn render(ui: &mut egui::Ui, cx: &Ctx<'_>, changed: bool) {
    if !changed {
        return;
    }
    forja_ui_kit::tag(ui, &cx.t("explorer.tree.modified"))
        .on_hover_text(cx.t("explorer.tree.modified-hint"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn files_and_their_parent_folders_are_marked() {
        let changed: BTreeSet<String> = ["src/login.ts".to_owned(), "docs/guia.md".to_owned()]
            .into_iter()
            .collect();
        let root = PathBuf::from("C:/bancadas/relatorio");
        assert!(is_changed(&root, &root.join("src/login.ts"), &changed));
        assert!(is_changed(&root, &root.join("src"), &changed));
        assert!(is_changed(&root, &root.join("docs"), &changed));
        assert!(!is_changed(&root, &root.join("README.md"), &changed));
        assert!(!is_changed(&root, &PathBuf::from("C:/outra/src/login.ts"), &changed));
    }

    #[test]
    fn status_of_another_bench_is_ignored_and_reset_clears_marks() {
        let shared: Shared = std::sync::Arc::new(std::sync::Mutex::new(crate::state::State::default()));
        lock(&shared).tree.set_root(Some(PathBuf::from("C:/bancadas/relatorio")));
        let event = |path: &str| GitStatusChanged {
            bench_path: path.to_owned(),
            summary: forja_contracts::GitSummary::default(),
            files: vec!["src/login.ts".to_owned()],
        };
        apply(&shared, &event("C:/bancadas/outra"));
        assert!(lock(&shared).changed_paths.is_empty());
        apply(&shared, &event("c:/bancadas/relatorio/"));
        assert_eq!(lock(&shared).changed_paths.len(), 1);
        lock(&shared).changed_paths.clear();
        assert!(lock(&shared).changed_paths.is_empty());
    }
}
