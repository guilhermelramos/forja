//! Escolha de shell, pasta inicial e processos externos de apoio.
//!
//! O catálogo só tem `auto`, `pwsh`, `powershell`, `cmd` e `custom`. Git Bash e
//! WSL aparecem no menu do terminal e em `terminal.open` pelo campo `shell`,
//! sem criar chaves de configuração fora do catálogo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Programa e argumentos comuns de uma opção de shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShellCommand {
    /// Executável.
    pub program: String,
    /// Argumentos fixos (sem o comando do usuário).
    pub arguments: String,
}

impl ShellCommand {
    /// Nome curto para o título da aba.
    pub fn label(&self) -> String {
        Path::new(&self.program)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.program.clone())
    }
}

/// `terminal.shell.default` resolvido para um executável real.
pub fn configured(default: &str, custom: &str) -> ShellCommand {
    match default {
        "pwsh" => {
            if which("pwsh.exe").is_some() {
                powershell("pwsh.exe")
            } else {
                powershell("powershell.exe")
            }
        }
        "powershell" => powershell("powershell.exe"),
        "cmd" => ShellCommand {
            program: "cmd.exe".to_owned(),
            arguments: String::new(),
        },
        "custom" if !custom.trim().is_empty() => ShellCommand {
            program: custom.trim().to_owned(),
            arguments: arguments_for(custom),
        },
        _ => {
            if which("pwsh.exe").is_some() {
                powershell("pwsh.exe")
            } else if which("powershell.exe").is_some() {
                powershell("powershell.exe")
            } else {
                ShellCommand {
                    program: "cmd.exe".to_owned(),
                    arguments: String::new(),
                }
            }
        }
    }
}

fn powershell(program: &str) -> ShellCommand {
    ShellCommand {
        program: program.to_owned(),
        arguments: "-NoLogo".to_owned(),
    }
}

/// Argumentos fixos de um executável conhecido.
pub fn arguments_for(program: &str) -> String {
    let stem = Path::new(program)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    match stem.as_str() {
        "powershell" | "pwsh" => "-NoLogo".to_owned(),
        "bash" | "sh" | "zsh" => "--login -i".to_owned(),
        _ => String::new(),
    }
}

/// Git Bash instalado, quando existe.
pub fn git_bash() -> Option<ShellCommand> {
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(variable) {
            let path = PathBuf::from(root).join("Git").join("bin").join("bash.exe");
            if path.is_file() {
                return Some(ShellCommand {
                    program: path.to_string_lossy().into_owned(),
                    arguments: "--login -i".to_owned(),
                });
            }
        }
    }
    None
}

/// `wsl.exe` e a distro pedida, ou a padrão quando `distro` é vazio.
pub fn wsl(distro: Option<&str>) -> Option<ShellCommand> {
    which("wsl.exe")?;
    let arguments = distro
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| format!("--distribution \"{}\"", name.replace('"', "")))
        .unwrap_or_default();
    Some(ShellCommand {
        program: "wsl.exe".to_owned(),
        arguments,
    })
}

/// Lista distros WSL em segundo plano. Uma falha devolve lista vazia.
pub fn wsl_distros() -> Vec<String> {
    if which("wsl.exe").is_none() {
        return Vec::new();
    }
    let output = hidden_command("wsl.exe")
        .args(["-l", "-q"])
        .stdin(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .replace('\0', "")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Procura um executável no PATH e nos diretórios conhecidos do Windows.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return path.is_file().then(|| path.to_path_buf());
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for directory in std::env::split_paths(&paths) {
            let candidate = directory.join(program);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    for base in [
        std::env::var_os("ProgramFiles").map(PathBuf::from),
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    {
        let candidates = [
            base.join("PowerShell").join("7").join(program),
            base.join("Microsoft").join("WindowsApps").join(program),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Pasta inicial do terminal.
pub fn start_directory(
    choice: &str,
    bench: Option<&Path>,
    project: Option<&Path>,
) -> PathBuf {
    match choice {
        "project" => project
            .or(bench)
            .map(Path::to_path_buf)
            .unwrap_or_else(home_dir),
        "home" => home_dir(),
        _ => bench.map(Path::to_path_buf).unwrap_or_else(current_dir),
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(current_dir)
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Converte `NOME=valor` da configuração em variáveis do filho.
pub fn environment(list: &[String]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for item in list {
        if let Some((name, value)) = item.split_once('=') {
            let name = name.trim();
            if !name.is_empty() {
                map.insert(name.to_owned(), value.to_owned());
            }
        }
    }
    map
}

/// Abre uma URL ou pasta no programa padrão do sistema, sem janela de console.
/// Em teste, só registra o pedido no kernel.
pub fn open_external(target: &str) -> std::io::Result<()> {
    forja_kernel::process::open_external(target).map_err(std::io::Error::other)
}

/// Aviso sonoro do sistema, sem abrir janela.
pub fn bell() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBeep(alert: u32) -> i32;
        }
        unsafe {
            let _ = MessageBeep(0);
        }
    }
}

/// Mostra só o nome quando o shell publica o caminho inteiro no título.
pub fn short_title(title: &str) -> String {
    let trimmed = title.trim();
    let looks_like_path =
        (trimmed.contains('\\') || trimmed.contains('/')) && !trimmed.contains(' ')
            || trimmed.to_ascii_lowercase().ends_with(".exe");
    if looks_like_path {
        let name = trimmed.rsplit(['\\', '/']).next().unwrap_or(trimmed);
        let name = name
            .strip_suffix(".exe")
            .or_else(|| name.strip_suffix(".EXE"))
            .unwrap_or(name);
        if !name.is_empty() {
            return name.to_owned();
        }
    }
    trimmed.to_owned()
}

fn hidden_command(program: &str) -> Command {
    let mut command = forja_kernel::process::command(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_shells_get_their_arguments() {
        assert_eq!(arguments_for("pwsh.exe"), "-NoLogo");
        assert_eq!(arguments_for("powershell.exe"), "-NoLogo");
        assert_eq!(arguments_for("cmd.exe"), "");
        assert_eq!(arguments_for("C:/Git/bin/bash.exe"), "--login -i");
    }

    #[test]
    fn environment_parses_pairs_and_ignores_invalid_items() {
        let map = environment(&[
            "FORJA_TEST=1".into(),
            "SEM_VALOR".into(),
            " =x".into(),
            "PATH_EXTRA=a=b".into(),
        ]);
        assert_eq!(map.get("FORJA_TEST").map(String::as_str), Some("1"));
        assert_eq!(map.get("PATH_EXTRA").map(String::as_str), Some("a=b"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn titles_shorten_executable_paths() {
        assert_eq!(
            short_title("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            "powershell"
        );
        assert_eq!(short_title("/usr/bin/zsh"), "zsh");
        assert_eq!(short_title("vim README.md"), "vim README.md");
    }

    #[test]
    fn configured_falls_back_when_pwsh_is_missing() {
        let command = configured("powershell", "");
        assert!(command.program.ends_with("powershell.exe"));
        let custom = configured("custom", "C:/Windows/System32/cmd.exe");
        assert_eq!(custom.program, "C:/Windows/System32/cmd.exe");
    }
}
