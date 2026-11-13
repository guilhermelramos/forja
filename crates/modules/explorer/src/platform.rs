//! Operating-system calls of the Files panel: Recycle Bin and "reveal in Explorer".
//!
//! These run inside kernel tasks. Nothing here is called on the interface thread.

use std::io;
use std::path::Path;

/// Sends `path` to the Windows Recycle Bin.
///
/// Uses `SHFileOperationW` with `FOF_ALLOWUNDO`, so the item can be restored from
/// the Recycle Bin itself. Forja does not undo this step: Forja's undo history
/// does not restore items from the bin.
#[cfg(windows)]
pub fn trash(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_SILENT, FO_DELETE, SHFILEOPSTRUCTW,
    };

    // SHFileOperationW wants a double-NUL-terminated list of paths.
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    wide.push(0);
    let flags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT) as u16;
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: wide.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: flags,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `op` and the wide buffer stay alive for the duration of the call,
    // and the buffer is double-NUL-terminated as the API requires.
    let code = unsafe { SHFileOperationW(&mut op) };
    if code != 0 || op.fAnyOperationsAborted != 0 {
        return Err(io::Error::other(format!(
            "could not move to the Recycle Bin (code {code})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn trash(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the Recycle Bin is only available on Windows",
    ))
}

/// Opens Windows Explorer with `path` selected. The process is not awaited.
#[cfg(windows)]
pub fn reveal(path: &Path) -> io::Result<()> {
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .stdin(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
pub fn reveal(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "revealing files is only available on Windows",
    ))
}

/// Opens `path` with the system's default app. Used when no editor module answers.
#[cfg(windows)]
pub fn open_default(path: &Path) -> io::Result<()> {
    std::process::Command::new("explorer.exe")
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

#[cfg(test)]
mod tests {
    #[cfg(not(windows))]
    #[test]
    fn trash_reports_unsupported_off_windows() {
        let error = super::trash(std::path::Path::new("/tmp/x")).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    }
}
