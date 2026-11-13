//! Window icon pixels and the `.ico` bytes used by the tray.

use eframe::egui;

pub fn window_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../../../design/assets/logo-256.png");
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

pub fn tray_ico_bytes() -> &'static [u8] {
    include_bytes!("../../../design/assets/logo.ico")
}
