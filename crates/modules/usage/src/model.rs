//! Token totals, sessions and the aggregated usage summary.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub total_tokens: u64,
}

impl UsageTotals {
    pub fn add(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.total_tokens += other.total_tokens;
    }

    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub agent: String,
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub title: String,
    pub timestamp_ms: i64,
    pub totals: UsageTotals,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
    pub sessions: Vec<UsageSession>,
}

impl UsageSummary {
    pub fn recent_sessions(&self, limit: usize) -> &[UsageSession] {
        &self.sessions[..self.sessions.len().min(limit)]
    }

    pub fn day_series(&self, days: usize) -> Vec<(String, UsageTotals)> {
        let now = chrono::Utc::now().timestamp_millis();
        (0..days)
            .rev()
            .map(|offset| {
                let day = day_key(now - offset as i64 * 24 * 60 * 60 * 1000);
                let totals = self.by_day.get(&day).copied().unwrap_or_default();
                (day, totals)
            })
            .collect()
    }
}

pub fn day_key(timestamp_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .map(|time| time.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
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
        };
        total.add(&UsageTotals {
            input_tokens: 1,
            output_tokens: 2,
            cache_read_tokens: 3,
            cache_write_tokens: 4,
            total_tokens: 3,
        });
        assert_eq!(total.input_tokens, 11);
        assert_eq!(total.output_tokens, 22);
        assert_eq!(total.cache_read_tokens, 8);
        assert_eq!(total.cache_write_tokens, 5);
        assert_eq!(total.total_tokens, 33);
    }
}
