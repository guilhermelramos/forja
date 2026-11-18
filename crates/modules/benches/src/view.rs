//! Bench detail tab and the create/store/approve dialogs.

use crate::model::BenchRecord;
use crate::service;
use crate::state::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map, Value};

pub struct DetailView {
    state: Shared,
}

impl DetailView {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for DetailView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        forja_ui_kit::page_heading(ui, &cx.t("bench.detail.title"), None, |_| {});
        let Some(bench) = self.state.current() else {
            let message = format!(
                "{}\n{}",
                cx.t("bench.detail.empty"),
                cx.t("bench.detail.empty-hint")
            );
            let create = forja_ui_kit::empty_state(
                ui,
                forja_ui_kit::BOARD,
                &message,
                Some(&cx.t("bench.action.create")),
            );
            if create.is_some_and(|response| response.clicked()) {
                let _ = run(cx, "bench.create_dialog", Map::new());
            }
            ui.vertical_centered(|ui| {
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("bench.action.refresh"),
                )
                .clicked()
                {
                    let _ = run(cx, "bench.refresh", Map::new());
                }
            });
            self.messages(ui, cx);
            return;
        };

        let stage = stage_info(cx, &self.state, &bench);
        let frame = egui::Frame::new()
            .fill(c32(tokens.color.surface))
            .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(12, 10));
        frame.show(ui, |ui| {
            let max_w = ui.available_width().max(1.0);
            ui.set_max_width(max_w);
            ui.horizontal(|ui| {
                ui.set_max_width(max_w);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if bench.sleeping {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(cx.t("bench.detail.sleeping"))
                                .small()
                                .color(c32(tokens.color.info)),
                        );
                    }
                    if bench.archived {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(cx.t("bench.detail.archived"))
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        );
                    }
                    if bench.unread {
                        forja_ui_kit::icon(ui, forja_ui_kit::Icon::Info, 16.0);
                    }
                    if bench.pinned {
                        forja_ui_kit::icon(ui, forja_ui_kit::Icon::Pin, 16.0);
                    }
                    stage_badge(ui, &stage.0, &stage.1, &tokens);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        forja_ui_kit::truncated_label(
                            ui,
                            &bench.name,
                            forja_ui_kit::TextRole::Section.font(),
                            c32(tokens.color.text),
                        );
                    });
                });
            });
            ui.add_space(6.0);
            detail_row(
                ui,
                &tokens,
                &cx.t("bench.detail.project"),
                &bench.project_id,
            );
            detail_row(
                ui,
                &tokens,
                &cx.t("bench.detail.branch"),
                bench.branch.as_deref().unwrap_or("\u{2014}"),
            );
            detail_row(ui, &tokens, &cx.t("bench.detail.path"), &bench.path);
            if let Some(base) = &bench.base_branch {
                detail_row(ui, &tokens, &cx.t("bench.detail.base"), base);
            }
            if let Some(command) = &bench.request_command {
                detail_row(ui, &tokens, &cx.t("bench.detail.request"), command);
            }
        });

        ui.add_space(forja_ui_kit::Spacing::SM);
        forja_ui_kit::text(ui, 
            egui::RichText::new(cx.t("bench.detail.actions"))
                .small()
                .color(c32(tokens.color.text_muted)),
        );
        ui.horizontal_wrapped(|ui| {
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Primary,
                &cx.t("bench.action.create"),
            )
            .clicked()
            {
                let _ = run(cx, "bench.create_dialog", Map::new());
            }
            let mut args = Map::new();
            args.insert("bench_id".to_owned(), json!(bench.bench_id));
            if !bench.archived
                && forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("bench.action.store"),
                )
                .clicked()
            {
                let _ = run(cx, "bench.store_dialog", args.clone());
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t(if bench.sleeping {
                    "bench.action.wake"
                } else {
                    "bench.action.sleep"
                }),
            )
            .clicked()
            {
                let _ = run(
                    cx,
                    if bench.sleeping {
                        "bench.wake"
                    } else {
                        "bench.sleep"
                    },
                    args.clone(),
                );
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("bench.action.pin"),
            )
            .clicked()
            {
                let _ = run(cx, "bench.pin", args.clone());
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("bench.action.read"),
            )
            .clicked()
            {
                let mut mark = args.clone();
                mark.insert("unread".to_owned(), json!(!bench.unread));
                let _ = run(cx, "bench.read", mark);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("bench.action.approve"),
            )
            .clicked()
            {
                let mut approve = Map::new();
                approve.insert("project_id".to_owned(), json!(bench.project_id));
                let _ = run(cx, "bench.approve_dialog", approve);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("bench.action.refresh"),
            )
            .clicked()
            {
                let _ = run(cx, "bench.refresh", Map::new());
            }
        });
        self.messages(ui, cx);
    }
}

