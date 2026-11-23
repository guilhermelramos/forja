//! Read-only viewer tab.

use crate::model;
use crate::state::Shared;
use forja_contracts::MarkdownBlock;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct ViewerView {
    state: Shared,
    images: Option<ImageVisual>,
    image_slot: Arc<Mutex<Option<Vec<model::Frame>>>>,
    image_key: Option<String>,
    loading: bool,
}

struct ImageVisual {
    textures: Vec<egui::TextureHandle>,
    delays: Vec<f32>,
    started: Instant,
}

impl ViewerView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            images: None,
            image_slot: Arc::new(Mutex::new(None)),
            image_key: None,
            loading: false,
        }
    }
}

impl View for ViewerView {
    fn empty_by_nature(&self) -> bool {
        true
    }

    fn has_content(&self) -> bool {
        self.state.document().is_some()
            || self.state.path().is_some()
            || std::env::var("FORJA_VIEWER").is_ok_and(|value| !value.trim().is_empty())
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        if self.state.document().is_none() {
            if let Ok(path) = std::env::var("FORJA_VIEWER") {
                if !path.trim().is_empty() {
                    self.state.set_request(path);
                }
            }
        }
        if let Some(path) = self.state.path() {
            if self.state.document().is_none() {
                let mut args = Map::new();
                args.insert("path".to_owned(), json!(path));
                match cx.run_command("viewers.preview", Args::from_map(args)) {
                    Ok(value) => match serde_json::from_value(value) {
                        Ok(document) => self.state.set_document(document),
                        Err(error) => self.state.set_notice(Some(
                            cx.t_args("viewers.notice.preview", &[("message", &error.to_string())]),
                        )),
                    },
                    Err(error) => self.state.set_notice(Some(
                        cx.t_args("viewers.notice.preview", &[("message", &error.to_string())]),
                    )),
                }
            }
        }
        let Some(document) = self.state.document() else {
            ui.heading(cx.t("viewers.tab.title"));
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("viewers.viewer.empty"))
                    .color(c32(tokens.color.text_muted)),
            );
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(cx.t("viewers.tab.title"));
            forja_ui_kit::text(ui,
                egui::RichText::new(&document.title)
                    .strong()
                    .color(c32(tokens.color.text)),
            );
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t(&format!("viewers.descriptor.{}", document.kind)))
                    .small()
                    .color(c32(tokens.color.text_muted)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("viewers.action.open-external"),
                )
                .clicked()
                {
                    let mut args = Map::new();
                    args.insert("path".to_owned(), json!(document.path));
                    let _ = cx.run_command("viewers.open_external", Args::from_map(args));
                }
            });
        });
        ui.separator();
        if let Some(error) = &document.error {
            forja_ui_kit::text(ui, egui::RichText::new(error).color(c32(tokens.color.danger)));
            return;
        }
        match document.kind.as_str() {
            "markdown" => {
                if let Some(markdown) = &document.markdown {
                    render_markdown(ui, cx, markdown);
                }
            }
            "table" => {
                if let Some(table) = &document.table {
                    render_table(ui, cx, table, &self.state);
                }
            }
            "notebook" => {
                if let Some(notebook) = &document.notebook {
                    render_notebook(ui, cx, notebook);
                }
            }
            "image" => {
                if let Some(image) = &document.image {
                    self.render_image(ui, cx, image);
                }
            }
            "pdf" => {
                if let Some(pdf) = &document.pdf {
                    forja_ui_kit::text(ui, cx.t_args("viewers.pdf.pages", &[("pages", &pdf.pages.to_string())]));
                    forja_ui_kit::text(ui,
                        egui::RichText::new(cx.t("viewers.pdf.hint"))
                            .small()
                            .color(c32(tokens.color.text_muted)),
                    );
                }
            }
            _ => {}
        }
        if let Some(notice) = self.state.notice() {
            forja_ui_kit::text(ui,
                egui::RichText::new(notice)
                    .small()
                    .color(c32(tokens.color.info)),
            );
        }
    }
}

