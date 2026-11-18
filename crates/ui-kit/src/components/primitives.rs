//! Small, composable Brasa pieces. Copy always comes from the caller's i18n.
use crate::{
    egui::{self, Align, Layout, Response, Sense, Ui},
    paint, Icon, TextRole,
};

/// Fixed steps on the Brasa four-pixel grid (independent of text density).
pub struct Spacing;
impl Spacing {
    pub const XS: f32 = crate::GRID;
    pub const SM: f32 = crate::GRID * 2.0;
    pub const MD: f32 = crate::GRID * 3.0;
    pub const LG: f32 = crate::GRID * 4.0;
    pub const XL: f32 = crate::GRID * 6.0;
    pub const XXL: f32 = crate::GRID * 8.0;
}

/// Body text using the applied Brasa typography. Explicit RichText/code styling
/// is retained; this also accepts formatted layout jobs without losing spans.
pub fn text(ui: &mut Ui, text: impl Into<egui::WidgetText>) -> Response {
    ui.add(egui::Label::new(text))
}

/// Section title, never the larger page-title role.
pub fn section_heading(ui: &mut Ui, title: &str) -> Response {
    text(ui, TextRole::Section.rich(title))
}

/// Horizontal actions with density-aware spacing and a minimum control height.
pub fn toolbar<R>(ui: &mut Ui, actions: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    ui.horizontal(|ui| {
        let metrics = paint::tokens(ui).metrics();
        ui.set_min_height(metrics.control);
        ui.spacing_mut().item_spacing.x = metrics.gap;
        actions(ui)
    })
}

/// Quiet vertical boundary between toolbar groups.
pub fn toolbar_separator(ui: &mut Ui) -> Response {
    ui.separator()
}

/// Overflow menu reuses the same mandatory accessible name and menu as pages.
pub fn toolbar_more(ui: &mut Ui, label: &str, items: &[crate::MenuItem<'_>]) -> Option<usize> {
    crate::icon_menu(ui, Icon::More, label, items)
}

/// One list row; copy, selection and optional adornments belong to the caller.
pub struct ListRow<'a> {
    pub icon: Option<Icon>,
    pub title: &'a str,
    pub detail: Option<&'a str>,
    pub tag: Option<&'a str>,
    pub selected: bool,
}

/// Full-width selectable row. Actions appear on hover or keyboard focus within
/// the row, but retain their tab stops and names when the pointer is elsewhere.
pub fn list_row(ui: &mut Ui, row: ListRow<'_>, actions: impl FnOnce(&mut Ui)) -> Response {
    let tokens = paint::tokens(ui);
    let m = tokens.metrics();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), m.row), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            row.selected,
            row.title,
        )
    });
    let painter = paint::painter_for(ui);
    if row.selected || response.hovered() {
        painter.rect_filled(
            rect,
            m.radius_control,
            if row.selected {
                tokens.color.accent_soft
            } else {
                tokens.color.surface_alt
            }
            .to_color32(),
        );
    }
    let focus_inside = ui
        .memory(|m| m.focused())
        .and_then(|id| ui.ctx().read_response(id))
        .is_some_and(|r| rect.contains_rect(r.rect));
    let show_actions = response.hovered() || response.has_focus() || focus_inside;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(m.pad_x, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            if let Some(icon) = row.icon {
                crate::icon(ui, icon, 16.0);
            }
            text(ui, row.title);
            if let Some(detail) = row.detail {
                text(
                    ui,
                    TextRole::Caption
                        .rich(detail)
                        .color(tokens.color.text_muted.to_color32()),
                );
            }
            if let Some(label) = row.tag {
                tag(ui, label);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !show_actions {
                    ui.set_opacity(0.0);
                }
                actions(ui);
            });
        },
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        m.radius_control,
        tokens.color.accent.to_color32(),
    );
    response
}

/// A form row without settings-specific restore/search/modified machinery.
/// Settings continue to use `SettingRow` instead of duplicating that contract.
pub struct FormRow<'a> {
    pub label: &'a str,
    pub help: Option<&'a str>,
    pub error: Option<&'a str>,
}

