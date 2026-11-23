//! Five-zone page frame shared by built-in rail destinations.
use crate::paint;
use eframe::egui::{self, Align, Layout, Rect, RichText, Sense, Ui};

/// Width policy: reading/form pages are capped; boards/tables may use the full slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageWidth {
    #[default]
    Content,
    Wide,
}

/// An auxiliary, named icon in the page header.
pub struct PageIconAction<'a> {
    pub icon: crate::Icon,
    pub label: &'a str,
}

/// Page metadata and the only allowed primary header action. All copy is translated by callers.
pub struct PageSpec<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: Option<&'a str>,
    pub width: PageWidth,
    pub primary: Option<&'a str>,
    pub refresh: Option<&'a str>,
    pub icons: &'a [PageIconAction<'a>],
    pub has_footer: bool,
}
impl<'a> PageSpec<'a> {
    pub fn new(id: &'a str, title: &'a str) -> Self {
        Self {
            id,
            title,
            subtitle: None,
            width: PageWidth::Content,
            primary: None,
            refresh: None,
            icons: &[],
            has_footer: false,
        }
    }
    pub fn subtitle(mut self, text: &'a str) -> Self {
        self.subtitle = Some(text);
        self
    }
    pub fn wide(mut self) -> Self {
        self.width = PageWidth::Wide;
        self
    }
    pub fn primary(mut self, label: &'a str) -> Self {
        self.primary = Some(label);
        self
    }
    pub fn refresh(mut self, label: &'a str) -> Self {
        self.refresh = Some(label);
        self
    }
    pub fn icons(mut self, icons: &'a [PageIconAction<'a>]) -> Self {
        self.icons = icons;
        self
    }
    pub fn footer(mut self) -> Self {
        self.has_footer = true;
        self
    }
}
/// Header gestures; callers perform commands after drawing, without callback borrow conflicts.
#[derive(Default)]
pub struct PageResponse {
    pub primary_clicked: bool,
    pub refresh_clicked: bool,
    pub icon_clicked: Option<usize>,
}

/// Geometry/identity contract, recorded in the egui context by the actual page component.
#[derive(Clone, Debug)]
pub struct PageMarker {
    pub id: String,
    pub title: String,
    pub bounds: Rect,
    pub header: Rect,
    pub filters: Rect,
    pub body: Rect,
    pub footer: Option<Rect>,
    pub refresh: Option<Rect>,
}
fn marker_id(id: &str) -> egui::Id {
    egui::Id::new(("forja.page.contract", id))
}
/// Last frame's page contract for a destination. No disk, global state or product text.
pub fn page_marker(ctx: &egui::Context, id: &str) -> Option<PageMarker> {
    ctx.data(|data| data.get_temp(marker_id(id)))
}

