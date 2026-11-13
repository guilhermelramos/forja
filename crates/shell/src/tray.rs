//! Windows tray. Closing the window hides it here when `app.tray.enabled` is on.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

const SHOW: u8 = 1;
const QUIT: u8 = 2;

static ACTION: AtomicU8 = AtomicU8::new(0);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static LABELS: Mutex<Labels> = Mutex::new(Labels {
    open: Vec::new(),
    quit: Vec::new(),
    tip: Vec::new(),
});

struct Labels {
    open: Vec<u16>,
    quit: Vec<u16>,
    tip: Vec<u16>,
}

pub enum TrayAction {
    Show,
    Quit,
}

pub fn poll() -> Option<TrayAction> {
    match ACTION.swap(0, Ordering::Relaxed) {
        SHOW => Some(TrayAction::Show),
        QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

pub fn ensure(open: &str, quit: &str, tip: &str) {
    if let Ok(mut labels) = LABELS.lock() {
        labels.open = wide(open);
        labels.quit = wide(quit);
        labels.tip = wide(tip);
    }
    #[cfg(windows)]
    {
        if !INSTALLED.load(Ordering::Relaxed) {
            install();
        }
    }
}

pub fn remove() {
    #[cfg(windows)]
    {
        if INSTALLED.swap(false, Ordering::Relaxed) {
            delete_icon();
        }
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn install() {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;

    static HWND_SLOT: OnceLock<usize> = OnceLock::new();
    let hwnd = *HWND_SLOT.get_or_init(|| unsafe { create_message_window() as usize });
    if hwnd == 0 {
        return;
    }
    let icon = load_icon();
    let tip = LABELS.lock().map(|labels| labels.tip.clone()).unwrap_or_else(|_| wide("Forja"));
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd as HWND;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = WM_APP + 1;
    data.hIcon = icon;
    copy_units(&mut data.szTip, &tip);
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
    if added != 0 {
        INSTALLED.store(true, Ordering::Relaxed);
    }
}

#[cfg(windows)]
unsafe fn create_message_window() -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, RegisterClassExW, HWND_MESSAGE, WNDCLASSEXW,
    };
    let class = class_name();
    let instance = GetModuleHandleW(std::ptr::null());
    let info = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(tray_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..WNDCLASSEXW::default()
    };
    RegisterClassExW(&info);
    CreateWindowExW(
        0,
        class.as_ptr(),
        class.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    )
}

#[cfg(windows)]
fn class_name() -> &'static [u16] {
    use std::sync::OnceLock;
    static NAME: OnceLock<Vec<u16>> = OnceLock::new();
    NAME.get_or_init(|| wide("ForjaTray"))
}

#[cfg(windows)]
fn load_icon() -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
    use windows_sys::Win32::UI::WindowsAndMessaging::{LoadImageW, IMAGE_ICON, LR_LOADFROMFILE};
    let path = std::env::temp_dir().join("forja-window.ico");
    if !path.exists() {
        let _ = std::fs::write(&path, crate::icon::tray_ico_bytes());
    }
    let wide_path = wide(&path.display().to_string());
    unsafe { LoadImageW(std::ptr::null_mut(), wide_path.as_ptr(), IMAGE_ICON, 32, 32, LR_LOADFROMFILE) as _ }
}

#[cfg(windows)]
fn delete_icon() {
    use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NOTIFYICONDATAW, NIM_DELETE};
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.uID = 1;
    unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
}

#[cfg(windows)]
fn copy_units(slot: &mut [u16], text: &[u16]) {
    slot.fill(0);
    for (index, unit) in text.iter().take(slot.len().saturating_sub(1)).enumerate() {
        slot[index] = *unit;
    }
}

#[cfg(windows)]
unsafe extern "system" fn tray_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, WM_APP, WM_COMMAND, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP,
    };
    if msg == WM_APP + 1 {
        let event = lparam as u32;
        if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
            ACTION.store(SHOW, Ordering::Relaxed);
            return 0;
        }
        if event == WM_RBUTTONUP {
            show_menu(hwnd);
            return 0;
        }
    }
    if msg == WM_COMMAND {
        match wparam & 0xFFFF {
            1 => ACTION.store(SHOW, Ordering::Relaxed),
            2 => ACTION.store(QUIT, Ordering::Relaxed),
            _ => {}
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(windows)]
unsafe fn show_menu(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, GetCursorPos, SetForegroundWindow, TrackPopupMenu, MF_STRING,
        TPM_RIGHTALIGN,
    };
    let (open, quit) = match LABELS.lock() {
        Ok(labels) => (labels.open.clone(), labels.quit.clone()),
        Err(_) => return,
    };
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return;
    }
    AppendMenuW(menu, MF_STRING, 1, open.as_ptr());
    AppendMenuW(menu, MF_STRING, 2, quit.as_ptr());
    let mut point = POINT { x: 0, y: 0 };
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    TrackPopupMenu(menu, TPM_RIGHTALIGN, point.x, point.y, 0, hwnd, std::ptr::null());
}
