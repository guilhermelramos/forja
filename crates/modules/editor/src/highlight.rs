//! Syntax highlighting and the per-tab layout cache.
//!
//! `syntect` parses with the grammars bundled by `two-face` (including the
//! PowerShell grammar) and the layout is cached until the text, the wrap width,
//! the theme or the font size changes.

use forja_ui_kit::egui::{
    self, text::LayoutJob, text::TextWrapping, Color32, FontFamily, FontId, TextFormat,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme};
use syntect::parsing::{SyntaxDefinition, SyntaxSet};
use two_face::theme::EmbeddedThemeName;

fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(|| {
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        if let Ok(syntax) = SyntaxDefinition::load_from_str(
            include_str!("../syntaxes/powershell.sublime-syntax"),
            true,
            Some("PowerShell"),
        ) {
            builder.add(syntax);
        }
        builder.build()
    })
}

fn theme(dark: bool) -> Theme {
    let themes = two_face::theme::extra();
    let name = if dark {
        EmbeddedThemeName::Base16OceanDark
    } else {
        EmbeddedThemeName::Base16OceanLight
    };
    themes.get(name).clone()
}

#[derive(Default)]
pub struct HighlightCache {
    key: u64,
    layout: LayoutJob,
    built: bool,
}

/// Visual inputs that invalidate the cached layout.
#[derive(Clone, Copy, Debug)]
pub struct LayoutStyle<'a> {
    pub dark: bool,
    pub font_size: f32,
    pub family: Option<&'a str>,
    pub line_height: f32,
}

impl HighlightCache {
    pub fn key_for(path: &str, text: &str, wrap_width: f32, style: LayoutStyle<'_>) -> u64 {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        text.hash(&mut hasher);
        wrap_width.to_bits().hash(&mut hasher);
        style.dark.hash(&mut hasher);
        style.font_size.to_bits().hash(&mut hasher);
        style.family.hash(&mut hasher);
        style.line_height.to_bits().hash(&mut hasher);
        hasher.finish()
    }

    pub fn get_or_build(
        &mut self,
        ui: &egui::Ui,
        path: &str,
        text: &str,
        wrap_width: f32,
        style: LayoutStyle<'_>,
    ) -> LayoutJob {
        let key = Self::key_for(path, text, wrap_width, style);
        if !self.built || self.key != key {
            self.key = key;
            self.layout = build_layout(ui, path, text, wrap_width, style);
            self.built = true;
        }
        self.layout.clone()
    }
}

fn build_layout(
    ui: &egui::Ui,
    path: &str,
    text: &str,
    wrap_width: f32,
    style: LayoutStyle<'_>,
) -> LayoutJob {
    let LayoutStyle {
        dark,
        font_size,
        family,
        line_height,
    } = style;
    let syntax_set = syntaxes();
    let syntax = syntax_for(syntax_set, path, text);
    let theme = theme(dark);
    let mut highlighter = HighlightLines::new(syntax, &theme);
    let font_family = family
        .map(|name| FontFamily::Name(name.to_owned().into()))
        .unwrap_or(FontFamily::Monospace);
    let font = FontId::new(font_size, font_family);
    let row_height = (font_size * line_height.clamp(1.0, 1.8)).max(font_size);
    let default = TextFormat {
        font_id: font.clone(),
        line_height: Some(row_height),
        color: ui
            .visuals()
            .override_text_color
            .unwrap_or_else(|| ui.visuals().text_color()),
        ..TextFormat::default()
    };
    let mut job = LayoutJob {
        text: text.to_owned(),
        wrap: TextWrapping {
            max_width: if wrap_width <= 0.0 {
                f32::INFINITY
            } else {
                wrap_width
            },
            ..TextWrapping::default()
        },
        ..LayoutJob::default()
    };
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();
        if ranges.is_empty() {
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: offset..offset + line.len(),
                format: default.clone(),
            });
        } else {
            let mut line_offset = 0usize;
            for (style, piece) in ranges {
                let start = offset + line_offset;
                let end = start + piece.len();
                let mut format = default.clone();
                format.color = Color32::from_rgb( // ui-lint: allow syntax colors from the selected code-content theme
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                );
                format.italics = style.font_style.contains(FontStyle::ITALIC);
                job.sections.push(egui::text::LayoutSection {
                    leading_space: 0.0,
                    byte_range: start..end,
                    format,
                });
                line_offset += piece.len();
            }
        }
        offset += line.len();
    }
    job
}

fn syntax_for<'a>(
    syntax_set: &'a SyntaxSet,
    path: &str,
    text: &str,
) -> &'a syntect::parsing::SyntaxReference {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if let Some(syntax) = syntax_set.find_syntax_by_extension(extension) {
        return syntax;
    }
    if let Some(first_line) = text.lines().next() {
        if let Some(syntax) = syntax_set.find_syntax_by_first_line(first_line) {
            return syntax;
        }
    }
    syntax_set.find_syntax_plain_text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_changes_when_the_text_changes() {
        let style = LayoutStyle {
            dark: true,
            font_size: 13.0,
            family: None,
            line_height: 1.4,
        };
        let a = HighlightCache::key_for("a.rs", "fn main() {}", 100.0, style);
        let b = HighlightCache::key_for("a.rs", "fn main() { }", 100.0, style);
        let c = HighlightCache::key_for("a.rs", "fn main() {}", 90.0, style);
        let d = HighlightCache::key_for(
            "a.rs",
            "fn main() {}",
            100.0,
            LayoutStyle {
                family: Some("forja-editor-consolas"),
                ..style
            },
        );
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(
            a,
            HighlightCache::key_for("a.rs", "fn main() {}", 100.0, style)
        );
    }

    #[test]
    fn powershell_uses_a_syntax() {
        let set = syntaxes();
        let syntax = syntax_for(set, "a.ps1", "Get-ChildItem");
        assert_ne!(syntax.name, "Plain Text");
    }
}
