//! WebView2 user-data folders, one stable folder per scope.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Shared,
    Repository,
    Workspace,
}

impl Scope {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "repository" | "repo" | "per-repo" => Self::Repository,
            "workspace" | "per-workspace" => Self::Workspace,
            _ => Self::Shared,
        }
    }
}

/// Resolves the user-data folder. The same path always gets the same folder,
/// so logins survive restarts even when the display name changes.
pub fn directory(
    scope: Scope,
    root: &Path,
    repository: Option<&Path>,
    workspace: Option<&Path>,
) -> PathBuf {
    let key = match scope {
        Scope::Shared => None,
        Scope::Repository => repository,
        Scope::Workspace => workspace.or(repository),
    };
    match key {
        Some(path) => root.join(format!("{}-{:08x}", slug(path), path_hash(path))),
        None => root.join("shared"),
    }
}

fn slug(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workspace".into());
    let slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "workspace".into()
    } else {
        slug.chars().take(40).collect()
    }
}

fn path_hash(path: &Path) -> u32 {
    let mut hash: u32 = 2_166_136_261;
    for byte in path.to_string_lossy().replace('\\', "/").bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_is_one_folder() {
        let root = Path::new("C:/data/webview");
        assert_eq!(
            directory(Scope::Shared, root, None, None),
            root.join("shared")
        );
    }

    #[test]
    fn repository_and_workspace_folders_are_stable_and_distinct() {
        let root = Path::new("C:/data/webview");
        let repo = Path::new("C:/code/app");
        let workspace = Path::new("C:/code/app/worktrees/feature");
        let repo_dir = directory(Scope::Repository, root, Some(repo), Some(workspace));
        assert_eq!(
            repo_dir,
            directory(Scope::Repository, root, Some(repo), None)
        );
        assert!(repo_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("app-"));
        let workspace_dir = directory(Scope::Workspace, root, Some(repo), Some(workspace));
        assert_ne!(workspace_dir, repo_dir);
        assert!(workspace_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("feature-"));
    }

    #[test]
    fn scopes_parse_from_settings_values() {
        assert_eq!(Scope::parse("repository"), Scope::Repository);
        assert_eq!(Scope::parse("workspace"), Scope::Workspace);
        assert_eq!(Scope::parse("anything"), Scope::Shared);
    }
}
