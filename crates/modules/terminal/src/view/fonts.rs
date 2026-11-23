//! Carregamento sob demanda de fontes monoespaçadas e fallback CJK/emoji.

use forja_kernel::ui_contract::egui::{self, FontData, FontFamily, FontId};
use std::sync::{Arc, OnceLock};

pub(super) fn fonts_epoch(ctx: &egui::Context) -> u64 {
    ctx.data(|data| {
        data.get_temp::<u64>(egui::Id::new("forja-terminal-fonts-epoch"))
            .unwrap_or(0)
    })
}

fn bump_fonts_epoch(ctx: &egui::Context) {
    let id = egui::Id::new("forja-terminal-fonts-epoch");
    ctx.data_mut(|data| {
        let next = data.get_temp::<u64>(id).unwrap_or(0).wrapping_add(1);
        data.insert_temp(id, next);
    });
}

/// Instala uma família monoespaçada pedida, uma vez por sessão.
pub fn terminal_font(ctx: &egui::Context, family: &str, size: f32) -> FontId {
    let (key, files) = match family {
        "cascadia_code" => (
            "forja-terminal-cascadia-code",
            &["CascadiaCode.ttf", "CascadiaCode-VariableFont_wght.ttf"][..],
        ),
        "cascadia_mono" => ("forja-terminal-cascadia-mono", &["CascadiaMono.ttf"][..]),
        "consolas" => ("forja-terminal-consolas", &["consola.ttf"][..]),
        _ => return FontId::monospace(size),
    };
    let flag = egui::Id::new(("forja-terminal-font", key));
    if !ctx.data(|data| data.get_temp::<bool>(flag).unwrap_or(false)) {
        ctx.data_mut(|data| data.insert_temp(flag, true));
        if let Some(bytes) = read_windows_font(files) {
            let mut definitions = ctx.fonts_mut(|fonts| fonts.definitions().clone());
            definitions
                .font_data
                .insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
            definitions
                .families
                .insert(FontFamily::Name(key.into()), vec![key.to_owned()]);
            ctx.set_fonts(definitions);
            bump_fonts_epoch(ctx);
            ctx.request_repaint();
        }
    }
    if ctx.fonts_mut(|fonts| {
        fonts
            .definitions()
            .families
            .contains_key(&FontFamily::Name(key.into()))
    }) {
        FontId::new(size, FontFamily::Name(key.into()))
    } else {
        FontId::monospace(size)
    }
}

fn read_windows_font(files: &[&str]) -> Option<Vec<u8>> {
    let root = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("Fonts");
    for file in files {
        if let Ok(bytes) = std::fs::read(root.join(file)) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
    }
    None
}

/// Instala fallback para CJK e emoji, uma vez por contexto.
pub fn ensure_wide_fonts(ctx: &egui::Context) {
    static FONTS: OnceLock<Vec<(&'static str, Arc<FontData>)>> = OnceLock::new();
    let flag = egui::Id::new("forja-terminal-wide-fonts");
    if ctx.data(|data| data.get_temp::<bool>(flag).unwrap_or(false)) {
        return;
    }
    ctx.data_mut(|data| data.insert_temp(flag, true));
    let fonts = FONTS.get_or_init(|| {
        let root = std::env::var_os("WINDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
            .join("Fonts");
        [
            ("forja-terminal-cjk", "msyh.ttc"),
            ("forja-terminal-japanese", "msgothic.ttc"),
            ("forja-terminal-korean", "malgun.ttf"),
            ("forja-terminal-emoji", "seguiemj.ttf"),
        ]
        .into_iter()
        .filter_map(|(name, file)| {
            std::fs::read(root.join(file))
                .ok()
                .map(|bytes| (name, Arc::new(FontData::from_owned(bytes))))
        })
        .collect()
    });
    if fonts.is_empty() {
        return;
    }
    let mut definitions = ctx.fonts_mut(|fonts| fonts.definitions().clone());
    for (name, data) in fonts {
        definitions
            .font_data
            .insert((*name).to_owned(), Arc::clone(data));
        definitions
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push((*name).to_owned());
    }
    ctx.set_fonts(definitions);
    bump_fonts_epoch(ctx);
    ctx.request_repaint();
}
