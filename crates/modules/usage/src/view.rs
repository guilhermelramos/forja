//! Rail page "Uso" and the compact status bar meter.

use crate::budget;
use crate::model::{
    compact_tokens, day_key, format_day, format_usd, UsagePeriod, UsageRecord, UsageSummary,
    UsageTotals,
};
use crate::parse::now_ms;
use crate::pricing::Pricing;
use crate::scan::{summarize, Roots};
use crate::state::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, Locale, SettingValue};

pub struct UsagePage {
    shared: Shared,
    period: UsagePeriod,
}

impl UsagePage {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            period: UsagePeriod::Last7,
        }
    }
}

impl View for UsagePage {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ensure_refresh(cx, &self.shared);
        let summary = self.shared.summary();
        let locale = cx.i18n().locale();
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        let title = cx.t("usage.page.title");
        let refresh = cx.t("usage.page-state.refresh");
        let mut page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("usage.page", &title)
                .wide()
                .refresh(&refresh),
        );
        if page.response.refresh_clicked {
            let _ = cx.run_command("usage.refresh", Args::new());
        }
        let mut period_index = UsagePeriod::ALL
            .iter()
            .position(|period| *period == self.period)
            .unwrap_or(1);
        let labels: Vec<String> = UsagePeriod::ALL
            .iter()
            .map(|period| cx.t(period.label_key()))
            .collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let filter = cx.t("usage.filter");
        page.filters(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(filter).small().color(muted));
                forja_ui_kit::segmented(ui, &mut period_index, &label_refs);
            });
        });
        self.period = UsagePeriod::ALL[period_index];
        page.body(|ui| {
            if self.shared.with(|state| state.busy).unwrap_or(false) {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Loading,
                    &cx.t("usage.page-state.loading"),
                    None,
                );
            }
            if let Some(error) = self.shared.error() {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Error,
                    &cx.t("usage.page-state.error"),
                    Some(&error),
                );
            }
            let pricing_errors = self.shared.pricing_errors();
            if !pricing_errors.is_empty() {
                let text = cx.t_args(
                    "usage.pricing.errors",
                    &[("count", &pricing_errors.len().to_string())],
                );
                ui.label(
                    egui::RichText::new(text)
                        .small()
                        .color(c32(tokens.color.danger)),
                )
                .on_hover_text(pricing_errors.join("\n"));
            }
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .id_salt("usage-body")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if summary.total.is_empty() {
                        self.empty(ui, cx);
                    }
                    self.totals(ui, cx, &summary, locale, muted);
                    if summary.total.is_empty() {
                        return;
                    }
                    let slice = summary.slice(self.period);
                    self.days(ui, cx, &summary, locale);
                    self.group(
                        ui,
                        cx,
                        "usage.section.agents",
                        &slice.by_agent,
                        slice.totals.total_tokens,
                        locale,
                        |name| agent_label(cx, name),
                    );
                    self.group(
                        ui,
                        cx,
                        "usage.section.models",
                        &slice.by_model,
                        slice.totals.total_tokens,
                        locale,
                        |name| name.to_owned(),
                    );
                    self.group(
                        ui,
                        cx,
                        "usage.section.projects",
                        &slice.by_project,
                        slice.totals.total_tokens,
                        locale,
                        |name| {
                            if name.is_empty() {
                                cx.t("usage.unknown")
                            } else {
                                name.to_owned()
                            }
                        },
                    );
                    self.sessions(ui, cx, &summary, locale, muted);
                });
        });
    }
}

impl UsagePage {
    fn empty(&self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        forja_ui_kit::page_state(
            ui,
            forja_ui_kit::PageState::Empty,
            &cx.t("usage.page-state.empty"),
            Some(&cx.t("usage.empty.body")),
        );
    }

