//! Execução de um turno headless com stream JSON.

use crate::profile::{self, ChatProfile};
use crate::stream::{parse_stream_line, StreamEvent};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
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
        let mut command = Command::new("taskkill.exe");
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

/// Inicia um turno. O prompt vai por stdin, como documentado nos perfis.
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
    let args = profile::turn_args(profile, session_id, permission, model, effort)?;
    start_process(
        &profile.id,
        &profile.start.command,
        &args,
        project,
        prompt,
        environment,
    )
}

/// Inicia um processo já com os argumentos prontos. Separado para os testes
/// usarem um executável falso sem depender de um CLI instalado.
pub fn start_process(
    profile_id: &str,
    program: &str,
    args: &[String],
    project: &Path,
    prompt: &str,
    environment: &BTreeMap<String, String>,
) -> Result<Turn, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(project)
        .stdin(Stdio::piped())
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
    let mut child = command
        .spawn()
        .map_err(|error| format!("não foi possível iniciar `{program}`: {error}"))?;
    let pid = child.id();
    if let Some(mut stdin) = child.stdin.take() {
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
        let _ = std::fs::remove_dir_all(dir);
    }
}
