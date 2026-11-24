//! Child processes that never open a window.
//!
//! Forja is a window app. On Windows, a console program (git, tasklist,
//! pwsh…) started by a window app gets a brand-new console window unless it
//! is created with `CREATE_NO_WINDOW`. Every module starts processes through
//! [`command`], so no terminal ever flashes on the person's screen. The
//! integrated terminal uses ConPTY (headless) and is not affected.
//!
//! [`open_external`], [`reveal`] and [`reveal_folder`] are the single exits to
//! the system's default apps (browser, Explorer, file associations). Every
//! module uses them, so a test run has one place where those exits can be
//! closed: with `--test-driver`, `--screenshot`, `--demo` or `cfg(test)` nothing
//! is launched, and the request is recorded for [`external_requests`] instead.
//! The shell also forwards egui's URL and clipboard output to
//! [`record_external`] in a test run.

use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
#[cfg(test)]
use std::sync::atomic::AtomicU64;

/// `CREATE_NO_WINDOW` process creation flag.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

static TEST_MODE: AtomicBool = AtomicBool::new(false);
static TEST_ENV: OnceLock<bool> = OnceLock::new();
static EXTERNALS: Mutex<Vec<ExternalRequest>> = Mutex::new(Vec::new());
/// Keeps a long play-test from growing the log without bound.
const MAX_RECORDED_EXTERNALS: usize = 256;
#[cfg(test)]
static LAUNCHES: AtomicU64 = AtomicU64::new(0);

/// What a recorded exit to the system was asking for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalKind {
    /// Open a URL or path in the system's default app.
    Open,
    /// Write text (or an image) to the system clipboard.
    Copy,
    /// Show a path in Windows Explorer (selected or as an opened folder).
    Reveal,
}

impl ExternalKind {
    /// Stable name used by the test driver's `externals` field.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Copy => "copy",
            Self::Reveal => "reveal",
        }
    }
}

/// One exit to the system that a test run recorded instead of performing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalRequest {
    pub kind: ExternalKind,
    pub target: String,
}

/// Turns off every real process launch, including [`open_external`], for this
/// process. `forja-app` calls it with `--test-driver`, `--screenshot` and
/// `--demo`; those flags also set the environment variables below before the
/// modules are built.
pub fn set_test_mode(enabled: bool) {
    TEST_MODE.store(enabled, Ordering::Relaxed);
}

/// Whether this process must never reach the person's browser, Explorer or
/// clipboard. Unit tests of the kernel are test runs too.
///
/// `forja-app` sets `FORJA_TEST_DRIVER` / `FORJA_SCREENSHOT` itself when it
/// starts with those flags (see `crates/shell/src/main.rs`), and other modules
/// use the same convention. OR-ing the environment with the in-process flag
/// keeps the kernel in step with them even before `set_test_mode` runs. It is
/// read once per process instead of twice on every frame.
pub fn is_test_mode() -> bool {
    TEST_MODE.load(Ordering::Relaxed)
        || cfg!(test)
        || *TEST_ENV.get_or_init(|| {
            env_flag("FORJA_TEST_DRIVER") || env_flag("FORJA_SCREENSHOT")
        })
}

fn env_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| {
        let value = value.to_string_lossy();
        !value.is_empty() && value != "0" && value != "false"
    })
}

/// Records an egui URL or clipboard request. The shell calls this in a test
/// run, before eframe can hand the output to the operating system.
pub fn record_external(kind: ExternalKind, target: impl Into<String>) {
    let mut requests = EXTERNALS.lock().unwrap_or_else(|error| error.into_inner());
    if requests.len() >= MAX_RECORDED_EXTERNALS {
        requests.remove(0);
    }
    requests.push(ExternalRequest {
        kind,
        target: target.into(),
    });
}

/// Requests recorded so far, in order. Exposed by the shell's `ui.*` driver.
pub fn external_requests() -> Vec<ExternalRequest> {
    EXTERNALS.lock().unwrap_or_else(|error| error.into_inner()).clone()
}

/// Forgets recorded requests, e.g. between two play-test scenarios in one process.
pub fn clear_external_requests() {
    EXTERNALS.lock().unwrap_or_else(|error| error.into_inner()).clear();
}

