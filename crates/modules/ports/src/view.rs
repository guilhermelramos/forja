//! Right panel "Portas" and the status bar count.

use crate::model::{group_ports, BenchRoot, PortGroupKey, PortInfo};
use crate::scan::{self, ScanContext};
use crate::state::Shared;
use forja_kernel::human_error::{humanize_ctx, humanize_kernel_error};
use forja_kernel::paths::clean_path_str;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;
use std::collections::BTreeSet;

pub struct PortsView {
    shared: Shared,
    filter: String,
    show_all: bool,
}

impl PortsView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            filter: String::new(),
            show_all: false,
        }
    }
}

impl View for PortsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_scan(cx, &self.shared);
        let snapshot = self.shared.snapshot();
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        let busy = self.shared.busy();
        let groups = group_ports(&snapshot.ports, self.show_all, &self.filter);
        let shown_count: usize = groups.iter().map(|group| group.ports.len()).sum();

        ui.horizontal_wrapped(|ui| {
            ui.heading(cx.t("ports.panel.title"));
            let count_text = if shown_count == snapshot.ports.len() {
                cx.t_args(
                    "ports.view.count",
                    &[("count", &shown_count.to_string())],
                )
            } else {
                cx.t_args(
                    "ports.view.count-filtered",
                    &[
                        ("shown", &shown_count.to_string()),
                        ("total", &snapshot.ports.len().to_string()),
                    ],
                )
            };
            forja_ui_kit::text(ui, egui::RichText::new(count_text).small().color(muted));
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("ports.view.refresh"),
            )
            .clicked()
            {
                let _ = cx.run_command("ports.refresh", Args::new());
            }
            if busy {
                forja_ui_kit::spinner_if_active(ui);
            }
        });

        ui.add_space(forja_ui_kit::Spacing::XS);
        forja_ui_kit::text_field(ui, &mut self.filter, &cx.t("ports.view.filter-hint"));
        if !self.filter.is_empty() {
            ui.add_space(forja_ui_kit::Spacing::XS);
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("ports.view.clear-filter"),
            )
            .clicked()
            {
                self.filter.clear();
            }
        }
        ui.add_space(forja_ui_kit::Spacing::XS);
        toggle_wrapped(ui, &mut self.show_all, &cx.t("ports.view.show-all"));
        ui.separator();

        let active_error = self
            .shared
            .human_error()
            .or_else(|| snapshot.error.as_deref().map(|err| humanize_ctx(cx, err)));
        if let Some(human) = active_error {
            let dismiss_label = cx.t("ports.view.dismiss-error");
            if forja_ui_kit::error_notice(
                ui,
                &human.summary,
                human.detail.as_deref(),
                &cx.t("kernel.error.details"),
                Some(&dismiss_label),
            ) {
                self.shared.clear_error();
            }
            ui.add_space(forja_ui_kit::Spacing::XS);
        }

        let pending = self.shared.pending_kill();
        egui::ScrollArea::vertical()
            .id_salt("ports.scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if snapshot.ports.is_empty() {
                    ui.add_space(forja_ui_kit::Spacing::SM);
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t("ports.view.empty")).color(muted));
                    return;
                }
                if groups.is_empty() {
                    ui.add_space(forja_ui_kit::Spacing::SM);
                    let empty_key = if self.filter.trim().is_empty() {
                        "ports.view.empty-benches"
                    } else {
                        "ports.view.empty-filter"
                    };
                    forja_ui_kit::text(ui, egui::RichText::new(cx.t(empty_key)).color(muted));
                    if !self.show_all && self.filter.trim().is_empty() {
                        ui.add_space(6.0);
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Secondary,
                            &cx.t("ports.view.show-all"),
                        )
                        .clicked()
                        {
                            self.show_all = true;
                        }
                    }
                    return;
                }

                for group in &groups {
                    let title = group_title(cx, &group.key);
                    let header_color = match group.key {
                        PortGroupKey::Bench(_) | PortGroupKey::Forja => c32(tokens.color.spark),
                        PortGroupKey::Other | PortGroupKey::System => muted,
                    };
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("{title} ({})", group.ports.len()))
                            .strong()
                            .color(header_color),
                    )
                    .id_salt(("ports.group", &title))
                    .default_open(true)
                    .show(ui, |ui| {
                        for port in &group.ports {
                            self.row(ui, cx, port, pending, muted);
                            ui.add_space(forja_ui_kit::Spacing::XS);
                        }
                    });
                }
            });
    }
}

