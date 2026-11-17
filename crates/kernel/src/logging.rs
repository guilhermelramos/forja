//! Local application diagnostics with size-based rotation.
//!
//! The shell initializes the writer once. Producers only copy a short record
//! into a channel; directory creation, rotation and file writes run on the
//! logger thread. Crash records use [`write_crash_log`] synchronously so a
//! fatal panic leaves a durable trace before the process exits.

use crate::settings::{SettingValue, Settings};
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

/// The least detailed message level written to the app log.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Important failures that prevent or degrade an operation.
    Error,
    /// Recoverable conditions that need attention.
    Warn,
    /// Routine application activity.
    Info,
    /// Additional information useful during diagnosis.
    Debug,
    /// The most detailed diagnostic level.
    Trace,
}

impl LogLevel {
    /// Parses one of the values accepted by `app.log.level`.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "error" => Self::Error,
            "warn" => Self::Warn,
            "debug" => Self::Debug,
            "trace" => Self::Trace,
            _ => Self::Info,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

/// Active settings for the local app log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogConfig {
    /// Highest detail level included in `forja.log`.
    pub level: LogLevel,
    /// Maximum size of each log file, in mebibytes.
    pub max_mb: u64,
    /// Total files to retain, including `forja.log`.
    pub keep: usize,
    /// Whether terminal output is copied to the local app log.
    pub include_terminal_output: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            max_mb: 20,
            keep: 5,
            include_terminal_output: false,
        }
    }
}

impl LogConfig {
    /// Reads the log settings from the runtime's validated settings snapshot.
    pub fn from_settings(settings: &Settings) -> Self {
        let defaults = Self::default();
        let level = settings
            .get("app.log.level")
            .and_then(SettingValue::as_str)
            .map(LogLevel::parse)
            .unwrap_or(defaults.level);
        let max_mb = settings
            .get("app.log.max_mb")
            .and_then(SettingValue::as_f64)
            .map(|value| value.round() as u64)
            .unwrap_or(defaults.max_mb)
            .clamp(1, 100);
        let keep = settings
            .get("app.log.keep")
            .and_then(SettingValue::as_f64)
            .map(|value| value.round() as usize)
            .unwrap_or(defaults.keep)
            .clamp(1, 20);
        let include_terminal_output = settings
            .get("app.log.include_terminal_output")
            .and_then(SettingValue::as_bool)
            .unwrap_or(false);
        Self {
            level,
            max_mb,
            keep,
            include_terminal_output,
        }
    }

    fn max_bytes(&self) -> u64 {
        self.max_mb.saturating_mul(1024 * 1024)
    }
}

struct LoggerState {
    data_dir: PathBuf,
    config: LogConfig,
}

struct Record {
    state: LoggerState,
    level: LogLevel,
    message: String,
    thread_name: String,
    thread_id: String,
}

enum Message {
    Record(Record),
    Flush(mpsc::Sender<()>),
}

struct Logger {
    state: Mutex<LoggerState>,
    sender: mpsc::Sender<Message>,
}

static LOGGER: OnceLock<Logger> = OnceLock::new();

/// Starts the background writer. Repeated initialization updates its settings.
pub fn initialize(data_dir: impl Into<PathBuf>, config: LogConfig) {
    let data_dir = data_dir.into();
    if LOGGER.get().is_some() {
        configure(data_dir, config);
        return;
    }
    let (sender, receiver) = mpsc::channel::<Message>();
    let _ = std::thread::Builder::new()
        .name("forja-log-writer".to_owned())
        .spawn(move || {
            while let Ok(message) = receiver.recv() {
                match message {
                    Message::Record(record) => {
                        let _ = append_record(
                            &record.state.data_dir,
                            &record.state.config,
                            record.level,
                            &record.message,
                            &record.thread_name,
                            &record.thread_id,
                        );
                    }
                    Message::Flush(sender) => {
                        let _ = sender.send(());
                    }
                }
            }
        });
    let logger = Logger {
        state: Mutex::new(LoggerState { data_dir, config }),
        sender,
    };
    let _ = LOGGER.set(logger);
}

/// Updates the folder or settings used for later records without doing disk I/O.
pub fn configure(data_dir: impl Into<PathBuf>, config: LogConfig) {
    if let Some(logger) = LOGGER.get() {
        if let Ok(mut state) = logger.state.lock() {
            state.data_dir = data_dir.into();
            state.config = config;
        }
    }
}

/// Queues one diagnostic record for the background writer.
pub fn log(level: LogLevel, message: impl std::fmt::Display) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let Ok(state) = logger.state.lock() else {
        return;
    };
    if level > state.config.level {
        return;
    }
    let record = Record {
        state: LoggerState {
            data_dir: state.data_dir.clone(),
            config: state.config.clone(),
        },
        level,
        message: message.to_string(),
        thread_name: thread_name(),
        thread_id: format!("{:?}", std::thread::current().id()),
    };
    let _ = logger.sender.send(Message::Record(record));
}

