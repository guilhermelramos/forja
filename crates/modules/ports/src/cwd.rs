//! Best-effort current directory and parent PID of another process.
//!
//! Windows has no public `GetProcessCurrentDirectory`. On 64-bit processes we
//! read `NtQueryInformationProcess` (`ProcessBasicInformation`, which gives both
//! PEB address and `InheritedFromUniqueProcessId`) and `ReadProcessMemory` on
//! `CurrentDirectory.DosPath`. Access denied is normal for services: the caller
//! gets `None` and falls back cleanly.

use std::path::PathBuf;

/// Current directory and parent PID retrieved for a process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessInspection {
    pub cwd: Option<PathBuf>,
    pub parent_pid: Option<u32>,
}

#[cfg(windows)]
pub fn inspect_process(pid: u32) -> ProcessInspection {
    if pid == 0 || pid == 4 || !cfg!(target_pointer_width = "64") {
        return ProcessInspection::default();
    }
    unsafe { read_inspection(pid) }
}

#[cfg(not(windows))]
pub fn inspect_process(_pid: u32) -> ProcessInspection {
    ProcessInspection::default()
}

#[cfg(windows)]
unsafe fn read_inspection(pid: u32) -> ProcessInspection {
    use windows_sys::Wdk::System::Threading::NtQueryInformationProcess;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
    if !process.is_null() {
        let result = inspect_handle(process, Some(ReadProcessMemory), NtQueryInformationProcess);
        let _ = CloseHandle(process);
        return result;
    }

    let limited = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if !limited.is_null() {
        let result = inspect_handle(limited, None, NtQueryInformationProcess);
        let _ = CloseHandle(limited);
        return result;
    }

    ProcessInspection::default()
}

#[cfg(windows)]
unsafe fn inspect_handle(
    process: windows_sys::Win32::Foundation::HANDLE,
    read: Option<
        unsafe extern "system" fn(
            windows_sys::Win32::Foundation::HANDLE,
            *const core::ffi::c_void,
            *mut core::ffi::c_void,
            usize,
            *mut usize,
        ) -> i32,
    >,
    query: unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HANDLE,
        i32,
        *mut core::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32,
) -> ProcessInspection {
    let mut info = [0u8; 48];
    let mut returned = 0u32;
    let status = query(
        process,
        0,
        info.as_mut_ptr() as *mut core::ffi::c_void,
        info.len() as u32,
        &mut returned,
    );
    if status < 0 {
        return ProcessInspection::default();
    }
    let parent_pid = info[40..48]
        .try_into()
        .ok()
        .map(u64::from_le_bytes)
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);

    let cwd = read.and_then(|read_fn| read_cwd_from_info(process, read_fn, &info));
    ProcessInspection { cwd, parent_pid }
}

#[cfg(windows)]
unsafe fn read_cwd_from_info(
    process: windows_sys::Win32::Foundation::HANDLE,
    read: unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HANDLE,
        *const core::ffi::c_void,
        *mut core::ffi::c_void,
        usize,
        *mut usize,
    ) -> i32,
    info: &[u8; 48],
) -> Option<PathBuf> {
    let peb = usize::from_le_bytes(info[8..16].try_into().ok()?);
    if peb == 0 {
        return None;
    }
    let parameters = read_pointer(process, read, peb + 0x20)?;
    if parameters == 0 {
        return None;
    }
    let mut current = [0u8; 0x48];
    let mut returned = 0usize;
    if read(
        process,
        parameters as *const core::ffi::c_void,
        current.as_mut_ptr() as *mut core::ffi::c_void,
        current.len(),
        &mut returned,
    ) == 0
    {
        return None;
    }
    let length = u16::from_le_bytes(current[0x38..0x3A].try_into().ok()?) as usize;
    if length == 0 || length > 32_768 {
        return None;
    }
    let buffer = usize::from_le_bytes(current[0x40..0x48].try_into().ok()?);
    if buffer == 0 {
        return None;
    }
    let mut wide = vec![0u16; length / 2];
    if read(
        process,
        buffer as *const core::ffi::c_void,
        wide.as_mut_ptr() as *mut core::ffi::c_void,
        length,
        &mut returned,
    ) == 0
    {
        return None;
    }
    let text = String::from_utf16_lossy(&wide);
    let text = text.trim_matches('\0').trim();
    if text.is_empty() {
        None
    } else {
        Some(PathBuf::from(text))
    }
}

#[cfg(windows)]
unsafe fn read_pointer(
    process: windows_sys::Win32::Foundation::HANDLE,
    read: unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HANDLE,
        *const core::ffi::c_void,
        *mut core::ffi::c_void,
        usize,
        *mut usize,
    ) -> i32,
    address: usize,
) -> Option<usize> {
    let mut buffer = [0u8; 8];
    let mut returned = 0usize;
    if read(
        process,
        address as *const core::ffi::c_void,
        buffer.as_mut_ptr() as *mut core::ffi::c_void,
        buffer.len(),
        &mut returned,
    ) == 0
    {
        None
    } else {
        Some(usize::from_le_bytes(buffer))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn self_inspection_reads_cwd_and_parent_pid() {
        let info = inspect_process(std::process::id());
        assert!(info.cwd.is_some());
        assert!(info.parent_pid.is_some());
        assert_eq!(inspect_process(0), ProcessInspection::default());
        assert_eq!(inspect_process(4), ProcessInspection::default());

        let temp = std::env::temp_dir();
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 2 127.0.0.1 >NUL"])
            .current_dir(&temp)
            .spawn()
            .expect("spawn cmd child");
        std::thread::sleep(std::time::Duration::from_millis(150));
        let child_info = inspect_process(child.id());
        let _ = child.kill();
        let _ = child.wait();
        assert!(child_info.cwd.is_some(), "child cwd: {child_info:?}");
        assert_eq!(child_info.parent_pid, Some(std::process::id()));
    }
}