impl ViewerView {
    fn render_image(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        image: &forja_contracts::ImageView,
    ) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        if self.image_key.as_deref() != Some(image.path.as_str()) && !self.loading {
            self.start_image(cx, &image.path);
        }
        if self.images.is_none() {
            let frames = self.image_slot.lock().ok().and_then(|mut slot| slot.take());
            if let Some(frames) = frames {
                self.loading = false;
                let textures = frames
                    .iter()
                    .enumerate()
                    .map(|(index, frame)| {
                        let color = egui::ColorImage::from_rgba_unmultiplied(
                            [frame.width, frame.height],
                            &frame.rgba,
                        );
                        ui.ctx().load_texture(
                            format!("viewer:{}:{index}", image.path),
                            color,
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .collect();
                let delays = frames.iter().map(|frame| frame.delay).collect();
                self.images = Some(ImageVisual {
                    textures,
                    delays,
                    started: Instant::now(),
                });
            }
        }
        let zoom = self.state.zoom();
        ui.horizontal(|ui| {
            if forja_ui_kit::icon_button(
                ui,
                forja_ui_kit::Icon::Minimize,
                &cx.t("viewers.action.zoom-out"),
            )
            .clicked()
            {
                self.state.add_zoom(-0.25);
            }
            forja_ui_kit::text(ui, format!("{:.0}%", zoom * 100.0));
            if forja_ui_kit::icon_button(
                ui,
                forja_ui_kit::Icon::Add,
                &cx.t("viewers.action.zoom-in"),
            )
            .clicked()
            {
                self.state.add_zoom(0.25);
            }
            if image.frames > 1 {
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t_args(
                        "viewers.image.frames",
                        &[("frames", &image.frames.to_string())],
                    ))
                    .small()
                    .color(c32(tokens.color.text_muted)),
                );
            }
        });
        let Some(visual) = &self.images else {
            forja_ui_kit::text(ui,
                egui::RichText::new(cx.t("viewers.image.loading"))
                    .small()
                    .color(c32(tokens.color.text_muted)),
            );
            return;
        };
        if visual.textures.is_empty() {
            return;
        }
        let index = if visual.textures.len() > 1 {
            let total: f32 = visual.delays.iter().sum();
            let elapsed = visual.started.elapsed().as_secs_f32() % total.max(0.05);
            let mut cursor = 0.0;
            let mut index = 0;
            let mut remaining = 0.1_f32;
            for (position, duration) in visual.delays.iter().enumerate() {
                cursor += duration;
                if elapsed <= cursor {
                    index = position;
                    remaining = (cursor - elapsed).max(0.016);
                    break;
                }
            }
            forja_ui_kit::request_repaint_after_active(
                ui.ctx(),
                Duration::from_secs_f32(remaining),
            );
            index
        } else {
            0
        };
        let texture = &visual.textures[index];
        let size = texture.size_vec2() * zoom;
        ui.add(egui::Image::new((texture.id(), size)));
    }

    fn start_image(&mut self, cx: &mut Ctx<'_>, path: &str) {
        self.loading = true;
        self.image_key = Some(path.to_owned());
        self.images = None;
        let slot = Arc::clone(&self.image_slot);
        let target = path.to_owned();
        cx.spawn(
            move || model::load_frames(&target),
            move |_cx, result| {
                if let Ok(frames) = result {
                    if let Ok(mut slot) = slot.lock() {
                        *slot = Some(frames);
                    }
                }
            },
        );
    }
}

fn render_markdown(ui: &mut egui::Ui, cx: &mut Ctx<'_>, markdown: &forja_contracts::MarkdownView) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    ui.horizontal_top(|ui| {
        if !markdown.toc.is_empty() {
            ui.vertical(|ui| {
                ui.set_width(180.0);
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t("viewers.markdown.toc"))
                        .small()
                        .color(c32(tokens.color.text_muted)),
                );
                for entry in &markdown.toc {
                    let indent = (entry.level.saturating_sub(1) as f32) * 10.0;
                    ui.horizontal(|ui| {
                        ui.add_space(indent);
                        forja_ui_kit::text(ui,
                            egui::RichText::new(&entry.text)
                                .small()
                                .color(c32(tokens.color.text)),
                        );
                    });
                }
            });
            ui.separator();
        }
        egui::ScrollArea::vertical()
            .id_salt("viewer-markdown")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical(|ui| render_blocks(ui, cx, &markdown.blocks));
            });
    });
}

fn render_blocks(ui: &mut egui::Ui, cx: &mut Ctx<'_>, blocks: &[MarkdownBlock]) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    for block in blocks {
        match block.kind.as_str() {
            "heading" => {
                let size = match block.level {
                    1 => tokens.metrics().title + 6.0,
                    2 => tokens.metrics().title + 2.0,
                    3 => tokens.metrics().body + 4.0,
                    _ => tokens.metrics().body + 1.0,
                };
                ui.add_space(6.0);
                forja_ui_kit::text(ui,
                    egui::RichText::new(&block.text)
                        .size(size)
                        .strong()
                        .color(c32(tokens.color.text)),
                );
            }
            "paragraph" | "quote" => {
                forja_ui_kit::text(ui,
                    egui::RichText::new(&block.text).color(c32(if block.kind == "quote" {
                        tokens.color.text_muted
                    } else {
                        tokens.color.text
                    })),
                );
            }
            "list_item" => {
                forja_ui_kit::text(ui,
                    egui::RichText::new(format!("\u{2022} {}", block.text))
                        .color(c32(tokens.color.text)),
                );
            }
            "code" => {
                let frame = egui::Frame::new()
                    .fill(c32(tokens.color.surface_alt))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 6));
                frame.show(ui, |ui| {
                    if let Some(language) = &block.language {
                        forja_ui_kit::text(ui,
                            egui::RichText::new(language)
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        );
                    }
                    forja_ui_kit::text(ui, egui::RichText::new(&block.text).monospace());
                    if block.language.as_deref() == Some("mermaid") {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Secondary,
                            &cx.t("viewers.markdown.mermaid"),
                        )
                        .clicked()
                        {
                            let mut args = Map::new();
                            args.insert("source".to_owned(), json!(block.text));
                            let _ = cx.run_command("browser.mermaid.render", Args::from_map(args));
                        }
                        forja_ui_kit::text(ui,
                            egui::RichText::new(cx.t("viewers.markdown.mermaid-hint"))
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        );
                    }
                });
            }
            "callout" => {
                let kind = block.callout.as_deref().unwrap_or("note");
                let color = match kind {
                    "warning" | "caution" => tokens.color.spark,
                    "important" => tokens.color.info,
                    "tip" => tokens.color.ok,
                    _ => tokens.color.info,
                };
                let frame = egui::Frame::new()
                    .fill(c32(tokens.color.surface_alt))
                    .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(10, 8));
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (bar, _) =
                            ui.allocate_exact_size(egui::vec2(3.0, 16.0), egui::Sense::hover());
                        ui.painter().rect_filled(bar, 2, c32(color));
                        forja_ui_kit::text(ui,
                            egui::RichText::new(cx.t(&format!("viewers.callout.{kind}")))
                                .strong()
                                .color(c32(tokens.color.text)),
                        );
                    });
                    forja_ui_kit::text(ui, egui::RichText::new(&block.text).color(c32(tokens.color.text)));
                });
            }
            "rule" => {
                ui.separator();
            }
            _ => {
                if !block.text.is_empty() {
                    forja_ui_kit::text(ui, egui::RichText::new(&block.text).color(c32(tokens.color.text)));
                }
            }
        }
        ui.add_space(2.0);
    }
}