impl PortsView {
    fn row(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        port: &PortInfo,
        pending: Option<u32>,
        muted: egui::Color32,
    ) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let text_color = if port.is_system {
            muted
        } else {
            c32(tokens.color.text)
        };
        let spark = c32(tokens.color.spark);

        ui.push_id((port.port, port.pid, &port.address), |ui| {
            egui::Frame::new()
                .fill(c32(tokens.color.surface_alt))
                .corner_radius(5)
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(format!(":{}", port.port))
                                .strong()
                                .color(c32(tokens.color.text)),
                        );
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(format!("{} · {}", port.process, port.pid))
                                .small()
                                .color(text_color),
                        );
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(&port.address)
                                .small()
                                .monospace()
                                .color(muted),
                        );
                        if let Some(bench) = port.bench_label() {
                            forja_ui_kit::text(ui, egui::RichText::new(bench).small().color(spark));
                        } else if port.from_tree {
                            forja_ui_kit::text(ui, 
                                egui::RichText::new(cx.t("ports.view.group-forja"))
                                    .small()
                                    .color(spark),
                            );
                        } else {
                            forja_ui_kit::text(ui, 
                                egui::RichText::new(cx.t("ports.view.no-bench"))
                                    .small()
                                    .color(muted),
                            );
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        if port.is_http()
                            && forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Secondary,
                                &cx.t("ports.view.open-browser"),
                            )
                            .clicked()
                        {
                            open_in_browser(cx, &self.shared, &port.local_url());
                        }

                        if pending == Some(port.pid) {
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Primary,
                                &cx.t("ports.view.confirm"),
                            )
                            .clicked()
                            {
                                self.shared.set_pending_kill(None);
                                self.run_kill(cx, port, true);
                            }
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Ghost,
                                &cx.t("ports.view.cancel"),
                            )
                            .clicked()
                            {
                                self.shared.set_pending_kill(None);
                            }
                        } else if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("ports.view.kill"),
                        )
                        .clicked()
                        {
                            let ask_confirm = cx
                                .setting("ports.confirm.kill")
                                .and_then(SettingValue::as_bool)
                                .unwrap_or(true);
                            let protected = port.is_system
                                || port.pid == 0
                                || port.pid == 4
                                || port.pid == std::process::id();
                            if protected || !ask_confirm {
                                self.shared.set_pending_kill(None);
                                self.run_kill(cx, port, true);
                            } else {
                                self.shared.set_pending_kill(Some(port.pid));
                            }
                        }
                    });
                });
        });
    }

    fn run_kill(&self, cx: &mut Ctx<'_>, port: &PortInfo, confirm: bool) {
        self.shared.clear_error();
        let mut args = Args::new();
        args.insert("pid", json!(port.pid));
        args.insert("port", json!(port.port));
        args.insert("confirm", json!(confirm));
        if let Err(error) = cx.run_command("ports.kill", args) {
            self.shared
                .set_human_error(humanize_kernel_error(cx.i18n(), &error));
        }
    }
}

fn open_in_browser(cx: &mut Ctx<'_>, shared: &Shared, url: &str) {
    shared.clear_error();
    let mut args = Args::new();
    args.insert("url", json!(url));
    match cx.run_command("browser.open", args) {
        Ok(value) => {
            if value.get("opened").and_then(serde_json::Value::as_bool) == Some(true) {
                let mut tab_args = Args::new();
                tab_args.insert("view_id", json!("browser.tab"));
                let _ = cx.run_command("shell.tab.open", tab_args);
            }
        }
        Err(error) => {
            shared.set_human_error(humanize_kernel_error(cx.i18n(), &error));
        }
    }
}