    fn totals(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        summary: &UsageSummary,
        locale: Locale,
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
                    let (cost, note) = cost_lines(totals, cx, locale);
                    ui.label(forja_ui_kit::TextRole::Display.rich(cost));
                    if let Some(note) = note {
                        ui.label(egui::RichText::new(note).small().color(muted));
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            compact_tokens(totals.total_tokens),
                            cx.t("usage.tokens")
                        ))
                        .font(forja_ui_kit::TextRole::Section.font()),
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
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} · {} {}",
                            compact_tokens(totals.cache_read_tokens),
                            cx.t("usage.cache_read"),
                            compact_tokens(totals.cache_write_tokens),
                            cx.t("usage.cache_write")
                        ))
                        .small()
                        .color(muted),
                    );
                });
                ui.add_space(4.0);
            }
        });
    }

    fn days(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, summary: &UsageSummary, locale: Locale) {
        let now = now_ms();
        let series = summary.day_series(self.period, now);
        if series.is_empty() {
            return;
        }
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
                    ui.label(forja_ui_kit::TextRole::Caption.rich(format_day(&day, locale)));
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
                    ui.label(
                        egui::RichText::new(cost_short(&totals, cx, locale))
                            .small()
                            .color(c32(tokens.color.text_muted)),
                    );
                });
            }
        });
        ui.add_space(4.0);
    }

    #[allow(clippy::too_many_arguments)]
    fn group(
        &self,
        ui: &mut egui::Ui,
        cx: &Ctx<'_>,
        title: &str,
        values: &std::collections::BTreeMap<String, UsageTotals>,
        total: u64,
        locale: Locale,
        name_for: impl Fn(&str) -> String,
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
                    ui.label(forja_ui_kit::TextRole::Caption.rich(name_for(name)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} · {} · {share}%",
                                cost_short(totals, cx, locale),
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
        locale: Locale,
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
                            agent_label(cx, &session.agent),
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
                    ui.label(
                        egui::RichText::new(cost_short(&session.totals, cx, locale))
                            .small()
                            .color(muted),
                    );
                    if session.updated_ms > 0 {
                        ui.label(
                            egui::RichText::new(format_day(&day_key(session.updated_ms), locale))
                                .small()
                                .color(muted),
                        );
                    }
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
        let locale = cx.i18n().locale();
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let text = if summary.today.has_unpriced() {
            cx.t_args(
                "usage.status.suffix_unpriced",
                &[("tokens", &compact_tokens(summary.today.total_tokens))],
            )
        } else {
            cx.t_args(
                "usage.status.suffix",
                &[
                    ("cost", &format_usd(summary.today.cost_usd, locale)),
                    ("tokens", &compact_tokens(summary.today.total_tokens)),
                ],
            )
        };
        ui.label(
            egui::RichText::new(text)
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
    let user_dir = cx.user_dir().to_path_buf();
    let project = current_project(cx);
    let shared = shared.clone();
    cx.spawn(
        move || {
            let mut cache = cache.unwrap_or_default();
            let roots = Roots::from_accounts(&accounts);
            let (pricing, pricing_errors) = Pricing::load(&user_dir, project.as_deref());
            let stats = cache.scan(&roots);
            let summary = cache.summary_at(now_ms(), &pricing);
            (cache, summary, stats, pricing_errors)
        },
        move |cx, (cache, summary, stats, pricing_errors)| {
            shared.finish_scan(cache, summary, stats, pricing_errors);
            budget::check(cx, &shared);
        },
    );
}

fn current_project(cx: &mut Ctx<'_>) -> Option<std::path::PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(std::path::PathBuf::from)
        })
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

fn agent_label(cx: &Ctx<'_>, agent: &str) -> String {
    match agent {
        "claude" => cx.t("usage.agent.claude"),
        "codex" => cx.t("usage.agent.codex"),
        "opencode" => cx.t("usage.agent.opencode"),
        other => other.to_owned(),
    }
}

fn cost_lines(totals: &UsageTotals, cx: &Ctx<'_>, locale: Locale) -> (String, Option<String>) {
    if totals.cost_usd <= 0.0 && totals.unpriced_tokens > 0 {
        return (cx.t("usage.cost.unpriced"), None);
    }
    let cost = cx.t_args(
        "usage.cost.value",
        &[("amount", &format_usd(totals.cost_usd, locale))],
    );
    let note = (totals.unpriced_tokens > 0).then(|| cx.t("usage.cost.partial_note"));
    (cost, note)
}

fn cost_short(totals: &UsageTotals, cx: &Ctx<'_>, locale: Locale) -> String {
    if totals.cost_usd <= 0.0 && totals.unpriced_tokens > 0 {
        return cx.t("usage.cost.unpriced");
    }
    let cost = cx.t_args(
        "usage.cost.value",
        &[("amount", &format_usd(totals.cost_usd, locale))],
    );
    if totals.unpriced_tokens > 0 {
        format!("{cost} {}", cx.t("usage.cost.partial"))
    } else {
        cost
    }
}

/// Deterministic data for screenshots.
pub fn sample_summary() -> UsageSummary {
    let now = now_ms();
    let mut records = Vec::new();
    let models = ["claude-sonnet-4-5", "gpt-5", "qwen3-coder"];
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
                cost_usd: 0.0,
                unpriced_tokens: 0,
            },
            cost_usd: None,
        });
    }
    summarize(records.iter(), now, &Pricing::builtin())
}

fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
