//! Token totals, cost, periods, sessions and the aggregated usage summary.

use forja_kernel::Locale;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
    /// Reported by the helper log when it exists, otherwise estimated from the
    /// editable pricing table. Zero when no price is known.
    #[serde(default)]
    pub cost_usd: f64,
    /// Tokens whose cost could not be reported or estimated. They stay out of
    /// `cost_usd` so the interface can say "no price" instead of guessing.
    #[serde(default)]
    pub unpriced_tokens: u64,
}

impl UsageTotals {
    pub fn add(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.total_tokens += other.total_tokens;
        self.cost_usd += other.cost_usd;
        self.unpriced_tokens += other.unpriced_tokens;
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn has_unpriced(&self) -> bool {
        self.unpriced_tokens > 0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub agent: String,
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub title: String,
    pub timestamp_ms: i64,
    pub totals: UsageTotals,
    /// Cost in USD as reported by the helper transcript, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageSession {
    pub agent: String,
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub title: String,
    pub started_ms: i64,
    pub updated_ms: i64,
    pub totals: UsageTotals,
}

/// Period selected by the page filter and used by `usage.spend`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsagePeriod {
    Today,
    #[default]
    Last7,
    Last30,
    All,
}

impl UsagePeriod {
    pub const ALL: [Self; 4] = [Self::Today, Self::Last7, Self::Last30, Self::All];

    pub fn id(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Last7 => "last7",
            Self::Last30 => "last30",
            Self::All => "all",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Today => "usage.today",
            Self::Last7 => "usage.week",
            Self::Last30 => "usage.month",
            Self::All => "usage.total",
        }
    }

    /// Accepts the ids used by the RPC plus the labels of the filter.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "today" | "hoje" => Self::Today,
            "last7" | "last_7_days" | "7" | "7d" => Self::Last7,
            "last30" | "last_30_days" | "30" | "30d" => Self::Last30,
            "all" | "total" | "tudo" => Self::All,
            _ => Self::Last7,
        }
    }

    pub fn includes(self, day: &str, today: &str, week_start: &str, month_start: &str) -> bool {
        match self {
            Self::Today => day == today,
            Self::Last7 => day >= week_start,
            Self::Last30 => day >= month_start,
            Self::All => true,
        }
    }
}

/// Aggregated usage for one period.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsageSlice {
    pub totals: UsageTotals,
    pub by_agent: BTreeMap<String, UsageTotals>,
    pub by_model: BTreeMap<String, UsageTotals>,
    pub by_project: BTreeMap<String, UsageTotals>,
    pub by_day: BTreeMap<String, UsageTotals>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UsageSummary {
    pub today: UsageTotals,
    pub last_7_days: UsageTotals,
    pub last_30_days: UsageTotals,
    pub total: UsageTotals,
    pub by_agent: BTreeMap<String, UsageTotals>,
    pub by_model: BTreeMap<String, UsageTotals>,
    pub by_project: BTreeMap<String, UsageTotals>,
    pub by_day: BTreeMap<String, UsageTotals>,
    /// Keyed by [`UsagePeriod::id`], so the page and `usage.spend` read the
    /// same aggregation with the selected filter.
    pub periods: BTreeMap<String, UsageSlice>,
    pub sessions: Vec<UsageSession>,
}

impl UsageSummary {
    pub fn recent_sessions(&self, limit: usize) -> &[UsageSession] {
        &self.sessions[..self.sessions.len().min(limit)]
    }

    pub fn slice(&self, period: UsagePeriod) -> UsageSlice {
        self.periods.get(period.id()).cloned().unwrap_or_default()
    }

    /// Cost of the current calendar month (UTC day keys, like the logs).
    pub fn month_to_date(&self, now_ms: i64) -> UsageTotals {
        let month = &month_key(now_ms);
        let mut totals = UsageTotals::default();
        for (day, value) in &self.by_day {
            if day.starts_with(month) {
                totals.add(value);
            }
        }
        totals
    }

