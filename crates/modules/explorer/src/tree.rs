//! Directory listing cache of the Files panel.
//!
//! [`TreeCache`] is pure state: it never touches the disk. Listings are read by
//! [`read_dir`] inside a kernel task and stored back with the generation they
//! were requested under, so a result for a bench that is no longer active is
//! dropped instead of shown.

use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Entries listed per folder. Larger folders are cut here and the rest is not shown.
pub const LIST_LIMIT: usize = 2_000;

/// The two view settings that change what a listing contains.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TreeOptions {
    pub show_ignored: bool,
    pub follow_symlinks: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub ignored: bool,
    pub is_symlink: bool,
}

/// `.gitignore` matchers per folder, shared with worker threads.
#[derive(Clone, Default)]
pub struct IgnoreCache {
    inner: Arc<Mutex<HashMap<PathBuf, Arc<Gitignore>>>>,
}

impl IgnoreCache {
    /// True when `path` is ignored by a `.gitignore` between `root` and its parent.
    pub fn is_ignored(&self, root: &Path, path: &Path, is_dir: bool) -> bool {
        let mut dir = path.parent();
        while let Some(current) = dir {
            if !current.starts_with(root) {
                break;
            }
            let matcher = self.matcher(current);
            if matcher
                .matched_path_or_any_parents(path, is_dir)
                .is_ignore()
            {
                return true;
            }
            if current == root {
                break;
            }
            dir = current.parent();
        }
        false
    }

    fn matcher(&self, dir: &Path) -> Arc<Gitignore> {
        let Ok(mut map) = self.inner.lock() else {
            return Arc::new(Gitignore::empty());
        };
        map.entry(dir.to_path_buf())
            .or_insert_with(|| Arc::new(build_matcher(dir)))
            .clone()
    }

    /// Forgets matchers, so a changed `.gitignore` is read again.
    pub fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }
}

