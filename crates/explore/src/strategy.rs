//! Coverage-guided UI exploration strategy (zero AI tokens).

use crate::rng::Xorshift64;
use crate::snapshot::{screen_signature, Element, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// One entry from `forja-app --dump-catalog` (`keybinds` array).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CatalogKeybind {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub module: String,
}

/// Action sent to `forja-app --test-driver`.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    ClickTarget { target: String, index: usize, double: bool },
    ClickPoint { x: f64, y: f64, double: bool },
    Type { text: String },
    Key { keys: String },
    Scroll { x: f64, y: f64, dy: f64 },
}

impl Action {
    /// Converts the action into `(rpc_method, rpc_params)` for the pipe.
    pub fn to_rpc(&self) -> (&'static str, Value) {
        match self {
            Self::ClickTarget { target, index, double } => {
                let mut map = serde_json::Map::new();
                map.insert("target".into(), json!(target));
                if *index > 0 {
                    map.insert("index".into(), json!(index));
                }
                if *double {
                    map.insert("double".into(), json!(true));
                }
                ("ui.click", Value::Object(map))
            }
            Self::ClickPoint { x, y, double } => {
                let mut map = serde_json::Map::new();
                map.insert("x".into(), json!(x.round()));
                map.insert("y".into(), json!(y.round()));
                if *double {
                    map.insert("double".into(), json!(true));
                }
                ("ui.click", Value::Object(map))
            }
            Self::Type { text } => ("ui.type", json!({ "text": text })),
            Self::Key { keys } => ("ui.key", json!({ "keys": keys })),
            Self::Scroll { x, y, dy } => (
                "ui.scroll",
                json!({ "x": x.round(), "y": y.round(), "dy": dy.round() }),
            ),
        }
    }

    /// Formats the action as a single JSON step compatible with `playtest/drive.ps1`.
    pub fn to_drive_json(&self) -> String {
        let (method, params) = self.to_rpc();
        let mut obj = serde_json::Map::new();
        obj.insert("m".into(), json!(method));
        if let Value::Object(map) = params {
            for (k, v) in map {
                obj.insert(k, v);
            }
        }
        Value::Object(obj).to_string()
    }

    pub fn is_escape(&self) -> bool {
        matches!(self, Self::Key { keys } if keys.eq_ignore_ascii_case("escape"))
    }
}

/// Difficult inputs used to stress-test editable fields.
pub const TRICKY_INPUTS: &[&str] = &[
    "",
    "   ",
    "a-very-long-input-forja-explore-stress-test-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz",
    "🔥🚀🧪 Forja çãõ",
    "<>:\"/\\|?*",
    r"C:\forja-missing-folder\ghost\999",
    "-999999999999999999",
    "99999999999999999999",
    "日本語テスト Ελληνικά Кириллица العربية",
];

#[derive(Debug, Clone)]
struct Candidate {
    key: String,
    global_key: String,
    action: Action,
    priority: u8,
    is_editable: bool,
}

/// Coverage-guided exploration state machine.
pub struct Strategy {
    rng: Xorshift64,
    keybinds: Vec<CatalogKeybind>,
    visited_elements: HashMap<u64, HashSet<String>>,
    visited_global_names: HashSet<String>,
    typed_fields: HashMap<String, usize>,
    tried_keybinds: HashSet<String>,
    seen_screens: HashSet<u64>,
    last_sig: Option<u64>,
    pending_type: Option<String>,
    pending_clear_type: Option<String>,
    depth: usize,
    max_depth: usize,
    stale_steps: usize,
    returning_home: u8,
}

impl Strategy {
    pub fn new(seed: u64, keybinds: Vec<CatalogKeybind>) -> Self {
        Self {
            rng: Xorshift64::new(seed),
            keybinds,
            visited_elements: HashMap::new(),
            visited_global_names: HashSet::new(),
            typed_fields: HashMap::new(),
            tried_keybinds: HashSet::new(),
            seen_screens: HashSet::new(),
            last_sig: None,
            pending_type: None,
            pending_clear_type: None,
            depth: 0,
            max_depth: 12,
            stale_steps: 0,
            returning_home: 0,
        }
    }

    pub fn screens_covered(&self) -> usize {
        self.seen_screens.len()
    }

    pub fn elements_covered(&self) -> usize {
        self.visited_elements.values().map(HashSet::len).sum()
    }

    pub fn keybinds_covered(&self) -> usize {
        self.tried_keybinds.len()
    }

