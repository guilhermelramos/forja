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

/// Procura um executável no PATH com as extensões do Windows.
pub fn resolve_in(name: &str, path: Option<&OsStr>, extensions: Option<&OsStr>) -> Option<PathBuf> {
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let mut suffixes = vec![String::new()];
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
    suffixes.dedup();
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
    let mut command = Command::new(executable);
    command
        .args(args)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Layer;

    #[test]
    fn finds_an_executable_through_a_fake_path() {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-detect-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fake-helper.cmd"), "@echo off\r\n").unwrap();
        let path = std::ffi::OsString::from(dir.display().to_string());
        let resolved = resolve_in(
            "fake-helper",
            Some(&path),
            Some(&std::ffi::OsString::from(".CMD")),
        );
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("fake-helper.cmd"));
        assert!(resolve_in("missing-helper", Some(&path), None).is_none());
        let _ = std::fs::remove_dir_all(dir);
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
