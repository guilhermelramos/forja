//! Operating-system call of the finder: opens a file with the system's default app.
//! It runs inside a kernel task, never on the interface thread.

use std::io;
use std::path::Path;

/// Opens `path` with its associated app. Used when no editor module answers.
#[cfg(windows)]
pub fn open_default(path: &Path) -> io::Result<()> {
    forja_kernel::process::command("explorer.exe")
        .arg(path)
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
pub fn open_default(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "opening with the default app is only available on Windows",
    ))
}