    /// Picks the next action based on the current snapshot and coverage history.
    pub fn next_action(&mut self, snapshot: &Snapshot) -> Action {
        let sig = screen_signature(&snapshot.elements);
        let is_new_screen = self.seen_screens.insert(sig);
        if is_new_screen {
            self.stale_steps = 0;
        } else if self.last_sig == Some(sig) {
            self.stale_steps += 1;
        } else {
            self.stale_steps = self.stale_steps.saturating_sub(1);
        }
        if self.last_sig.is_some_and(|prev| prev != sig) {
            if snapshot.window_count() == 0 {
                self.depth = (self.depth + 1).min(self.max_depth + 2);
            } else {
                self.depth += 1;
            }
        }
        self.last_sig = Some(sig);

        // Finish queued clear-and-type sequence for an editable field.
        if let Some(text) = self.pending_type.take() {
            if text.is_empty() {
                return Action::Key { keys: "Backspace".to_owned() };
            }
            return Action::Type { text };
        }
        if let Some(text) = self.pending_clear_type.take() {
            self.pending_type = Some(text);
            return Action::Key { keys: "Ctrl+A".to_owned() };
        }

        // Return home if depth limit reached or stuck.
        if self.returning_home > 0 || self.depth >= self.max_depth || self.stale_steps >= 8 {
            if self.returning_home == 0 {
                self.returning_home = 3;
            }
            return self.step_home(snapshot);
        }

        // Type into a newly focused editable field we have not fuzzed yet.
        if let Some(focused) = snapshot.elements.iter().find(|el| el.editable && el.is_focused() && !el.is_disabled()) {
            let field_key = element_key(sig, focused, 0);
            let count = self.typed_fields.get(&field_key).copied().unwrap_or(0);
            if count == 0 {
                self.typed_fields.insert(field_key, 1);
                let input = self.pick_tricky_input();
                self.pending_type = Some(input);
                return Action::Key { keys: "Ctrl+A".to_owned() };
            }
        }

        // 1. Prioritize unvisited clickable/editable elements on the current screen.
        let candidates = build_candidates(snapshot);
        let visited = self.visited_elements.entry(sig).or_default();
        let unvisited: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| !visited.contains(&c.key))
            .collect();

        if !unvisited.is_empty() {
            // Prefer elements not yet clicked anywhere across the app before re-clicking
            // shared chrome/rail elements on every screen.
            let globally_new: Vec<&Candidate> = unvisited
                .iter()
                .copied()
                .filter(|c| !self.visited_global_names.contains(&c.global_key))
                .collect();
            if globally_new.is_empty() && self.has_untried_keybind() && self.rng.chance(1, 3) {
                if let Some(key) = self.pick_keybind() {
                    return Action::Key { keys: key };
                }
            }
            let active_set = if globally_new.is_empty() {
                unvisited
            } else {
                globally_new
            };
            let best_prio = active_set.iter().map(|c| c.priority).min().unwrap_or(2);
            let pool: Vec<&Candidate> = active_set
                .into_iter()
                .filter(|c| c.priority == best_prio)
                .collect();
            let chosen = *self.rng.choose(&pool).expect("non-empty pool");
            self.visited_elements
                .entry(sig)
                .or_default()
                .insert(chosen.key.clone());
            self.visited_global_names.insert(chosen.global_key.clone());
            self.stale_steps = 0;

            if chosen.is_editable {
                let field_key = format!("edit:{}", chosen.key);
                let idx = self.typed_fields.entry(field_key).or_insert(0);
                *idx += 1;
                let input = self.pick_tricky_input();
                self.pending_clear_type = Some(input);
            }
            return chosen.action.clone();
        }

        // 2. Try catalog keybinds (prioritizing untried ones), Escape, Tab, or Scroll.
        if snapshot.window_count() > 0 && self.rng.chance(1, 2) {
            return Action::Key { keys: "Escape".to_owned() };
        }

        if let Some(key) = self.pick_keybind() {
            return Action::Key { keys: key };
        }

