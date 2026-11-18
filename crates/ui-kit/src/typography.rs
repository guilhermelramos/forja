//! The single Brasa text scale. Density changes spacing, never type sizes.
use eframe::egui::{FontFamily, FontId, RichText};
/// Named text measurements from design/ui.md, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Typography {
    pub caption: f32,
    pub body: f32,
    pub section: f32,
    pub page: f32,
    pub display: f32,
}
impl Typography {
    pub const BRASA: Self = Self {
        caption: 12.0,
        body: 14.0,
        section: 16.0,
        page: 20.0,
        display: 28.0,
    };
}
/// Semantic roles. Only explicit code/path widgets opt into a monospace family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRole {
    Caption,
    Body,
    Section,
    Page,
    Display,
}
impl TextRole {
    pub fn size(self) -> f32 {
        let t = Typography::BRASA;
        match self {
            Self::Caption => t.caption,
            Self::Body => t.body,
            Self::Section => t.section,
            Self::Page => t.page,
            Self::Display => t.display,
        }
    }
    pub fn font(self) -> FontId {
        FontId::new(
            self.size(),
            if matches!(self, Self::Section | Self::Page) {
                crate::apply::semibold_family()
            } else {
                FontFamily::Proportional
            },
        )
    }
    pub fn rich(self, text: impl Into<RichText>) -> RichText {
        text.into().font(self.font())
    }
}
