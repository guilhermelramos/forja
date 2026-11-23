//! `--ui-tree <json>`: exports the widget tree of the captured frame.
//!
//! egui builds an AccessKit tree each frame once `enable_accesskit` is on.
//! [`UiTreeCapture`] keeps the latest one, and [`write`] saves it as JSON
//! next to the screenshot: role, label, value, state, actions and the rect of
//! every node, in points and in pixels. Used by the atlas (docs/atlas).

use eframe::egui::{self, accesskit};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Shared slot that holds the most recent AccessKit tree.
pub type LatestTree = Arc<Mutex<Option<accesskit::TreeUpdate>>>;

/// egui plugin that copies the AccessKit tree out of every frame's output.
pub struct UiTreeCapture {
    pub latest: LatestTree,
}

impl egui::Plugin for UiTreeCapture {
    fn debug_name(&self) -> &'static str {
        "forja.ui_tree"
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        if let Some(update) = &output.platform_output.accesskit_update {
            if let Ok(mut slot) = self.latest.lock() {
                *slot = Some(update.clone());
            }
        }
    }
}

/// Turns the AccessKit update into the atlas JSON shape.
pub fn to_json(update: &accesskit::TreeUpdate, pixels_per_point: f32) -> Value {
    let mut parents: HashMap<accesskit::NodeId, accesskit::NodeId> = HashMap::new();
    for (id, node) in &update.nodes {
        for child in node.children() {
            parents.insert(*child, *id);
        }
    }
    let ppp = f64::from(pixels_per_point);
    let nodes: Vec<Value> = update
        .nodes
        .iter()
        .map(|(id, node)| {
            let bounds = node.bounds().map(|rect| {
                json!({
                    "points": [rect.x0, rect.y0, rect.x1, rect.y1],
                    "pixels": [
                        (rect.x0 * ppp).round(),
                        (rect.y0 * ppp).round(),
                        (rect.x1 * ppp).round(),
                        (rect.y1 * ppp).round()
                    ],
                })
            });
            let actions: Vec<&str> = [
                (accesskit::Action::Click, "click"),
                (accesskit::Action::Focus, "focus"),
                (accesskit::Action::SetValue, "set_value"),
                (accesskit::Action::Expand, "expand"),
                (accesskit::Action::Collapse, "collapse"),
                (accesskit::Action::ScrollDown, "scroll"),
            ]
            .into_iter()
            .filter(|(action, _)| node.supports_action(*action))
            .map(|(_, name)| name)
            .collect();
            json!({
                "id": id.0,
                "parent": parents.get(id).map(|parent| parent.0),
                "role": format!("{:?}", node.role()),
                "button_kind": node.author_id(),
                "primary": node.author_id() == Some("forja.button.primary"),
                "name": node.label().or_else(|| node.value()),
                "label": node.label(),
                "value": node.value(),
                "description": node.description(),
                "disabled": node.is_disabled(),
                "selected": node.is_selected(),
                "toggled": node.toggled().map(|toggled| format!("{toggled:?}")),
                "actions": actions,
                "bounds": bounds,
                "children": node.children().iter().map(|child| child.0).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "format": "forja.ui-tree/1",
        "pixels_per_point": pixels_per_point,
        "root": update.tree.as_ref().map(|tree| tree.root.0),
        "focus": update.focus.0,
        "nodes": nodes,
    })
}

/// Writes the latest captured tree to `path`.
pub fn write(path: &Path, latest: &LatestTree, pixels_per_point: f32) -> Result<(), String> {
    let update = latest
        .lock()
        .map_err(|_| "ui tree lock poisoned".to_owned())?
        .clone()
        .ok_or_else(|| "no AccessKit tree was produced".to_owned())?;
    let text = serde_json::to_string_pretty(&to_json(&update, pixels_per_point))
        .map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}