        // 3. Fallback: scroll, Tab, or Escape.
        match self.rng.next_usize(4) {
            0 => {
                let (x, y) = scroll_point(snapshot);
                let dy = if self.rng.chance(3, 4) { -450.0 } else { 450.0 };
                Action::Scroll { x, y, dy }
            }
            1 => Action::Key { keys: "Tab".to_owned() },
            2 => Action::Key { keys: "Shift+Tab".to_owned() },
            _ => Action::Key { keys: "Escape".to_owned() },
        }
    }

    fn step_home(&mut self, snapshot: &Snapshot) -> Action {
        self.returning_home = self.returning_home.saturating_sub(1);
        if snapshot.window_count() > 0 {
            if self.returning_home == 0 {
                // Escape did not close every modal; reset counters so we can try clicking close buttons.
                self.depth = 0;
                self.stale_steps = 0;
            }
            return Action::Key { keys: "Escape".to_owned() };
        }
        self.depth = 0;
        self.stale_steps = 0;
        self.returning_home = 0;
        // Prefer clicking a safe home navigation element on the rail or sidebar.
        for home_name in ["Projetos", "Projects", "Bancada", "Bench", "Terminal"] {
            if snapshot
                .elements
                .iter()
                .any(|el| el.clickable && !el.is_disabled() && el.name == home_name)
            {
                return Action::ClickTarget {
                    target: home_name.to_owned(),
                    index: 0,
                    double: false,
                };
            }
        }
        Action::Key { keys: "Escape".to_owned() }
    }

    fn pick_tricky_input(&mut self) -> String {
        let choice = *self.rng.choose(TRICKY_INPUTS).unwrap_or(&"test");
        choice.to_owned()
    }

    fn has_untried_keybind(&self) -> bool {
        self.keybinds
            .iter()
            .any(|kb| !self.tried_keybinds.contains(&kb.key) && is_safe_keybind(&kb.key))
    }

    fn pick_keybind(&mut self) -> Option<String> {
        if self.keybinds.is_empty() {
            return None;
        }
        let untried: Vec<&CatalogKeybind> = self
            .keybinds
            .iter()
            .filter(|kb| !self.tried_keybinds.contains(&kb.key) && is_safe_keybind(&kb.key))
            .collect();
        if let Some(chosen) = self.rng.choose(&untried) {
            self.tried_keybinds.insert(chosen.key.clone());
            return Some(chosen.key.clone());
        }
        let safe: Vec<&CatalogKeybind> = self
            .keybinds
            .iter()
            .filter(|kb| is_safe_keybind(&kb.key))
            .collect();
        self.rng.choose(&safe).map(|kb| kb.key.clone())
    }
}

fn is_safe_keybind(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    !lower.contains("f4") && !lower.is_empty()
}

fn scroll_point(snapshot: &Snapshot) -> (f64, f64) {
    if let Some(win) = snapshot.elements.iter().rfind(|el| el.role == "Window") {
        return win.centre();
    }
    (
        (snapshot.window.width / 2.0).max(100.0),
        (snapshot.window.height / 2.0).max(100.0),
    )
}

fn element_key(sig: u64, el: &Element, occurrence: usize) -> String {
    let name = el.name.trim();
    if name.is_empty() {
        format!("{sig:016x}:{}:@({:.0},{:.0})", el.role, el.rect[0], el.rect[1])
    } else {
        format!("{sig:016x}:{}:{name}#{occurrence}", el.role)
    }
}

fn build_candidates(snapshot: &Snapshot) -> Vec<Candidate> {
    let top_window = snapshot
        .elements
        .iter()
        .rfind(|el| el.role == "Window")
        .map(|el| el.rect);

    let mut name_counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();

    for el in &snapshot.elements {
        let name = el.name.trim();
        // Match `test_driver::find_by_name` ordering (clickable elements first, including disabled ones).
        let occ = if !name.is_empty() && el.clickable {
            let entry = name_counts.entry(name.to_owned()).or_insert(0);
            let current = *entry;
            *entry += 1;
            current
        } else {
            0
        };
        if el.is_disabled() || (!el.clickable && !el.editable) {
            continue;
        }
        if el.width() < 6.0 || el.height() < 6.0 {
            continue;
        }
        // Skip window close/minimize/maximize buttons in the top-right title bar so we stay inside the app.
        if is_window_chrome_button(el, snapshot.window.width) {
            continue;
        }
        let inside_top_window = top_window.is_some_and(|w| {
            let (cx, cy) = el.centre();
            cx >= w[0] && cx <= w[2] && cy >= w[1] && cy <= w[3]
        });

        let priority = if top_window.is_some() && !inside_top_window {
            3
        } else if el.editable || matches!(
            el.role.as_str(),
            "Button" | "CheckBox" | "ComboBox" | "SpinButton" | "Tab" | "MenuItem" | "Link"
        ) {
            0
        } else if el.role == "Unknown" {
            2
        } else {
            1
        };

        let action = if !name.is_empty() && can_target_by_name(snapshot, el, name) {
            Action::ClickTarget {
                target: name.to_owned(),
                index: occ,
                double: false,
            }
        } else {
            let (x, y) = el.centre();
            Action::ClickPoint { x, y, double: false }
        };

        let key = element_key(0, el, occ);
        let global_key = if name.is_empty() {
            key.clone()
        } else {
            format!("{}:{name}", el.role)
        };
        out.push(Candidate {
            key,
            global_key,
            action,
            priority,
            is_editable: el.editable,
        });
    }
    out
}

fn can_target_by_name(snapshot: &Snapshot, target_el: &Element, name: &str) -> bool {
    // Check if `find_by_name` tier-0 exact match would resolve cleanly, or fall back to point click.
    let exact_clickable_count = snapshot
        .elements
        .iter()
        .filter(|el| el.name.trim() == name && el.clickable)
        .count();
    exact_clickable_count >= 1 && target_el.clickable
}

