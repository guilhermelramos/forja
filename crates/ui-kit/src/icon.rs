//! Original Forja stroke icons; SVG sources live in design/icons and never use a font.
pub use crate::icons_gen::Icon;
use eframe::egui::{self, Color32, Rect, Response, Stroke, Ui};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PathCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Quadratic([f32; 2], [f32; 2]),
    Close,
}

/// Draws an original icon at a rectangle, with an explicit token-derived color.
pub fn paint_icon(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let scale = rect.width().min(rect.height()) / 24.0;
    let origin = rect.center() - egui::vec2(12.0 * scale, 12.0 * scale);
    let point = |p: [f32; 2]| origin + egui::vec2(p[0] * scale, p[1] * scale);
    let stroke = Stroke::new(2.0 * scale, color);
    let (mut pen, mut start) = (origin, origin);
    let cap = |p| {
        painter.circle_filled(p, stroke.width * 0.5, color);
    };
    for command in icon.commands() {
        match *command {
            PathCommand::Move(p) => {
                pen = point(p);
                start = pen;
            }
            PathCommand::Line(p) => {
                let end = point(p);
                painter.add(egui::Shape::line(vec![pen, end], stroke));
                cap(pen);
                cap(end);
                pen = end;
            }
            PathCommand::Cubic(a, b, p) => {
                let end = point(p);
                painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                    [pen, point(a), point(b), end],
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
                cap(pen);
                cap(end);
                pen = end;
            }
            PathCommand::Quadratic(a, p) => {
                let end = point(p);
                painter.add(egui::epaint::QuadraticBezierShape::from_points_stroke(
                    [pen, point(a), end],
                    false,
                    Color32::TRANSPARENT,
                    stroke,
                ));
                cap(pen);
                cap(end);
                pen = end;
            }
            PathCommand::Close => {
                painter.add(egui::Shape::line(vec![pen, start], stroke));
                cap(pen);
                cap(start);
                pen = start;
            }
        }
    }
}
/// Draws a token-colored icon without adding a focusable control.
pub fn icon(ui: &mut Ui, icon: Icon, size: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    paint_icon(
        &crate::paint::painter_for(ui),
        icon,
        rect,
        crate::paint::tokens(ui).color.text.to_color32(),
    );
    response
}
/// Icon-only action. A nonempty translated label is its accessible name and tooltip.
pub fn icon_button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    assert!(!label.trim().is_empty(), "icon buttons require a label");
    let tokens = crate::paint::tokens(ui);
    let size = tokens.metrics().control;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if response.hovered() {
        crate::paint::painter_for(ui).rect_filled(rect, 6.0, tokens.color.surface_alt.to_color32());
    }
    paint_icon(
        &crate::paint::painter_for(ui),
        icon,
        Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
        tokens.color.text.to_color32(),
    );
    crate::paint::paint_focus(ui, &response, rect, 6.0, tokens.color.accent.to_color32());
    crate::tooltip(response, label)
}
/// Text action with a leading icon, preserving the normal button hierarchy.
pub fn button_with_icon(ui: &mut Ui, icon: Icon, text: &str, kind: crate::ButtonKind) -> Response {
    assert!(!text.trim().is_empty(), "buttons require a label");
    let tokens = crate::paint::tokens(ui);
    let metrics = tokens.metrics();
    let font = crate::paint::body_font(&metrics);
    let text_width =
        crate::paint::text_size(ui, text, font.clone(), tokens.color.text.to_color32()).x;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(text_width + metrics.pad_x * 2.0 + 24.0, metrics.control),
        egui::Sense::click(),
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    let kind = crate::hierarchy::resolve(ui, &response, kind);
    crate::hierarchy::mark(ui, &response, kind);
    let (fill, fg, outline) = match kind {
        crate::ButtonKind::Primary => (tokens.color.accent, tokens.color.on_accent, false),
        crate::ButtonKind::DangerConfirm => (tokens.color.danger, tokens.color.on_danger, false),
        crate::ButtonKind::Secondary | crate::ButtonKind::Danger => {
            (tokens.color.surface, tokens.color.text, true)
        }
        crate::ButtonKind::Ghost => (tokens.color.surface, tokens.color.text, false),
    };
    if kind != crate::ButtonKind::Ghost || response.hovered() {
        crate::paint::painter_for(ui).rect_filled(
            rect,
            6.0,
            crate::paint::hover_fill(fill.to_color32(), &response, &tokens),
        );
    }
    if outline {
        crate::paint::painter_for(ui).rect_stroke(
            rect,
            6.0,
            Stroke::new(1.0, tokens.color.border.to_color32()),
            egui::StrokeKind::Inside,
        );
    }
    paint_icon(
        &crate::paint::painter_for(ui),
        icon,
        Rect::from_center_size(
            egui::pos2(rect.left() + metrics.pad_x + 8.0, rect.center().y),
            egui::vec2(16.0, 16.0),
        ),
        fg.to_color32(),
    );
    let label_rect = Rect::from_min_max(
        rect.min + egui::vec2(metrics.pad_x + 24.0, 0.0),
        rect.max - egui::vec2(metrics.pad_x, 0.0),
    );
    crate::paint::paint_text(
        &crate::paint::painter_for(ui),
        label_rect,
        egui::Align2::LEFT_CENTER,
        text,
        font,
        fg.to_color32(),
    );
    crate::paint::paint_focus(ui, &response, rect, 6.0, tokens.color.accent.to_color32());
    response
}