/// Opens a URL or path in the system's default app.
///
/// In a test run this only records the request and returns `Ok(())`. The real
/// launch happens on a helper thread so no process is created on the UI
/// thread; launch errors are logged because callers do not wait for the thread.
pub fn open_external(target: impl Into<String>) -> Result<(), String> {
    let target = target.into();
    if is_test_mode() {
        record_external(ExternalKind::Open, target);
        return Ok(());
    }
    let label = target.clone();
    std::thread::Builder::new()
        .name("forja-external".to_owned())
        .spawn(move || {
            if let Err(error) = launch_external(&target) {
                crate::logging::log(
                    crate::logging::LogLevel::Warn,
                    format!("forja: could not open {label}: {error}"),
                );
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Shows `path` selected in Windows Explorer.
///
/// In a test run this only records a [`ExternalKind::Reveal`] request and
/// launches nothing. Outside a test run it performs the same
/// `explorer.exe /select,<path>` launch the Files panel used before exits were
/// centralised here.
pub fn reveal(path: impl AsRef<Path>) -> std::io::Result<()> {
    launch_reveal(path.as_ref(), true)
}

/// Opens a folder in Windows Explorer. Also used for "open with the default
/// app" paths, which is what `explorer.exe <path>` does on Windows.
///
/// Like [`reveal`], a test run only records the request.
pub fn reveal_folder(path: impl AsRef<Path>) -> std::io::Result<()> {
    launch_reveal(path.as_ref(), false)
}

fn launch_reveal(path: &Path, select: bool) -> std::io::Result<()> {
    if is_test_mode() {
        record_external(ExternalKind::Reveal, path.display().to_string());
        return Ok(());
    }
    #[cfg(test)]
    LAUNCHES.fetch_add(1, Ordering::Relaxed);
    #[cfg(windows)]
    {
        let arg = if select {
            format!("/select,{}", path.display())
        } else {
            path.display().to_string()
        };
        command("explorer.exe")
            .arg(arg)
            .stdin(std::process::Stdio::null())
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "revealing files is only available on Windows",
        ))
    }
}

fn launch_external(target: &str) -> std::io::Result<()> {
    #[cfg(test)]
    LAUNCHES.fetch_add(1, Ordering::Relaxed);
    #[cfg(windows)]
    {
        command("cmd")
            .args(["/C", "start", "", target])
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(windows))]
    {
        command("xdg-open").arg(target).spawn().map(|_| ())
    }
}

/// A `std::process::Command` that never opens a console window.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_window(&mut command);
    command
}

/// Marks an existing command so it never opens a console window.
pub fn hide_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn hidden_command_still_runs() {
        let output = super::command("cmd").args(["/C", "echo forja"]).output();
        if let Ok(output) = output {
            assert!(String::from_utf8_lossy(&output.stdout).contains("forja"));
        }
    }

    #[test]
    fn test_mode_open_external_records_without_spawning() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        assert!(is_test_mode());
        clear_external_requests();
        let launches = LAUNCHES.load(Ordering::Relaxed);
        let target = "https://example.invalid/forja-isolar-externos";

        open_external(target).unwrap();

        assert_eq!(LAUNCHES.load(Ordering::Relaxed), launches);
        let requests = external_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, ExternalKind::Open);
        assert_eq!(requests[0].target, target);
    }

    #[test]
    fn test_mode_reveal_records_without_spawning() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        assert!(is_test_mode());
        clear_external_requests();
        let launches = LAUNCHES.load(Ordering::Relaxed);
        let path = std::path::Path::new("C:\\forja-test\\arquivo.txt");

        reveal(path).unwrap();
        reveal_folder(path).unwrap();

        assert_eq!(LAUNCHES.load(Ordering::Relaxed), launches);
        let requests = external_requests();
        assert_eq!(requests.len(), 2);
        for request in &requests {
            assert_eq!(request.kind, ExternalKind::Reveal);
            assert_eq!(request.kind.as_str(), "reveal");
            assert_eq!(request.target, path.display().to_string());
        }
    }

    #[test]
    fn recorded_externals_keep_order_and_are_bounded() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        clear_external_requests();
        record_external(ExternalKind::Copy, "copiar isto");
        for index in 0..MAX_RECORDED_EXTERNALS {
            record_external(ExternalKind::Open, format!("https://example.invalid/{index}"));
        }
        let requests = external_requests();
        assert_eq!(requests.len(), MAX_RECORDED_EXTERNALS);
        assert_eq!(requests[0].kind, ExternalKind::Open);
        assert_eq!(requests[0].target, "https://example.invalid/0");
        assert_eq!(requests.last().unwrap().target, format!("https://example.invalid/{}", MAX_RECORDED_EXTERNALS - 1));
        clear_external_requests();
    }
}