fn is_window_chrome_button(el: &Element, win_width: f64) -> bool {
    if el.rect[1] > 60.0 || win_width <= 0.0 || el.rect[0] < win_width - 260.0 {
        return false;
    }
    matches!(
        el.name.trim(),
        "×" | "–" | "□" | "Fechar" | "Minimizar" | "Maximizar" | "Close" | "Minimize" | "Maximize"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::WindowSize;

    fn button(id: u64, name: &str, x: f64) -> Element {
        Element {
            id,
            role: "Button".to_owned(),
            name: name.to_owned(),
            value: String::new(),
            rect: [x, 100.0, x + 80.0, 140.0],
            clickable: true,
            editable: false,
            state: vec![],
            window_id: None,
        }
    }

    #[test]
    fn coverage_guided_choice_visits_all_unvisited_elements_before_repeating() {
        let snap = Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Default::default(),
            view: String::new(),
            elements: vec![
                button(1, "Alpha", 10.0),
                button(2, "Beta", 100.0),
                button(3, "Gamma", 200.0),
            ],
            frames: 0,
            closed: false,
            externals: Vec::new(),
        };
        let mut strat = Strategy::new(123, vec![]);
        let mut clicked = HashSet::new();
        for _ in 0..3 {
            let action = strat.next_action(&snap);
            match action {
                Action::ClickTarget { target, .. } => {
                    assert!(clicked.insert(target), "repeated element while unvisited remained");
                }
                other => panic!("expected ClickTarget, got {other:?}"),
            }
        }
        assert_eq!(clicked.len(), 3);
    }

    #[test]
    fn uses_catalog_keybinds_after_screen_elements_are_covered() {
        let snap = Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Default::default(),
            view: String::new(),
            elements: vec![button(1, "Alpha", 10.0)],
            frames: 0,
            closed: false,
            externals: Vec::new(),
        };
        let keybinds = vec![CatalogKeybind {
            id: "anvil.open".into(),
            key: "Ctrl+K".into(),
            scope: "global".into(),
            module: "anvil".into(),
        }];
        let mut strat = Strategy::new(7, keybinds);
        // First action clicks Alpha.
        assert!(matches!(strat.next_action(&snap), Action::ClickTarget { .. }));
        // Next action tries Ctrl+K from catalog.
        assert_eq!(
            strat.next_action(&snap),
            Action::Key { keys: "Ctrl+K".to_owned() }
        );
    }

    #[test]
    fn editable_field_triggers_tricky_input_sequence() {
        let snap = Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Default::default(),
            view: String::new(),
            elements: vec![Element {
                id: 5,
                role: "TextInput".to_owned(),
                name: String::new(),
                value: String::new(),
                rect: [100.0, 100.0, 400.0, 140.0],
                clickable: true,
                editable: true,
                state: vec!["focused".to_owned()],
                window_id: None,
            }],
            frames: 0,
            closed: false,
            externals: Vec::new(),
        };
        let mut strat = Strategy::new(9, vec![]);
        let a1 = strat.next_action(&snap);
        assert_eq!(a1, Action::Key { keys: "Ctrl+A".to_owned() });
        let a2 = strat.next_action(&snap);
        assert!(matches!(a2, Action::Type { .. } | Action::Key { .. }));
    }

    #[test]
    fn prefers_globally_new_elements_over_previously_clicked_chrome() {
        let s1 = Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Default::default(),
            view: String::new(),
            elements: vec![button(1, "SharedRail", 10.0)],
            frames: 0,
            closed: false,
            externals: Vec::new(),
        };
        let s2 = Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Default::default(),
            view: String::new(),
            elements: vec![button(1, "SharedRail", 10.0), button(2, "ScreenSpecific", 100.0)],
            frames: 0,
            closed: false,
            externals: Vec::new(),
        };
        let mut strat = Strategy::new(1, vec![]);
        assert_eq!(
            strat.next_action(&s1),
            Action::ClickTarget {
                target: "SharedRail".into(),
                index: 0,
                double: false
            }
        );
        assert_eq!(
            strat.next_action(&s2),
            Action::ClickTarget {
                target: "ScreenSpecific".into(),
                index: 0,
                double: false
            }
        );
    }

    #[test]
    fn drive_json_matches_drive_ps1_format() {
        let step = Action::ClickTarget {
            target: "Ajustes".into(),
            index: 0,
            double: false,
        }
        .to_drive_json();
        let v: Value = serde_json::from_str(&step).unwrap();
        assert_eq!(v["m"], "ui.click");
        assert_eq!(v["target"], "Ajustes");
    }
}
