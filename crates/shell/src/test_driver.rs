//! `forja-app --test-driver`: lets a play-test agent use the app like a person.
//!
//! Only with that flag, RPC methods `ui.*` are answered by the window itself:
//!
//! | method | params | effect |
//! |---|---|---|
//! | `ui.snapshot` | `{image?}` | current widgets (and a PNG when `image`) |
//! | `ui.click` | `{target? \| id? \| x?,y?, index?, button?, double?}` | real pointer events at the widget centre |
//! | `ui.type` | `{text}` | text input into the focused field |
//! | `ui.key` | `{keys}` e.g. `"Ctrl+K"`, `"Enter"`, `"Escape"` | key press and release |
//! | `ui.scroll` | `{target? \| id? \| x?,y?, dy}` | mouse wheel over a point |
//! | `ui.wait` | `{ms}` | lets background work finish |
//!
//! Input goes through [`eframe::App::raw_input_hook`] as egui events, one step
//! per frame, so widgets see exactly what a mouse and keyboard would send.
//! Every action replies after the UI settled, with the new widget list and
//! what appeared or disappeared, and is appended to `FORJA_TEST_LOG` (JSONL).

use crate::ui_tree::LatestTree;
use base64::Engine as _;
use eframe::egui::{self, accesskit, Event, Key, Modifiers, PointerButton, Pos2, Vec2};
use forja_kernel::forja_protocol::codec::{self, RpcError};
use serde_json::{json, Map, Value};
use std::collections::{BTreeSet, VecDeque};
use std::io::Write as _;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Frames and minimum time an action waits before it reports the new state.
const SETTLE_FRAMES: u32 = 4;
const SETTLE_TIME: Duration = Duration::from_millis(250);

enum Step {
    /// Events fed to egui in one frame.
    Input(Vec<Event>),
    /// Waits until both the frame count and the instant are reached.
    Settle { frames: u32, until: Instant },
    /// Asks egui for a screenshot and waits for it.
    Screenshot,
    /// Builds the reply from the current tree.
    Reply(Box<Pending>),
}

struct Pending {
    id: Value,
    method: String,
    params: Map<String, Value>,
    note: Value,
    reply: Sender<Option<String>>,
}

pub struct Driver {
    pub tree: LatestTree,
    steps: VecDeque<Step>,
    image: Option<String>,
    screenshot_requested: bool,
    previous: BTreeSet<String>,
    pointer: Pos2,
    log: Option<std::fs::File>,
    started: Instant,
}

