//! Native WebView2 tab managed by `wry`.
//!
//! The webview is a child of the Forja window. Its bounds and visibility follow
//! the egui content rectangle; callbacks update a small event queue drained by
//! the view on the next frame.

use crate::download;
use raw_window_handle::{
    HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
};
use std::num::NonZeroIsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};
use wry::dpi::{LogicalPosition, LogicalSize, Position, Size};

#[derive(Clone, Debug, Default)]
pub struct Events {
    pub loading: Option<bool>,
    pub title: Option<String>,
    pub finished: Vec<(String, String)>,
    pub downloads: Vec<String>,
    pub commands: Vec<String>,
    pub error: Option<String>,
}

pub struct NativeTab {
    webview: wry::WebView,
    events: Arc<Mutex<Events>>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub profile_dir: PathBuf,
    pub download_dir: PathBuf,
    pub devtools: bool,
    /// Page zoom applied on top of the DPI scale.
    pub zoom: f64,
    /// `browser.download.ask_each`: ask where to save each download.
    pub ask_each_download: bool,
    /// `browser.auth.open_external`: sign-in flows open in the system browser.
    pub open_auth_external: bool,
    /// `browser.privacy.do_not_track`.
    pub do_not_track: bool,
    pub proxy: Option<wry::ProxyConfig>,
}

impl NativeTab {
    pub fn events(&self) -> Arc<Mutex<Events>> {
        Arc::clone(&self.events)
    }

    pub fn create(
        parent: &impl HasWindowHandle,
        config: &Config,
        target: Target<'_>,
        bounds: wry::Rect,
    ) -> Result<Self, String> {
        preload_loader();
        let events = Arc::new(Mutex::new(Events::default()));
        let load_events = Arc::clone(&events);
        let title_events = Arc::clone(&events);
        let download_events = Arc::clone(&events);
        let command_events = Arc::clone(&events);
        let download_dir = config.download_dir.clone();
        let ask_download = config.ask_each_download;
        let open_auth = config.open_auth_external;
        let mut context = wry::WebContext::new(Some(config.profile_dir.clone()));
        let mut builder = wry::WebViewBuilder::new_with_web_context(&mut context)
            .with_bounds(bounds)
            .with_visible(true)
            .with_devtools(config.devtools)
            .with_initialization_script(SHORTCUTS)
            .with_ipc_handler(move |request| {
                if let Ok(mut events) = command_events.lock() {
                    events.commands.push(request.body().clone());
                }
            })
            .with_on_page_load_handler(move |event, url| {
                if let Ok(mut events) = load_events.lock() {
                    match event {
                        wry::PageLoadEvent::Started => events.loading = Some(true),
                        wry::PageLoadEvent::Finished => {
                            events.loading = Some(false);
                            let title = events.title.clone().unwrap_or_default();
                            events.finished.push((url, title));
                        }
                    }
                }
            })
            .with_document_title_changed_handler(move |title| {
                if let Ok(mut events) = title_events.lock() {
                    events.title = Some(title);
                }
            })
            .with_download_started_handler(move |url, path| {
                let name = download::file_name(&url, path);
                let target = if ask_download {
                    // The question is the user's; the native dialog answers it on this thread.
                    match rfd::FileDialog::new()
                        .set_directory(&download_dir)
                        .set_file_name(&name)
                        .save_file()
                    {
                        Some(chosen) => chosen,
                        None => return false,
                    }
                } else {
                    download_dir.join(&name)
                };
                if let Some(parent) = target.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                *path = target.clone();
                if let Ok(mut events) = download_events.lock() {
                    events.downloads.push(target.display().to_string());
                }
                true
            })
            .with_navigation_handler(move |url| {
                if open_auth && crate::auth::is_auth_flow(&url) {
                    let _ = crate::open_external(&url);
                    return false;
                }
                true
            })
            .with_new_window_req_handler(|_url, _features| wry::NewWindowResponse::Deny);
        if config.do_not_track {
            builder = builder.with_initialization_script(crate::auth::DO_NOT_TRACK_SCRIPT);
        }
        if let Some(proxy) = &config.proxy {
            builder = builder.with_proxy_config(proxy.clone());
        }
        builder = match target {
            Target::Url(url) => builder.with_url(url),
            Target::Html(html) => {
                builder
                    .with_html(html)
                    .with_custom_protocol("forja".into(), |_id, request| {
                        if request.uri().path().ends_with("mermaid.min.js") {
                            wry::http::Response::builder()
                                .header("Content-Type", "application/javascript")
                                .header("Cache-Control", "max-age=31536000")
                                .body(std::borrow::Cow::Borrowed(
                                    crate::mermaid::bundled_js().as_bytes(),
                                ))
                                .expect("static Mermaid response")
                        } else {
                            wry::http::Response::builder()
                                .status(404)
                                .body(std::borrow::Cow::Borrowed(&b""[..]))
                                .expect("static 404")
                        }
                    })
            }
        };
        let webview = {
            #[cfg(windows)]
            let previous = unsafe {
                use windows_sys::Win32::UI::HiDpi::{
                    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                };
                SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
            };
            let built = builder
                .build_as_child(parent)
                .map_err(|error| format!("could not create the webview: {error}"));
            #[cfg(windows)]
            if !previous.is_null() {
                unsafe {
                    windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(previous);
                }
            }
            built?
        };
        #[cfg(windows)]
        {
            use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller3;
            use windows_core::Interface;
            use wry::WebViewExtWindows;
            if let Ok(controller) = webview.controller().cast::<ICoreWebView2Controller3>() {
                let _ = unsafe { controller.SetShouldDetectMonitorScaleChanges(false) };
                let _ = unsafe { controller.SetRasterizationScale(1.0) };
            }
        }
        let _ = webview.zoom(config.zoom);
        Ok(Self { webview, events })
    }

