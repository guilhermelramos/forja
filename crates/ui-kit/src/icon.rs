//! Glyphs from Segoe Fluent Icons, with Segoe MDL2 Assets behind them.
//!
//! The code points are the Private Use Area values Microsoft publishes for
//! those two fonts (the same point renders in either file). [`install_fonts`]
//! loads `SegoeIcons.ttf` first and `segmdl2.ttf` as the fallback. Nothing is
//! embedded.

use eframe::egui::{self, FontFamily, FontId};

use crate::fonts::ICON_FAMILY;

/// Grid view. Quadro.
pub const BOARD: &str = "\u{F0E2}";
/// Mail. Pedidos.
pub const REQUESTS: &str = "\u{E715}";
/// People. Ajudantes.
pub const HELPERS: &str = "\u{E716}";
/// Calendar. Rotinas.
pub const ROUTINES: &str = "\u{E787}";
/// Zip folder. Arquivo.
pub const ARCHIVE: &str = "\u{F012}";
/// Knowledge article. Skills.
pub const SKILLS: &str = "\u{F000}";
/// Command prompt. Terminal.
pub const TERMINAL: &str = "\u{E756}";
/// Gear.
pub const SETTINGS: &str = "\u{E713}";
/// All apps. Extensões.
pub const EXTENSION: &str = "\u{E71D}";
/// Magnifying glass.
pub const SEARCH: &str = "\u{E721}";
/// Window close.
pub const CLOSE: &str = "\u{E8BB}";
/// Pane docked to the right.
pub const SPLIT_RIGHT: &str = "\u{E90D}";
/// Pane docked to the bottom.
pub const SPLIT_DOWN: &str = "\u{E90E}";
/// Brightness. Theme.
pub const THEME: &str = "\u{E706}";

pub fn font_id(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON_FAMILY.into()))
}

/// Square icon button. The caller supplies the tooltip.
pub fn icon_button(ui: &mut egui::Ui, glyph: &str) -> egui::Response {
    let tokens = crate::paint::tokens(ui);
    let metrics = tokens.metrics();
    let size = egui::vec2(metrics.control, metrics.control);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.hovered() || response.clicked() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
            tokens.color.surface_alt.to_color32(),
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font_id(16.0),
        tokens.color.text.to_color32(),
    );
    response
}