impl Driver {
    pub fn new() -> Self {
        let log = std::env::var_os("FORJA_TEST_LOG").and_then(|path| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .ok()
        });
        Self {
            tree: LatestTree::default(),
            steps: VecDeque::new(),
            image: None,
            screenshot_requested: false,
            previous: BTreeSet::new(),
            pointer: Pos2::new(1.0, 1.0),
            log,
            started: Instant::now(),
        }
    }

    /// Takes an RPC line when it is a `ui.*` request; returns it back otherwise.
    pub fn accept(
        &mut self,
        line: &str,
        reply: Sender<Option<String>>,
        ctx: &egui::Context,
    ) -> Option<Sender<Option<String>>> {
        let Ok(request) = codec::parse(line) else {
            return Some(reply);
        };
        if !request.method.starts_with("ui.") {
            return Some(reply);
        }
        let id = request.id.clone().unwrap_or(Value::Null);
        match self.plan(&request.method, &request.params, ctx) {
            Ok((steps, note)) => {
                self.steps.extend(steps);
                self.steps.push_back(Step::Reply(Box::new(Pending {
                    id,
                    method: request.method,
                    params: request.params,
                    note,
                    reply,
                })));
            }
            Err(error) => {
                self.write_log(&request.method, &request.params, &json!({ "error": error.message }));
                let _ = reply.send(Some(codec::error_response(id, &error)));
            }
        }
        ctx.request_repaint();
        None
    }

    fn plan(
        &mut self,
        method: &str,
        params: &Map<String, Value>,
        ctx: &egui::Context,
    ) -> Result<(Vec<Step>, Value), RpcError> {
        let settle = |ms: u64| Step::Settle {
            frames: SETTLE_FRAMES,
            until: Instant::now() + SETTLE_TIME.max(Duration::from_millis(ms)),
        };
        match method {
            "ui.snapshot" => {
                let mut steps = vec![settle(0)];
                if params.get("image").and_then(Value::as_bool) == Some(true) {
                    steps.push(Step::Screenshot);
                }
                Ok((steps, Value::Null))
            }
            "ui.wait" => {
                let ms = params.get("ms").and_then(Value::as_u64).unwrap_or(500).min(20_000);
                Ok((vec![settle(ms)], Value::Null))
            }
            "ui.click" => {
                let (pos, note) = self.locate(params, ctx)?;
                let button = match params.get("button").and_then(Value::as_str) {
                    Some("right") => PointerButton::Secondary,
                    Some("middle") => PointerButton::Middle,
                    _ => PointerButton::Primary,
                };
                let double = params.get("double").and_then(Value::as_bool) == Some(true);
                let press = |pressed| Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Modifiers::NONE,
                };
                let mut steps = vec![Step::Input(vec![Event::PointerMoved(pos)])];
                for _ in 0..if double { 2 } else { 1 } {
                    steps.push(Step::Input(vec![press(true)]));
                    steps.push(Step::Input(vec![press(false)]));
                }
                self.pointer = pos;
                steps.push(settle(0));
                Ok((steps, note))
            }
            "ui.scroll" => {
                let (pos, note) = self.locate(params, ctx)?;
                let dy = params.get("dy").and_then(Value::as_f64).unwrap_or(-300.0) as f32;
                self.pointer = pos;
                Ok((
                    vec![
                        Step::Input(vec![Event::PointerMoved(pos)]),
                        Step::Input(vec![Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, dy),
                            modifiers: Modifiers::NONE,
                        }]),
                        settle(0),
                    ],
                    note,
                ))
            }
            "ui.type" => {
                let text = params
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("text is required"))?;
                let mut steps = Vec::new();
                for (index, line) in text.split('\n').enumerate() {
                    if index > 0 {
                        steps.extend(key_steps(Key::Enter, Modifiers::NONE));
                    }
                    if !line.is_empty() {
                        steps.push(Step::Input(vec![Event::Text(line.to_owned())]));
                    }
                }
                steps.push(settle(0));
                Ok((steps, Value::Null))
            }
            "ui.key" => {
                let keys = params
                    .get("keys")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("keys is required, e.g. \"Ctrl+K\""))?;
                let (key, modifiers) = parse_keys(keys)?;
                let mut steps = key_steps(key, modifiers);
                steps.push(settle(0));
                Ok((steps, Value::Null))
            }
            other => Err(RpcError::new(
                codec::METHOD_NOT_FOUND,
                format!("unknown test-driver method `{other}`"),
            )),
        }
    }

    /// Point to act on: `x,y` in screen pixels, a widget `id`, or a `target` name.
    fn locate(&self, params: &Map<String, Value>, ctx: &egui::Context) -> Result<(Pos2, Value), RpcError> {
        let ppp = ctx.pixels_per_point();
        if let (Some(x), Some(y)) = (
            params.get("x").and_then(Value::as_f64),
            params.get("y").and_then(Value::as_f64),
        ) {
            return Ok((Pos2::new(x as f32 / ppp, y as f32 / ppp), json!({ "at": [x, y] })));
        }
        let elements = self.elements(ctx);
        let chosen = if let Some(id) = params.get("id").and_then(Value::as_u64) {
            elements
                .iter()
                .find(|element| element["id"] == id)
                .ok_or_else(|| RpcError::app(format!("no widget with id {id}; call ui.snapshot")))?
        } else if let Some(target) = params.get("target").and_then(Value::as_str) {
            let matches = find_by_name(&elements, target);
            let index = params.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let Some(found) = matches.get(index) else {
                let names: Vec<&str> = elements
                    .iter()
                    .filter_map(|element| element["name"].as_str())
                    .filter(|name| !name.is_empty())
                    .take(60)
                    .collect();
                return Err(RpcError::app(format!(
                    "no widget named \"{target}\" (index {index}); visible names: {}",
                    names.join(" | ")
                )));
            };
            *found
        } else {
            return Err(RpcError::invalid_params("give target (widget name), id, or x and y"));
        };
        let rect = chosen["rect"].as_array().cloned().unwrap_or_default();
        let value = |index: usize| rect.get(index).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        let screen = ctx.content_rect();
        let centre = Pos2::new(
            ((value(0) + value(2)) / 2.0 / ppp).clamp(screen.left() + 1.0, screen.right() - 1.0),
            ((value(1) + value(3)) / 2.0 / ppp).clamp(screen.top() + 1.0, screen.bottom() - 1.0),
        );
        Ok((centre, json!({ "clicked": chosen })))
    }

    /// Feeds the next input step to egui; called from `raw_input_hook`.
    pub fn feed(&mut self, raw: &mut egui::RawInput) {
        if let Some(Step::Input(_)) = self.steps.front() {
            if let Some(Step::Input(events)) = self.steps.pop_front() {
                raw.events.extend(events);
            }
        }
    }

    /// Advances waits, screenshots and replies; called at the end of each frame.
    pub fn tick(&mut self, ctx: &egui::Context) {
        loop {
            match self.steps.front_mut() {
                None => return,
                Some(Step::Input(_)) => break,
                Some(Step::Settle { frames, until }) => {
                    if *frames > 0 {
                        *frames -= 1;
                        break;
                    }
                    if Instant::now() < *until {
                        ctx.request_repaint_after(Duration::from_millis(30));
                        return;
                    }
                    self.steps.pop_front();
                }
                Some(Step::Screenshot) => {
                    if !self.screenshot_requested {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                        self.screenshot_requested = true;
                        break;
                    }
                    let image = ctx.input(|input| {
                        input.events.iter().find_map(|event| match event {
                            Event::Screenshot { image, .. } => Some(image.clone()),
                            _ => None,
                        })
                    });
                    let Some(image) = image else { break };
                    self.image = encode_png(&image);
                    self.screenshot_requested = false;
                    self.steps.pop_front();
                }
                Some(Step::Reply(_)) => {
                    let Some(Step::Reply(pending)) = self.steps.pop_front() else { return };
                    self.reply(*pending, ctx);
                }
            }
        }
        ctx.request_repaint();
    }

    fn reply(&mut self, pending: Pending, ctx: &egui::Context) {
        let ppp = ctx.pixels_per_point();
        let elements = self.elements(ctx);
        let names: BTreeSet<String> = elements
            .iter()
            .filter_map(|element| element["name"].as_str())
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        let appeared: Vec<&String> = names.difference(&self.previous).take(40).collect();
        let disappeared: Vec<&String> = self.previous.difference(&names).take(40).collect();
        let screen = ctx.content_rect();
        let mut result = json!({
            "window": { "width": (screen.width() * ppp).round(), "height": (screen.height() * ppp).round() },
            "changes": { "appeared": appeared, "disappeared": disappeared },
            "view": view_text(&elements),
            "elements": elements,
        });
        if !pending.note.is_null() {
            result["action"] = pending.note.clone();
        }
        if let Some(png) = self.image.take() {
            result["png_base64"] = Value::String(png);
        }
        self.write_log(&pending.method, &pending.params, &json!({
            "action": pending.note,
            "changes": result["changes"],
            "elements": result["elements"].as_array().map_or(0, Vec::len),
        }));
        self.previous = names;
        let _ = pending.reply.send(Some(codec::ok_response(&pending.id, result)));
    }

    /// Visible widgets worth acting on, in screen pixels, top to bottom.
    /// Widgets with no area or entirely outside the window are left out.
    fn elements(&self, ctx: &egui::Context) -> Vec<Value> {
        let ppp = ctx.pixels_per_point();
        let window = ctx.content_rect();
        let Some(update) = self.tree.lock().ok().and_then(|tree| tree.clone()) else {
            return Vec::new();
        };
        // Widgets scrolled or clipped out of their window are invisible to a person.
        let mut parents = std::collections::HashMap::new();
        let mut nodes = std::collections::HashMap::new();
        for (id, node) in &update.nodes {
            nodes.insert(*id, node);
            for child in node.children() {
                parents.insert(*child, *id);
            }
        }
        let window_of = |mut id: accesskit::NodeId| {
            while let Some(parent) = parents.get(&id) {
                if let Some(node) = nodes.get(parent) {
                    if node.role() == accesskit::Role::Window {
                        return node.bounds();
                    }
                }
                id = *parent;
            }
            None
        };
        let mut out: Vec<Value> = update
            .nodes
            .iter()
            .filter(|(_, node)| node.role() != accesskit::Role::TextRun)
            .filter_map(|(id, node)| {
                let bounds = node.bounds()?;
                let outside = bounds.x1 <= f64::from(window.left())
                    || bounds.y1 <= f64::from(window.top())
                    || bounds.x0 >= f64::from(window.right())
                    || bounds.y0 >= f64::from(window.bottom());
                if bounds.x1 - bounds.x0 < 1.0 || bounds.y1 - bounds.y0 < 1.0 || outside {
                    return None;
                }
                if let Some(frame) = window_of(*id) {
                    let centre_x = (bounds.x0 + bounds.x1) / 2.0;
                    let centre_y = (bounds.y0 + bounds.y1) / 2.0;
                    if centre_x < frame.x0 || centre_x > frame.x1 || centre_y < frame.y0 || centre_y > frame.y1 {
                        return None;
                    }
                }
                let name = node.label().or_else(|| node.value()).unwrap_or("").trim().to_owned();
                let clickable = node.supports_action(accesskit::Action::Click);
                let editable = matches!(node.role(), accesskit::Role::TextInput | accesskit::Role::MultilineTextInput);
                if name.is_empty() && !clickable && !editable {
                    return None;
                }
                let scale = f64::from(ppp);
                let mut state = Vec::new();
                if node.is_disabled() {
                    state.push("disabled");
                }
                if node.toggled() == Some(accesskit::Toggled::True) || node.is_selected() == Some(true) {
                    state.push("selected");
                }
                if update.focus == *id {
                    state.push("focused");
                }
                Some(json!({
                    "id": id.0,
                    "role": format!("{:?}", node.role()),
                    "name": name,
                    "rect": [
                        (bounds.x0 * scale).round(), (bounds.y0 * scale).round(),
                        (bounds.x1 * scale).round(), (bounds.y1 * scale).round()
                    ],
                    "clickable": clickable,
                    "editable": editable,
                    "state": state,
                }))
            })
            .collect();
        let key = |value: &Value, index: usize| value["rect"][index].as_f64().unwrap_or(0.0);
        out.sort_by(|a, b| {
            (key(a, 1), key(a, 0))
                .partial_cmp(&(key(b, 1), key(b, 0)))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out
    }

    fn write_log(&mut self, method: &str, params: &Map<String, Value>, outcome: &Value) {
        let Some(file) = self.log.as_mut() else { return };
        let line = json!({
            "t_ms": self.started.elapsed().as_millis() as u64,
            "method": method,
            "params": params,
            "outcome": outcome,
        });
        let _ = writeln!(file, "{line}");
    }
}

fn key_steps(key: Key, modifiers: Modifiers) -> Vec<Step> {
    let event = |pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    vec![Step::Input(vec![event(true)]), Step::Input(vec![event(false)])]
}

/// `"Ctrl+Shift+K"` -> (`K`, ctrl+shift).
pub fn parse_keys(keys: &str) -> Result<(Key, Modifiers), RpcError> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in keys.split('+').map(str::trim).filter(|part| !part.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => {
                modifiers.ctrl = true;
                modifiers.command = true;
            }
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            "esc" => key = Some(Key::Escape),
            "del" => key = Some(Key::Delete),
            "return" => key = Some(Key::Enter),
            "up" => key = Some(Key::ArrowUp),
            "down" => key = Some(Key::ArrowDown),
            "left" => key = Some(Key::ArrowLeft),
            "right" => key = Some(Key::ArrowRight),
            "space" => key = Some(Key::Space),
            _ => {
                let name = if part.len() == 1 { part.to_ascii_uppercase() } else { part.to_owned() };
                key = Some(Key::from_name(&name).ok_or_else(|| {
                    RpcError::invalid_params(format!("unknown key `{part}` in `{keys}`"))
                })?);
            }
        }
    }
    let key = key.ok_or_else(|| RpcError::invalid_params(format!("no key in `{keys}`")))?;
    Ok((key, modifiers))
}

