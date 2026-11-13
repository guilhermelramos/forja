//! File operations of the Files panel and the undo/redo history that goes with them.
//!
//! Every function here touches the disk. The module calls them only from kernel
//! tasks (`Ctx::spawn`), never from the interface thread. Each reversible action
//! is symmetric: [`undo`] and [`redo`] take the same [`Action`] and invert it, so
//! the history keeps one value per step.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Steps kept on the undo stack. Older steps are dropped.
pub const HISTORY_LIMIT: usize = 100;

/// One reversible step on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// A new empty file or folder at `path`.
    Created { path: PathBuf, is_dir: bool },
    /// `from` was renamed to `to`.
    Renamed { from: PathBuf, to: PathBuf },
    /// `source` was copied to `copy`. Undo removes the copy.
    Duplicated { source: PathBuf, copy: PathBuf },
}

/// Undo and redo stacks. A new action clears the redo stack, as in any editor.
#[derive(Clone, Debug, Default)]
pub struct History {
    undo: Vec<Action>,
    redo: Vec<Action>,
}

impl History {
    /// Records an action that just happened on disk.
    pub fn record(&mut self, action: Action) {
        self.redo.clear();
        self.undo.push(action);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Takes the newest undo step. Give it back with [`History::push_undo`] if the disk step fails.
    pub fn take_undo(&mut self) -> Option<Action> {
        self.undo.pop()
    }

    pub fn take_redo(&mut self) -> Option<Action> {
        self.redo.pop()
    }

    pub fn push_undo(&mut self, action: Action) {
        self.undo.push(action);
    }

    pub fn push_redo(&mut self, action: Action) {
        self.redo.push(action);
    }

    #[cfg(test)]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

/// Why a name typed by the user cannot be used as a file name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    Invalid,
}

/// Checks a single path component. Separators, reserved Windows characters,
/// and `.` / `..` are refused so a name cannot escape its folder.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(NameError::Empty);
    }
    if trimmed == "." || trimmed == ".." {
        return Err(NameError::Invalid);
    }
    let bad = trimmed.chars().any(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    });
    if bad {
        return Err(NameError::Invalid);
    }
    Ok(())
}

/// Joins a folder and a name that already passed [`validate_name`].
pub fn join(dir: &Path, name: &str) -> PathBuf {
    dir.join(name.trim())
}

/// Creates an empty file or folder. Refuses to overwrite anything.
pub fn create(path: &Path, is_dir: bool) -> io::Result<Action> {
    create_entry(path, is_dir)?;
    Ok(Action::Created {
        path: path.to_path_buf(),
        is_dir,
    })
}

/// Renames `from` to `to`. Refuses to replace a different existing entry.
/// A change of letter case alone is allowed on case-insensitive file systems.
pub fn rename(from: &Path, to: &Path) -> io::Result<Action> {
    if from == to {
        return Ok(Action::Renamed {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
        });
    }
    let case_only = from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
    if to.exists() && !case_only {
        return Err(already_exists(to));
    }
    fs::rename(from, to)?;
    Ok(Action::Renamed {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
    })
}

/// Copies `source` next to itself under a free name built from `suffix`
/// (for example `notes copy.md`). Folders are copied recursively.
pub fn duplicate(source: &Path, suffix: &str) -> io::Result<Action> {
    let copy = unique_copy_path(source, suffix)?;
    copy_tree(source, &copy)?;
    Ok(Action::Duplicated {
        source: source.to_path_buf(),
        copy,
    })
}

/// Applies the inverse of `action`. Used by the undo command.
pub fn undo(action: &Action) -> io::Result<()> {
    match action {
        Action::Created { path, is_dir } => {
            if *is_dir {
                fs::remove_dir(path)
            } else {
                fs::remove_file(path)
            }
        }
        Action::Renamed { from, to } => fs::rename(to, from),
        Action::Duplicated { source, copy } => {
            if !matches_copy(source, copy)? {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "duplicate changed since it was created; refusing to remove its data",
                ));
            }
            if copy.is_dir() {
                fs::remove_dir_all(copy)
            } else {
                fs::remove_file(copy)
            }
        }
    }
}

