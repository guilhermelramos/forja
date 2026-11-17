//! Pipe client with strict timeout and isolated `forja-app --test-driver` process manager.

use crate::snapshot::Snapshot;
use crate::strategy::CatalogKeybind;
use forja_protocol::codec::{self, RpcError};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// Invariant 1 requires every `ui.*` call to answer in under 10 seconds.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum PipeError {
    NotRunning,
    Timeout,
    Io(std::io::Error),
    Rpc(RpcError),
}

impl std::fmt::Display for PipeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "the Forja app pipe is not open"),
            Self::Timeout => write!(f, "the Forja app did not answer within 10 s"),
            Self::Io(err) => write!(f, "pipe I/O error: {err}"),
            Self::Rpc(err) => write!(f, "RPC error: {err}"),
        }
    }
}

/// Normalizes a pipe argument (`name` or `\\.\pipe\name`) or reads `FORJA_PIPE`.
pub fn resolve_pipe_name(explicit: Option<&str>) -> String {
    match explicit {
        Some(name) if !name.is_empty() => {
            if name.starts_with(r"\\.\pipe\") {
                name.to_owned()
            } else {
                format!(r"\\.\pipe\{name}")
            }
        }
        _ => forja_protocol::pipe_name(),
    }
}

/// Calls a `ui.*` method over `pipe` with a hard timeout.
pub fn call_ui(
    pipe: &str,
    method: &str,
    params: Value,
    timeout: Duration,
) -> Result<Value, PipeError> {
    let line = codec::request_line(json!(1), method, params);
    let pipe_owned = pipe.to_owned();
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("forja-explore-rpc".into())
        .spawn(move || {
            let _ = tx.send(roundtrip(&pipe_owned, &line));
        })
        .map_err(PipeError::Io)?;
    match rx.recv_timeout(timeout) {
        Ok(result) => result.and_then(|reply| codec::response_result(&reply).map_err(PipeError::Rpc)),
        Err(_) => Err(PipeError::Timeout),
    }
}

fn roundtrip(pipe: &str, line: &str) -> Result<String, PipeError> {
    let mut stream = open_pipe(pipe)?;
    stream.write_all(line.as_bytes()).map_err(PipeError::Io)?;
    stream.write_all(b"\n").map_err(PipeError::Io)?;
    stream.flush().map_err(PipeError::Io)?;
    let mut reader = BufReader::new(stream);
    let mut reply = String::new();
    let bytes = reader.read_line(&mut reply).map_err(PipeError::Io)?;
    if bytes == 0 {
        return Err(PipeError::NotRunning);
    }
    Ok(reply)
}

