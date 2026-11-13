//! Persisted browser history, capped at 500 entries.

use forja_kernel::store::write_atomic_bytes;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;

pub const LIMIT: usize = 500;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub visited_at: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub entries: VecDeque<Entry>,
}

impl History {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        write_atomic_bytes(path, &bytes).map_err(|error| error.to_string())
    }

    pub fn add(&mut self, url: &str, title: &str) {
        let url = url.trim();
        if url.is_empty() || url.starts_with("about:") || url.starts_with("forja:") {
            return;
        }
        let entry = Entry {
            url: url.to_owned(),
            title: title.trim().to_owned(),
            visited_at: now_ms(),
        };
        self.entries.retain(|existing| existing.url != url);
        self.entries.push_front(entry);
        while self.entries.len() > LIMIT {
            self.entries.pop_back();
        }
    }

    pub fn update_title(&mut self, url: &str, title: &str) {
        if title.trim().is_empty() {
            return;
        }
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.url == url) {
            entry.title = title.trim().to_owned();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-browser-history-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("history.json")
    }

    #[test]
    fn repeated_urls_move_to_the_front() {
        let mut history = History::default();
        history.add("https://a.test", "A");
        history.add("https://b.test", "B");
        history.add("https://a.test", "A2");
        assert_eq!(history.len(), 2);
        assert_eq!(history.entries[0].url, "https://a.test");
        assert_eq!(history.entries[0].title, "A2");
    }

    #[test]
    fn cap_keeps_the_last_five_hundred() {
        let mut history = History::default();
        for index in 0..LIMIT + 20 {
            history.add(&format!("https://example.test/{index}"), "x");
        }
        assert_eq!(history.len(), LIMIT);
        assert_eq!(history.entries[0].url, "https://example.test/519");
        assert_eq!(
            history.entries.back().unwrap().url,
            "https://example.test/20"
        );
    }

    #[test]
    fn saves_and_loads_json() {
        let path = temp_path("roundtrip");
        let mut history = History::default();
        history.add("https://example.test", "Example");
        history.save(&path).unwrap();
        assert_eq!(History::load(&path), history);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
