//! Daily/monthly spend limits and their 80%/100% notices.

use crate::model::{day_key, format_usd, month_key, UsageSummary};
use crate::parse::now_ms;
use crate::state::Shared;
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;

pub const WARN_PERCENT: u32 = 80;
pub const OVER_PERCENT: u32 = 100;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BudgetLimits {
    pub daily_usd: f64,
    pub monthly_usd: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetPeriod {
    Day,
    Month,
}

impl BudgetPeriod {
    pub fn id(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Month => "month",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Day => "usage.budget.day",
            Self::Month => "usage.budget.month",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BudgetAlert {
    pub period: BudgetPeriod,
    pub percent: u32,
    pub spent_usd: f64,
    pub limit_usd: f64,
}

impl BudgetAlert {
    pub fn key(self, day: &str, month: &str) -> String {
        let period = match self.period {
            BudgetPeriod::Day => day,
            BudgetPeriod::Month => month,
        };
        format!("{}:{period}:{}", self.period.id(), self.percent)
    }
}

/// Highest threshold reached in each period. `is_new` marks a threshold so the
/// caller does not notify twice; it is only called for the winning threshold.
pub fn alerts(
    summary: &UsageSummary,
    now_ms: i64,
    limits: BudgetLimits,
    mut is_new: impl FnMut(&str) -> bool,
) -> Vec<BudgetAlert> {
    let day = day_key(now_ms);
    let month = month_key(now_ms);
    let mut alerts = Vec::new();
    for (period, limit, totals) in [
        (BudgetPeriod::Day, limits.daily_usd, summary.today),
        (
            BudgetPeriod::Month,
            limits.monthly_usd,
            summary.month_to_date(now_ms),
        ),
    ] {
        if limit <= 0.0 || !limit.is_finite() {
            continue;
        }
        let percent = if totals.cost_usd >= limit {
            OVER_PERCENT
        } else if totals.cost_usd >= limit * (WARN_PERCENT as f64 / 100.0) {
            WARN_PERCENT
        } else {
            continue;
        };
        let alert = BudgetAlert {
            period,
            percent,
            spent_usd: totals.cost_usd,
            limit_usd: limit,
        };
        if is_new(&alert.key(&day, &month)) {
            alerts.push(alert);
        }
    }
    alerts
}

pub fn check(cx: &mut Ctx<'_>, shared: &Shared) {
    let summary = shared.summary();
    let now = now_ms();
    let day = day_key(now);
    let month = month_key(now);
    shared.sync_budget_period(&format!("{day}|{month}"));
    let limits = BudgetLimits {
        daily_usd: number(cx, "usage.budget.daily_usd"),
        monthly_usd: number(cx, "usage.budget.monthly_usd"),
    };
    let locale = cx.i18n().locale();
    let alerts = alerts(&summary, now, limits, |key| {
        shared.mark_budget_notified(key)
    });
    for alert in alerts {
        let body_key = if alert.percent >= OVER_PERCENT {
            "usage.notify.budget.over"
        } else {
            "usage.notify.budget.warn"
        };
        let body = cx.t_args(
            body_key,
            &[
                ("spent", &format_usd(alert.spent_usd, locale)),
                ("limit", &format_usd(alert.limit_usd, locale)),
                ("period", &cx.t(alert.period.label_key())),
            ],
        );
        let mut args = Args::new();
        args.insert("title", json!(cx.t("usage.notify.budget.title")));
        args.insert("body", json!(body));
        args.insert(
            "level",
            json!(if alert.percent >= OVER_PERCENT {
                "error"
            } else {
                "warning"
            }),
        );
        let _ = cx.run_command("notify.show", args);
    }
}

fn number(cx: &Ctx<'_>, key: &str) -> f64 {
    cx.setting(key)
        .and_then(SettingValue::as_f64)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::UsageTotals;
    use std::collections::BTreeMap;

    fn summary_with(cost: f64, now: i64) -> UsageSummary {
        let mut summary = UsageSummary::default();
        let totals = UsageTotals {
            cost_usd: cost,
            total_tokens: 100,
            ..UsageTotals::default()
        };
        summary.today = totals;
        let mut by_day = BTreeMap::new();
        by_day.insert(day_key(now), totals);
        summary.by_day = by_day;
        summary
    }

    #[test]
    fn daily_and_monthly_thresholds_fire_at_eighty_and_one_hundred() {
        let now = 1_762_000_000_000;
        let limits = BudgetLimits {
            daily_usd: 10.0,
            monthly_usd: 100.0,
        };
        let summary = summary_with(8.0, now);
        let fired = alerts(&summary, now, limits, |_| true);
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].period, BudgetPeriod::Day);
        assert_eq!(fired[0].percent, WARN_PERCENT);

        let summary = summary_with(10.0, now);
        let fired = alerts(&summary, now, limits, |_| true);
        assert_eq!(fired[0].percent, OVER_PERCENT);

        let summary = summary_with(7.99, now);
        assert!(alerts(&summary, now, limits, |_| true).is_empty());
    }

    #[test]
    fn each_threshold_fires_only_once() {
        let now = 1_762_000_000_000;
        let limits = BudgetLimits {
            daily_usd: 10.0,
            monthly_usd: 100.0,
        };
        let summary = summary_with(9.0, now);
        let mut seen = BTreeMap::new();
        let first = alerts(&summary, now, limits, |key| {
            seen.insert(key.to_owned(), true).is_none()
        });
        assert_eq!(first.len(), 1);
        let second = alerts(&summary, now, limits, |key| {
            seen.insert(key.to_owned(), true).is_none()
        });
        assert!(second.is_empty());
    }

    #[test]
    fn zero_limits_disable_the_notice() {
        let now = 1_762_000_000_000;
        let summary = summary_with(999.0, now);
        assert!(alerts(&summary, now, BudgetLimits::default(), |_| true).is_empty());
    }

    #[test]
    fn monthly_uses_the_calendar_month_not_the_last_thirty_days() {
        let now = 1_762_000_000_000;
        let mut summary = summary_with(0.0, now);
        let previous_month = day_key(now - 40 * crate::model::DAY_MS);
        summary.by_day.insert(
            previous_month,
            UsageTotals {
                cost_usd: 50.0,
                ..UsageTotals::default()
            },
        );
        assert_eq!(summary.month_to_date(now).cost_usd, 0.0);
    }
}