impl DetailView {
    fn messages(&self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        if let Some(error) = self.state.error() {
            ui.add_space(6.0);
            let human = crate::create::humanize_error(cx, &error);
            forja_ui_kit::error_banner(
                ui,
                &human.summary,
                human.detail.as_deref(),
                &cx.t("kernel.error.details"),
            );
        }
        if let Some(notice) = self.state.notice() {
            ui.add_space(6.0);
            let human = crate::create::humanize_error(cx, &notice);
            if human.detail.is_some() {
                forja_ui_kit::error_banner(
                    ui,
                    &human.summary,
                    human.detail.as_deref(),
                    &cx.t("kernel.error.details"),
                );
            } else {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(
                        cx.t_args("bench.detail.notice", &[("message", &human.summary)]),
                    )
                    .small()
                    .color(c32(tokens.color.info)),
                );
            }
        }
    }
}

fn stage_info(cx: &mut Ctx<'_>, state: &Shared, bench: &BenchRecord) -> (String, String) {
    let value = service::stage_list(cx, state, Some(&bench.project_id)).unwrap_or(Value::Null);
    let stages = value
        .get("stages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(stage) = stages
        .iter()
        .find(|stage| stage.get("id").and_then(Value::as_str) == Some(bench.stage.as_str()))
    else {
        return (bench.stage.clone(), "text_muted".to_owned());
    };
    (
        stage
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(&bench.stage)
            .to_owned(),
        stage
            .get("dot")
            .and_then(Value::as_str)
            .unwrap_or("text_muted")
            .to_owned(),
    )
}

fn stage_badge(ui: &mut egui::Ui, label: &str, dot: &str, tokens: &forja_ui_kit::Tokens) {
    let font = forja_ui_kit::TextRole::Caption.font();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, c32(tokens.color.text));
    let width = galley.size().x + 26.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 6, c32(tokens.color.surface_alt));
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 6.0, rect.center().y - 4.0),
        egui::vec2(4.0, 8.0),
    );
    ui.painter().rect_filled(bar, 2, dot_color(tokens, dot));
    ui.painter().galley(
        egui::pos2(rect.left() + 14.0, rect.center().y - galley.size().y * 0.5),
        galley,
        c32(tokens.color.text),
    );
}

fn dot_color(tokens: &forja_ui_kit::Tokens, dot: &str) -> egui::Color32 {
    c32(match dot {
        "accent" => tokens.color.accent,
        "spark" => tokens.color.spark,
        "ok" => tokens.color.ok,
        "info" => tokens.color.info,
        "danger" => tokens.color.danger,
        _ => tokens.color.text_muted,
    })
}

fn detail_row(ui: &mut egui::Ui, tokens: &forja_ui_kit::Tokens, label: &str, value: &str) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [110.0, 18.0],
            egui::Label::new(
                egui::RichText::new(label)
                    .small()
                    .color(c32(tokens.color.text_muted)),
            ),
        );
        ui.vertical(|ui| {
            let max_w = ui.available_width().max(1.0);
            ui.set_max_width(max_w);
            forja_ui_kit::wrapped_label(
                ui,
                value,
                egui::FontId::proportional(tokens.metrics().caption),
                c32(tokens.color.text),
            );
        });
    });
}

pub(crate) fn field_label(ui: &mut egui::Ui, label: &str) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    forja_ui_kit::text(ui, 
        egui::RichText::new(label)
            .small()
            .color(c32(tokens.color.text_muted)),
    );
}

fn run(cx: &mut Ctx<'_>, id: &str, map: Map<String, Value>) -> forja_kernel::Result<Value> {
    cx.run_command(id, Args::from_map(map))
}

pub(crate) fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
