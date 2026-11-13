//! Funções puras do hub: shell, caminho absoluto e comparação de pastas.

use crate::settings::TermSettings;
use crate::shells::{self, ShellCommand};
use std::path::{Path, PathBuf};

pub(super) fn resolve_shell(
    settings: &TermSettings,
    shell: Option<&str>,
    command: Option<&str>,
    args: Option<&str>,
) -> ShellCommand {
    if let Some(command) = command {
        return ShellCommand {
            program: command.to_owned(),
            arguments: args
                .map(str::to_owned)
                .unwrap_or_else(|| shells::arguments_for(command)),
        };
    }
    match shell.unwrap_or_default() {
        "" | "auto" => shells::configured(&settings.shell_default, &settings.shell_path),
        "pwsh" => shells::configured("pwsh", ""),
        "powershell" => shells::configured("powershell", ""),
        "cmd" => shells::configured("cmd", ""),
        "custom" => shells::configured("custom", &settings.shell_path),
        "git-bash" => shells::git_bash().unwrap_or_else(|| shells::configured("cmd", "")),
        "wsl" => shells::wsl(None).unwrap_or_else(|| shells::configured("cmd", "")),
        other if other.starts_with("wsl:") => {
            shells::wsl(Some(&other[4..])).unwrap_or_else(|| shells::configured("cmd", ""))
        }
        other => ShellCommand {
            program: other.to_owned(),
            arguments: args
                .map(str::to_owned)
                .unwrap_or_else(|| shells::arguments_for(other)),
        },
    }
}

/// Caminho absoluto normalizado sem tocar no disco.
pub(super) fn absolute_dir(path: &Path) -> PathBuf {
    let mut result = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => result.push(prefix.as_os_str()),
            std::path::Component::RootDir => {
                result.push(std::path::Component::RootDir.as_os_str())
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::Normal(name) => result.push(name),
        }
    }
    result
}

pub(super) fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.to_string_lossy().replace('/', "\\");
    let right = right.to_string_lossy().replace('/', "\\");
    left.eq_ignore_ascii_case(&right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_become_absolute_without_disk_access() {
        let absolute = absolute_dir(Path::new("."));
        assert!(absolute.is_absolute());
        assert_eq!(absolute, std::env::current_dir().unwrap());
        let nested = absolute_dir(Path::new("sub/../other"));
        assert!(nested.ends_with("other"));
        assert!(!nested.to_string_lossy().contains(".."));
    }
}


