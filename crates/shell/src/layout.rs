//! Fixed layout regions from `design/product.md` §3.

use crate::actions::AnvilMode;
use crate::app::ForjaApp;
use crate::icon;
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Stroke, ViewportCommand};
use forja_ui_kit::{button, ButtonKind};
use std::sync::Arc;

pub(crate) struct RailPage {
    pub(crate) id: String,
    title: String,
    glyph: forja_ui_kit::Icon,
    png: Option<Arc<[u8]>>,
}

fn rail_tooltip(title: &str, shortcut: &str) -> String {
    if shortcut.is_empty() {
        title.to_owned()
    } else {
        format!("{title}\n{shortcut}")
    }
}

fn panel_max_width(available: f32, reserved: f32) -> f32 {
    (available - reserved).clamp(180.0, 480.0)
}

impl ForjaApp {
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context) {
        if self.focus_replaces_top_bar() {
            self.focus_exit_strip(ctx);
            return;
        }
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(10, 4))
            .stroke(Stroke::new(1.0_f32, self.paint(self.tokens.color.border)));
        let crumb = self.active_project_crumb();
        let crumb_label = self.text("shell.top.crumb-label");
        // The accessible name carries both the label and the live value so a
        // screen reader announces which project and bench are active.
        let crumb_name = self.text_args("shell.top.crumb-name", &[("value", &crumb)]);
        let search_label = format!(
            "{} {}",
            self.text("shell.top.search"),
            self.chord_for("anvil.open")
        );
        let response = egui::TopBottomPanel::top("forja.top")
            .exact_height(40.0)
            .frame(frame)
            .show(ctx, |ui| {
                let bounds = ui.max_rect();
                let drag = ui.interact(
                    bounds,
                    egui::Id::new("forja.top.drag"),
                    // ui-lint: allow bar drag moves the window; the crumb carries the name
                    Sense::click_and_drag(),
                );
                if drag.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                let left_width = (bounds.width() * 0.24).clamp(100.0, 280.0);
                let controls_width = 286.0;
                let search_width =
                    (bounds.width() - left_width - controls_width - 40.0).clamp(180.0, 420.0);
                let search_x =
                    (bounds.center().x - search_width * 0.5).max(bounds.left() + left_width + 12.0);
                let search_rect = egui::Rect::from_min_size(
                    egui::pos2(search_x, bounds.top()),
                    egui::vec2(search_width, 32.0),
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                        bounds.min,
                        egui::vec2(left_width, 32.0),
                    )),
                    |ui| {
                        let title = ui.add(
                            egui::Label::new(RichText::new(&crumb).strong())
                                .truncate()
                                .sense(Sense::click_and_drag()),
                        );
                        let title_name = crumb_name.clone();
                        title.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                &title_name,
                            )
                        });
                        let title = forja_ui_kit::tooltip(title, crumb_label.clone());
                        if title.drag_started_by(egui::PointerButton::Primary) {
                            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                        }
                        if title.clicked() {
                            self.open_anvil(AnvilMode::Files);
                        }
                        for view in self
                            .top_views
                            .iter_mut()
                            .filter(|view| view.module != "find")
                        {
                            let mut cx = self.runtime.ctx(&view.module);
                            ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                        }
                    },
                );
                let search = ui.interact(
                    search_rect,
                    egui::Id::new("forja.top.search"),
                    Sense::click(),
                );
                search.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        &search_label,
                    )
                });
                ui.painter().rect(
                    search_rect,
                    6.0,
                    self.paint(self.tokens.color.surface_alt),
                    Stroke::new(1.0_f32, self.paint(self.tokens.color.border)),
                    egui::StrokeKind::Inside,
                );
                forja_ui_kit::paint_icon(
                    ui.painter(),
                    forja_ui_kit::Icon::Search,
                    egui::Rect::from_center_size(
                        search_rect.left_center() + egui::vec2(16.0, 0.0),
                        egui::vec2(16.0, 16.0),
                    ),
                    self.paint(self.tokens.color.text_muted),
                );
                ui.painter().text(
                    search_rect.left_center() + egui::vec2(32.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &search_label,
                    egui::FontId::proportional(self.tokens.metrics().body),
                    self.paint(self.tokens.color.text),
                );
                if search.clicked() {
                    self.open_anvil(AnvilMode::All);
                }
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(egui::Rect::from_min_max(
                            egui::pos2(bounds.right() - controls_width, bounds.top()),
                            bounds.max,
                        ))
                        .layout(Layout::right_to_left(Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if window_button(
                            ui,
                            forja_ui_kit::Icon::Close,
                            &self.text("shell.window.close"),
                        )
                        .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                        let maximized =
                            ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                        let (icon, key) = if maximized {
                            (forja_ui_kit::Icon::Restore, "shell.window.restore")
                        } else {
                            (forja_ui_kit::Icon::Maximize, "shell.window.maximize")
                        };
                        if window_button(ui, icon, &self.text(key)).clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                        if window_button(
                            ui,
                            forja_ui_kit::Icon::Minimize,
                            &self.text("shell.window.minimize"),
                        )
                        .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        let labels = [
                            self.text("shell.theme.system"),
                            self.text("shell.theme.dark"),
                            self.text("shell.theme.light"),
                        ];
                        let modes: Vec<_> = labels
                            .iter()
                            .map(|label| forja_ui_kit::MenuItem {
                                label,
                                danger: false,
                            })
                            .collect();
                        let tip = self.text(self.theme_label_key());
                        if let Some(index) =
                            forja_ui_kit::icon_menu(ui, forja_ui_kit::Icon::Theme, &tip, &modes)
                        {
                            self.set_theme_mode(["system", "dark", "light"][index]);
                        }
                        self.layout_selector(ui);
                    },
                );
            });
        self.slot_rects.insert("top".into(), response.response.rect);
    }

    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        if !self.layout_status_bar_visible() || self.focus_hides_status_bar() {
            return;
        }
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface_alt))
            .inner_margin(Margin::symmetric(10, 2));
        let status = self.status.clone();
        let response = egui::TopBottomPanel::bottom("forja.status")
            .exact_height(self.tokens.metrics().row)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(status) = status {
                        ui.separator();
                        let summary_max = (ui.available_width() - 280.0).max(120.0);
                        ui.scope(|ui| {
                            ui.set_max_width(summary_max);
                            let summary = ui.add(
                                egui::Label::new(
                                    RichText::new(&status.summary)
                                        .color(self.paint(self.tokens.color.danger)),
                                )
                                .truncate(),
                            );
                            forja_ui_kit::tooltip(summary, &status.summary);
                        });
                        let details_title = self.text("shell.status.details-title");
                        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &details_title)
                            .clicked()
                        {
                            self.status_details_open = !self.status_details_open;
                        }
                        let dismiss = self.text("shell.status.dismiss");
                        let button = forja_ui_kit::icon_button(ui, forja_ui_kit::CLOSE, &dismiss);
                        button.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                &dismiss,
                            )
                        });
                        if button.clicked() {
                            self.clear_status();
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let order = crate::status_bar_items::ordered_view_indexes(self);
                        for index in order.into_iter().rev() {
                            let view = &mut self.status_views[index];
                            let module = view.module.clone();
                            let mut cx = self.runtime.ctx(&module);
                            ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                        }
                    });
                });
            });
        self.slot_rects
            .insert("status".into(), response.response.rect);
        if response.response.secondary_clicked() {
            crate::status_bar_items::open_status_items_menu(ctx);
        }
        self.status_items_context(ctx);
        if self.status_details_open {
            if let Some(status) = self.status.clone() {
                let mut open = true;
                let title = self.text("shell.status.details-title");
                let copy_label = self.text("shell.status.copy");
                forja_ui_kit::modal(ctx, &mut open, &title, |ui| {
                    ui.set_max_width(480.0);
                    ui.add(
                        egui::Label::new(status.summary.clone())
                            .selectable(true)
                            .wrap(),
                    );
                    if let Some(detail) = &status.detail {
                        ui.add_space(self.tokens.metrics().gap);
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(detail)
                                            .small()
                                            .monospace()
                                            .color(self.paint(self.tokens.color.text_muted)),
                                    )
                                    .selectable(true)
                                    .wrap(),
                                );
                            });
                    }
                    ui.add_space(self.tokens.metrics().gap);
                    if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &copy_label)
                        .clicked()
                    {
                        let full = match &status.detail {
                            Some(detail) => format!("{}\n\n{detail}", status.summary),
                            None => status.summary.clone(),
                        };
                        ctx.copy_text(full);
                    }
                });
                if !open {
                    self.status_details_open = false;
                }
            } else {
                self.status_details_open = false;
            }
        }
    }

    pub(crate) fn left_panels(&mut self, ctx: &egui::Context) {
        if self.layout_rail_visible() && !self.focus_hides_rail() {
            self.rail(ctx);
        }
        if !self.no_project() && !self.focus_hides_side_panels() {
            self.sidebar(ctx);
        }
    }

    fn rail(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface_alt))
            .inner_margin(Margin::symmetric(6, 8));
        let settings_tip = rail_tooltip(
            &self.text("shell.rail.settings"),
            &self.chord_for("settings.open"),
        );
        // Ctrl+1 selects the first tool tab, not the Bancada destination.
        // Do not advertise a shortcut that navigates somewhere else.
        let bench_tip = self.text("shell.rail.bench");
        let pages: Vec<RailPage> = self
            .rail_views
            .iter()
            .map(|view| RailPage {
                id: view.id.clone(),
                title: rail_tooltip(
                    &self.text(&view.title_key),
                    &if view.id == "muster.page" {
                        self.chord_for("muster.open")
                    } else {
                        String::new()
                    },
                ),
                glyph: super::app::tab_bar::view_icon(view),
                png: view.icon_png.clone(),
            })
            .collect();
        let rail = egui::SidePanel::left("forja.rail")
            .exact_width(52.0)
            .resizable(false)
            .show_separator_line(true)
            .frame(frame)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let logo = self.logo(ui);
                    if logo.clicked() {
                        self.settings_open = false;
                        self.rail_page = None;
                    }
                    forja_ui_kit::tooltip(logo, bench_tip.clone());
                    ui.add_space(8.0);
                    if self
                        .rail_button(
                            ui,
                            "shell.bench",
                            None,
                            forja_ui_kit::Icon::Workbench,
                            &bench_tip,
                            self.rail_page.is_none() && !self.settings_open,
                        )
                        .clicked()
                    {
                        self.rail_page = None;
                        self.settings_open = false;
                    }
                    for page in self.visible_rail_pages(pages) {
                        let selected = self.rail_page.as_deref() == Some(page.id.as_str());
                        let mark = page.glyph;
                        if self
                            .rail_button(
                                ui,
                                &page.id,
                                page.png.as_deref(),
                                mark,
                                &page.title,
                                selected,
                            )
                            .clicked()
                        {
                            self.settings_open = false;
                            if selected {
                                self.rail_page = None;
                            } else {
                                self.rail_page = Some(page.id.clone());
                            }
                        }
                    }
                    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                        if self
                            .rail_button(
                                ui,
                                "shell.settings",
                                None,
                                forja_ui_kit::SETTINGS,
                                &settings_tip,
                                self.settings_open,
                            )
                            .clicked()
                        {
                            self.settings_open = !self.settings_open;
                            self.settings_query.clear();
                        }
                    });
                });
            });
        self.slot_rects.insert("rail".into(), rail.response.rect);
    }

    fn logo(&mut self, ui: &mut egui::Ui) -> egui::Response {
        if self.logo.is_none() {
            if let Some(icon) = icon::window_icon() {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [icon.width as usize, icon.height as usize],
                    &icon.rgba,
                );
                self.logo = Some(ui.ctx().load_texture(
                    "forja.logo",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }
        let bench_name = self.text("shell.rail.bench");
        let response = if let Some(texture) = &self.logo {
            ui.add(
                egui::Image::from_texture(texture)
                    .fit_to_exact_size(egui::vec2(28.0, 28.0))
                    .sense(Sense::hover()),
            )
        } else {
            ui.add(
                egui::Label::new(
                    RichText::new(self.text("shell.rail.dot"))
                        .size(18.0)
                        .color(self.paint(self.tokens.color.accent)),
                )
                .sense(Sense::click()),
            )
        };
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &bench_name)
        });
        response
    }

    fn rail_button(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        png: Option<&[u8]>,
        glyph: forja_ui_kit::Icon,
        tip: &str,
        selected: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                selected,
                tip.lines().next().unwrap_or(tip),
            )
        });
        let fill = if selected {
            self.paint(self.tokens.color.accent_soft)
        } else if response.hovered() {
            self.paint(self.tokens.color.surface)
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 8.0, fill);
        if selected {
            forja_ui_kit::selection_mark(ui, egui::Id::new("forja.rail.mark"), egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())), true);
        }
        if let Some(texture) = png.and_then(|bytes| self.rail_texture(ui, id, bytes)) {
            let image_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0));
            ui.painter().image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            forja_ui_kit::paint_icon(
                ui.painter(),
                glyph,
                egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
                self.paint(self.tokens.color.text),
            );
        }
        forja_ui_kit::tooltip(response, tip.to_owned())
    }

    fn rail_texture(&mut self, ui: &egui::Ui, id: &str, png: &[u8]) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.rail_icons.get(id) {
            return Some(texture.clone());
        }
        let image = image::load_from_memory(png).ok()?.to_rgba8();
        let texture = ui.ctx().load_texture(
            format!("forja.rail.{id}"),
            egui::ColorImage::from_rgba_unmultiplied(
                [image.width() as usize, image.height() as usize],
                image.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        );
        self.rail_icons.insert(id.to_owned(), texture.clone());
        Some(texture)
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let width = self.setting_number("appearance.sidebar.width", 260.0) as f32;
        let max_width = panel_max_width(ctx.available_rect().width(), 680.0);
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(12, 8));
        let has_views = !self.sidebar_views.is_empty();
        let sidebar = egui::SidePanel::left("forja.sidebar")
            .default_width(width)
            .width_range(180.0..=max_width)
            .resizable(true)
            .show_separator_line(true)
            .frame(frame)
            .show_animated(ctx, self.show_sidebar, |ui| {
                if !has_views {
                    ui.heading(self.text("shell.sidebar.title"));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(self.text("shell.sidebar.empty"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                    return;
                }
                let index = self.panel_icon_strip(ui, true);
                self.sidebar_tab = index;
                self.paint_view(ui, true, index);
            });
        if let Some(sidebar) = sidebar {
            self.slot_rects.insert("sidebar".into(), sidebar.response.rect);
        }
    }

    pub(crate) fn right_panel(&mut self, ctx: &egui::Context) {
        if self.no_project() || self.focus_hides_side_panels() {
            return;
        }
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(12, 8));
        let has_views = !self.right_views.is_empty();
        let max_width = panel_max_width(ctx.available_rect().width(), 480.0);
        let panel = egui::SidePanel::right("forja.right")
            .default_width(300.0)
            .width_range(180.0..=max_width)
            .resizable(true)
            .show_separator_line(true)
            .frame(frame)
            .show_animated(ctx, self.show_right_panel, |ui| {
                if !has_views {
                    ui.label(
                        RichText::new(self.text("shell.slot.empty"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                    return;
                }
                let index = self.panel_icon_strip(ui, false);
                self.right_tab = index;
                self.paint_view(ui, false, index);
            });
        if let Some(panel) = panel {
            self.slot_rects.insert("right".into(), panel.response.rect);
        }
    }

    pub(crate) fn central_area(&mut self, ctx: &egui::Context) {
        let home = self.no_project();
        if home && self.project_was_open {
            self.rail_page = None;
        }
        self.project_was_open = !home;
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.bg))
            .inner_margin(if self.rail_page.is_some() && !self.settings_open {
                Margin::ZERO
            } else {
                Margin::symmetric(12, 8)
            });
        let response = egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if self.settings_open {
                self.settings_page_ui(ui);
                return;
            }
            if let Some(page) = self.rail_page.clone() {
                if self.rail_views.iter().any(|view| view.id == page) {
                    if let Some(view) = self.rail_views.iter_mut().find(|view| view.id == page) {
                        let module = view.module.clone();
                        let mut cx = self.runtime.ctx(&module);
                        ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                        return;
                    }
                }
                self.rail_page = None;
            }
            if home {
                if let Some(view) = self.home_view.as_mut() {
                    let mut cx = self.runtime.ctx(&view.module);
                    ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                }
                return;
            }
            let focus_active = std::mem::take(&mut self.focus_content_tab);
            self.bench_tab_strip(ui, focus_active);
            ui.add_space(8.0);
            let visible = self.tabs.visible(self.bench_views.len());
            if visible.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(88.0);
                    ui.heading(self.text("shell.bench.empty-title"));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(self.text("shell.bench.empty-hint"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                });
                return;
            }
            let active = self
                .tabs
                .clamp(self.active_tab, self.bench_views.len())
                .unwrap_or(visible[0]);
            self.active_tab = active;
            if let Some(view) = self.bench_views.get_mut(active) {
                let module = view.module.clone();
                let mut cx = self.runtime.ctx(&module);
                ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
            }
        });
        self.slot_rects
            .insert("bench".into(), response.response.rect);
    }

    fn paint_view(&mut self, ui: &mut egui::Ui, sidebar: bool, index: usize) {
        let salt = if sidebar {
            "forja.sidebar.body"
        } else {
            "forja.right.body"
        };
        let view = if sidebar {
            self.sidebar_views.get_mut(index)
        } else {
            self.right_views.get_mut(index)
        };
        let Some(view) = view else {
            return;
        };
        let module = view.module.clone();
        let view_id = view.id.clone();
        let title = self.runtime.i18n().t("shell.view-error.title");
        let retry_label = self.runtime.i18n().t("shell.view-error.retry");
        let failure_body = view.failure.as_deref().map(|message| {
            self.runtime.i18n().t_args(
                "shell.view-error.message",
                &[("view", &view_id), ("message", message)],
            )
        });
        let unknown_panic = self.runtime.i18n().t("shell.view-error.unknown");
        let debug_message = self.runtime.i18n().t("shell.view-error.debug-panic");
        let mut retry = false;
        egui::ScrollArea::vertical().id_salt(salt).show(ui, |ui| {
            if failure_body.is_some() {
                ui.vertical_centered(|ui| {
                    ui.heading(&title);
                    ui.add_space(8.0);
                    ui.label(failure_body.as_deref().unwrap_or_default());
                    ui.add_space(8.0);
                    retry = button(ui, ButtonKind::Secondary, &retry_label).clicked();
                });
            } else {
                let debug_panic = self
                    .driver
                    .as_mut()
                    .is_some_and(|driver| driver.take_view_panic(&view_id));
                let result = crate::crash::catch_view_panic(|| {
                    if debug_panic {
                        panic!("{debug_message}");
                    }
                    let mut cx = self.runtime.ctx(&module);
                    ui.push_id(&view_id, |ui| view.view.ui(ui, &mut cx));
                });
                if let Err(payload) = result {
                    let mut message = crate::crash::panic_payload_message(payload.as_ref());
                    if message == "non-string panic payload" {
                        message = unknown_panic.clone();
                    }
                    message = single_line(&message);
                    forja_kernel::logging::log(
                        forja_kernel::logging::LogLevel::Error,
                        format!("view panic in module `{module}`, view `{view_id}`: {message}"),
                    );
                    view.failure = Some(message);
                }
            }
        });
        if retry {
            view.failure = None;
            ui.ctx().request_repaint();
        }
    }

    pub(crate) fn modal_windows(&mut self, ctx: &egui::Context) {
        for view in self.modal_views.iter_mut() {
            let module = view.module.clone();
            let id = view.id.clone();
            {
                let mut cx = self.runtime.ctx(&module);
                view.view.poll(&mut cx);
            }
            // `--modal <id>` (captura) mostra a view mesmo fechada, com o payload vazio.
            let forced = self.modal_capture.as_deref() == Some(id.as_str());
            if !forced && !self.runtime.modal_is_open(&id) {
                continue;
            }
            let mut cx = self.runtime.ctx(&module);
            egui::Area::new(egui::Id::new(("forja.modal", &view.id)))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                });
        }
        self.storage_copy_modal(ctx);
        self.close_confirm_modal(ctx);
    }
}

