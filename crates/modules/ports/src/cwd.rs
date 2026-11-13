//! Best-effort current directory of another process.
//!
//! Windows has no public `GetProcessCurrentDirectory`. On 64-bit processes we
//! read the PEB, the process parameters and `CurrentDirectory.DosPath`, all with
//! `ReadProcessMemory`. Access denied is normal for services: the caller gets
//! `None` and the port stays without a bench guess.

use std::path::PathBuf;

#[cfg(windows)]
pub fn process_cwd(pid: u32) -> Option<PathBuf> {
    if pid == 0 || pid == 4 || !cfg!(target_pointer_width = "64") {
        return None;
    }
    unsafe { read_cwd(pid) }
}

#[cfg(not(windows))]
pub fn process_cwd(_pid: u32) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
unsafe fn read_cwd(pid: u32) -> Option<PathBuf> {
    use windows_sys::Wdk::System::Threading::NtQueryInformationProcess;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let process = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
    if process.is_null() {
        return None;
    }
    let result = read_cwd_handle(process, ReadProcessMemory, NtQueryInformationProcess);
    let _ = CloseHandle(process);
    result
}

#[cfg(windows)]
unsafe fn read_cwd_handle(
    process: windows_sys::Win32::Foundation::HANDLE,
    read: unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HANDLE,
        *const core::ffi::c_void,
        *mut core::ffi::c_void,
        usize,
        *mut usize,
    ) -> i32,
    query: unsafe extern "system" fn(
        windows_sys::Win32::Foundation::HANDLE,
        i32,
        *mut core::ffi::c_void,
        u32,
        *mut u32,
    ) -> i32,
) -> Option<PathBuf> {
    let mut info = [0u8; 48];
    let status = query(
        process,
        0,
        info.as_mut_ptr() as *mut core::ffi::c_void,
        info.len() as u32,
        std::ptr::null_mut(),
    );
    if status < 0 {
        return None;
    }
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
