//! Rail page "Uso" and the compact status bar meter.

use crate::model::{compact_tokens, UsageRecord, UsageSummary, UsageTotals};
use crate::parse::now_ms;
use crate::scan::{summarize, Roots};
use crate::state::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};

pub struct UsagePage {
    shared: Shared,
}

impl UsagePage {
    pub fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl View for UsagePage {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_refresh(cx, &self.shared);
        let summary = self.shared.summary();
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        ui.horizontal_wrapped(|ui| {
            ui.heading(cx.t("usage.page.title"));
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("usage.page.refresh"),
            )
            .clicked()
            {
                let _ = cx.run_command("usage.refresh", Args::new());
            }
            if self.shared.with(|state| state.busy).unwrap_or(false) {
                ui.spinner();
            }
        });
        if let Some(error) = self.shared.error() {
            ui.label(
                egui::RichText::new(error)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
        }
        ui.add_space(8.0);
        egui::ScrollArea::vertical()
            .id_salt("usage-body")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.totals(ui, cx, &summary, muted);
                self.days(ui, cx, &summary);
                self.group(
                    ui,
                    cx,
                    "usage.section.models",
                    &summary.by_model,
                    summary.total.total_tokens,
                );
                self.group(
                    ui,
                    cx,
                    "usage.section.projects",
                    &summary.by_project,
                    summary.total.total_tokens,
                );
                self.sessions(ui, cx, &summary, muted);
            });
    }
}

impl UsagePage {
    fn totals(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        summary: &UsageSummary,
        muted: egui::Color32,
    ) {
        let items: Vec<(String, UsageTotals)> = [
            ("usage.today", summary.today),
            ("usage.week", summary.last_7_days),
            ("usage.month", summary.last_30_days),
            ("usage.total", summary.total),
        ]
        .into_iter()
        .map(|(key, totals)| (cx.t(key), totals))
        .collect();
        ui.columns(2, |columns| {
            for (index, (label, totals)) in items.iter().enumerate() {
                let ui = &mut columns[index % 2];
                forja_ui_kit::card(ui, Some(label), |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(compact_tokens(totals.total_tokens))
                            .size(24.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} · {} {}",
                            compact_tokens(totals.input_tokens),
                            cx.t("usage.input"),
                            compact_tokens(totals.output_tokens),
                            cx.t("usage.output")
                        ))
                        .small()
                        .color(muted),
                    );
                });
                ui.add_space(4.0);
            }
        });
    }

    fn days(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, summary: &UsageSummary) {
        let series = summary.day_series(14);
        let max = series
            .iter()
            .map(|(_, totals)| totals.total_tokens)
            .max()
            .unwrap_or(0);
        let text = cx.t("usage.section.days");
        forja_ui_kit::card(ui, Some(&text), |ui| {
            ui.set_min_width(ui.available_width());
            for (day, totals) in series {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(day).small().monospace());
                    let ratio = if max == 0 {
                        0.0
                    } else {
                        totals.total_tokens as f32 / max as f32
                    };
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(160.0, 8.0), egui::Sense::hover());
                    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
                    let painter = ui.painter();
                    painter.rect_filled(rect, 3.0, c32(tokens.color.surface_alt));
                    let filled = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(rect.width() * ratio.max(0.02), rect.height()),
                    );
                    painter.rect_filled(filled, 3.0, c32(tokens.color.spark));
                    ui.label(
                        egui::RichText::new(compact_tokens(totals.total_tokens))
                            .small()
                            .color(c32(tokens.color.text_muted)),
                    );
                });
            }
        });
        ui.add_space(4.0);
    }

    fn group(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        title: &str,
        values: &std::collections::BTreeMap<String, UsageTotals>,
        total: u64,
    ) {
        if values.is_empty() {
            return;
        }
        let text = cx.t(title);
        forja_ui_kit::card(ui, Some(&text), |ui| {
            ui.set_min_width(ui.available_width());
            let mut items: Vec<_> = values.iter().collect();
            items.sort_by_key(|(_, totals)| std::cmp::Reverse(totals.total_tokens));
            for (name, totals) in items.into_iter().take(8) {
                let share = if total == 0 {
                    0
                } else {
                    (totals.total_tokens as f64 / total as f64 * 100.0).round() as u32
                };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(name).monospace().small());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {share}%",
                                compact_tokens(totals.total_tokens)
                            ))
                            .small(),
                        );
                    });
                });
            }
        });
        ui.add_space(4.0);
    }

    fn sessions(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        summary: &UsageSummary,
        muted: egui::Color32,
    ) {
        if summary.sessions.is_empty() {
            return;
        }
        let text = cx.t("usage.section.sessions");
        forja_ui_kit::card(ui, Some(&text), |ui| {
            ui.set_min_width(ui.available_width());
            for session in summary.recent_sessions(12) {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(if session.title.is_empty() {
                            &session.session_id
                        } else {
                            &session.title
                        })
                        .small()
                        .strong(),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {} · {}",
                            session.agent,
                            if session.model.is_empty() {
                                "?"
                            } else {
                                &session.model
                            },
                            compact_tokens(session.totals.total_tokens)
                        ))
                        .small()
                        .color(muted),
                    );
                });
                if !session.project.is_empty() {
                    ui.label(
                        egui::RichText::new(&session.project)
                            .small()
                            .color(muted)
                            .monospace(),
                    );
                }
            }
        });
    }
}