/// Adds a terminal-output record only when the opt-in setting is enabled.
/// Control bytes are escaped so terminal sequences cannot forge log lines.
pub fn log_terminal_output(bytes: &[u8]) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let Ok(state) = logger.state.lock() else {
        return;
    };
    if !state.config.include_terminal_output || LogLevel::Info > state.config.level {
        return;
    }
    let escaped = escape_terminal_bytes(bytes);
    let record = Record {
        state: LoggerState {
            data_dir: state.data_dir.clone(),
            config: state.config.clone(),
        },
        level: LogLevel::Info,
        message: format!("terminal output: {escaped}"),
        thread_name: thread_name(),
        thread_id: format!("{:?}", std::thread::current().id()),
    };
    let _ = logger.sender.send(Message::Record(record));
}

/// Waits briefly for queued log records to reach disk.
pub fn flush() {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    let (sender, receiver) = mpsc::channel();
    if logger.sender.send(Message::Flush(sender)).is_ok() {
        let _ = receiver.recv_timeout(Duration::from_secs(2));
    }
}

/// Path of the log file the running writer is appending to, when it is up.
/// Tests read it instead of assuming where `initialize` sent the lines: the
/// writer is global and the first initialization wins the destination.
pub fn current_log_path() -> Option<PathBuf> {
    let logger = LOGGER.get()?;
    let state = logger.state.lock().ok()?;
    Some(logs_dir(&state.data_dir).join("forja.log"))
}

/// Returns the Forja log folder for a data directory.
pub fn logs_dir(data_dir: impl AsRef<Path>) -> PathBuf {
    data_dir.as_ref().join("logs")
}

/// Writes a fatal panic report and returns its unique path.
pub fn write_crash_log(data_dir: impl AsRef<Path>, contents: &str) -> io::Result<PathBuf> {
    let directory = logs_dir(data_dir);
    fs::create_dir_all(&directory)?;
    let timestamp = Local::now().format("%Y-%m-%d-%H%M%S");
    for suffix in 0..1000u16 {
        let name = if suffix == 0 {
            format!("crash-{timestamp}.log")
        } else {
            format!("crash-{timestamp}-{suffix}.log")
        };
        let path = directory.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(contents.as_bytes())?;
                file.flush()?;
                file.sync_data()?;
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "too many crash records in one second",
    ))
}

fn thread_name() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned()
}

fn append_record(
    data_dir: &Path,
    config: &LogConfig,
    level: LogLevel,
    message: &str,
    thread_name: &str,
    thread_id: &str,
) -> io::Result<()> {
    let directory = logs_dir(data_dir);
    fs::create_dir_all(&directory)?;
    let path = directory.join("forja.log");
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!(
        "{timestamp} [{}] [{thread_name} {thread_id}] {message}\n",
        level.label()
    );
    let current_size = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
    if current_size.saturating_add(line.len() as u64) > config.max_bytes() {
        rotate(&directory, config.keep)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

fn rotate(directory: &Path, keep: usize) -> io::Result<()> {
    let current = directory.join("forja.log");
    if keep <= 1 {
        match fs::remove_file(&current) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    let oldest = directory.join(format!("forja.{}.log", keep - 1));
    match fs::remove_file(oldest) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    for index in (1..keep - 1).rev() {
        let source = directory.join(format!("forja.{index}.log"));
        let destination = directory.join(format!("forja.{}.log", index + 1));
        match fs::rename(source, destination) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if current.exists() {
        fs::rename(current, directory.join("forja.1.log"))?;
    }
    Ok(())
}

fn escape_terminal_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .flat_map(|character| match character {
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            '\u{1b}' => "\\x1b".chars().collect(),
            character if character.is_control() => format!("\\u{{{:x}}}", character as u32).chars().collect(),
            character => vec![character],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = Local::now().format("%Y%m%d%H%M%S%f");
        std::env::temp_dir().join(format!("forja-logs-{label}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn crash_record_uses_the_configured_data_directory_and_contains_report() {
        let data = temp_dir("crash");
        let path = write_crash_log(&data, "message\nlocation\nthread\nbacktrace").unwrap();
        assert_eq!(path.parent().unwrap(), logs_dir(&data));
        assert!(path.file_name().unwrap().to_string_lossy().starts_with("crash-"));
        assert!(path.file_name().unwrap().to_string_lossy().ends_with(".log"));
        assert_eq!(fs::read_to_string(path).unwrap(), "message\nlocation\nthread\nbacktrace");
        let _ = fs::remove_dir_all(data);
    }

    #[test]
    fn log_rotation_keeps_the_configured_number_of_files() {
        let data = temp_dir("rotation");
        let config = LogConfig {
            level: LogLevel::Trace,
            max_mb: 1,
            keep: 2,
            include_terminal_output: false,
        };
        let directory = logs_dir(&data);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("forja.log"), vec![b'x'; 1024 * 1024]).unwrap();
        append_record(
            &data,
            &config,
            LogLevel::Info,
            "after rotation",
            "test",
            "ThreadId(1)",
        )
        .unwrap();
        assert!(directory.join("forja.log").is_file());
        assert!(directory.join("forja.1.log").is_file());
        assert!(!directory.join("forja.2.log").exists());
        assert!(fs::read_to_string(directory.join("forja.log"))
            .unwrap()
            .contains("after rotation"));
        let _ = fs::remove_dir_all(data);
    }

    #[test]
    fn terminal_output_is_escaped_before_logging() {
        assert_eq!(escape_terminal_bytes(b"a\r\n\x1b[31m"), "a\\r\\n\\x1b[31m");
    }
}
