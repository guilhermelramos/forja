//! Text fonts from %WINDIR%/Fonts. Icons are original embedded vector strokes.
use eframe::egui::{self, FontData, FontDefinitions, FontFamily};
use std::{path::PathBuf, sync::Arc};
/// Missing text fonts. The caller logs this; the UI still gets working fallback fonts.
#[derive(Debug)]
pub struct FontError {
    pub message: String,
}
impl std::fmt::Display for FontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for FontError {}
/// Installs available text fonts and always retains egui's text fallbacks.
pub fn install_fonts(ctx: &egui::Context) -> Result<(), FontError> {
    let directory = font_directory(std::env::var_os("WINDIR"));
    let mut fonts = FontDefinitions::default();
    let mut missing = Vec::new();
    let mut load = |name: &str, file: &str, family: FontFamily| {
        let path = directory.join(file);
        match std::fs::read(&path).ok().filter(|bytes| !bytes.is_empty()) {
            Some(bytes) => {
                fonts
                    .font_data
                    .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .insert(0, name.to_owned());
            }
            None => missing.push(path.display().to_string()),
        }
    };
    load("forja-ui", "segoeui.ttf", FontFamily::Proportional);
    load("forja-variable", "SegUIVar.ttf", FontFamily::Proportional);
    load(
        "forja-semibold",
        "seguisb.ttf",
        FontFamily::Name("forja-semibold".into()),
    );
    load(
        "forja-code-fallback",
        "CascadiaMono.ttf",
        FontFamily::Monospace,
    );
    load("forja-code", "CascadiaCode.ttf", FontFamily::Monospace);
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .entry(FontFamily::Name("forja-semibold".into()))
        .or_default()
        .extend(fallback);
    ctx.set_fonts(fonts);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(FontError {
            message: format!(
                "Text font unavailable; using fallback: {}",
                missing.join(", ")
            ),
        })
    }
}
fn font_directory(windir: Option<std::ffi::OsString>) -> PathBuf {
    windir.map(PathBuf::from).unwrap_or_default().join("Fonts")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_font_root_uses_the_windows_environment_instead_of_a_fixed_drive() {
        assert_eq!(
            font_directory(Some("D:/System".into())),
            PathBuf::from("D:/System/Fonts")
        );
    }
}
