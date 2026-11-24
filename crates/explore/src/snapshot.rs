//! Typed view of `forja-app --test-driver` snapshots and screen signatures.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

/// Dimensions of the application window in screen pixels.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct WindowSize {
    pub width: f64,
    pub height: f64,
}

/// Diff of visible element names reported by the test driver.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Changes {
    #[serde(default)]
    pub appeared: Vec<String>,
    #[serde(default)]
    pub disappeared: Vec<String>,
}

/// One widget entry from `elements` in a `ui.*` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: u64,
    pub role: String,
    #[serde(default)]
    pub name: String,
    /// Current text of an editable field, when it differs from the label.
    #[serde(default)]
    pub value: String,
    pub rect: [f64; 4],
    #[serde(default)]
    pub clickable: bool,
    #[serde(default)]
    pub editable: bool,
    #[serde(default)]
    pub state: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<u64>,
}

impl Element {
    pub fn is_disabled(&self) -> bool {
        self.state.iter().any(|s| s == "disabled")
    }

    pub fn is_focused(&self) -> bool {
        self.state.iter().any(|s| s == "focused")
    }

    pub fn is_selected(&self) -> bool {
        self.state.iter().any(|s| s == "selected")
    }

    pub fn width(&self) -> f64 {
        self.rect[2] - self.rect[0]
    }

    pub fn height(&self) -> f64 {
        self.rect[3] - self.rect[1]
    }

    pub fn centre(&self) -> (f64, f64) {
        ((self.rect[0] + self.rect[2]) / 2.0, (self.rect[1] + self.rect[3]) / 2.0)
    }
}

/// One exit to the system that the app recorded instead of performing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRequest {
    /// `open` for a URL/path, `copy` for clipboard text, `reveal` for Explorer.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub target: String,
}

/// Parsed response from `ui.snapshot` or any `ui.*` action.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub window: WindowSize,
    #[serde(default)]
    pub changes: Changes,
    #[serde(default)]
    pub view: String,
    #[serde(default)]
    pub elements: Vec<Element>,
    #[serde(default)]
    pub frames: u64,
    #[serde(default)]
    pub closed: bool,
    /// Recorded browser/Explorer/clipboard requests (test runs only).
    #[serde(default)]
    pub externals: Vec<ExternalRequest>,
}

impl Snapshot {
    /// Parses a `ui.*` RPC result object into a [`Snapshot`].
    pub fn from_rpc_result(value: &Value) -> Result<Self, String> {
        serde_json::from_value(value.clone()).map_err(|err| format!("invalid snapshot JSON: {err}"))
    }

    /// Number of visible `Window` nodes in the accessibility tree.
    pub fn window_count(&self) -> usize {
        self.elements.iter().filter(|el| el.role == "Window").count()
    }

    /// Short human-readable label describing the active screen (topmost Window or first tab).
    pub fn screen_label(&self) -> String {
        let windows: Vec<&str> = self
            .elements
            .iter()
            .filter(|el| el.role == "Window" && !el.name.is_empty())
            .map(|el| el.name.as_str())
            .collect();
        if !windows.is_empty() {
            return windows.join(" > ");
        }
        let selected: Vec<&str> = self
            .elements
            .iter()
            .filter(|el| el.is_selected() && !el.name.is_empty())
            .map(|el| el.name.as_str())
            .take(3)
            .collect();
        if !selected.is_empty() {
            return selected.join(" | ");
        }
        "main".to_owned()
    }
}

/// Computes a deterministic 64-bit FNV-1a hash of the set of visible `(role, name)` pairs.
pub fn screen_signature(elements: &[Element]) -> u64 {
    let mut keys: BTreeSet<(&str, &str)> = BTreeSet::new();
    for el in elements {
        keys.insert((el.role.as_str(), el.name.trim()));
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (role, name) in keys {
        for byte in role.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
        for byte in name.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        hash ^= 0xfe;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signature_depends_on_visible_roles_and_names_not_coordinates() {
        let a = vec![
            Element {
                id: 1,
                role: "Button".into(),
                name: "Ajustes".into(),
                value: String::new(),
                rect: [10.0, 20.0, 50.0, 60.0],
                clickable: true,
                editable: false,
                state: vec![],
                window_id: None,
            },
            Element {
                id: 2,
                role: "Label".into(),
                name: "Pronto".into(),
                value: String::new(),
                rect: [0.0, 0.0, 40.0, 20.0],
                clickable: false,
                editable: false,
                state: vec![],
                window_id: None,
            },
        ];
        let b = vec![
            Element {
                id: 99,
                role: "Label".into(),
                name: "Pronto".into(),
                value: String::new(),
                rect: [100.0, 200.0, 140.0, 220.0],
                clickable: false,
                editable: false,
                state: vec![],
                window_id: None,
            },
            Element {
                id: 88,
                role: "Button".into(),
                name: "Ajustes".into(),
                value: String::new(),
                rect: [30.0, 40.0, 70.0, 80.0],
                clickable: true,
                editable: false,
                state: vec![],
                window_id: None,
            },
        ];
        assert_eq!(screen_signature(&a), screen_signature(&b));

        let c = vec![Element {
            id: 1,
            role: "Window".into(),
            name: "Ajustes".into(),
            value: String::new(),
            rect: [10.0, 10.0, 300.0, 300.0],
            clickable: false,
            editable: false,
            state: vec![],
            window_id: None,
        }];
        assert_ne!(screen_signature(&a), screen_signature(&c));
    }

    #[test]
    fn parses_rpc_snapshot() {
        let raw = json!({
            "window": { "width": 1920.0, "height": 1080.0 },
            "changes": { "appeared": ["Ajustes"], "disappeared": [] },
            "view": "",
            "elements": [
                {
                    "id": 10,
                    "role": "Window",
                    "name": "Ajustes",
                    "rect": [100.0, 100.0, 800.0, 600.0],
                    "clickable": false,
                    "editable": false,
                    "state": []
                }
            ]
        });
        let snap = Snapshot::from_rpc_result(&raw).unwrap();
        assert_eq!(snap.window_count(), 1);
        assert_eq!(snap.screen_label(), "Ajustes");
    }
}
