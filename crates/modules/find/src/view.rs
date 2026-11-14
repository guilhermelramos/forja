//! Finder views: the top-bar trigger with its floating picker (Ctrl+P), and the
//! ranked rows the shell mixes into the Anvil.
//!
//! Each frame reads a snapshot under the lock, draws, and collects what the user
//! asked for into [`Action`]s. The actions run after drawing, so no lock is held
//! while `Ctx` runs.

use crate::commands;
use crate::fuzzy;
use crate::index::Source;
use crate::state::{lock, Shared, Which};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{AnvilHit, Ctx};
use serde_json::{json, Map};
use std::time::Duration;

/// Rows shown in the Anvil source section.
const ANVIL_ROWS: usize = 12;

/// Top-bar trigger and floating picker. Always drawn, so the picker can open from anywhere.
pub struct FindView {
    pub shared: Shared,
}

/// Bigorna source. The shell passes the query and lists the rows we return.
pub struct AnvilView {
    pub shared: Shared,
    /// Last query we asked the index to rank. Stays unset until an index exists.
    pub(crate) ranked_for: Option<String>,
}

/// A user request collected during a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Query(Which, String),
    Move(Which, isize),
    Open(Which, usize),
    OpenSelected(Which),
    Close,
}

impl View for FindView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        commands::sync_root(cx, &self.shared);
        // No bench yet: check again later, since one may appear. Otherwise bench events wake the panel.
        if lock(&self.shared).root.is_none() {
            ui.ctx().request_repaint_after(Duration::from_millis(800));
        }
        // Mesmo componente e mesmo estilo do botão da Bigorna: rótulo e atalho no mesmo botão.
        ui.horizontal(|ui| {
            let label = format!("{}  {}", cx.t("find.trigger"), cx.t("find.shortcut"));
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &label).clicked() {
                let _ = commands::open_picker(cx, &self.shared);
            }
        });
        self.picker(ui.ctx(), cx);
    }
}

impl View for AnvilView {
    fn provides_anvil_hits(&self) -> bool {
        true
    }

    fn anvil_hits(&mut self, query: &str, cx: &mut Ctx<'_>) -> Vec<AnvilHit> {
        commands::sync_root(cx, &self.shared);
        commands::ensure_index(cx, &self.shared);
        let ready = lock(&self.shared).index.is_some();
        if ready && self.ranked_for.as_deref() != Some(query) {
            commands::request_rank(cx, &self.shared, Which::Anvil, query.to_owned());
            self.ranked_for = Some(query.to_owned());
        }
        let (root, items) = {
            let state = lock(&self.shared);
            (state.root.clone(), state.anvil.items.clone())
        };
        let Some(root) = root else {
            return Vec::new();
        };
        let line = fuzzy::split_location(query).1;
        items
            .into_iter()
            .take(ANVIL_ROWS)
            .enumerate()
            .map(|(index, rel)| {
                let path = root.join(&rel);
                let mut args = Map::new();
                args.insert("path".into(), json!(path.display().to_string()));
                if let Some(line) = line {
                    args.insert("line".into(), json!(line));
                }
                AnvilHit {
                    title: rel,
                    detail: path.display().to_string(),
                    icon_glyph: None,
                    score: 400 - index as i32,
                    command: "file.open".into(),
                    args,
                }
            })
            .collect()
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        commands::sync_root(cx, &self.shared);
        commands::ensure_index(cx, &self.shared);
        let (query, items, selected, pending, indexing, has_root) = {
            let state = lock(&self.shared);
            (
                state.anvil.query.clone(),
                state.anvil.items.clone(),
                state.anvil.selected,
                state.anvil.pending,
                state.indexing,
                state.root.is_some(),
            )
        };
        let mut actions: Vec<Action> = Vec::new();
        let mut text = query.clone();
        let edit = ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text(cx.t("find.anvil.hint"))
                .desired_width(f32::INFINITY),
        );
        if edit.changed() {
            actions.push(Action::Query(Which::Anvil, text.clone()));
        }
        if !has_root {
            ui.label(egui::RichText::new(cx.t("find.notice.no_bench")).weak());
        } else if indexing && items.is_empty() {
            ui.label(egui::RichText::new(cx.t("find.picker.indexing")).weak());
        } else if items.is_empty() && !query.trim().is_empty() && !pending {
            ui.label(egui::RichText::new(cx.t("find.picker.empty")).weak());
        }
        for (row, rel) in items.iter().take(ANVIL_ROWS).enumerate() {
            if ui.selectable_label(row == selected, rel).clicked() {
                actions.push(Action::Open(Which::Anvil, row));
            }
        }
        apply_actions(&self.shared, cx, actions);
    }
}

