//! Open-file history of the finder, kept per bench root in the module store.
//!
//! The history is only a ranking hint. It is loaded and saved through kernel
//! tasks, and a missing or unreadable file simply means "no history yet".

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Usage of one file: how many times it was opened and when (Unix seconds).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub count: u32,
    pub last: u64,
}

/// `root → relative path → usage`. The root key is the bench path as text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub roots: BTreeMap<String, BTreeMap<String, Usage>>,
}

/// Per-file cap on remembered entries for one root, so the file stays small.
const MAX_ENTRIES_PER_ROOT: usize = 5_000;

impl History {
    /// Notes one open of `rel` under `root`.
    pub fn record(&mut self, root: &str, rel: &str, now: u64) {
        let entries = self.roots.entry(root.to_owned()).or_default();
        let usage = entries.entry(rel.to_owned()).or_default();
        usage.count = usage.count.saturating_add(1);
        usage.last = now;
        if entries.len() > MAX_ENTRIES_PER_ROOT {
            // Forget the least recently used entry.
            if let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, usage)| usage.last)
                .map(|(key, _)| key.clone())
            {
                entries.remove(&oldest);
            }
        }
    }

    /// The most recently opened files under `root`, newest first.
    pub fn recent(&self, root: &str, count: usize) -> Vec<String> {
        let Some(entries) = self.roots.get(root) else {
            return Vec::new();
        };
        let mut items: Vec<(&String, &Usage)> = entries.iter().collect();
        items.sort_by(|a, b| b.1.last.cmp(&a.1.last).then_with(|| a.0.cmp(b.0)));
        items
            .into_iter()
            .take(count)
            .map(|(rel, _)| rel.clone())
            .collect()
    }

    /// Ranking bonus: recent opens score more, and frequent ones a little more.
    pub fn bonus(&self, root: &str, rel: &str, now: u64) -> i32 {
        let Some(usage) = self.roots.get(root).and_then(|entries| entries.get(rel)) else {
            return 0;
        };
        let age = now.saturating_sub(usage.last);
        let recency = if age < 3_600 {
            40
        } else if age < 86_400 {
            25
        } else if age < 7 * 86_400 {
            10
        } else {
            0
        };
        recency + i32::try_from(usage.count.min(50)).unwrap_or(50) * 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_counts_and_orders_by_last_open() {
        let mut history = History::default();
        history.record("/r", "a.rs", 100);
        history.record("/r", "b.rs", 200);
        history.record("/r", "a.rs", 300);
        assert_eq!(history.recent("/r", 5), vec!["a.rs", "b.rs"]);
        assert_eq!(history.roots["/r"]["a.rs"].count, 2);
        assert_eq!(history.recent("/other", 5), Vec::<String>::new());
    }

    #[test]
    fn bonus_fades_with_age_and_grows_with_use() {
        let mut history = History::default();
        history.record("/r", "x.rs", 1_000);
        let fresh = history.bonus("/r", "x.rs", 1_000 + 60);
        let day_old = history.bonus("/r", "x.rs", 1_000 + 2 * 86_400);
        let old = history.bonus("/r", "x.rs", 1_000 + 30 * 86_400);
        assert!(fresh > day_old && day_old > old);
        assert_eq!(history.bonus("/r", "missing.rs", 0), 0);
    }

    #[test]
    fn round_trips_through_json() {
        let mut history = History::default();
        history.record("/r", "lib.rs", 42);
        let text = serde_json::to_string(&history).unwrap();
        let back: History = serde_json::from_str(&text).unwrap();
        assert_eq!(back, history);
    }

    #[test]
    fn empty_json_object_is_a_valid_history() {
        let history: History = serde_json::from_str("{}").unwrap();
        assert!(history.roots.is_empty());
    }
}
