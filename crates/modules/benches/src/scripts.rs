//! Runs the approved setup and teardown scripts with a hard timeout.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

const MAX_OUTPUT: usize = 16 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScriptOutcome {
    pub ran: bool,
    pub success: bool,
    pub timed_out: bool,
    pub output: String,
}

pub fn run(script: &str, cwd: &Path, timeout_secs: i64) -> ScriptOutcome {
    let script = script.trim();
    if script.is_empty() {
        return ScriptOutcome {
            ran: false,
            success: true,
            ..ScriptOutcome::default()
        };
    }
    let Some(child) = spawn(script, cwd) else {
        return ScriptOutcome {
            ran: true,
            success: false,
            output: "no shell found".to_owned(),
            ..ScriptOutcome::default()
        };
    };
    wait(child, cwd, timeout_secs)
}

fn spawn(script: &str, cwd: &Path) -> Option<Child> {
    let candidates: &[(&str, &[&str])] = if cfg!(windows) {
        &[
            (
                "pwsh",
                &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"],
            ),
            (
                "powershell",
                &["-NoLogo", "-NoProfile", "-NonInteractive", "-Command"],
            ),
            ("cmd", &["/C"]),
        ]
    } else {
        &[("sh", &["-c"])]
    };
    for (program, prefix) in candidates {
        let mut command = forja_kernel::process::command(program);
        command
            .args(*prefix)
            .arg(script)
            .current_dir(cwd)
            .env("FORJA_BENCH", cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match command.spawn() {
            Ok(child) => return Some(child),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return None,
        }
    }
    None
}

fn wait(mut child: Child, _cwd: &Path, timeout_secs: i64) -> ScriptOutcome {
    let stdout = child.stdout.take().map(read_thread);
    let stderr = child.stderr.take().map(read_thread);
    let timeout = Duration::from_secs(timeout_secs.clamp(1, 86_400) as u64);
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(_) => break None,
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let pid = child.id();
            let _ = child.kill();
            if cfg!(windows) {
                let _ = forja_kernel::process::command("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(40));
    };
    let mut output = String::new();
    if let Some(handle) = stdout {
        if let Ok(bytes) = handle.join() {
            output.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    if let Some(handle) = stderr {
        if let Ok(bytes) = handle.join() {
            output.push_str(&String::from_utf8_lossy(&bytes));
        }
    }
    if output.len() > MAX_OUTPUT {
        output.truncate(MAX_OUTPUT);
        output.push_str("\n…");
    }
    ScriptOutcome {
        ran: true,
        success: status.map(|status| status.success()).unwrap_or(false),
        timed_out,
        output: output.trim().to_owned(),
    }
}

fn read_thread<R: Read + Send + 'static>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = reader.read_to_end(&mut buffer);
        buffer
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_script_is_a_noop() {
        let outcome = run("   ", Path::new("."), 5);
        assert!(!outcome.ran);
        assert!(outcome.success);
    }

    #[test]
    fn script_runs_in_the_bench_folder() {
        let outcome = run("echo forja-script", Path::new("."), 20);
        if !outcome.ran {
            return;
        }
        assert!(outcome.success, "{}", outcome.output);
        assert!(
            outcome.output.contains("forja-script"),
            "{}",
            outcome.output
        );
    }
}
