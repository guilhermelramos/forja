//! Leitura somente leitura dos arquivos do Harbor 0.1.
//!
//! O formato é o nosso: `session.json` (projetos e notas) e `settings.json`
//! (quick commands, aparência, fonte do terminal e destinos SSH). Só os campos
//! listados aqui são lidos; qualquer outro, inclusive segredos, é ignorado.
//! Nada deste arquivo escreve no Harbor.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Nome do arquivo de sessão dentro da pasta do Harbor.
pub const SESSION_FILE: &str = "session.json";
/// Nome do arquivo de ajustes dentro da pasta do Harbor.
pub const SETTINGS_FILE: &str = "settings.json";

/// Pasta padrão do Harbor: `%APPDATA%\Harbor`. `FORJA_HARBOR_DIR` substitui
/// (usado por testes e capturas, que nunca tocam o Harbor de verdade).
///
/// Em execução de teste (`--test-driver`, `--screenshot`) ou com dados
/// isolados por `FORJA_DATA_DIR`, a pasta real do Harbor não é lida: só uma
/// origem explícita em `FORJA_HARBOR_DIR` é usada.
pub fn default_source_dir() -> Option<PathBuf> {
    source_dir(
        env_path("FORJA_HARBOR_DIR"),
        isolated_run(),
        env_path("APPDATA").map(|appdata| appdata.join("Harbor")),
    )
}

/// Uma origem explícita vence sempre; sem ela, uma execução isolada não pode
/// enxergar o Harbor da máquina.
fn source_dir(
    harbor_override: Option<PathBuf>,
    isolated: bool,
    machine_harbor: Option<PathBuf>,
) -> Option<PathBuf> {
    if harbor_override.is_some() {
        return harbor_override;
    }
    if isolated {
        return None;
    }
    machine_harbor
}