    /// Days of the selected period, oldest first. `all` returns every day with
    /// data; the other periods return the full window, including empty days.
    pub fn day_series(&self, period: UsagePeriod, now_ms: i64) -> Vec<(String, UsageTotals)> {
        let slice = self.slice(period);
        match period {
            UsagePeriod::Today => vec![(day_key(now_ms), slice.totals)],
            UsagePeriod::Last7 => window_series(&slice, 7, now_ms),
            UsagePeriod::Last30 => window_series(&slice, 30, now_ms),
            UsagePeriod::All => slice
                .by_day
                .into_iter()
                .filter(|(_, totals)| !totals.is_empty())
                .collect(),
        }
    }
}

fn window_series(slice: &UsageSlice, days: usize, now_ms: i64) -> Vec<(String, UsageTotals)> {
    (0..days)
        .rev()
        .map(|offset| {
            let day = day_key(now_ms - offset as i64 * DAY_MS);
            let totals = slice.by_day.get(&day).copied().unwrap_or_default();
            (day, totals)
        })
        .collect()
}

pub fn day_key(timestamp_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .map(|time| time.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

pub fn month_key(timestamp_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .map(|time| time.format("%Y-%m").to_string())
        .unwrap_or_default()
}

/// Day in the selected locale: `01/11/2026` in pt-BR, `2026-11-01` in en.
pub fn format_day(day: &str, locale: Locale) -> String {
    let Ok(date) = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d") else {
        return day.to_owned();
    };
    match locale {
        Locale::PtBr => date.format("%d/%m/%Y").to_string(),
        Locale::En => date.format("%Y-%m-%d").to_string(),
    }
}

/// Dollar amount without the currency symbol; the interface adds the label.
pub fn format_usd(value: f64, locale: Locale) -> String {
    let decimals = if value > 0.0 && value < 0.01 { 4 } else { 2 };
    let text = format!("{value:.decimals$}");
    match locale {
        Locale::PtBr => text.replace('.', ","),
        Locale::En => text,
    }
}

pub fn compact_tokens(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_tokens_rounds_to_one_decimal() {
        assert_eq!(compact_tokens(999), "999");
        assert_eq!(compact_tokens(1_500), "1.5k");
        assert_eq!(compact_tokens(2_450_000), "2.5M");
    }

    #[test]
    fn totals_add_every_field() {
        let mut total = UsageTotals {
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 5,
            cache_write_tokens: 1,
            total_tokens: 30,
            cost_usd: 0.5,
            unpriced_tokens: 0,
        };
        total.add(&UsageTotals {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_tokens: 3,
            cache_write_tokens: 4,
            total_tokens: 3,
            cost_usd: 0.25,
            unpriced_tokens: 7,
        });
        assert_eq!(total.input_tokens, 11);
        assert_eq!(total.output_tokens, 22);
        assert_eq!(total.cache_read_tokens, 8);
        assert_eq!(total.cache_write_tokens, 5);
        assert_eq!(total.total_tokens, 33);
        assert!((total.cost_usd - 0.75).abs() < f64::EPSILON);
        assert_eq!(total.unpriced_tokens, 7);
    }

    #[test]
    fn days_follow_the_locale() {
        assert_eq!(format_day("2026-11-01", Locale::PtBr), "01/11/2026");
        assert_eq!(format_day("2026-11-01", Locale::En), "2026-11-01");
        assert_eq!(format_day("not-a-day", Locale::PtBr), "not-a-day");
    }

    #[test]
    fn dollars_follow_the_locale_and_keep_small_values_visible() {
        assert_eq!(format_usd(12.345, Locale::PtBr), "12,35");
        assert_eq!(format_usd(12.345, Locale::En), "12.35");
        assert_eq!(format_usd(0.0042, Locale::PtBr), "0,0042");
        assert_eq!(format_usd(0.0, Locale::En), "0.00");
    }

    #[test]
    fn periods_parse_ids_and_labels() {
        assert_eq!(UsagePeriod::parse("today"), UsagePeriod::Today);
        assert_eq!(UsagePeriod::parse("Hoje"), UsagePeriod::Today);
        assert_eq!(UsagePeriod::parse("7d"), UsagePeriod::Last7);
        assert_eq!(UsagePeriod::parse("last30"), UsagePeriod::Last30);
        assert_eq!(UsagePeriod::parse("tudo"), UsagePeriod::All);
    }
}