impl Icon {
    /// Maps common file extensions to original file-type strokes, without filesystem access.
    pub fn for_extension(extension: &str) -> Self {
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Self::FileRust,
            "ts" | "tsx" | "js" | "jsx" | "py" | "c" | "h" | "cpp" | "dart" | "html" | "css" => {
                Self::FileCode
            }
            "toml" | "yaml" | "yml" | "ini" => Self::FileConfig,
            "json" | "csv" | "sql" => Self::FileData,
            "md" | "txt" | "ftl" => Self::FileText,
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => Self::FileImage,
            "sh" | "ps1" | "cmd" | "bat" => Self::FileScript,
            "exe" | "dll" | "bin" | "zip" => Self::FileBinary,
            _ => Self::File,
        }
    }
}

// Typed aliases keep the existing Forja vocabulary while callers migrate away from font glyphs.
pub const BOARD: Icon = Icon::Board;
pub const REQUESTS: Icon = Icon::Requests;
pub const HELPERS: Icon = Icon::Helpers;
pub const ROUTINES: Icon = Icon::Routines;
pub const ARCHIVE: Icon = Icon::Archive;
pub const SKILLS: Icon = Icon::Skills;
pub const TERMINAL: Icon = Icon::Terminal;
pub const SETTINGS: Icon = Icon::Settings;
pub const EXTENSION: Icon = Icon::Extensions;
pub const SEARCH: Icon = Icon::Search;
pub const CLOSE: Icon = Icon::Close;
pub const SPLIT_RIGHT: Icon = Icon::SplitRight;
pub const SPLIT_DOWN: Icon = Icon::SplitDown;
pub const THEME: Icon = Icon::Theme;
pub const ADD: Icon = Icon::Add;
pub const CHEVRON_DOWN: Icon = Icon::ChevronDown;
pub const CHEVRON_RIGHT: Icon = Icon::ChevronRight;
pub const CODE: Icon = Icon::Editor;
pub const CHAT: Icon = Icon::Chat;
pub const DIFF: Icon = Icon::Diff;
pub const GLOBE: Icon = Icon::Browser;
pub const PREVIEW: Icon = Icon::Preview;
pub const HISTORY: Icon = Icon::History;
pub const REVIEW: Icon = Icon::Review;
pub const FOLDER: Icon = Icon::Folder;
pub const PROJECTS: Icon = Icon::Projects;
pub const LAYERS: Icon = Icon::WorkbenchTab;
pub const CHANGES: Icon = Icon::Changes;
pub const PORTS: Icon = Icon::Ports;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vector_actions_expose_accessible_names() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                icon_button(ui, Icon::Refresh, "Refresh data");
                button_with_icon(ui, Icon::Copy, "Copy path", crate::ButtonKind::Secondary);
            });
        });
        let update = output.platform_output.accesskit_update.unwrap();
        for label in ["Refresh data", "Copy path"] {
            assert!(update
                .nodes
                .iter()
                .any(|(_, node)| node.label() == Some(label)));
        }
    }
    #[test]
    fn disabled_icon_actions_use_the_disabled_opacity() {
        let render = |enabled| {
            let ctx = egui::Context::default();
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        icon_button(ui, Icon::Add, "Add");
                    });
                });
            });
            output
                .shapes
                .into_iter()
                .find_map(|shape| match shape.shape {
                    egui::Shape::Circle(circle) => Some(circle.fill.a()),
                    _ => None,
                })
                .unwrap()
        };
        assert_eq!(render(false), (f32::from(render(true)) * 0.4).round() as u8);
    }
    #[test]
    fn every_original_icon_draws_without_an_icon_font() {
        let ctx = egui::Context::default();
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for glyph in Icon::ALL {
                    icon(ui, *glyph, 16.0);
                }
            });
        });
        assert!(!output.shapes.is_empty());
        assert!(Icon::ALL.len() >= 75);
        for glyph in Icon::ALL {
            assert_eq!(Icon::from_id(glyph.id()), Some(*glyph));
        }
    }
    #[test]
    fn every_icon_has_distinct_stroke_data() {
        let mut seen = std::collections::HashSet::new();
        for icon in Icon::ALL {
            assert!(
                seen.insert(format!("{:?}", icon.commands())),
                "duplicate drawing: {:?}",
                icon
            );
        }
        assert_eq!(Icon::for_extension("RS"), Icon::FileRust);
        assert_eq!(Icon::for_extension("unknown"), Icon::File);
    }
    #[test]
    #[should_panic(expected = "require a label")]
    fn icon_actions_reject_empty_names() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                icon_button(ui, Icon::Refresh, "");
            });
        });
    }
}
