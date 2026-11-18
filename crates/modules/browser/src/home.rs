//! Native home page (`forja://inicio`) UI rendered in egui without external network calls.

use crate::home_data::{
    discover_docs, filter_bench_servers, open_doc_in_viewer, parse_ports_response,
    recent_shortcuts, resolve_active_root, ActiveRoot, DocLink, PortEntry,
};
use crate::state::Shared;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use crate::home_data::is_home_url;

const PORTS_POLL_INTERVAL: Duration = Duration::from_millis(1500);
const MAX_RECENT_SHORTCUTS: usize = 8;

#[derive(Default)]
struct DocsCache {
    root_path: Option<PathBuf>,
    scanning: bool,
    links: Vec<DocLink>,
}

/// Mutable UI/cache state for `forja://inicio`.
pub struct HomeState {
    ports: Vec<PortEntry>,
    port_filter: String,
    only_active_bench: bool,
    active_root: Option<ActiveRoot>,
    last_poll: Option<Instant>,
    docs: Arc<Mutex<DocsCache>>,
}

impl Default for HomeState {
    fn default() -> Self {
        Self::new()
    }
}

impl HomeState {
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            port_filter: String::new(),
            only_active_bench: false,
            active_root: None,
            last_poll: None,
            docs: Arc::new(Mutex::new(DocsCache::default())),
        }
    }

    pub fn force_refresh(&mut self, cx: &mut Ctx<'_>) {
        self.last_poll = None;
        let _ = cx.run_command("ports.refresh", Args::new());
        if let Ok(mut docs) = self.docs.lock() {
            docs.root_path = None;
        }
        self.poll(cx);
    }

    /// Renders `forja://inicio` inside `rect` using egui & `forja_ui_kit`.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        shared: &Shared,
        rect: egui::Rect,
    ) {
        self.poll(cx);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let metrics = tokens.metrics();
        let muted = color32(tokens.color.text_muted);

        let content_rect = rect.shrink2(egui::vec2(12.0, 8.0));
        if content_rect.width() <= 40.0 || content_rect.height() <= 40.0 {
            return;
        }

        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("browser.home.scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.heading(cx.t("browser.view.home-title"));
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(cx.t("browser.home.offline-note"))
                                .small()
                                .color(muted),
                        );
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(cx.t("browser.view.home-body"))
                                .small()
                                .color(muted),
                        )
                        .wrap(),
                    );
                    ui.add_space(metrics.gap);

                    let wide = ui.available_width() >= 480.0;
                    if wide {
                        ui.columns(2, |columns| {
                            self.servers_card(&mut columns[0], cx, shared, muted);
                            columns[0].add_space(metrics.gap);
                            self.recent_card(&mut columns[0], cx, shared, muted);
                            self.docs_card(&mut columns[1], cx, shared, muted);
                        });
                    } else {
                        self.servers_card(ui, cx, shared, muted);
                        ui.add_space(metrics.gap);
                        self.docs_card(ui, cx, shared, muted);
                        ui.add_space(metrics.gap);
                        self.recent_card(ui, cx, shared, muted);
                    }
                });
        });
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        let should_poll = self
            .last_poll
            .map(|at| at.elapsed() >= PORTS_POLL_INTERVAL)
            .unwrap_or(true);
        if !should_poll {
            return;
        }
        self.last_poll = Some(Instant::now());

        if let Ok(value) = cx.call_method("ports.list", Map::new()) {
            self.ports = parse_ports_response(&value);
        }
        self.active_root = resolve_active_root(cx);

        let target_path = self.active_root.as_ref().map(|root| root.path.clone());
        let needs_scan = self
            .docs
            .lock()
            .ok()
            .map(|docs| !docs.scanning && docs.root_path != target_path)
            .unwrap_or(false);
        if needs_scan {
            if let Some(root_path) = target_path {
                if let Ok(mut docs) = self.docs.lock() {
                    docs.root_path = Some(root_path.clone());
                    docs.scanning = true;
                }
                let docs_arc = Arc::clone(&self.docs);
                let scan_root = root_path.clone();
                cx.spawn(
                    move || discover_docs(&scan_root),
                    move |_cx, links| {
                        if let Ok(mut docs) = docs_arc.lock() {
                            if docs.root_path.as_ref() == Some(&root_path) {
                                docs.links = links;
                                docs.scanning = false;
                            }
                        }
                    },
                );
            } else if let Ok(mut docs) = self.docs.lock() {
                docs.root_path = None;
                docs.scanning = false;
                docs.links.clear();
            }
        }
    }

    fn servers_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        shared: &Shared,
        muted: egui::Color32,
    ) {
        let title = cx.t("browser.home.servers-title");
        forja_ui_kit::card(ui, Some(&title), |ui| {
            ui.horizontal_wrapped(|ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("browser.home.servers-refresh"),
                )
                .clicked()
                {
                    self.force_refresh(cx);
                }
                if let Some(root) = &self.active_root {
                    if root.is_bench {
                        let toggle_label = cx.t_args(
                            "browser.home.servers-active-only",
                            &[("bench", &root.name)],
                        );
                        ui.checkbox(&mut self.only_active_bench, toggle_label);
                    }
                }
            });
            ui.add_space(forja_ui_kit::Spacing::XS);
            forja_ui_kit::text_field(
                ui,
                &mut self.port_filter,
                &cx.t("browser.home.servers-filter-hint"),
            );
            ui.add_space(6.0);

            let filtered = filter_bench_servers(
                &self.ports,
                &self.port_filter,
                if self.only_active_bench {
                    self.active_root.as_ref()
                } else {
                    None
                },
                self.active_root.as_ref(),
            );

            if filtered.is_empty() {
                let empty_key = if self.ports.iter().any(|p| p.is_forja_port() && p.is_http()) {
                    "browser.home.servers-empty-bench"
                } else {
                    "browser.home.servers-empty"
                };
                ui.add(
                    egui::Label::new(egui::RichText::new(cx.t(empty_key)).small().color(muted))
                        .wrap(),
                );
                return;
            }

            let default_bench = cx.t("browser.home.servers-forja");
            for port in filtered {
                let url = port.local_url();
                let bench = port.bench_label().unwrap_or(default_bench.as_str());
                let primary = if port.process.is_empty() {
                    url.clone()
                } else {
                    format!("{url} · {}", port.process)
                };
                if row_button(ui, &primary, bench, &format!("{url} ({bench})")).clicked() {
                    navigate_to(cx, shared, &url);
                }
                ui.add_space(forja_ui_kit::Spacing::XS);
            }
        });
    }

    fn docs_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        shared: &Shared,
        muted: egui::Color32,
    ) {
        let title = cx.t("browser.home.docs-title");
        forja_ui_kit::card(ui, Some(&title), |ui| {
            let Some(root) = self.active_root.clone() else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(cx.t("browser.home.docs-no-project"))
                            .small()
                            .color(muted),
                    )
                    .wrap(),
                );
                return;
            };

            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "{} — {}",
                        root.name,
                        cx.t("browser.home.docs-hint")
                    ))
                    .small()
                    .color(muted),
                )
                .wrap(),
            );
            ui.add_space(6.0);

            let (scanning, links) = self
                .docs
                .lock()
                .map(|docs| (docs.scanning, docs.links.clone()))
                .unwrap_or((false, Vec::new()));

            if scanning {
                ui.horizontal(|ui| {
                    forja_ui_kit::spinner_if_active(ui);
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t("browser.home.docs-loading"))
                            .small()
                            .color(muted),
                    );
                });
                return;
            }

            if links.is_empty() {
                let msg = cx.t_args("browser.home.docs-empty", &[("project", &root.name)]);
                ui.add(egui::Label::new(egui::RichText::new(msg).small().color(muted)).wrap());
                return;
            }

            for doc in links {
                let hover = doc.abs_path.display().to_string();
                if row_button(ui, &doc.label, "", &hover).clicked()
                    && !open_doc_in_viewer(cx, &doc.abs_path)
                {
                    shared.set_notice(Some(cx.t("browser.home.docs-open-failed")));
                }
                ui.add_space(forja_ui_kit::Spacing::XS);
            }
        });
    }

    fn recent_card(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        shared: &Shared,
        muted: egui::Color32,
    ) {
        let title = cx.t("browser.home.recent-title");
        let shortcuts = recent_shortcuts(&shared.history(), MAX_RECENT_SHORTCUTS);
        forja_ui_kit::card(ui, Some(&title), |ui| {
            if shortcuts.is_empty() {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(cx.t("browser.home.recent-empty"))
                            .small()
                            .color(muted),
                    )
                    .wrap(),
                );
                return;
            }
            for shortcut in shortcuts {
                let primary = if shortcut.title.is_empty() {
                    shortcut.url.clone()
                } else {
                    shortcut.title.clone()
                };
                let detail = if shortcut.title.is_empty() {
                    ""
                } else {
                    shortcut.url.as_str()
                };
                if row_button(ui, &primary, detail, &shortcut.url).clicked() {
                    navigate_to(cx, shared, &shortcut.url);
                }
                ui.add_space(forja_ui_kit::Spacing::XS);
            }
        });
    }
}