fn single_line(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = collapsed.chars();
    let message: String = characters.by_ref().take(180).collect();
    if characters.next().is_some() {
        format!("{}…", message.trim_end())
    } else {
        message
    }
}

fn window_button(ui: &mut egui::Ui, glyph: forja_ui_kit::Icon, label: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(46.0, 32.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            4.0,
            forja_ui_kit::applied_tokens(ui.ctx())
                .color
                .surface_alt
                .to_color32(),
        );
    }
    forja_ui_kit::paint_icon(
        ui.painter(),
        glyph,
        egui::Rect::from_center_size(rect.center(), egui::vec2(16.0, 16.0)),
        forja_ui_kit::applied_tokens(ui.ctx())
            .color
            .text
            .to_color32(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    forja_ui_kit::tooltip(response, label)
}

#[cfg(test)]
mod tests {
    use super::{panel_max_width, rail_tooltip, single_line};
    use forja_kernel::i18n::{BundleSource, I18n, Locale};

    fn shell_i18n(locale: Locale) -> I18n {
        let source = BundleSource {
            module: "shell".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        };
        let (i18n, warnings) = I18n::build(&[source], locale);
        assert!(warnings.is_empty(), "{warnings:?}");
        i18n
    }

    #[test]
    fn crumb_accessible_name_carries_label_and_live_value() {
        let value = "sample · bancada-teste";
        assert_eq!(
            shell_i18n(Locale::PtBr).t_args("shell.top.crumb-name", &[("value", value)]),
            "Projeto e bancada ativos: sample · bancada-teste"
        );
        assert_eq!(
            shell_i18n(Locale::En).t_args("shell.top.crumb-name", &[("value", value)]),
            "Active project and bench: sample · bancada-teste"
        );
    }

    #[test]
    fn rail_tooltips_keep_the_accessible_name_separate_from_the_shortcut() {
        assert_eq!(
            rail_tooltip("Mutirão", "Ctrl+Shift+M"),
            "Mutirão\nCtrl+Shift+M"
        );
        assert_eq!(
            rail_tooltip("Ajustes", "Ctrl+Comma").lines().next(),
            Some("Ajustes")
        );
        assert_eq!(rail_tooltip("Bancada", ""), "Bancada");
    }

    #[test]
    fn side_panels_leave_room_for_the_center() {
        let after_rail = 1280.0 - 52.0;
        let sidebar = panel_max_width(after_rail, 680.0);
        let right = panel_max_width(after_rail - sidebar, 480.0);
        assert!(after_rail - sidebar - right >= 480.0);
        assert_eq!(panel_max_width(4000.0, 480.0), 480.0);
    }

    #[test]
    fn panic_summary_is_kept_on_one_line_and_capped() {
        assert_eq!(
            single_line("first line\n second line"),
            "first line second line"
        );
        let message = single_line(&"x".repeat(200));
        assert_eq!(message.chars().count(), 181);
        assert!(message.ends_with('…'));
    }
}