fn group_title(cx: &Ctx<'_>, key: &PortGroupKey) -> String {
    match key {
        PortGroupKey::Bench(name) => name.clone(),
        PortGroupKey::Forja => cx.t("ports.view.group-forja"),
        PortGroupKey::Other => cx.t("ports.view.group-other"),
        PortGroupKey::System => cx.t("ports.view.group-system"),
    }
}

/// Accessible toggle that wraps long labels cleanly in narrow sidebars while keeping
/// the standard `forja_ui_kit::toggle` look and `WidgetType::Checkbox` semantics.
fn toggle_wrapped(ui: &mut egui::Ui, on: &mut bool, label: &str) -> egui::Response {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let metrics = tokens.metrics();
    let (track_w, track_h) = metrics.toggle;
    let gap = metrics.gap;
    let font = egui::FontId::proportional(metrics.body);
    let text_color = c32(tokens.color.text);
    let avail_text = (ui.available_width() - track_w - gap).max(80.0);
    let galley = ui
        .painter()
        .layout(label.to_owned(), font, text_color, avail_text);
    if galley.size().y <= track_h + 4.0 && track_w + gap + galley.size().x <= ui.available_width()
    {
        return forja_ui_kit::toggle(ui, on, label);
    }

    let height = track_h.max(galley.size().y);
    let width = ui.available_width().max(track_w + gap + 60.0);
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let state = *on;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), state, label)
    });

    let track = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.center().y - track_h * 0.5),
        egui::vec2(track_w, track_h),
    );
    let travel = ui
        .ctx()
        .animate_bool_with_time(response.id, *on, tokens.motion_secs());
    let painter = ui.painter();
    let track_fill = if *on {
        c32(tokens.color.accent)
    } else {
        c32(tokens.color.surface_alt)
    };
    let track_stroke = if *on {
        egui::Stroke::NONE
    } else {
        egui::Stroke::new(1.0_f32, c32(tokens.color.border))
    };
    painter.rect(
        track,
        egui::CornerRadius::same((track_h * 0.5).round().clamp(0.0, 255.0) as u8),
        track_fill,
        track_stroke,
        egui::StrokeKind::Inside,
    );
    let x = egui::lerp(
        (track.left() + track_h * 0.5)..=(track.right() - track_h * 0.5),
        travel,
    );
    let knob = if *on {
        c32(tokens.color.on_accent)
    } else {
        c32(tokens.color.text)
    };
    painter.circle_filled(egui::pos2(x, track.center().y), metrics.knob, knob);
    let text_pos = egui::pos2(
        track.right() + gap,
        rect.center().y - galley.size().y * 0.5,
    );
    painter.galley(text_pos, galley, text_color);

    if response.has_focus() && ui.is_enabled() {
        painter.rect_stroke(
            rect.expand(2.0),
            egui::CornerRadius::same(metrics.radius_control.round().clamp(0.0, 255.0) as u8),
            egui::Stroke::new(2.0_f32, c32(tokens.color.accent)),
            egui::StrokeKind::Outside,
        );
    }
    response
}

/// Status bar count; also keeps the background scan alive.
pub struct PortsStatus {
    shared: Shared,
}

impl PortsStatus {
    pub fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl View for PortsStatus {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_scan(cx, &self.shared);
        let snapshot = self.shared.snapshot();
        if snapshot.ports.is_empty() && snapshot.error.is_none() {
            return;
        }
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let color = if snapshot.error.is_some() {
            c32(tokens.color.danger)
        } else {
            c32(tokens.color.text_muted)
        };
        forja_ui_kit::text(ui, 
            egui::RichText::new(cx.t_args(
                "ports.status.count",
                &[("count", &snapshot.ports.len().to_string())],
            ))
            .small()
            .color(color),
        );
    }
}