impl FindView {
    fn picker(&self, ctx: &egui::Context, cx: &mut Ctx<'_>) {
        let (open, query, items, selected, pending, indexing, focus, has_root, truncated, by_walk) = {
            let state = lock(&self.shared);
            (
                state.open,
                state.picker.query.clone(),
                state.picker.items.clone(),
                state.picker.selected,
                state.picker.pending,
                state.indexing,
                state.focus_query,
                state.root.is_some(),
                state.index.as_ref().is_some_and(|index| index.truncated),
                state
                    .index
                    .as_ref()
                    .is_some_and(|index| index.source == Source::Walk),
            )
        };
        if !open {
            return;
        }
        let layer = egui::Id::new("forja.find.picker.keyboard");
        if forja_ui_kit::consume_layer_escape(ctx, layer) {
            commands::close_picker(&self.shared);
            restore_picker_focus(ctx);
            return;
        }
        // Typing must always reach the query, even after Tab moved the focus to
        // the window close button or a result row.
        let search_id = egui::Id::new("forja.find.picker.search");
        focus_search_when_typing(ctx, search_id);
        // Rebuilds the listing if the bench changed or the listing is old (throttled inside).
        commands::ensure_index(cx, &self.shared);
        let mut window_open = true;
        let mut text = query;
        let mut focus_now = focus;
        let mut actions: Vec<Action> = Vec::new();
        let title = cx.t("find.picker.title");
        let hint = cx.t("find.picker.hint");
        egui::Window::new(title)
            .open(&mut window_open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .show(ctx, |ui| {
                forja_ui_kit::register_keyboard_layer(ui, layer);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .id(search_id)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY),
                );
                if focus_now {
                    let focused = ui.memory(|memory| memory.focused());
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new("forja.find.return_focus"), focused)
                    });
                    edit.request_focus();
                    if let Some(mut state) =
                        egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit.id)
                    {
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(text.chars().count()),
                            )));
                        state.store(ui.ctx(), edit.id);
                    }
                    focus_now = false;
                }
                if edit.changed() {
                    actions.push(Action::Query(Which::Picker, text.clone()));
                }
                let down = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                });
                let up = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                });
                if down {
                    actions.push(Action::Move(Which::Picker, 1));
                }
                if up {
                    actions.push(Action::Move(Which::Picker, -1));
                }
                if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                    actions.push(Action::OpenSelected(Which::Picker));
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (row, rel) in items.iter().enumerate() {
                            if ui.selectable_label(row == selected, rel).clicked() {
                                actions.push(Action::Open(Which::Picker, row));
                            }
                        }
                    });
                ui.separator();
                let status = if !has_root {
                    cx.t("find.picker.no_bench")
                } else if indexing && items.is_empty() {
                    cx.t("find.picker.indexing")
                } else if pending {
                    cx.t("find.picker.searching")
                } else if items.is_empty() {
                    cx.t("find.picker.empty")
                } else if truncated {
                    cx.t("find.picker.truncated")
                } else if by_walk {
                    cx.t("find.picker.by_walk")
                } else {
                    String::new()
                };
                ui.label(egui::RichText::new(status).weak());
            });
        if !window_open {
            restore_picker_focus(ctx);
            actions.push(Action::Close);
        }
        lock(&self.shared).focus_query = focus_now;
        apply_actions(&self.shared, cx, actions);
    }
}

/// True for events that change the search text. Tab, arrows, Enter and Escape
/// keep their navigation meaning.
fn wants_search_input(event: &egui::Event) -> bool {
    match event {
        egui::Event::Text(text) => !text.is_empty(),
        egui::Event::Paste(text) => !text.is_empty(),
        egui::Event::Key {
            key: egui::Key::Backspace | egui::Key::Delete,
            pressed: true,
            ..
        } => true,
        _ => false,
    }
}

/// Moves the focus to the picker query before the text edit is drawn, so the
/// same frame's text event is inserted instead of being dropped.
fn focus_search_when_typing(ctx: &egui::Context, search_id: egui::Id) {
    let typing = ctx.input(|input| input.events.iter().any(wants_search_input));
    if typing && !ctx.memory(|memory| memory.has_focus(search_id)) {
        ctx.memory_mut(|memory| memory.request_focus(search_id));
    }
}

fn restore_picker_focus(ctx: &egui::Context) {
    let focused = ctx.data_mut(|data| {
        data.remove_temp::<Option<egui::Id>>(egui::Id::new("forja.find.return_focus"))
            .flatten()
    });
    if let Some(id) = focused {
        ctx.memory_mut(|memory| memory.request_focus(id));
    }
}

/// Runs the collected actions, after the frame has been drawn.
fn apply_actions(shared: &Shared, cx: &mut Ctx<'_>, actions: Vec<Action>) {
    for action in actions {
        match action {
            Action::Query(which, query) => commands::request_rank(cx, shared, which, query),
            Action::Move(which, step) => {
                let mut state = lock(shared);
                let slot = which.slot(&mut state);
                let last = slot.items.len().saturating_sub(1) as isize;
                slot.selected = (slot.selected as isize + step).clamp(0, last.max(0)) as usize;
            }
            Action::Open(which, row) => open_row(shared, cx, which, Some(row)),
            Action::OpenSelected(which) => open_row(shared, cx, which, None),
            Action::Close => commands::close_picker(shared),
        }
    }
}

/// Opens one result. `row` defaults to the selected row. A `:line` suffix in the query moves the cursor.
fn open_row(shared: &Shared, cx: &mut Ctx<'_>, which: Which, row: Option<usize>) {
    let (rel, line) = {
        let mut state = lock(shared);
        let slot = which.slot(&mut state);
        let row = row.unwrap_or(slot.selected);
        let rel = slot.items.get(row).cloned();
        let line = fuzzy::split_location(&slot.query).1;
        (rel, line)
    };
    if let Some(rel) = rel {
        commands::open_result(cx, shared, &rel, line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Text(text.to_owned()));
        raw
    }

    #[test]
    fn typing_moves_focus_from_the_close_button_to_the_query() {
        let ctx = egui::Context::default();
        let search = egui::Id::new("forja.find.test.search");
        let mut text = String::new();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.button("Close window").request_focus();
            });
        });
        assert!(!ctx.memory(|memory| memory.has_focus(search)));
        let _ = ctx.run(typed("main"), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                focus_search_when_typing(ctx, search);
                ui.add(egui::TextEdit::singleline(&mut text).id(search));
            });
        });
        assert_eq!(text, "main", "the first typed key was lost");
        assert!(ctx.memory(|memory| memory.has_focus(search)));
    }
}