/// Re-applies `action` after an undo. Used by the redo command.
pub fn redo(action: &Action) -> io::Result<()> {
    match action {
        Action::Created { path, is_dir } => create_entry(path, *is_dir),
        Action::Renamed { from, to } => {
            if to.exists()
                && !(from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase())
            {
                return Err(already_exists(to));
            }
            fs::rename(from, to)
        }
        Action::Duplicated { source, copy } => copy_tree(source, copy),
    }
}

/// Finds a free sibling name: `name{suffix}.ext`, then `name{suffix} 2.ext`, and so on.
pub fn unique_copy_path(source: &Path, suffix: &str) -> io::Result<PathBuf> {
    let parent = source
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no parent"))?;
    let is_dir = source.is_dir();
    let stem = if is_dir {
        source.file_name().map(|n| n.to_string_lossy().into_owned())
    } else {
        source.file_stem().map(|n| n.to_string_lossy().into_owned())
    }
    .unwrap_or_default();
    let ext = if is_dir {
        None
    } else {
        source.extension().map(|e| e.to_string_lossy().into_owned())
    };
    let mut index = 1usize;
    loop {
        let base = if index == 1 {
            format!("{stem}{suffix}")
        } else {
            format!("{stem}{suffix} {index}")
        };
        let name = match &ext {
            Some(ext) => format!("{base}.{ext}"),
            None => base,
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return Ok(candidate);
        }
        index += 1;
        if index > 10_000 {
            return Err(already_exists(&candidate));
        }
    }
}

/// Copies a file or folder tree. Symbolic links are skipped, never followed,
/// so a link cannot make the copy escape the folder or loop.
pub fn copy_tree(source: &Path, dest: &Path) -> io::Result<()> {
    if dest.exists() {
        return Err(already_exists(dest));
    }
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are not copied",
        ));
    }
    if meta.is_dir() {
        fs::create_dir(dest)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                continue;
            }
            copy_tree(&entry.path(), &dest.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(source, dest).map(|_| ())
    }
}

