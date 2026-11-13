//! Segoe UI Variable / Segoe UI, Cascadia Code e os ícones Fluent, lidos do Windows.

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// A fonte pedida não está na máquina.
#[derive(Debug)]
pub struct FontError {
    /// O que faltou, em uma frase.
    pub message: String,
}

impl std::fmt::Display for FontError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FontError {}

const VARIABLE: &str = r"C:\Windows\Fonts\SegUIVar.ttf";
const UI_REGULAR: &str = r"C:\Windows\Fonts\segoeui.ttf";
const UI_SEMIBOLD: &str = r"C:\Windows\Fonts\seguisb.ttf";
const CODE: &[&str] = &[
    r"C:\Windows\Fonts\CascadiaCode.ttf",
    r"C:\Windows\Fonts\CascadiaMono.ttf",
];
const FLUENT_ICONS: &str = r"C:\Windows\Fonts\SegoeIcons.ttf";
const MDL2_ICONS: &str = r"C:\Windows\Fonts\segmdl2.ttf";

/// Family name used by [`crate::icon::font_id`].
pub const ICON_FAMILY: &str = "forja-icons";

/// Instala a interface e o código. Não embute os arquivos: só lê `%WINDIR%\Fonts`.
pub fn install_fonts(ctx: &egui::Context) -> Result<(), FontError> {
    let mut fonts = FontDefinitions::default();
    let mut proportional: Vec<String> = Vec::new();

    if let Some(bytes) = read_optional(VARIABLE) {
        insert(&mut fonts, "forja-variable", bytes);
        proportional.push("forja-variable".to_owned());
    }
    let regular = read_required(UI_REGULAR, "Segoe UI")?;
    insert(&mut fonts, "forja-ui", regular);
    proportional.push("forja-ui".to_owned());

    let semibold = read_required(UI_SEMIBOLD, "Segoe UI Semibold")?;
    insert(&mut fonts, "forja-semibold", semibold);
    fonts.families.insert(
        FontFamily::Name("forja-semibold".into()),
        vec!["forja-semibold".to_owned()],
    );

    let code = read_first(CODE, "Cascadia Code")?;
    insert(&mut fonts, "forja-code", code);

    let prop = fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .ok_or_else(|| FontError {
            message: "família proporcional ausente".to_owned(),
        })?;
    for (index, name) in proportional.into_iter().enumerate() {
        prop.insert(index, name);
    }
    let mono = fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .ok_or_else(|| FontError {
            message: "família monoespaçada ausente".to_owned(),
        })?;
    mono.insert(0, "forja-code".to_owned());

    let mut icons = Vec::new();
    if let Some(bytes) = read_optional(FLUENT_ICONS) {
        insert(&mut fonts, "forja-fluent", bytes);
        icons.push("forja-fluent".to_owned());
    }
    if let Some(bytes) = read_optional(MDL2_ICONS) {
        insert(&mut fonts, "forja-mdl2", bytes);
        icons.push("forja-mdl2".to_owned());
    }
    icons.push("forja-ui".to_owned());
    fonts
        .families
        .insert(FontFamily::Name(ICON_FAMILY.into()), icons);

    ctx.set_fonts(fonts);
    Ok(())
}

fn insert(fonts: &mut FontDefinitions, name: &str, bytes: Vec<u8>) {
    fonts
        .font_data
        .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
}

fn read_optional(path: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

fn read_required(path: &str, label: &str) -> Result<Vec<u8>, FontError> {
    let bytes = std::fs::read(path).map_err(|_| FontError {
        message: format!("{label} não encontrado em {path}"),
    })?;
    if bytes.is_empty() {
        return Err(FontError {
            message: format!("{label} está vazio em {path}"),
        });
    }
    Ok(bytes)
}

fn read_first(paths: &[&str], label: &str) -> Result<Vec<u8>, FontError> {
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
        }
    }
    Err(FontError {
        message: format!("{label} não encontrado em {}", paths.join(", ")),
    })
}