fn open_pipe(name: &str) -> Result<std::fs::File, PipeError> {
    let mut last_error = None;
    for _ in 0..8 {
        match std::fs::OpenOptions::new().read(true).write(true).open(name) {
            Ok(file) => return Ok(file),
            Err(error) => {
                if is_not_running(&error) {
                    return Err(PipeError::NotRunning);
                }
                if error.raw_os_error() != Some(231) {
                    return Err(PipeError::Io(error));
                }
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
    Err(PipeError::Io(last_error.unwrap_or_else(|| {
        std::io::Error::other("pipe is busy")
    })))
}

fn is_not_running(error: &std::io::Error) -> bool {
    matches!(error.kind(), std::io::ErrorKind::NotFound)
        || matches!(error.raw_os_error(), Some(2 | 3 | 53 | 1058))
}

/// Runs `<app> --dump-catalog --locale <locale>` and extracts `keybinds`.
pub fn load_catalog_keybinds(app_exe: &Path, locale: &str) -> Vec<CatalogKeybind> {
    let Ok(output) = Command::new(app_exe)
        .args(["--dump-catalog", "--locale", locale])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_slice::<Value>(&output.stdout) else {
        return Vec::new();
    };
    json.get("keybinds")
        .and_then(|v| serde_json::from_value::<Vec<CatalogKeybind>>(v.clone()).ok())
        .unwrap_or_default()
}

/// Ensures a sample git repository exists for `FORJA_TEST_PICK_DIR`.
pub fn ensure_sample_pick_dir(dir: &Path) -> std::io::Result<()> {
    if dir.join(".git").exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dir.join("src"))?;
    std::fs::create_dir_all(dir.join("docs"))?;
    std::fs::write(
        dir.join("README.md"),
        "# Exemplo\n\nProjeto de exemplo para o explorador da Forja.\n",
    )?;
    std::fs::write(
        dir.join("src/login.ts"),
        "export function login(user: string) {\n  return user.length > 0;\n}\n",
    )?;
    std::fs::write(
        dir.join("src/app.py"),
        "def main():\n    print('ola')\n\nif __name__ == '__main__':\n    main()\n",
    )?;
    std::fs::write(dir.join("docs/guia.md"), "# Guia\n\n- passo 1\n- passo 2\n")?;

    let run_git = |args: &[&str]| {
        let _ = Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    run_git(&["init", "-q", "-b", "main"]);
    run_git(&["config", "user.name", "Exemplo"]);
    run_git(&["config", "user.email", "exemplo@example.invalid"]);
    run_git(&["add", "-A"]);
    run_git(&["commit", "-q", "-m", "Primeiro commit"]);
    // Leave an unstaged change so diff/changes views have content.
    std::fs::write(
        dir.join("src/login.ts"),
        "export function login(user: string) {\n  return user.length > 0;\n}\nexport const retry = 2;\n",
    )?;
    Ok(())
}

/// Owns a spawned `forja-app --test-driver` process and kills it on drop.
pub struct AppSession {
    child: Option<Child>,
    pipe: String,
    app_exe: PathBuf,
    locale: String,
    out_dir: PathBuf,
    pick_dir: PathBuf,
    test_helpers: Option<String>,
    demo: bool,
    restarts: u32,
}

impl AppSession {
    pub fn start(
        app_exe: PathBuf,
        pipe_base: String,
        locale: String,
        out_dir: PathBuf,
        pick_dir: PathBuf,
        test_helpers: Option<String>,
        demo: bool,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(out_dir.join("data")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(out_dir.join("user")).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(out_dir.join("downloads")).map_err(|e| e.to_string())?;
        ensure_sample_pick_dir(&pick_dir).map_err(|e| format!("failed to seed pick dir: {e}"))?;

        let mut session = Self {
            child: None,
            pipe: pipe_base,
            app_exe,
            locale,
            out_dir,
            pick_dir,
            test_helpers,
            demo,
            restarts: 0,
        };
        session.spawn_and_wait()?;
        Ok(session)
    }

    pub fn pipe(&self) -> &str {
        &self.pipe
    }

    pub fn spawn_and_wait(&mut self) -> Result<Snapshot, String> {
        self.kill();
        self.restarts += 1;
        let stderr_path = self.out_dir.join("app-stderr.txt");
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&stderr_path)
            .map_err(|e| format!("failed to open {}: {e}", stderr_path.display()))?;

        let pick_file = self.pick_dir.join("README.md");
        let mut args = vec!["--test-driver".to_owned(), "--locale".to_owned(), self.locale.clone()];
        if self.demo {
            // Fixed sample data: the regression exercises the demo mode itself.
            args.push("--demo".to_owned());
        }
        let child = Command::new(&self.app_exe)
            .args(&args)
            .env("FORJA_PIPE", &self.pipe)
            .env("FORJA_DATA_DIR", self.out_dir.join("data"))
            .env("FORJA_CONTENT_DIR", self.out_dir.join("user"))
            .env("FORJA_TEST_LOG", self.out_dir.join("actions.jsonl"))
            .env("FORJA_TEST_PICK_DIR", &self.pick_dir)
            .env("FORJA_TEST_PICK_FILE", pick_file)
            .env("FORJA_TEST_SAVE_DIR", self.out_dir.join("downloads"))
            .env(
                "FORJA_TEST_HELPERS",
                self.test_helpers.as_deref().unwrap_or(""),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|e| format!("failed to launch {}: {e}", self.app_exe.display()))?;

        self.child = Some(child);

        let deadline = Instant::now() + Duration::from_secs(25);
        while Instant::now() < deadline {
            if let Some(code) = self.poll_exit() {
                return Err(format!("forja-app exited during startup with code {code:?}"));
            }
            match call_ui(&self.pipe, "ui.snapshot", json!({}), Duration::from_secs(3)) {
                Ok(result) => return Snapshot::from_rpc_result(&result),
                // Keep polling until the deadline: a starting app can still be
                // wiring the pipe or briefly answer with a transient RPC error.
                Err(_) => std::thread::sleep(Duration::from_millis(250)),
            }
        }
        Err("timed out waiting for forja-app --test-driver pipe".to_owned())
    }

    /// Returns `Some(exit_code)` if the child process has terminated.
    pub fn poll_exit(&mut self) -> Option<Option<i32>> {
        let child = self.child.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                self.child = None;
                Some(status.code())
            }
            Ok(None) => None,
            Err(_) => None,
        }
    }

    /// Reads the last few lines of `app-stderr.txt` for crash diagnostics.
    pub fn stderr_tail(&self, max_lines: usize) -> String {
        let path = self.out_dir.join("app-stderr.txt");
        let Ok(text) = std::fs::read_to_string(path) else {
            return String::new();
        };
        let lines: Vec<&str> = text.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        lines[start..].join(" | ")
    }

    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for AppSession {
    fn drop(&mut self) {
        self.kill();
    }
}