fn navigate_to(cx: &mut Ctx<'_>, shared: &Shared, url: &str) {
    let mut args = Args::new();
    args.insert("url", json!(url));
    if cx.run_command("browser.navigate", args).is_err() {
        shared.open(url, None, false);
    }
}

fn row_button(ui: &mut egui::Ui, primary: &str, detail: &str, hover: &str) -> egui::Response {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let metrics = tokens.metrics();
    let width = ui.available_width().max(120.0);
    let height = metrics.row.max(28.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, primary));

    let fill = if response.is_pointer_button_down_on() || response.hovered() || response.has_focus()
    {
        color32(tokens.color.surface_alt)
    } else {
        color32(tokens.color.surface)
    };
    let stroke = if response.has_focus() {
        egui::Stroke::new(2.0_f32, color32(tokens.color.accent))
    } else {
        egui::Stroke::new(1.0_f32, color32(tokens.color.border))
    };
    let radius = egui::CornerRadius::same(metrics.radius_control.clamp(0.0, 255.0) as u8);
    ui.painter()
        .rect(rect, radius, fill, stroke, egui::StrokeKind::Inside);

    let pad = metrics.pad_x.max(8.0);
    let text_color = color32(tokens.color.text);
    let muted_color = color32(tokens.color.text_muted);
    let body_font = egui::FontId::new(metrics.body, egui::FontFamily::Proportional);
    let caption_font = egui::FontId::new(metrics.caption, egui::FontFamily::Proportional);

    let detail_width = if detail.is_empty() {
        0.0
    } else {
        let galley = ui
            .painter()
            .layout_no_wrap(detail.to_owned(), caption_font, muted_color);
        let max_detail = (rect.width() * 0.42).max(40.0);
        let draw_width = galley.size().x.min(max_detail);
        let detail_pos = egui::pos2(
            rect.right() - pad - draw_width,
            rect.center().y - galley.size().y * 0.5,
        );
        ui.painter()
            .with_clip_rect(egui::Rect::from_min_max(
                egui::pos2(rect.right() - pad - max_detail, rect.top()),
                egui::pos2(rect.right() - pad, rect.bottom()),
            ))
            .galley(detail_pos, galley, muted_color);
        draw_width + 8.0
    };

    let primary_max = (rect.width() - pad * 2.0 - detail_width).max(24.0);
    let mut job = egui::text::LayoutJob::single_section(
        primary.to_owned(),
        egui::TextFormat {
            font_id: body_font,
            color: text_color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: primary_max,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let primary_galley = ui.painter().layout_job(job);
    let primary_pos = egui::pos2(
        rect.left() + pad,
        rect.center().y - primary_galley.size().y * 0.5,
    );
    ui.painter().galley(primary_pos, primary_galley, text_color);

    if hover.is_empty() {
        response
    } else {
        response.on_hover_text(hover)
    }
}

fn color32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