/// Checks whether `copy` still matches the tree that `copy_tree` would create now.
/// Symbolic links in the source are skipped just as they are during duplication.
fn matches_copy(source: &Path, copy: &Path) -> io::Result<bool> {
    let source_meta = fs::symlink_metadata(source)?;
    let copy_meta = match fs::symlink_metadata(copy) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if source_meta.file_type().is_symlink() || copy_meta.file_type().is_symlink() {
        return Ok(false);
    }
    if source_meta.is_dir() != copy_meta.is_dir() || source_meta.is_file() != copy_meta.is_file() {
        return Ok(false);
    }
    if source_meta.is_file() {
        return Ok(fs::read(source)? == fs::read(copy)?);
    }
    if !source_meta.is_dir() {
        return Ok(false);
    }

    let mut source_entries = std::collections::BTreeMap::new();
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if !entry.file_type()?.is_symlink() {
            source_entries.insert(entry.file_name(), entry.path());
        }
    }
    let copy_entries = fs::read_dir(copy)?
        .map(|entry| entry.map(|entry| (entry.file_name(), entry.path())))
        .collect::<io::Result<std::collections::BTreeMap<_, _>>>()?;
    if source_entries.keys().ne(copy_entries.keys()) {
        return Ok(false);
    }
    for (name, source_path) in source_entries {
        if !matches_copy(&source_path, &copy_entries[&name])? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn create_entry(path: &Path, is_dir: bool) -> io::Result<()> {
    if path.exists() {
        return Err(already_exists(path));
    }
    if is_dir {
        fs::create_dir(path)
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map(|_| ())
    }
}

fn already_exists(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("already exists: {}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-explorer-ops-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_reject_separators_and_empties() {
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name("   "), Err(NameError::Empty));
        assert_eq!(validate_name(".."), Err(NameError::Invalid));
        assert_eq!(validate_name("a/b"), Err(NameError::Invalid));
        assert_eq!(validate_name("a\\b"), Err(NameError::Invalid));
        assert_eq!(validate_name("what?.txt"), Err(NameError::Invalid));
        assert_eq!(validate_name("notes.md"), Ok(()));
    }

    #[test]
    fn create_then_undo_then_redo_restores_the_file() {
        let dir = temp("create");
        let path = dir.join("new.txt");
        let action = create(&path, false).unwrap();
        assert!(path.is_file());
        undo(&action).unwrap();
        assert!(!path.exists());
        redo(&action).unwrap();
        assert!(path.is_file());
        assert!(create(&path, false).is_err(), "never overwrites");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rename_undo_and_redo_move_the_entry_back_and_forth() {
        let dir = temp("rename");
        let from = dir.join("a.txt");
        fs::write(&from, "x").unwrap();
        let to = dir.join("b.txt");
        let action = rename(&from, &to).unwrap();
        assert!(to.exists() && !from.exists());
        undo(&action).unwrap();
        assert!(from.exists() && !to.exists());
        redo(&action).unwrap();
        assert!(to.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rename_refuses_to_replace_another_entry() {
        let dir = temp("rename-clash");
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        assert!(rename(&a, &b).is_err());
        assert_eq!(fs::read_to_string(&b).unwrap(), "b");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_folder_copies_tree_and_undo_removes_only_the_copy() {
        let dir = temp("dup");
        let folder = dir.join("src");
        fs::create_dir_all(folder.join("inner")).unwrap();
        fs::write(folder.join("inner").join("f.txt"), "data").unwrap();
        let action = duplicate(&folder, " copy").unwrap();
        let copy = dir.join("src copy");
        assert_eq!(
            fs::read_to_string(copy.join("inner").join("f.txt")).unwrap(),
            "data"
        );
        undo(&action).unwrap();
        assert!(!copy.exists());
        assert!(
            folder.join("inner").join("f.txt").exists(),
            "original stays"
        );
        redo(&action).unwrap();
        assert!(copy.join("inner").join("f.txt").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn undo_preserves_a_duplicate_that_changed_after_copying() {
        let dir = temp("dup-changed");
        let source = dir.join("src");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("original.txt"), "original").unwrap();
        let action = duplicate(&source, " copy").unwrap();
        let copy = dir.join("src copy");
        fs::write(copy.join("new.txt"), "user data").unwrap();

        let error = undo(&action).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            fs::read_to_string(copy.join("new.txt")).unwrap(),
            "user data"
        );
        assert!(copy.join("original.txt").is_file());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_picks_a_free_name_with_extension() {
        let dir = temp("dup-name");
        let file = dir.join("notes.md");
        fs::write(&file, "1").unwrap();
        let first = duplicate(&file, " copy").unwrap();
        let second = duplicate(&file, " copy").unwrap();
        assert_eq!(
            first,
            Action::Duplicated {
                source: file.clone(),
                copy: dir.join("notes copy.md")
            }
        );
        assert_eq!(
            second,
            Action::Duplicated {
                source: file.clone(),
                copy: dir.join("notes copy 2.md")
            }
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn history_clears_redo_on_new_action_and_limits_depth() {
        let mut history = History::default();
        let a = Action::Created {
            path: PathBuf::from("a"),
            is_dir: false,
        };
        history.record(a.clone());
        let taken = history.take_undo().unwrap();
        history.push_redo(taken);
        assert!(history.can_redo());
        history.record(a.clone());
        assert!(!history.can_redo(), "a new action drops the redo stack");
        for _ in 0..HISTORY_LIMIT + 5 {
            history.record(a.clone());
        }
        assert_eq!(history.undo_len(), HISTORY_LIMIT);
    }
}
