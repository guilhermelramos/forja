//! Execução de um turno headless com stream JSON.

use crate::profile::{self, ChatProfile};
use crate::stream::{parse_stream_line, StreamEvent};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};

/// Processo em execução e o canal de eventos.
pub struct Turn {
    /// Eventos lidos do stdout.
    pub rx: Receiver<StreamEvent>,
    /// Id do processo, usado para cancelar.
    pub pid: u32,
}

impl Turn {
    /// Encerra a árvore do processo.
    pub fn cancel(&self) {
        cancel_pid(self.pid);
    }
}

/// Encerra um processo e seus filhos.
pub fn cancel_pid(pid: u32) {
    std::thread::spawn(move || {
        let mut command = forja_kernel::process::command("taskkill.exe");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        let _ = command.status();
    });
}

/// Inicia um turno. O prompt vai por stdin para claude/codex, ou por argumento para opencode.
#[allow(clippy::too_many_arguments)]
pub fn start_turn(
    profile: &ChatProfile,
    project: &Path,
    session_id: Option<&str>,
    prompt: &str,
    permission: &str,
    model: Option<&str>,
    effort: Option<&str>,
    environment: &BTreeMap<String, String>,
) -> Result<Turn, String> {
    let mut args = profile::turn_args(profile, session_id, permission, model, effort)?;
    let positional_prompt = profile.id == "opencode";
    if positional_prompt {
        args.push(prompt.to_owned());
    }
    start_process_inner(
        &profile.id,
        &profile.start.command,
        &args,
        project,
        (!positional_prompt).then_some(prompt),
        environment,
    )
}

/// Procura um executável no PATH respeitando PATHEXT no Windows.
pub fn resolve_program(name: &str) -> Option<PathBuf> {
    resolve_in(
        name,
        std::env::var_os("PATH").as_deref(),
        std::env::var_os("PATHEXT").as_deref(),
    )
}

fn resolve_in(name: &str, path: Option<&OsStr>, extensions: Option<&OsStr>) -> Option<PathBuf> {
    let candidate = Path::new(name);
    let mut suffixes = Vec::new();
    let pathext = extensions
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD;.PS1".to_owned());
    for extension in pathext.split(';') {
        let extension = extension.trim();
        if !extension.is_empty() {
            suffixes.push(extension.to_ascii_lowercase());
            suffixes.push(extension.to_ascii_uppercase());
        }
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

fn build_command(program: &str, args: &[String]) -> Command {
    let resolved = resolve_program(program).unwrap_or_else(|| PathBuf::from(program));
    let ext = resolved
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if ext == "cmd" || ext == "bat" {
        let mut command = forja_kernel::process::command("cmd.exe");
        command.arg("/d").arg("/c").arg(&resolved).args(args);
        return command;
    }
    if ext == "ps1" {
        let mut command = forja_kernel::process::command("powershell.exe");
        command
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&resolved)
            .args(args);
        return command;
    }
    let mut command = forja_kernel::process::command(&resolved);
    command.args(args);
    command
}

/// Inicia um processo já com os argumentos prontos. Separado para os testes
/// usarem um executável falso sem depender de um CLI instalado.
#[cfg(test)]
pub fn start_process(
    profile_id: &str,
    program: &str,
    args: &[String],
    project: &Path,
    prompt: &str,
    environment: &BTreeMap<String, String>,
) -> Result<Turn, String> {
    start_process_inner(profile_id, program, args, project, Some(prompt), environment)
}

fn start_process_inner(
    profile_id: &str,
    program: &str,
    args: &[String],
    project: &Path,
    stdin_prompt: Option<&str>,
    environment: &BTreeMap<String, String>,
) -> Result<Turn, String> {
    let mut command = build_command(program, args);
    command
        .current_dir(project)
        .stdin(if stdin_prompt.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in environment {
        command.env(key, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!("executable-not-found:{program}")
        } else {
            format!("launch-failed:{program}:{error}")
        }
    })?;
    let pid = child.id();
    if let (Some(mut stdin), Some(prompt)) = (child.stdin.take(), stdin_prompt) {
        let prompt = prompt.to_owned();
        std::thread::spawn(move || {
            let _ = stdin.write_all(prompt.as_bytes());
            let _ = stdin.write_all(b"\n");
        });
    }
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "o processo do chat não tem stdout".to_owned())?;
    if let Some(mut stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut sink = Vec::new();
            let _ = stderr.read_to_end(&mut sink);
        });
    }
    let (tx, rx) = mpsc::channel();
    let profile_id = profile_id.to_owned();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            for event in parse_stream_line(&profile_id, &line) {
                if tx.send(event).is_err() {
                    break;
                }
            }
        }
        let code = child.wait().ok().and_then(|status| status.code());
        let _ = tx.send(StreamEvent::Done(code));
    });
    Ok(Turn { rx, pid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::StreamEvent;
    use std::time::Duration;

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    #[test]
    fn fake_stream_process_emits_session_message_and_done() {
        let dir = std::env::temp_dir().join(format!(
            "forja-chat-runner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let guard = TempGuard(dir.clone());
        let script = dir.join("fake-claude.cmd");
        std::fs::write(
            &script,
            "@echo off\r\necho {\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"test\"}\r\necho {\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hello\"}],\"usage\":{\"input_tokens\":2,\"output_tokens\":3}}}\r\necho {\"type\":\"result\",\"session_id\":\"test\",\"result\":\"done\"}\r\n",
        )
        .unwrap();
        let turn = start_process(
            "claude-code",
            "cmd.exe",
            &[
                "/d".to_owned(),
                "/c".to_owned(),
                script.display().to_string(),
            ],
            &dir,
            "prompt",
            &BTreeMap::new(),
        )
        .expect("fake process starts");
        let mut events = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            match turn.rx.recv_timeout(Duration::from_millis(200)) {
                Ok(event) => {
                    let done = matches!(event, StreamEvent::Done(_));
                    events.push(event);
                    if done {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::SessionId(id) if id == "test")));
        assert!(events.iter().any(
            |event| matches!(event, StreamEvent::Message(message) if message.summary() == "hello")
        ));
        assert!(events.iter().any(
            |event| matches!(event, StreamEvent::Usage(usage) if usage.total_tokens == 5)
        ));
        assert!(events.iter().any(|event| matches!(event, StreamEvent::Done(Some(0)))));
        drop(guard);
    }

    #[cfg(windows)]
    #[test]
    fn resolve_in_prefers_cmd_shim_over_extensionless_file() {
        let dir = std::env::temp_dir().join(format!(
            "forja-chat-shim-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let guard = TempGuard(dir);
        std::fs::write(guard.0.join("opencode"), "#!/bin/sh\n").unwrap();
        std::fs::write(guard.0.join("opencode.cmd"), "@echo off\r\n").unwrap();
        let resolved = resolve_in(
            "opencode",
            Some(guard.0.as_os_str()),
            Some(OsStr::new(".COM;.EXE;.BAT;.CMD")),
        );
        assert_eq!(resolved, Some(guard.0.join("opencode.cmd")));
    }
}