fn build_matcher(dir: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(dir);
    let file = dir.join(".gitignore");
    if file.is_file() {
        let _ = builder.add(&file);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// Lists one folder: `.git` is always hidden, ignored entries follow the option,
/// symlinks are followed only when the option is on. Folders come first, then names.
pub fn read_dir(
    dir: &Path,
    root: &Path,
    options: TreeOptions,
    ignores: &IgnoreCache,
) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    for item in read.flatten() {
        let Ok(kind) = item.file_type() else {
            continue;
        };
        let path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        if name == ".git" {
            continue;
        }
        let is_symlink = kind.is_symlink();
        let (is_dir, is_file) = if is_symlink {
            if !options.follow_symlinks {
                continue;
            }
            match std::fs::metadata(&path) {
                Ok(meta) => (meta.is_dir(), meta.is_file()),
                Err(_) => continue,
            }
        } else {
            (kind.is_dir(), kind.is_file())
        };
        if !is_dir && !is_file {
            continue;
        }
        let ignored = ignores.is_ignored(root, &path, is_dir);
        if ignored && !options.show_ignored {
            continue;
        }
        items.push(Entry {
            path,
            name,
            is_dir,
            ignored,
            is_symlink,
        });
        if items.len() >= LIST_LIMIT {
            break;
        }
    }
    sort_entries(&mut items);
    items
}

/// Folders first, then case-insensitive name order.
pub fn sort_entries(items: &mut [Entry]) {
    items.sort_by(|a, b| {
        (!a.is_dir)
            .cmp(&!b.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });
}

/// Cached listings for the active root, plus which folders are loading.
#[derive(Default)]
pub struct TreeCache {
    root: Option<PathBuf>,
    options: TreeOptions,
    generation: u64,
    children: HashMap<PathBuf, Arc<Vec<Entry>>>,
    loading: HashSet<PathBuf>,
}

impl TreeCache {
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn options(&self) -> TreeOptions {
        self.options
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Switches the root. Returns false when the root did not change.
    pub fn set_root(&mut self, root: Option<PathBuf>) -> bool {
        if self.root == root {
            return false;
        }
        self.root = root;
        self.reset();
        true
    }

    /// Changes the options. Returns false when they did not change.
    pub fn set_options(&mut self, options: TreeOptions) -> bool {
        if self.options == options {
            return false;
        }
        self.options = options;
        self.reset();
        true
    }

    fn reset(&mut self) {
        self.generation += 1;
        self.children.clear();
        self.loading.clear();
    }

    pub fn children(&self, dir: &Path) -> Option<Arc<Vec<Entry>>> {
        self.children.get(dir).cloned()
    }

    /// Marks a folder as loading. Returns false if it is already cached or loading.
    pub fn begin_load(&mut self, dir: &Path) -> bool {
        if self.children.contains_key(dir) || self.loading.contains(dir) {
            return false;
        }
        self.loading.insert(dir.to_path_buf());
        true
    }

    /// Stores a finished listing. Results from an older generation are dropped.
    pub fn finish_load(&mut self, dir: &Path, generation: u64, entries: Vec<Entry>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.loading.remove(dir);
        self.children.insert(dir.to_path_buf(), Arc::new(entries));
        true
    }

    /// Drops one folder's listing so the next request reads it again.
    pub fn invalidate(&mut self, dir: &Path) -> bool {
        self.loading.remove(dir);
        self.children.remove(dir).is_some()
    }

    /// Drops every listing. Used by the refresh command.
    pub fn invalidate_all(&mut self) {
        self.reset();
    }

    #[cfg(test)]
    pub fn is_cached(&self, dir: &Path) -> bool {
        self.children.contains_key(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "forja-explorer-tree-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        std::fs::write(root.join("README.md"), "hi").unwrap();
        std::fs::write(root.join("debug.log"), "x").unwrap();
        std::fs::write(root.join("src").join("main.rs"), "fn main() {}").unwrap();
        root
    }

    fn names(items: &[Entry]) -> Vec<&str> {
        items.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn ignored_entries_are_hidden_unless_the_option_is_on() {
        let root = temp_repo();
        let ignores = IgnoreCache::default();
        let hidden = read_dir(&root, &root, TreeOptions::default(), &ignores);
        assert_eq!(names(&hidden), vec!["src", ".gitignore", "README.md"]);

        let shown = read_dir(
            &root,
            &root,
            TreeOptions {
                show_ignored: true,
                follow_symlinks: false,
            },
            &ignores,
        );
        let target = shown.iter().find(|e| e.name == "target").unwrap();
        assert!(target.ignored && target.is_dir);
        let log = shown.iter().find(|e| e.name == "debug.log").unwrap();
        assert!(log.ignored && !log.is_dir);
        assert!(shown.iter().all(|e| e.name != ".git"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn nested_gitignore_applies_to_its_own_folder() {
        let root = temp_repo();
        std::fs::write(root.join("src").join(".gitignore"), "generated.rs\n").unwrap();
        std::fs::write(root.join("src").join("generated.rs"), "").unwrap();
        let ignores = IgnoreCache::default();
        let options = TreeOptions {
            show_ignored: true,
            follow_symlinks: false,
        };
        let inner = read_dir(&root.join("src"), &root, options, &ignores);
        let generated = inner.iter().find(|e| e.name == "generated.rs").unwrap();
        assert!(generated.ignored);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn folders_sort_before_files_and_names_ignore_case() {
        let mut items = vec![
            Entry {
                path: PathBuf::from("b.txt"),
                name: "b.txt".into(),
                is_dir: false,
                ignored: false,
                is_symlink: false,
            },
            Entry {
                path: PathBuf::from("Zeta"),
                name: "Zeta".into(),
                is_dir: true,
                ignored: false,
                is_symlink: false,
            },
            Entry {
                path: PathBuf::from("alpha"),
                name: "alpha".into(),
                is_dir: true,
                ignored: false,
                is_symlink: false,
            },
        ];
        sort_entries(&mut items);
        assert_eq!(names(&items), vec!["alpha", "Zeta", "b.txt"]);
    }

    #[test]
    fn stale_loads_are_dropped_after_the_root_changes() {
        let mut cache = TreeCache::default();
        cache.set_root(Some(PathBuf::from("/one")));
        let dir = PathBuf::from("/one");
        assert!(cache.begin_load(&dir));
        assert!(
            !cache.begin_load(&dir),
            "a second request does not start twice"
        );
        let old = cache.generation();
        cache.set_root(Some(PathBuf::from("/two")));
        assert!(
            !cache.finish_load(&dir, old, Vec::new()),
            "old root result is ignored"
        );
        assert!(!cache.is_cached(&dir));
        assert!(
            !cache.set_root(Some(PathBuf::from("/two"))),
            "same root is a no-op"
        );
    }

    #[test]
    fn invalidate_forgets_one_folder_and_options_reset_everything() {
        let mut cache = TreeCache::default();
        cache.set_root(Some(PathBuf::from("/r")));
        let dir = PathBuf::from("/r");
        cache.begin_load(&dir);
        let generation = cache.generation();
        cache.finish_load(&dir, generation, Vec::new());
        assert!(cache.is_cached(&dir));
        assert!(cache.invalidate(&dir));
        assert!(!cache.is_cached(&dir));
        cache.finish_load(&dir, generation, Vec::new());
        assert!(cache.set_options(TreeOptions {
            show_ignored: true,
            follow_symlinks: false
        }));
        assert!(!cache.is_cached(&dir));
    }
}
