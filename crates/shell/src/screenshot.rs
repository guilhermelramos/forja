//! Automated screenshots for acceptance evidence.
//!
//! `forja-app --screenshot <png> [--slot <name>]` renders three frames, asks
//! egui for a screenshot, optionally crops one slot and closes the window.

use eframe::egui::{ColorImage, Rect};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct ScreenshotJob {
    pub path: PathBuf,
    pub slot: Option<String>,
    pub frames: u32,
    pub requested: bool,
    pub done: bool,
    /// `--ui-tree`: where to write the widget tree of the captured frame.
    pub ui_tree: Option<PathBuf>,
    /// Latest AccessKit tree, filled by [`crate::ui_tree::UiTreeCapture`].
    pub tree: crate::ui_tree::LatestTree,
}

impl ScreenshotJob {
    pub fn new(path: PathBuf, slot: Option<String>) -> Self {
        Self {
            path,
            slot,
            frames: 0,
            requested: false,
            done: false,
            ui_tree: None,
            tree: Default::default(),
        }
    }

    pub fn with_ui_tree(mut self, path: Option<PathBuf>) -> Self {
        self.ui_tree = path;
        self
    }
}

/// Saves the egui image, cropping to the named slot when one is known.
pub fn save(
    image: &ColorImage,
    path: &Path,
    slot: Option<&str>,
    rects: &BTreeMap<String, Rect>,
    pixels_per_point: f32,
) -> Result<(), String> {
    let [width, height] = image.size;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }
    let full = image::RgbaImage::from_raw(width as u32, height as u32, rgba)
        .ok_or_else(|| "egui returned an inconsistent image".to_owned())?;
    let output = match slot.filter(|slot| *slot != "all").and_then(|slot| rects.get(slot)) {
        Some(rect) => crop(&full, *rect, pixels_per_point),
        None => full,
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    output.save(path).map_err(|error| error.to_string())
}

fn crop(image: &image::RgbaImage, rect: Rect, pixels_per_point: f32) -> image::RgbaImage {
    let width = image.width() as f32;
    let height = image.height() as f32;
    let x = (rect.min.x * pixels_per_point).floor().clamp(0.0, width) as u32;
    let y = (rect.min.y * pixels_per_point).floor().clamp(0.0, height) as u32;
    let x2 = (rect.max.x * pixels_per_point).ceil().clamp(0.0, width) as u32;
    let y2 = (rect.max.y * pixels_per_point).ceil().clamp(0.0, height) as u32;
    if x2 <= x || y2 <= y {
        return image.clone();
    }
    image::imageops::crop_imm(image, x, y, x2 - x, y2 - y).to_image()
}
