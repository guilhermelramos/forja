//! Detecção de executáveis no PATH, com cache em memória.

use crate::profile::Profile;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resultado de detecção de um perfil.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Detection {
    /// Executável encontrado.
    pub executable: Option<PathBuf>,
    /// Primeira linha de `--version`.
    pub version: Option<String>,
    /// Data da verificação.
    pub checked_at: String,
    /// Falha ao executar o binário.
    pub error: Option<String>,
}

/// Detecta todos os perfis. Roda em segundo plano.
pub fn detect_all(profiles: &[Profile]) -> BTreeMap<String, Detection> {
    let path = std::env::var_os("PATH");
    let extensions = std::env::var_os("PATHEXT");
    profiles
        .iter()
        .map(|profile| {
            let executable = profile
                .detect
                .executables
                .iter()
                .find_map(|name| resolve_in(name, path.as_deref(), extensions.as_deref()));
            let mut detection = Detection {
                executable,
                checked_at: chrono::Utc::now().to_rfc3339(),
                ..Detection::default()
            };
            if let Some(executable) = &detection.executable {
                if !profile.detect.version.is_empty() {
                    match run_version(executable, &profile.detect.version) {
                        Ok(version) => detection.version = Some(version),
                        Err(error) => detection.error = Some(error),
                    }
                }
            }
            (profile.id.clone(), detection)
        })
        .collect()
}

/// Procura um executável no PATH e PATHEXT do processo atual.
pub fn resolve(name: &str) -> Option<PathBuf> {
    resolve_in(
        name,
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PATHEXT").as_deref(),
    )
}

/// Comando oficial sugerido para instalar o CLI quando não estiver presente.
pub fn install_hint(profile_id: &str) -> Option<&'static str> {
    match profile_id {
        "aider" => Some("python -m pip install -U aider-chat"),
        "claude-code" => Some("npm install -g @anthropic-ai/claude-code"),
        "codex" => Some("npm install -g @openai/codex"),
        "gemini" => Some("npm install -g @google/gemini-cli"),
        "opencode" => Some("npm install -g opencode-ai"),
        _ => None,
    }
}

/// Procura um executável no PATH com as extensões do Windows.
pub fn resolve_in(name: &str, path: Option<&OsStr>, extensions: Option<&OsStr>) -> Option<PathBuf> {
    let candidate = Path::new(name);
    let mut suffixes = Vec::new();
    let pathext = extensions
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD;.PS1".to_owned());
    for extension in pathext.split(';') {
        let extension = extension.trim();
        if extension.is_empty() {
            continue;
        }
        suffixes.push(extension.to_ascii_lowercase());
        suffixes.push(extension.to_ascii_uppercase());
    }
    for suffix in [".exe", ".cmd", ".ps1"] {
        suffixes.push(suffix.to_owned());
    }
    suffixes.push(String::new());
    suffixes.dedup();
    if candidate.components().count() > 1 {
        if let Some(ext) = candidate.extension().and_then(|e| e.to_str()) {
            if !ext.is_empty() {
                return candidate.is_file().then(|| candidate.to_path_buf());
            }
        }
        for suffix in &suffixes {
            let file = if suffix.is_empty() {
                candidate.to_path_buf()
            } else {
                PathBuf::from(format!("{}{suffix}", candidate.display()))
            };
            if file.is_file() {
                return Some(file);
            }
        }
        return None;
    }
    for directory in std::env::split_paths(path.unwrap_or_default()) {
        for suffix in &suffixes {
            let file = if suffix.is_empty() {
                name.to_owned()
            } else {
                format!("{name}{suffix}")
            };
            let full = directory.join(file);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

fn run_version(executable: &Path, args: &[String]) -> Result<String, String> {
    let mut command = version_command(executable, args);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).into_owned()
    } else {
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if output.status.success() || !line.is_empty() {
        Ok(line.to_owned())
    } else {
        Err(format!("exit code {}", output.status.code().unwrap_or(-1)))
    }
}

fn version_command(executable: &Path, args: &[String]) -> Command {
    let ext = executable
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if matches!(ext.as_str(), "cmd" | "bat") {
        let comspec = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        let mut command = Command::new(comspec);
        command.arg("/d").arg("/c").arg(executable).args(args);
        return command;
    }
    if ext == "ps1" {
        let mut command = Command::new("powershell.exe");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(executable)
            .args(args);
        return command;
    }
    let mut command = Command::new(executable);
    command.args(args);
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Layer;

    struct TempGuard(PathBuf);
    impl TempGuard {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "forja-helper-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn finds_an_executable_through_a_fake_path() {
        let guard = TempGuard::new("detect");
        std::fs::write(guard.0.join("fake-helper.cmd"), "@echo off\r\n").unwrap();
        let path = std::ffi::OsString::from(guard.0.display().to_string());
        let resolved = resolve_in(
            "fake-helper",
            Some(&path),
            Some(&std::ffi::OsString::from(".CMD")),
        );
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("fake-helper.cmd"));
        assert!(resolve_in("missing-helper", Some(&path), None).is_none());
    }

    #[test]
    fn prefers_cmd_shim_over_extensionless_posix_script() {
        let guard = TempGuard::new("npm-shim");
        std::fs::write(guard.0.join("opencode"), "#!/bin/sh\necho posix\n").unwrap();
        std::fs::write(guard.0.join("opencode.cmd"), "@echo off\r\n").unwrap();
        let path = std::ffi::OsString::from(guard.0.display().to_string());
        let resolved = resolve_in(
            "opencode",
            Some(&path),
            Some(&std::ffi::OsString::from(".COM;.EXE;.BAT;.CMD")),
        );
        assert_eq!(resolved, Some(guard.0.join("opencode.cmd")));
    }

    #[test]
    fn detection_records_version_without_inventing_a_binary() {
        let profile = Profile::parse(
            r#"
id = "fake"
name = "Fake"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["definitely-not-installed-forja"]
version = ["--version"]
[start]
command = "definitely-not-installed-forja"
[prompt]
mode = "argument"
"#,
            std::path::PathBuf::from("fake.toml"),
            Layer::User,
        )
        .unwrap();
        let detections = detect_all(&[profile]);
        let detection = &detections["fake"];
        assert!(detection.executable.is_none());
        assert!(!detection.checked_at.is_empty());
    }
}