/// Widgets whose name matches: exact, then case-insensitive, then containing.
fn find_by_name<'a>(elements: &'a [Value], target: &str) -> Vec<&'a Value> {
    let lower = target.to_lowercase();
    let matches = |tier: u8, name: &str| match tier {
        0 => name == target,
        1 => name.to_lowercase() == lower,
        _ => name.to_lowercase().contains(&lower),
    };
    for tier in 0..3u8 {
        let mut found: Vec<&Value> = elements
            .iter()
            .filter(|element| matches(tier, element["name"].as_str().unwrap_or("")))
            .collect();
        if !found.is_empty() {
            // Prefer what reacts to a click (a button over its label).
            found.sort_by_key(|element| !element["clickable"].as_bool().unwrap_or(false));
            return found;
        }
    }
    Vec::new()
}

/// One line per widget, the form the agent reads.
fn view_text(elements: &[Value]) -> String {
    let mut text = String::new();
    for element in elements {
        let rect: Vec<String> = element["rect"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|value| value.as_f64().unwrap_or(0.0).to_string())
            .collect();
        let name = element["name"].as_str().unwrap_or("");
        let name = if name.is_empty() {
            "(sem nome)".to_owned()
        } else if name.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)) {
            "(ícone)".to_owned()
        } else {
            format!("\"{}\"", name.replace('\n', " / "))
        };
        let state: Vec<&str> = element["state"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let mut tags = state;
        if element["clickable"] == true {
            tags.push("clicável");
        }
        if element["editable"] == true {
            tags.push("editável");
        }
        text.push_str(&format!(
            "#{} {} {} ({},{})-({},{}) [{}]\n",
            element["id"],
            element["role"].as_str().unwrap_or(""),
            name,
            rect.first().map_or("?", String::as_str),
            rect.get(1).map_or("?", String::as_str),
            rect.get(2).map_or("?", String::as_str),
            rect.get(3).map_or("?", String::as_str),
            tags.join(", ")
        ));
    }
    text
}

