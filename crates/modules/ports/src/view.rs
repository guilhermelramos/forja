//! Right panel "Portas" and the status bar count.

use crate::model::{BenchRoot, PortInfo};
use crate::scan;
use crate::state::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;

pub struct PortsView {
    shared: Shared,
}

impl PortsView {
    pub fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl View for PortsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_scan(cx, &self.shared);
        let snapshot = self.shared.snapshot();
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        let busy = self.shared.busy();
        ui.horizontal_wrapped(|ui| {
            ui.heading(cx.t("ports.panel.title"));
            ui.label(
                egui::RichText::new(cx.t_args(
                    "ports.view.count",
                    &[("count", &snapshot.ports.len().to_string())],
                ))
                .small()
                .color(muted),
            );
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
                ui.spinner();
            }
        });
        ui.separator();
        if let Some(error) = &snapshot.error {
            ui.label(
                egui::RichText::new(error)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
        }
        if snapshot.ports.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(cx.t("ports.view.empty")).color(muted));
            return;
        }
        let pending = self.shared.pending_kill();
        for port in &snapshot.ports {
            self.row(ui, cx, port, pending, muted);
            ui.add_space(4.0);
        }
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
        egui::Frame::new()
            .fill(c32(tokens.color.surface_alt))
            .corner_radius(6)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(format!(":{}", port.port))
                            .strong()
                            .color(c32(tokens.color.text)),
                    );
                    ui.label(
                        egui::RichText::new(format!("{} · {}", port.process, port.pid))
                            .small()
                            .color(muted),
                    );
                    ui.label(egui::RichText::new(&port.address).small().color(muted));
                    if !port.bench_name.is_empty() {
                        ui.label(
                            egui::RichText::new(&port.bench_name)
                                .small()
                                .color(c32(tokens.color.spark)),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(cx.t("ports.view.no-bench"))
                                .small()
                                .color(muted),
                        );
                    }
                });
                if !port.cwd.is_empty() {
                    ui.label(
                        egui::RichText::new(&port.cwd)
                            .small()
                            .color(muted)
                            .monospace(),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Secondary,
                        &cx.t("ports.view.open"),
                    )
                    .clicked()
                    {
                        let mut args = Args::new();
                        args.insert("url", json!(port.local_url()));
                        if let Err(error) = cx.run_command("browser.open", args) {
                            self.error(cx, error.to_string());
                        }
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("ports.view.system"),
                    )
                    .clicked()
                    {
                        let mut args = Args::new();
                        args.insert("url", json!(port.local_url()));
                        if let Err(error) = cx.run_command("browser.open_external", args) {
                            self.error(cx, error.to_string());
                        }
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
                            let mut args = Args::new();
                            args.insert("pid", json!(port.pid));
                            args.insert("port", json!(port.port));
                            args.insert("confirm", json!(true));
                            if let Err(error) = cx.run_command("ports.kill", args) {
                                self.error(cx, error.to_string());
                            }
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
                        self.shared.set_pending_kill(Some(port.pid));
                    }
                });
            });
    }

    fn error(&self, cx: &Ctx<'_>, message: String) {
        let _ = cx;
        self.shared.with(|state| {
            state.snapshot.error = Some(message);
        });
    }
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
        ui.label(
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
    let benches = bench_roots(cx);
    let shared = shared.clone();
    cx.spawn(
        move || scan::scan(&benches),
        move |_cx, snapshot| shared.finish_scan(snapshot),
    );
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

fn fixture() -> bool {
    std::env::var("FORJA_PORTS_FIXTURE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