fn render_table(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    table: &forja_contracts::TableView,
    state: &Shared,
) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let page_size = 40;
    let pages = table.rows.len().div_ceil(page_size).max(1);
    let page_index = state.page().min(pages - 1);
    let start = page_index * page_size;
    let end = (start + page_size).min(table.rows.len());
    forja_ui_kit::text(ui,
        egui::RichText::new(format!(
            "{} {}",
            cx.t("viewers.table.rows"),
            table.rows.len()
        ))
        .small()
        .color(c32(tokens.color.text_muted)),
    );
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("viewer.table.grid")
                .striped(true)
                .show(ui, |ui| {
                    for header in &table.headers {
                        forja_ui_kit::text(ui, egui::RichText::new(header).strong());
                    }
                    ui.end_row();
                    for row in &table.rows[start..end] {
                        for index in 0..table.headers.len().max(row.len()) {
                            forja_ui_kit::text(ui, row.get(index).map(String::as_str).unwrap_or(""));
                        }
                        ui.end_row();
                    }
                });
        });
    ui.horizontal(|ui| {
        if forja_ui_kit::icon_button(
            ui,
            forja_ui_kit::Icon::Back,
            &cx.t("viewers.action.previous"),
        )
        .clicked()
        {
            state.set_page(page_index.saturating_sub(1));
        }
        forja_ui_kit::text(ui, format!("{}/{}", page_index + 1, pages));
        if forja_ui_kit::icon_button(
            ui,
            forja_ui_kit::Icon::Forward,
            &cx.t("viewers.action.next"),
        )
        .clicked()
        {
            state.set_page((page_index + 1).min(pages - 1));
        }
    });
    if table.truncated {
        forja_ui_kit::text(ui,
            egui::RichText::new(cx.t("viewers.table.truncated"))
                .small()
                .color(c32(tokens.color.spark)),
        );
    }
}

fn render_notebook(ui: &mut egui::Ui, cx: &mut Ctx<'_>, notebook: &forja_contracts::NotebookView) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    forja_ui_kit::text(ui,
        egui::RichText::new(format!(
            "{} {}",
            cx.t("viewers.notebook.cells"),
            notebook.cells.len()
        ))
        .small()
        .color(c32(tokens.color.text_muted)),
    );
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, cell) in notebook.cells.iter().enumerate() {
                let frame = egui::Frame::new()
                    .fill(c32(if cell.kind == "code" {
                        tokens.color.surface
                    } else {
                        tokens.color.bg
                    }))
                    .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(8, 6));
                frame.show(ui, |ui| {
                    forja_ui_kit::text(ui,
                        egui::RichText::new(format!(
                            "[{}] {}",
                            index + 1,
                            cx.t(&format!("viewers.notebook.kind.{}", cell.kind))
                        ))
                        .small()
                        .color(c32(tokens.color.text_muted)),
                    );
                    if cell.kind == "markdown" {
                        let markdown = crate::model::parse_markdown(&cell.source);
                        render_blocks(ui, cx, &markdown.blocks);
                    } else {
                        forja_ui_kit::text(ui, egui::RichText::new(&cell.source).monospace());
                        for output in &cell.outputs {
                            forja_ui_kit::text(ui,
                                egui::RichText::new(output)
                                    .monospace()
                                    .small()
                                    .color(c32(tokens.color.text_muted)),
                            );
                        }
                    }
                });
                ui.add_space(forja_ui_kit::Spacing::XS);
            }
        });
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    color.to_color32()
}