pub fn ensure_scan(cx: &mut Ctx<'_>, shared: &Shared) {
    if fixture() {
        shared.seed(scan::sample());
        return;
    }
    let interval = cx
        .setting("ports.scan.interval_s")
        .and_then(SettingValue::as_i64)
        .unwrap_or(30)
        .clamp(0, 600) as u64;
    if !shared.due(interval) || !shared.begin_scan() {
        return;
    }
    let ctx = collect_scan_context(cx);
    let shared = shared.clone();
    cx.spawn(
        move || scan::scan_with_context(&ctx),
        move |_cx, snapshot| shared.finish_scan(snapshot),
    );
}

fn collect_scan_context(cx: &mut Ctx<'_>) -> ScanContext {
    let mut benches = bench_roots(cx);
    for project in project_roots(cx) {
        let normalized = normalize_path(&project.path);
        if !benches
            .iter()
            .any(|bench| normalize_path(&bench.path) == normalized)
        {
            benches.push(project);
        }
    }

    let mut active_paths = BTreeSet::new();
    if let Ok(value) = cx.run_command("terminal.list", Args::new()) {
        if let Some(terminals) = value
            .get("terminals")
            .and_then(serde_json::Value::as_array)
        {
            for terminal in terminals {
                if let Some(path) = terminal
                    .get("bench_path")
                    .and_then(serde_json::Value::as_str)
                    .filter(|p| !p.trim().is_empty())
                {
                    active_paths.insert(normalize_path(path));
                }
            }
        }
    }
    if let Ok(value) = cx.run_command("helper.status", Args::new()) {
        if let Some(runs) = value.get("runs").and_then(serde_json::Value::as_array) {
            for run in runs {
                if let Some(path) = run
                    .get("bench_path")
                    .and_then(serde_json::Value::as_str)
                    .filter(|p| !p.trim().is_empty())
                {
                    active_paths.insert(normalize_path(path));
                }
            }
        }
    }

    let fallback_bench = if active_paths.len() == 1 {
        active_paths
            .iter()
            .next()
            .and_then(|path| crate::model::bench_for(path, &benches).cloned())
    } else if active_paths.is_empty() {
        current_bench(cx)
    } else {
        None
    };

    ScanContext {
        benches,
        fallback_bench,
        own_pid: std::process::id(),
    }
}

fn bench_roots(cx: &mut Ctx<'_>) -> Vec<BenchRoot> {
    let Ok(value) = cx.run_command("bench.list", Args::new()) else {
        return Vec::new();
    };
    value
        .get("benches")
        .and_then(serde_json::Value::as_array)
        .map(|benches| {
            benches
                .iter()
                .filter_map(|bench| {
                    Some(BenchRoot {
                        id: bench.get("bench_id")?.as_str()?.to_owned(),
                        name: bench.get("name")?.as_str()?.to_owned(),
                        path: bench.get("path")?.as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn project_roots(cx: &mut Ctx<'_>) -> Vec<BenchRoot> {
    let Ok(value) = cx.run_command("project.list", Args::new()) else {
        return Vec::new();
    };
    value
        .get("projects")
        .and_then(serde_json::Value::as_array)
        .map(|projects| {
            projects
                .iter()
                .filter_map(|project| {
                    Some(BenchRoot {
                        id: project.get("id")?.as_str()?.to_owned(),
                        name: project.get("name")?.as_str()?.to_owned(),
                        path: project.get("path")?.as_str()?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn current_bench(cx: &mut Ctx<'_>) -> Option<BenchRoot> {
    let value = cx.run_command("bench.current", Args::new()).ok()?;
    Some(BenchRoot {
        id: value.get("bench_id")?.as_str()?.to_owned(),
        name: value.get("name")?.as_str()?.to_owned(),
        path: value.get("path")?.as_str()?.to_owned(),
    })
}

fn normalize_path(path: &str) -> String {
    clean_path_str(path)
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

pub fn fixture() -> bool {
    std::env::var("FORJA_PORTS_FIXTURE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
