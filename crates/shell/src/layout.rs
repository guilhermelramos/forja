//! Fixed layout regions from `design/product.md` §3.

use crate::actions::AnvilMode;
use crate::app::ForjaApp;
use crate::icon;
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Stroke, ViewportCommand};
use forja_ui_kit::{button, ButtonKind};
use std::sync::Arc;

struct RailPage {
    id: String,
    title: String,
    glyph: Option<String>,
    png: Option<Arc<[u8]>>,
}

impl ForjaApp {
    pub(crate) fn top_bar(&mut self, ctx: &egui::Context) {
        let height = self.tokens.metrics().control + 8.0;
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(10, 4))
            .stroke(Stroke::new(1.0_f32, self.paint(self.tokens.color.border)));
        let find_label = format!(
            "{}  {}",
            self.text("shell.top.find"),
            self.chord_for("find.file")
        );
        let anvil_label = format!(
            "{}  {}",
            self.text("shell.top.anvil"),
            self.chord_for("anvil.open")
        );
        let crumb = self.text("shell.top.no-project");
        let has_top = !self.top_views.is_empty();
        let response = egui::TopBottomPanel::top("forja.top")
            .exact_height(height)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let title = ui.add(
                        egui::Label::new(
                            RichText::new(crumb)
                                .strong()
                                .color(self.paint(self.tokens.color.text)),
                        )
                        .sense(Sense::click_and_drag()),
                    );
                    if title.drag_started() {
                        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
                    }
                    ui.separator();
                    if has_top {
                        for view in self.top_views.iter_mut() {
                            let module = view.module.clone();
                            let mut cx = self.runtime.ctx(&module);
                            ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                        }
                    } else if button(ui, ButtonKind::Ghost, &find_label).clicked() {
                        self.open_anvil(AnvilMode::Files);
                    }
                    if button(ui, ButtonKind::Secondary, &anvil_label).clicked() {
                        self.open_anvil(AnvilMode::All);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if button(
                            ui,
                            ButtonKind::Ghost,
                            &self.text("shell.window.close-glyph"),
                        )
                        .on_hover_text(self.text("shell.window.close"))
                        .clicked()
                        {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                        }
                        if button(
                            ui,
                            ButtonKind::Ghost,
                            &self.text("shell.window.maximize-glyph"),
                        )
                        .on_hover_text(self.text("shell.window.maximize"))
                        .clicked()
                        {
                            let maximized = ui
                                .ctx()
                                .input(|input| input.viewport().maximized.unwrap_or(false));
                            ui.ctx()
                                .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                        if button(
                            ui,
                            ButtonKind::Ghost,
                            &self.text("shell.window.minimize-glyph"),
                        )
                        .on_hover_text(self.text("shell.window.minimize"))
                        .clicked()
                        {
                            ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        ui.separator();
                        let system = self.text("shell.theme.system");
                        let dark = self.text("shell.theme.dark");
                        let light = self.text("shell.theme.light");
                        let theme_tip = self.text(self.theme_label_key());
                        let modes = [
                            forja_ui_kit::MenuItem {
                                label: system.as_str(),
                                danger: false,
                            },
                            forja_ui_kit::MenuItem {
                                label: dark.as_str(),
                                danger: false,
                            },
                            forja_ui_kit::MenuItem {
                                label: light.as_str(),
                                danger: false,
                            },
                        ];
                        if let Some(index) =
                            forja_ui_kit::icon_menu(ui, forja_ui_kit::THEME, &theme_tip, &modes)
                        {
                            let mode = ["system", "dark", "light"][index];
                            self.set_theme_mode(mode);
                        }
                    });
                });
            });
        self.slot_rects.insert("top".into(), response.response.rect);
    }

    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        if !self.setting_bool("appearance.status_bar.visible", true) {
            return;
        }
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface_alt))
            .inner_margin(Margin::symmetric(10, 2));
        let status = self.status.clone();
        let modules = self.module_count;
        let module_key = if modules == 1 {
            "shell.status.module"
        } else {
            "shell.status.modules"
        };
        let module_label = format!("{modules} {}", self.text(module_key));
        let ready = self.text("shell.status.ready");
        let response = egui::TopBottomPanel::bottom("forja.status")
            .exact_height(self.tokens.metrics().row)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(ready).color(self.paint(self.tokens.color.ok)));
                    ui.separator();
                    ui.label(module_label);
                    if let Some(status) = status {
                        ui.separator();
                        ui.label(RichText::new(status).color(self.paint(self.tokens.color.danger)));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        for view in self.status_views.iter_mut().rev() {
                            let module = view.module.clone();
                            let mut cx = self.runtime.ctx(&module);
                            ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                        }
                    });
                });
            });
        self.slot_rects
            .insert("status".into(), response.response.rect);
    }

    pub(crate) fn left_panels(&mut self, ctx: &egui::Context) {
        if self.setting_bool("appearance.rail.visible", true) {
            self.rail(ctx);
        }
        if self.setting_bool("appearance.sidebar.visible", true) {
            self.sidebar(ctx);
        }
    }

    fn rail(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface_alt))
            .inner_margin(Margin::symmetric(6, 8));
        let settings_tip = self.text("shell.rail.settings");
        let bench_tip = self.text("shell.rail.bench");
        let pages: Vec<RailPage> = self
            .rail_views
            .iter()
            .map(|view| RailPage {
                id: view.id.clone(),
                title: self.text(&view.title_key),
                glyph: view.icon_glyph.clone(),
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
                        self.rail_page = None;
                    }
                    logo.on_hover_text(bench_tip.clone());
                    ui.add_space(8.0);
                    for page in &pages {
                        let selected = self.rail_page.as_deref() == Some(page.id.as_str());
                        let mark = page.glyph.clone().unwrap_or_else(|| {
                            page.title
                                .chars()
                                .next()
                                .map(|ch| ch.to_string())
                                .unwrap_or_default()
                        });
                        if self
                            .rail_button(
                                ui,
                                &page.id,
                                page.png.as_deref(),
                                &mark,
                                &page.title,
                                selected,
                            )
                            .clicked()
                        {
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
                                false,
                            )
                            .clicked()
                        {
                            self.settings_open = true;
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
        if let Some(texture) = &self.logo {
            ui.add(
                egui::Image::from_texture(texture)
                    .fit_to_exact_size(egui::vec2(28.0, 28.0))
                    .sense(Sense::click_and_drag()),
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
        }
    }

    fn rail_button(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        png: Option<&[u8]>,
        glyph: &str,
        tip: &str,
        selected: bool,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), Sense::click());
        let fill = if selected {
            self.paint(self.tokens.color.accent_soft)
        } else if response.hovered() {
            self.paint(self.tokens.color.surface)
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 8.0, fill);
        if let Some(texture) = png.and_then(|bytes| self.rail_texture(ui, id, bytes)) {
            let image_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0));
            ui.painter().image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                glyph,
                forja_ui_kit::icon_font(16.0),
                self.paint(self.tokens.color.text),
            );
        }
        response.on_hover_text(tip.to_owned())
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
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(12, 8));
        let titles: Vec<String> = self
            .sidebar_views
            .iter()
            .map(|view| self.text(&view.title_key))
            .collect();
        let sidebar = egui::SidePanel::left("forja.sidebar")
            .default_width(width)
            .width_range(180.0..=480.0)
            .resizable(true)
            .show_separator_line(true)
            .frame(frame)
            .show(ctx, |ui| {
                if titles.is_empty() {
                    ui.heading(self.text("shell.sidebar.title"));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(self.text("shell.sidebar.empty"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                    return;
                }
                if titles.len() == 1 {
                    ui.heading(self.text("shell.sidebar.title"));
                    ui.add_space(4.0);
                }
                let index = Self::slot_tabs(ui, &titles, self.sidebar_tab, "forja.sidebar.tabs");
                self.sidebar_tab = index;
                self.paint_view(ui, true, index);
            });
        self.slot_rects
            .insert("sidebar".into(), sidebar.response.rect);
    }

    pub(crate) fn right_panel(&mut self, ctx: &egui::Context) {
        if !self.show_right_panel {
            return;
        }
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface))
            .inner_margin(Margin::symmetric(12, 8));
        let titles: Vec<String> = self
            .right_views
            .iter()
            .map(|view| self.text(&view.title_key))
            .collect();
        let panel = egui::SidePanel::right("forja.right")
            .default_width(300.0)
            .resizable(true)
            .show_separator_line(true)
            .frame(frame)
            .show(ctx, |ui| {
                if titles.is_empty() {
                    ui.label(
                        RichText::new(self.text("shell.slot.empty"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                    return;
                }
                let index = Self::slot_tabs(ui, &titles, self.right_tab, "forja.right.tabs");
                self.right_tab = index;
                self.paint_view(ui, false, index);
            });
        self.slot_rects.insert("right".into(), panel.response.rect);
    }

    pub(crate) fn central_area(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.bg))
            .inner_margin(Margin::symmetric(12, 8));
        let visible = self.tabs.visible(self.bench_views.len());
        let titles: Vec<String> = visible
            .iter()
            .map(|index| self.text(&self.bench_views[*index].title_key))
            .collect();
        let response = egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
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
            if titles.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(96.0);
                    ui.heading(self.text("shell.bench.empty-title"));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(self.text("shell.bench.empty-hint"))
                            .color(self.paint(self.tokens.color.text_muted)),
                    );
                });
                return;
            }
            let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
            let mut selected = visible
                .iter()
                .position(|index| *index == self.active_tab)
                .unwrap_or(0);
            forja_ui_kit::tabs(ui, &mut selected, &refs);
            let active = visible.get(selected).copied().unwrap_or(visible[0]);
            self.active_tab = active;
            ui.add_space(8.0);
            if let Some(view) = self.bench_views.get_mut(active) {
                let module = view.module.clone();
                let mut cx = self.runtime.ctx(&module);
                ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
            }
        });
        self.slot_rects
            .insert("bench".into(), response.response.rect);
    }

    /// Tab strip when a slot has more than one contribution. Returns the kept index.
    fn slot_tabs(ui: &mut egui::Ui, titles: &[String], mut selected: usize, id: &str) -> usize {
        if titles.len() < 2 {
            return 0;
        }
        if selected >= titles.len() {
            selected = 0;
        }
        let refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        egui::ScrollArea::horizontal().id_salt(id).show(ui, |ui| {
            forja_ui_kit::tabs(ui, &mut selected, &refs);
        });
        ui.add_space(8.0);
        selected
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
        let mut cx = self.runtime.ctx(&module);
        egui::ScrollArea::vertical().id_salt(salt).show(ui, |ui| {
            ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
        });
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
    }
}