    pub fn set_bounds(&self, rect: wry::Rect) {
        let _ = self.webview.set_bounds(rect);
    }

    pub fn set_visible(&self, visible: bool) {
        let _ = self.webview.set_visible(visible);
    }

    pub fn navigate(&self, url: &str) {
        let _ = self.webview.load_url(url);
    }

    pub fn back(&self) {
        let _ = self.webview.evaluate_script("history.back()");
    }

    pub fn forward(&self) {
        let _ = self.webview.evaluate_script("history.forward()");
    }

    pub fn reload(&self) {
        let _ = self.webview.reload();
    }

    pub fn hard_reload(&self) {
        let _ = self.webview.evaluate_script("location.reload(true)");
    }

    pub fn zoom(&self, level: f64) {
        let _ = self.webview.zoom(level);
    }

    pub fn find(&self, query: &str, backwards: bool) {
        let query = serde_json::to_string(query).unwrap_or_else(|_| "\"\"".into());
        let script = format!(
            "(function(){{const q={query};if(!q){{window.getSelection()?.removeAllRanges();return;}}\
             if(!window.find){{return;}}window.find(q,false,{backwards},true,false,false,false);}})()"
        );
        let _ = self.webview.evaluate_script(&script);
    }

    pub fn open_devtools(&self) {
        #[cfg(debug_assertions)]
        self.webview.open_devtools();
    }
}

fn preload_loader() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let Some(exe) = std::env::current_exe().ok() else {
                return;
            };
            let Some(dir) = exe.parent() else {
                return;
            };
            let path = dir.join("WebView2Loader.dll");
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            let directory: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                windows_sys::Win32::System::LibraryLoader::SetDllDirectoryW(directory.as_ptr());
                windows_sys::Win32::System::LibraryLoader::LoadLibraryW(wide.as_ptr());
            }
        }
    });
}

pub enum Target<'a> {
    Url(&'a str),
    Html(&'a str),
}

/// WebView bounds are logical points; wry applies the window DPI scale.
pub fn logical_rect(x: f32, y: f32, width: f32, height: f32) -> wry::Rect {
    wry::Rect {
        position: Position::Logical(LogicalPosition::new(x.max(0.0) as f64, y.max(0.0) as f64)),
        size: Size::Logical(LogicalSize::new(
            width.max(1.0) as f64,
            height.max(1.0) as f64,
        )),
    }
}

#[derive(Clone, Copy)]
pub struct Parent(isize);

impl Parent {
    /// Finds this process's largest visible top-level window.
    pub fn find() -> Option<Self> {
        use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible,
        };

        struct Search {
            pid: u32,
            best: HWND,
            area: i64,
        }

        unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> i32 {
            let search = &mut *(lparam as *mut Search);
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == search.pid && IsWindowVisible(hwnd) != 0 {
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if GetWindowRect(hwnd, &mut rect) != 0 {
                    let area =
                        ((rect.right - rect.left) as i64) * ((rect.bottom - rect.top) as i64);
                    if area > 10_000 && area > search.area {
                        search.area = area;
                        search.best = hwnd;
                    }
                }
            }
            1
        }

        let mut search = Search {
            pid: std::process::id(),
            best: std::ptr::null_mut(),
            area: 0,
        };
        let ok = unsafe { EnumWindows(Some(callback), (&mut search as *mut Search) as LPARAM) };
        if ok == 0 || search.best.is_null() {
            None
        } else {
            Some(Self(search.best as isize))
        }
    }
}

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let hwnd = NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?;
        let handle = Win32WindowHandle::new(hwnd);
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(handle)) })
    }
}

const SHORTCUTS: &str = r#"
document.addEventListener('keydown', (event) => {
  const key = (event.key || '').toLowerCase();
  let command = null;
  if (event.ctrlKey && !event.shiftKey && key === 'l') command = 'address';
  else if (event.ctrlKey && !event.shiftKey && key === 'r') command = 'reload';
  else if (event.ctrlKey && event.shiftKey && key === 'r') command = 'hardReload';
  else if (event.ctrlKey && !event.shiftKey && key === 'f') command = 'find';
  else if (event.ctrlKey && !event.shiftKey && key === 't') command = 'newPage';
  else if (event.ctrlKey && event.shiftKey && key === 't') command = 'reopenPage';
  else if (event.ctrlKey && !event.shiftKey && key === 'w') command = 'closePage';
  else if (event.ctrlKey && (key === '=' || key === '+')) command = 'zoomIn';
  else if (event.ctrlKey && key === '-') command = 'zoomOut';
  else if (event.ctrlKey && key === '0') command = 'zoomReset';
  else if (event.altKey && key === 'arrowleft') command = 'back';
  else if (event.altKey && key === 'arrowright') command = 'forward';
  else if (key === 'f12') command = 'devtools';
  if (command) {
    event.preventDefault();
    window.ipc.postMessage(command);
  }
});
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_rect_never_has_zero_size() {
        let rect = logical_rect(-5.0, 2.0, 0.0, -1.0);
        assert_eq!(
            rect.position,
            Position::Logical(LogicalPosition::new(0.0, 2.0))
        );
        assert_eq!(rect.size, Size::Logical(LogicalSize::new(1.0, 1.0)));
    }

    #[test]
    fn missing_window_is_reported_as_unavailable() {
        let parent = Parent(0);
        assert!(parent.window_handle().is_err());
    }
}