/// Sequential zones, deliberately allowing each closure to borrow the same view/context in turn.
pub struct Page {
    ui: Ui,
    marker: PageMarker,
    cursor: f32,
    gap: f32,
    has_footer: bool,
    content_width: f32,
    filters_drawn: bool,
    pub response: PageResponse,
}
/// Starts a page and draws its fixed header. Refresh always occupies the far-right header slot.
pub fn page(ui: &mut Ui, spec: PageSpec<'_>) -> Page {
    assert!(
        !spec.id.is_empty() && !spec.title.trim().is_empty(),
        "pages require identity and title"
    );
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let gap = if tokens.density == crate::Density::Compact {
        8.0
    } else {
        16.0
    };
    let (slot, _) = ui.allocate_exact_size(
        ui.available_size().max(egui::vec2(1.0, 1.0)),
        Sense::hover(),
    );
    let bounds = slot.shrink(16.0);
    let content_width = bounds.width().min(if spec.width == PageWidth::Content {
        960.0
    } else {
        f32::MAX
    });
    let mut header = Rect::from_min_size(bounds.min, egui::vec2(bounds.width(), 56.0));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(spec.id).max_rect(bounds));
    child.set_clip_rect(bounds.intersect(ui.clip_rect()));
    let action_width = spec
        .primary
        .map(|label| {
            paint::text_size(
                &child,
                label,
                paint::body_font(&metrics),
                tokens.color.text.to_color32(),
            )
            .x + 2.0 * metrics.pad_x
                + 8.0
        })
        .unwrap_or(0.0)
        + (spec.icons.len() + usize::from(spec.refresh.is_some())) as f32 * (metrics.control + 8.0);
    let title_rect = Rect::from_min_max(
        header.min,
        egui::pos2(
            (header.right() - action_width - 8.0).max(header.left() + 40.0),
            header.bottom(),
        ),
    );
    let mut title_ui = child.new_child(egui::UiBuilder::new().max_rect(title_rect));
    let title = title_ui.add(
        egui::Label::new(
            RichText::new(spec.title)
                .font(egui::FontId::new(20.0, crate::apply::semibold_family()))
                .color(tokens.color.text.to_color32()),
        )
        .selectable(false)
        .wrap(),
    );
    title.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, spec.title));
    title_ui.ctx().accesskit_node_builder(title.id, |node| {
        node.set_role(egui::accesskit::Role::Heading);
    });
    if let Some(subtitle) = spec.subtitle {
        title_ui.add(
            egui::Label::new(
                RichText::new(subtitle)
                    .size(crate::TextRole::Caption.size())
                    .color(tokens.color.text_muted.to_color32()),
            )
            .selectable(false)
            .wrap(),
        );
    }
    header.max.y = header.bottom().max(title_ui.min_rect().bottom());
    let mut response = PageResponse::default();
    let mut refresh = None;
    let actions_rect = Rect::from_min_max(
        egui::pos2(header.right() - action_width, header.top()),
        header.max,
    );
    let mut actions = child.new_child(
        egui::UiBuilder::new()
            .max_rect(actions_rect)
            .layout(Layout::right_to_left(Align::Min)),
    );
    actions.spacing_mut().item_spacing.x = 8.0;
    if let Some(label) = spec.refresh {
        let button = crate::icon_button(&mut actions, crate::Icon::Refresh, label);
        refresh = Some(button.rect);
        response.refresh_clicked = button.clicked();
    }
    for (index, action) in spec.icons.iter().enumerate().rev() {
        if crate::icon_button(&mut actions, action.icon, action.label).clicked() {
            response.icon_clicked = Some(index);
        }
    }
    if let Some(label) = spec.primary {
        response.primary_clicked =
            crate::button(&mut actions, crate::ButtonKind::Primary, label).clicked();
    }
    let marker = PageMarker {
        id: spec.id.into(),
        title: spec.title.into(),
        bounds,
        header,
        filters: Rect::NOTHING,
        body: Rect::NOTHING,
        footer: None,
        refresh,
    };
    let mut result = Page {
        ui: child,
        marker,
        cursor: header.bottom() + gap,
        gap,
        has_footer: spec.has_footer,
        content_width,
        filters_drawn: false,
        response,
    };
    result.record();
    result
}
impl Page {
    fn record(&mut self) {
        self.ui
            .ctx()
            .data_mut(|data| data.insert_temp(marker_id(&self.marker.id), self.marker.clone()));
    }
    /// Filter zone. Draw search first, then segments, then selectors; omit absent controls.
    pub fn filters<R>(&mut self, render: impl FnOnce(&mut Ui) -> R) -> R {
        assert!(!self.filters_drawn, "a page has one filter zone");
        let rect = Rect::from_min_max(
            egui::pos2(self.marker.bounds.left(), self.cursor),
            self.marker.bounds.max,
        );
        let mut child = self
            .ui
            .new_child(egui::UiBuilder::new().id_salt("filters").max_rect(rect));
        child.set_min_height(40.0);
        let result = render(&mut child);
        self.marker.filters = Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width(), child.min_rect().height().max(40.0)),
        );
        self.cursor = self.marker.filters.bottom() + self.gap;
        self.filters_drawn = true;
        self.record();
        result
    }
    /// Body zone. Existing tables/boards keep their own scrolling and internal layout.
    pub fn body<R>(&mut self, render: impl FnOnce(&mut Ui) -> R) -> R {
        if !self.filters_drawn {
            self.filters(|_| {});
        }
        let bottom = self.marker.bounds.bottom()
            - if self.has_footer {
                32.0 + self.gap
            } else {
                0.0
            };
        let rect = Rect::from_min_max(
            egui::pos2(self.marker.bounds.left(), self.cursor),
            egui::pos2(
                self.marker.bounds.left() + self.content_width,
                bottom.max(self.cursor),
            ),
        );
        self.marker.body = rect;
        let mut child = self
            .ui
            .new_child(egui::UiBuilder::new().id_salt("body").max_rect(rect));
        child.set_clip_rect(rect.intersect(self.ui.clip_rect()));
        // Use an allocated scope: its response id is the actual accessibility
        // parent. Ui::id() alone would create an unrelated, non-clipping node.
        let result =
            super::settings::settings_viewport(&mut child, rect.width(), rect.height(), render);
        self.record();
        result
    }
    /// Optional, anchored footer: count on the left and pagination on the right.
    pub fn footer<R>(&mut self, count: Option<&str>, pagination: impl FnOnce(&mut Ui) -> R) -> R {
        assert!(self.has_footer, "reserve the footer in PageSpec");
        let rect = Rect::from_min_size(
            egui::pos2(
                self.marker.bounds.left(),
                self.marker.bounds.bottom() - 32.0,
            ),
            egui::vec2(self.marker.bounds.width(), 32.0),
        );
        self.marker.footer = Some(rect);
        let mut child = self
            .ui
            .new_child(egui::UiBuilder::new().id_salt("footer").max_rect(rect));
        if let Some(count) = count {
            let label = child.label(
                RichText::new(count)
                    .size(12.0)
                    .color(paint::tokens(&child).color.text_muted.to_color32()),
            );
            label.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, count));
        }
        // Pagination is a sibling on the same row, not a child after the count's
        // vertical cursor (which would push a 32px button below the footer).
        let mut actions = child.new_child(
            egui::UiBuilder::new()
                .id_salt("pagination")
                .max_rect(rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let result = pagination(&mut actions);
        self.record();
        result
    }
}
/// Common states displayed inside the page body; the translated caption stays a Label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageState {
    Empty,
    Loading,
    Error,
}
/// Shared empty/loading/error treatment. Copy and optional explanatory detail come from i18n.
pub fn page_state(ui: &mut Ui, state: PageState, message: &str, detail: Option<&str>) {
    let tokens = paint::tokens(ui);
    egui::Frame::NONE.inner_margin(24.0).show(ui, |ui| {
        ui.set_max_width(ui.available_width());
        ui.horizontal(|ui| {
            if state == PageState::Loading {
                crate::spinner_if_active(ui);
            } else {
                crate::icon(
                    ui,
                    if state == PageState::Error {
                        crate::Icon::Warning
                    } else {
                        crate::Icon::Info
                    },
                    20.0,
                );
            }
            let label = ui.add(
                egui::Label::new(RichText::new(message).color(tokens.color.text.to_color32()))
                    .selectable(false)
                    .wrap(),
            );
            label.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, message));
        });
        if let Some(detail) = detail {
            let label = ui.add(
                egui::Label::new(
                    RichText::new(detail)
                        .size(12.0)
                        .color(tokens.color.text_muted.to_color32()),
                )
                .selectable(false)
                .wrap(),
            );
            label.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, detail));
        }
    });
}

#[cfg(test)]
#[path = "page_layout_tests.rs"]
mod tests;
