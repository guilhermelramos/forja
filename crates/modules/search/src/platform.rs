//! Operating-system call of the finder: opens a file with the system's default app.
//! It runs inside a kernel task, never on the interface thread.

use std::io;
use std::path::Path;

/// Opens `path` with its associated app. Used when no editor module answers.
///
/// The kernel's single exit records the request instead of launching Explorer
/// when this is a test run.
pub fn open_default(path: &Path) -> io::Result<()> {
    forja_kernel::process::reveal_folder(path)
}
