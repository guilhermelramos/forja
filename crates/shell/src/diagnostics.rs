//! Diagnostics overlay: frame rate, frame time, repaint cause, memory, threads.
//!
//! Turned on by `app.diagnostics.overlay` (Ajustes) or by the Anvil command
//! "Mostrar diagnóstico", which flips the same key. While it is open it asks for
//! two frames per second; while it is closed it costs one settings lookup per
//! pass and schedules nothing, so the idle app keeps drawing no frames (#191).

use crate::app::ForjaApp;
use eframe::egui;
use std::time::{Duration, Instant};

/// How often the numbers are sampled while the panel is open.
pub(crate) const REFRESH: Duration = Duration::from_millis(500);

/// Numbers shown by the panel, sampled at most twice a second.
#[derive(Default)]
pub(crate) struct Diagnostics {
    sampled: Option<Instant>,
    window_start: Option<Instant>,
    window_frames: u64,
    fps: f64,
    frame_ms: f64,
    memory_mb: f64,
    threads: usize,
    cause: String,
    frames: u64,
}

impl Diagnostics {
    /// Refreshes the numbers when the refresh window is over.
    ///
    /// Returns how long until the next sample is allowed, so the caller can ask
    /// for exactly that frame instead of spinning.
    pub(crate) fn refresh(
        &mut self,
        ctx: &egui::Context,
        frames: u64,
        frame_time: Duration,
    ) -> Duration {
        let now = Instant::now();
        if let Some(sampled) = self.sampled {
            let since = now.saturating_duration_since(sampled);
            if since < REFRESH {
                return REFRESH - since;
            }
        }
        let start = *self.window_start.get_or_insert(now);
        let elapsed = now.saturating_duration_since(start);
        if elapsed >= REFRESH {
            self.fps = frames.saturating_sub(self.window_frames) as f64 / elapsed.as_secs_f64();
            self.window_start = Some(now);
            self.window_frames = frames;
        }
        self.sampled = Some(now);
        self.frames = frames;
        self.frame_ms = frame_time.as_secs_f64() * 1000.0;
        let (memory, threads) = process_usage();
        self.memory_mb = memory as f64 / (1024.0 * 1024.0);
        self.threads = threads;
        self.cause = repaint_cause(ctx);
        REFRESH
    }

    /// Rows of the panel: label key and value, already formatted.
    pub(crate) fn rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![
            ("shell.diagnostics.fps", format!("{:.0}", self.fps)),
            ("shell.diagnostics.frame-time", format!("{:.1} ms", self.frame_ms)),
        ];
        if self.cause.is_empty() {
            rows.push(("shell.diagnostics.frames", self.frames.to_string()));
        } else {
            rows.push(("shell.diagnostics.cause", self.cause.clone()));
        }
        rows.push(("shell.diagnostics.memory", format!("{:.0} MB", self.memory_mb)));
        rows.push(("shell.diagnostics.threads", self.threads.to_string()));
        rows
    }
}

/// Who asked for the frame being drawn: the reason when egui has one, else the
/// call site.
fn repaint_cause(ctx: &egui::Context) -> String {
    let causes = ctx.repaint_causes();
    let Some(cause) = causes.first() else {
        return String::new();
    };
    let reason = cause.reason.trim();
    if !reason.is_empty() {
        return reason.to_owned();
    }
    let file = std::path::Path::new(cause.file)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(cause.file);
    format!("{file}:{}", cause.line)
}

/// Working set in bytes and the number of threads of this process.
#[cfg(windows)]
fn process_usage() -> (u64, usize) {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, THREADENTRY32, TH32CS_SNAPTHREAD,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let mut memory = 0;
    // SAFETY: the counters are zeroed with `cb` set to their own size, which is
    // what the API expects, and the current-process handle is always valid.
    unsafe {
        let mut counters = std::mem::zeroed::<PROCESS_MEMORY_COUNTERS>();
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            memory = counters.WorkingSetSize as u64;
        }
    }

    let mut threads = 0;
    // SAFETY: the snapshot handle is checked against INVALID_HANDLE_VALUE, every
    // entry carries its own size, and the handle is closed on both paths.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot != INVALID_HANDLE_VALUE {
            let mut entry = std::mem::zeroed::<THREADENTRY32>();
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            if Thread32First(snapshot, &mut entry) != 0 {
                loop {
                    if entry.th32OwnerProcessID == std::process::id() {
                        threads += 1;
                    }
                    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
                    if Thread32Next(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snapshot);
        }
    }
    (memory, threads)
}

/// The panel is a Windows nicety; elsewhere it still shows frames and time.
#[cfg(not(windows))]
fn process_usage() -> (u64, usize) {
    (0, 0)
}

impl ForjaApp {
    /// Whether `app.diagnostics.overlay` is on.
    pub(crate) fn diagnostics_visible(&self) -> bool {
        self.setting_bool("app.diagnostics.overlay", false)
    }

    /// Draws the overlay in the bottom-right corner and keeps it at two frames
    /// per second while it is open.
    pub(crate) fn diagnostics_overlay(&mut self, ctx: &egui::Context) {
        if !self.diagnostics_visible() {
            return;
        }
        let wait = self
            .diagnostics
            .refresh(ctx, self.pacer.frames(), self.pacer.frame_time());
        let title = self.text("shell.diagnostics.title");
        let rows: Vec<(String, String)> = self
            .diagnostics
            .rows()
            .into_iter()
            .map(|(key, value)| (self.text(key), value))
            .collect();
        egui::Area::new(egui::Id::new("forja.shell.diagnostics"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -64.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                forja_ui_kit::card(ui, Some(&title), |ui| {
                    egui::Grid::new("forja.shell.diagnostics.grid")
                        .num_columns(2)
                        .spacing([12.0, 2.0])
                        .show(ui, |ui| {
                            for (label, value) in &rows {
                                forja_ui_kit::text(ui, label);
                                forja_ui_kit::text(ui, egui::RichText::new(value).strong());
                                ui.end_row();
                            }
                        });
                });
            });
        ctx.request_repaint_after(wait);
    }
}
