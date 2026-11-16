//! Crash records and the optional native error dialog.

use forja_kernel::i18n::{BundleSource, I18n, Locale};
use forja_kernel::logging::{self, LogConfig, LogLevel};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static DATA_DIR: OnceLock<Mutex<PathBuf>> = OnceLock::new();
static CRASH_DIALOG_ENABLED: AtomicBool = AtomicBool::new(false);
static CRASH_LOCALE: OnceLock<Mutex<Locale>> = OnceLock::new();

thread_local! {
    static HANDLED_VIEW_PANIC_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Installs the application panic hook before the event loop starts.
pub fn install_panic_hook() {
    let data_dir = forja_kernel::paths::default_data_dir();
    set_data_dir(data_dir.clone());
    set_locale(initial_locale());
    logging::initialize(data_dir, LogConfig::default());
    CRASH_DIALOG_ENABLED.store(
        should_show_dialog(std::env::args_os(), cfg!(test)),
        Ordering::Release,
    );
    std::panic::set_hook(Box::new(|info| {
        let report = panic_report(info);
        let path = logging::write_crash_log(current_data_dir(), &report);
        let path = match path {
            Ok(path) => path,
            Err(error) => {
                logging::log(
                    LogLevel::Error,
                    format!("could not write panic report: {error}"),
                );
                logging::logs_dir(current_data_dir()).join("crash-log-unavailable.log")
            }
        };
        logging::log(
            LogLevel::Error,
            format!("panic captured; report: {}", path.display()),
        );
        if CRASH_DIALOG_ENABLED.load(Ordering::Acquire) && !is_handled_view_panic() {
            show_native_error(&path);
        }
    }));
}

/// Changes the state folder used by the panic hook and the logger.
pub fn configure_data_dir(data_dir: impl Into<PathBuf>) {
    let data_dir = data_dir.into();
    set_data_dir(data_dir.clone());
    logging::configure(data_dir, LogConfig::default());
}

/// Updates the language shown by the native panic dialog.
pub fn set_locale(locale: Locale) {
    if let Some(slot) = CRASH_LOCALE.get() {
        if let Ok(mut current) = slot.lock() {
            *current = locale;
        }
    } else {
        let _ = CRASH_LOCALE.set(Mutex::new(locale));
    }
}

/// Runs a view callback with a panic hook context that suppresses the native
/// dialog while keeping the crash report. The caller decides how to recover.
pub fn catch_view_panic<T>(callback: impl FnOnce() -> T) -> std::thread::Result<T> {
    HANDLED_VIEW_PANIC_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback));
    HANDLED_VIEW_PANIC_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    result
}

/// Extracts a readable payload from a caught panic.
pub fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&'static str>().map(|text| (*text).to_owned()))
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

fn set_data_dir(data_dir: PathBuf) {
    if let Some(slot) = DATA_DIR.get() {
        if let Ok(mut current) = slot.lock() {
            *current = data_dir;
        }
    } else {
        let _ = DATA_DIR.set(Mutex::new(data_dir));
    }
}

fn current_data_dir() -> PathBuf {
    DATA_DIR
        .get()
        .and_then(|slot| slot.lock().ok().map(|path| path.clone()))
        .unwrap_or_else(forja_kernel::paths::default_data_dir)
}

fn initial_locale() -> Locale {
    std::env::var("FORJA_LOCALE")
        .ok()
        .filter(|locale| !locale.eq_ignore_ascii_case("auto"))
        .map(|locale| Locale::parse(&locale))
        .unwrap_or_else(crate::system::system_locale)
}

fn should_show_dialog(args: impl IntoIterator<Item = std::ffi::OsString>, test_build: bool) -> bool {
    !test_build
        && !args.into_iter().any(|argument| {
            let argument = argument.to_string_lossy();
            argument == "--test-driver"
                || argument == "--screenshot"
                || argument.starts_with("--screenshot=")
        })
}

fn is_handled_view_panic() -> bool {
    HANDLED_VIEW_PANIC_DEPTH.with(|depth| depth.get() > 0)
}

fn panic_report(info: &std::panic::PanicHookInfo<'_>) -> String {
    let message = panic_payload_message(info.payload());
    let location = info
        .location()
        .map(|location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "message: {message}\nlocation: {location}\nthread: {thread_name} ({:?})\n\nbacktrace:\n{backtrace}\n",
        thread.id()
    )
}

fn dialog_i18n() -> I18n {
    let locale = CRASH_LOCALE
        .get()
        .and_then(|slot| slot.lock().ok().map(|locale| *locale))
        .unwrap_or_else(initial_locale);
    I18n::build(
        &[BundleSource {
            module: "shell".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        }],
        locale,
    )
    .0
}

fn show_native_error(path: &Path) {
    let i18n = dialog_i18n();
    let title = i18n.t("shell.crash.title");
    let path = path.display().to_string();
    let message = i18n.t_args("shell.crash.body", &[("path", &path)]);
    show_message_box(&title, &message);
}

#[cfg(windows)]
fn show_message_box(title: &str, message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let message: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: Both strings are NUL-terminated and remain alive for the call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_message_box(_title: &str, _message: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_and_capture_modes_never_show_a_native_dialog() {
        let args = |values: &[&str]| values.iter().map(std::ffi::OsString::from).collect::<Vec<_>>();
        assert!(!should_show_dialog(args(&["forja-app", "--test-driver"]), false));
        assert!(!should_show_dialog(args(&["forja-app", "--screenshot", "out.png"]), false));
        assert!(!should_show_dialog(args(&["forja-app"]), true));
        assert!(should_show_dialog(args(&["forja-app"]), false));
    }

    #[test]
    fn view_panics_are_marked_as_recoverable_during_unwind() {
        assert!(!is_handled_view_panic());
        let result = catch_view_panic(|| {
            assert!(is_handled_view_panic());
            panic!("view failure");
        });
        assert!(result.is_err());
        assert!(!is_handled_view_panic());
    }
}