/// Label and help at left, control at right, validation below. Returns the
/// control result; the caller supplies its accessible name (the visible label).
pub fn form_row<R>(ui: &mut Ui, row: FormRow<'_>, control: impl FnOnce(&mut Ui) -> R) -> R {
    let tokens = paint::tokens(ui);
    let mut result = None;
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                text(ui, row.label);
                if let Some(help) = row.help {
                    text(
                        ui,
                        TextRole::Caption
                            .rich(help)
                            .color(tokens.color.text_muted.to_color32()),
                    );
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                result = Some(control(ui));
            });
        });
        if let Some(error) = row.error {
            text(
                ui,
                TextRole::Caption
                    .rich(error)
                    .color(tokens.color.danger.to_color32()),
            );
        }
    });
    result.expect("form control rendered")
}

/// Generic quiet tag for states, counts or keyboard shortcuts. State never
/// relies on color alone; unlike `stage_badge`, this does not imply a stage.
pub fn tag(ui: &mut Ui, label: &str) -> Response {
    let tokens = paint::tokens(ui);
    let m = tokens.metrics();
    egui::Frame::new()
        .fill(tokens.color.surface_alt.to_color32())
        .corner_radius(m.radius_control)
        .inner_margin(egui::Margin::symmetric(4, 0))
        .show(ui, |ui| {
            ui.set_min_height(m.badge);
            text(ui, TextRole::Caption.rich(label))
        })
        .inner
}

/// Animated loading indicator only in an active window with motion enabled;
/// otherwise a static token-colored ring, with the same reserved bounds.
pub fn spinner(ui: &mut Ui) -> Response {
    let tokens = paint::tokens(ui);
    if crate::window_active(ui.ctx()) && !tokens.reduce_motion {
        ui.add(egui::Spinner::new().color(tokens.color.text_muted.to_color32()))
    } else {
        let size = tokens.metrics().control;
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
        paint::painter_for(ui).circle_stroke(
            rect.center(),
            size * 0.25,
            egui::Stroke::new(2.0, tokens.color.text_muted.to_color32()),
        );
        response
    }
}

/// Stable loading placeholder; no shimmer, timer or layout changes.
pub fn skeleton(ui: &mut Ui, size: egui::Vec2) -> Response {
    let tokens = paint::tokens(ui);
    let (rect, response) = ui.allocate_exact_size(size.max(egui::Vec2::ZERO), Sense::hover());
    paint::painter_for(ui).rect_filled(
        rect,
        tokens.metrics().radius_control,
        tokens.color.surface_alt.to_color32(),
    );
    response
}

/// Metric card reuses card elevation and the single Display/Caption text scale.
/// The optional variation remains readable text, not a red/green-only signal.
pub fn number_card(ui: &mut Ui, value: &str, caption: &str, variation: Option<&str>) -> Response {
    let tokens = paint::tokens(ui);
    crate::card(ui, None, |ui| {
        text(ui, TextRole::Display.rich(value));
        text(
            ui,
            TextRole::Caption
                .rich(caption)
                .color(tokens.color.text_muted.to_color32()),
        );
        if let Some(variation) = variation {
            text(ui, TextRole::Caption.rich(variation));
        }
    })
    .response
}

/// Vertical draggable divider. `fraction` is the left panel's share of
/// `total_width`, constrained to 10–90%. Arrow keys change it by one percent.
/// Name is mandatory for keyboard and assistive navigation.
pub fn splitter(ui: &mut Ui, name: &str, fraction: &mut f32, total_width: f32) -> Response {
    assert!(!name.trim().is_empty(), "splitters require a label");
    let tokens = paint::tokens(ui);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(Spacing::SM, ui.available_height().max(tokens.metrics().row)),
        Sense::click_and_drag(),
    );
    let old = *fraction;
    if response.dragged() && ui.is_enabled() {
        *fraction += response.drag_delta().x / total_width.max(1.0);
    }
    if response.has_focus() && ui.is_enabled() {
        ui.input(|i| {
            if i.key_pressed(egui::Key::ArrowLeft) {
                *fraction -= 0.01;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                *fraction += 0.01;
            }
        });
    }
    *fraction = if fraction.is_finite() {
        fraction.clamp(0.1, 0.9)
    } else {
        0.5
    };
    if *fraction != old {
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*fraction), name));
    let color = if response.dragged() || response.hovered() {
        tokens.color.accent
    } else {
        tokens.color.border
    };
    paint::painter_for(ui).line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(2.0, color.to_color32()),
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        tokens.metrics().radius_control,
        tokens.color.accent.to_color32(),
    );
    response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
}