fn encode_png(image: &egui::ColorImage) -> Option<String> {
    let [width, height] = image.size;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_array());
    }
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, rgba)?;
    let mut bytes = std::io::Cursor::new(Vec::new());
    buffer.write_to(&mut bytes, image::ImageFormat::Png).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(bytes.into_inner()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_with_modifiers() {
        let (key, modifiers) = parse_keys("Ctrl+Shift+K").unwrap();
        assert_eq!(key, Key::K);
        assert!(modifiers.ctrl && modifiers.shift && !modifiers.alt);
        assert_eq!(parse_keys("Enter").unwrap().0, Key::Enter);
        assert_eq!(parse_keys("esc").unwrap().0, Key::Escape);
        assert!(parse_keys("Ctrl+").is_err());
        assert!(parse_keys("Ctrl+NotAKey").is_err());
    }

    #[test]
    fn names_match_exact_then_loose_and_prefer_clickable() {
        let elements = vec![
            json!({ "id": 1, "name": "Nova bancada", "clickable": false }),
            json!({ "id": 2, "name": "Nova bancada", "clickable": true }),
            json!({ "id": 3, "name": "Bancadas", "clickable": true }),
        ];
        let found = find_by_name(&elements, "Nova bancada");
        assert_eq!(found[0]["id"], 2);
        assert_eq!(find_by_name(&elements, "bancadas")[0]["id"], 3);
        assert_eq!(find_by_name(&elements, "nova")[0]["id"], 2);
        assert!(find_by_name(&elements, "inexistente").is_empty());
    }
}