pub struct UsageStatus {
    shared: Shared,
}

impl UsageStatus {
    pub fn new(shared: Shared) -> Self {
        Self { shared }
    }
}

impl View for UsageStatus {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let enabled = cx
            .setting("usage.display.status_bar")
            .and_then(SettingValue::as_bool)
            .unwrap_or(true);
        if !enabled {
            return;
        }
        ensure_refresh(cx, &self.shared);
        let summary = self.shared.summary();
        if summary.total.is_empty() {
            return;
        }
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        ui.label(
            egui::RichText::new(cx.t_args(
                "usage.status.suffix",
                &[("tokens", &compact_tokens(summary.today.total_tokens))],
            ))
            .small()
            .color(c32(tokens.color.text_muted)),
        );
    }
}

pub fn ensure_refresh(cx: &mut Ctx<'_>, shared: &Shared) {
    if fixture() {
        shared.seed(sample_summary());
        return;
    }
    let tracking = cx
        .setting("usage.cost.track")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    if !tracking || !shared.due(60) {
        return;
    }
    let Some(cache) = shared.begin_scan() else {
        return;
    };
    let accounts = account_roots(cx);
    let shared = shared.clone();
    cx.spawn(
        move || {
            let mut cache = cache.unwrap_or_default();
            let roots = Roots::from_accounts(&accounts);
            let stats = cache.scan(&roots);
            let summary = cache.summary_at(now_ms());
            (cache, summary, stats)
        },
        move |_cx, (cache, summary, stats)| shared.finish_scan(cache, summary, stats),
    );
}

fn account_roots(cx: &mut Ctx<'_>) -> Vec<(String, String)> {
    let Ok(value) = cx.run_command("accounts.roots", Args::new()) else {
        return Vec::new();
    };
    value
        .get("accounts")
        .and_then(serde_json::Value::as_array)
        .map(|accounts| {
            accounts
                .iter()
                .filter_map(|account| {
                    Some((
                        account.get("profile_id")?.as_str()?.to_owned(),
                        account.get("dir")?.as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn fixture() -> bool {
    std::env::var("FORJA_USAGE_FIXTURE")
        .map(|value| value == "1")
        .unwrap_or(false)
}

/// Deterministic data for screenshots.
pub fn sample_summary() -> UsageSummary {
    let now = now_ms();
    let mut records = Vec::new();
    let models = ["claude-sonnet-4", "gpt-5", "qwen3-coder"];
    let projects = ["C:/work/forja", "C:/work/site", "C:/work/api"];
    for day in 0..14i64 {
        let tokens = 40_000 + day * 13_000 + (day % 3) * 9_000;
        records.push(UsageRecord {
            agent: if day % 2 == 0 { "claude" } else { "codex" }.into(),
            session_id: format!("s{day}"),
            project: projects[(day % 3) as usize].into(),
            model: models[(day % 3) as usize].into(),
            title: format!("Sessão {}", 14 - day),
            timestamp_ms: now - day * 86_400_000,
            totals: UsageTotals {
                input_tokens: tokens as u64,
                output_tokens: (tokens / 8) as u64,
                cache_read_tokens: (tokens / 5) as u64,
                cache_write_tokens: (tokens / 20) as u64,
                total_tokens: (tokens + tokens / 8) as u64,
            },
        });
    }
    summarize(records.iter(), now)
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