fn isolated_run() -> bool {
    ["FORJA_DATA_DIR", "FORJA_TEST_DRIVER", "FORJA_SCREENSHOT"]
        .iter()
        .any(|name| env_path(name).is_some())
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Session {
    pub projects: Vec<Project>,
    pub notes: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub display_name: String,
    pub path: PathBuf,
    pub icon: String,
    pub badge_color: String,
    pub workspaces: Vec<Workspace>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub quick_commands: QuickCommands,
    pub appearance: Appearance,
    pub terminal: Terminal,
    pub remote: Remote,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct QuickCommands {
    pub entries: Vec<QuickCommand>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct QuickCommand {
    pub name: String,
    pub command: String,
    pub scope: Scope,
    #[serde(default = "yes")]
    pub append_enter: bool,
}

impl Default for QuickCommand {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            scope: Scope::Global,
            append_enter: true,
        }
    }
}

fn yes() -> bool {
    true
}

/// Escopo de um quick command: `{"kind": "global"}` ou `{"kind": "project", "path": ...}`.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Global,
    /// O caminho é lido só para validar a forma; a receita usa escopo de projeto.
    Project(#[allow(dead_code)] PathBuf),
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Appearance {
    pub dark: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Terminal {
    pub font_size: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Remote {
    pub ssh_targets: Vec<SshTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SshTarget {
    pub id: String,
    pub label: String,
    pub host: String,
    pub user: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub identity_file: String,
    pub proxy_jump: String,
    pub forwards: Vec<Forward>,
}

impl Default for SshTarget {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            host: String::new(),
            user: String::new(),
            port: default_port(),
            identity_file: String::new(),
            proxy_jump: String::new(),
            forwards: Vec::new(),
        }
    }
}

fn default_port() -> u16 {
    22
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Forward {
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
}

/// Lê `session.json`. `Ok(None)` quando o arquivo não existe; `Err` quando
/// existe mas não pode ser lido ou não é JSON válido.
pub fn read_session(dir: &Path) -> Result<Option<Session>, String> {
    read_optional(&dir.join(SESSION_FILE))
}

/// Lê `settings.json` com a mesma regra de `read_session`.
pub fn read_settings(dir: &Path) -> Result<Option<Settings>, String> {
    read_optional(&dir.join(SETTINGS_FILE))
}

fn read_optional<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("unreadable: {error}")),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| format!("corrupt: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("harbor")
    }

    #[test]
    fn reads_session_and_settings_fixtures() {
        let session = read_session(&fixture_dir()).unwrap().expect("session fixture");
        assert_eq!(session.projects.len(), 2);
        assert_eq!(session.projects[0].workspaces.len(), 1);
        assert!(session.notes.contains("Notas de exemplo"));

        let settings = read_settings(&fixture_dir()).unwrap().expect("settings fixture");
        assert_eq!(settings.quick_commands.entries.len(), 2);
        assert_eq!(settings.appearance.dark, Some(true));
        assert_eq!(settings.terminal.font_size, Some(15.0));
        assert_eq!(settings.remote.ssh_targets.len(), 1);
        assert_eq!(settings.remote.ssh_targets[0].port, 2222);
    }

    #[test]
    fn missing_files_are_none_not_errors() {
        let dir = std::env::temp_dir().join("forja-import-harbor-missing-nothing-here");
        assert!(read_session(&dir).unwrap().is_none());
        assert!(read_settings(&dir).unwrap().is_none());
    }

    #[test]
    fn corrupt_file_is_an_error_not_a_panic() {
        let dir = std::env::temp_dir().join(format!(
            "forja-import-harbor-corrupt-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SESSION_FILE), b"{ not json").unwrap();
        let error = read_session(&dir).unwrap_err();
        assert!(error.starts_with("corrupt"), "error was {error}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_fields_and_secrets_are_ignored() {
        let json = br#"{"projects": [], "notes": "", "token": "should-not-matter"}"#;
        let session: Session = serde_json::from_slice(json).unwrap();
        assert!(session.projects.is_empty());
    }

    #[test]
    fn explicit_harbor_wins_even_in_an_isolated_run() {
        let harbor = PathBuf::from("C:/fixtures/Harbor");
        assert_eq!(
            source_dir(Some(harbor.clone()), true, Some(PathBuf::from("C:/AppData/Harbor"))),
            Some(harbor)
        );
    }

    #[test]
    fn isolated_runs_never_read_the_machine_harbor() {
        let machine = Some(PathBuf::from("C:/AppData/Harbor"));
        assert_eq!(source_dir(None, true, machine), None);
    }

    #[test]
    fn normal_runs_use_the_appdata_harbor() {
        let machine = PathBuf::from("C:/AppData/Harbor");
        assert_eq!(source_dir(None, false, Some(machine.clone())), Some(machine));
        assert_eq!(source_dir(None, false, None), None);
    }

    #[test]
    fn isolated_data_dir_hides_the_real_harbor() {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap_or_else(|poison| poison.into_inner());
        let names = [
            "FORJA_DATA_DIR",
            "FORJA_HARBOR_DIR",
            "FORJA_TEST_DRIVER",
            "FORJA_SCREENSHOT",
            "APPDATA",
        ];
        let saved: Vec<(&str, Option<std::ffi::OsString>)> = names
            .iter()
            .map(|name| (*name, std::env::var_os(name)))
            .collect();
        let machine = std::env::temp_dir().join(format!(
            "forja-import-appdata-{}",
            std::process::id()
        ));
        std::env::set_var("APPDATA", &machine);
        std::env::remove_var("FORJA_HARBOR_DIR");
        std::env::remove_var("FORJA_DATA_DIR");
        std::env::remove_var("FORJA_TEST_DRIVER");
        std::env::remove_var("FORJA_SCREENSHOT");

        std::env::set_var("FORJA_DATA_DIR", machine.join("data"));
        assert_eq!(default_source_dir(), None, "isolated run read %APPDATA%");

        std::env::remove_var("FORJA_DATA_DIR");
        std::env::set_var("FORJA_TEST_DRIVER", "1");
        assert_eq!(default_source_dir(), None, "test driver read %APPDATA%");

        std::env::remove_var("FORJA_TEST_DRIVER");
        std::env::set_var("FORJA_SCREENSHOT", "1");
        assert_eq!(default_source_dir(), None, "screenshot read %APPDATA%");

        let harbor = machine.join("Harbor");
        std::env::set_var("FORJA_HARBOR_DIR", &harbor);
        assert_eq!(default_source_dir(), Some(harbor.clone()));

        for (name, value) in saved {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}
