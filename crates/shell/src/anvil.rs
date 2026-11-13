//! Bigorna: fuzzy search across commands, benches, files and settings.

use crate::anvil_keys::AnvilKeys;
use crate::app::ForjaApp;
use eframe::egui::{self, Align2, FontId, RichText, Sense};
use forja_kernel::keymap::Scope;
use forja_kernel::Args;
use forja_ui_kit::{text_field, Color, Tokens};
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub(crate) enum HitKind {
    Command(String),
    Bench(String),
    File(String),
    Setting(String),
    Action {
        command: String,
        args: serde_json::Map<String, Value>,
    },
}

struct Hit {
    kind: HitKind,
    title: String,
    detail: String,
    icon: Option<String>,
    chord: String,
    score: i32,
}

impl ForjaApp {
    pub(crate) fn anvil_window(&mut self, ctx: &egui::Context) {
        if !self.anvil_open {
            return;
        }
        let layer = egui::Id::new("forja.anvil.keyboard");
        let active = forja_ui_kit::keyboard_layer_active(ctx, layer);
        let active_id = egui::Id::new("forja.anvil.was_active");
        let reactivated = ctx.data_mut(|data| {
            let was_active = data.get_temp::<bool>(active_id).unwrap_or(false);
            data.insert_temp(active_id, active);
            active && !was_active
        });
        let keys = AnvilKeys::from_keymap(self.runtime.keymap());
        let search_id = egui::Id::new("forja.anvil.search_id");
        let search_focused = ctx
            .data(|data| data.get_temp::<egui::Id>(search_id))
            .is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)));
        // TextEdit consumes Ctrl+Enter, so route the Anvil binding first.
        let keep = active
            && search_focused
            && !egui::Popup::is_any_open(ctx)
            && ctx
                .input_mut(|input| input.consume_key(keys.run_background.0, keys.run_background.1));
        if active
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|input| input.consume_key(keys.close.0, keys.close.1))
        {
            self.anvil_open = false;
            restore_focus(ctx);
            return;
        }
        if self.focus_anvil {
            let focused = ctx.memory(|memory| memory.focused());
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("forja.anvil.return_focus"), focused)
            });
        }
        if self.anvil_query != self.anvil_query_seen {
            self.anvil_query_seen.clone_from(&self.anvil_query);
            self.anvil_selected = 0;
        }
        let hits = self.collect_hits();
        let mut chosen: Option<(HitKind, bool)> = None;
        let mut cycle = 0i32;
        let title = self.text("shell.anvil.title");
        let footer = self.text("shell.anvil.footer");
        let empty_key = self.empty_key();
        let group_labels = [
            self.text("shell.anvil.group.all"),
            self.text("shell.anvil.group.commands"),
            self.text("shell.anvil.group.benches"),
            self.text("shell.anvil.group.files"),
            self.text("shell.anvil.group.settings"),
        ];
        let tokens = self.tokens.clone();
        let modal = egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(620.0)
            .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
            .show(ctx, |ui| {
                forja_ui_kit::register_keyboard_layer(ui, layer);
                let hint = self.text("shell.anvil.search");
                let search = text_field(ui, &mut self.anvil_query, &hint);
                ui.ctx()
                    .data_mut(|data| data.insert_temp(search_id, search.id));
                if self.focus_anvil || reactivated {
                    search.request_focus();
                    self.focus_anvil = false;
                }
                ui.add_space(6.0);
                let mut index = self.active_group();
                let refs: Vec<&str> = group_labels.iter().map(String::as_str).collect();
                forja_ui_kit::segmented(ui, &mut index, &refs);
                if index != self.anvil_group || self.anvil_query.trim_start().starts_with('>') {
                    self.anvil_group = index;
                    if let Some(stripped) = self.anvil_query.trim_start().strip_prefix('>') {
                        self.anvil_query = stripped.trim_start().to_owned();
                    }
                }
                if active
                    && ui.input_mut(|input| input.consume_key(keys.next.0, keys.next.1))
                    && !hits.is_empty()
                {
                    self.anvil_selected = (self.anvil_selected + 1).min(hits.len() - 1);
                }
                if active
                    && ui.input_mut(|input| input.consume_key(keys.previous.0, keys.previous.1))
                {
                    self.anvil_selected = self.anvil_selected.saturating_sub(1);
                }
                if active
                    && ui.input_mut(|input| input.consume_key(keys.group_next.0, keys.group_next.1))
                {
                    cycle = 1;
                }
                if active
                    && ui.input_mut(|input| {
                        input.consume_key(keys.group_previous.0, keys.group_previous.1)
                    })
                {
                    cycle = -1;
                }
                // Enter on a filter chip belongs to the chip, not the selected result.
                let search_active = active && (search.has_focus() || search.lost_focus());
                let run = search_active
                    && ui.input_mut(|input| input.consume_key(keys.run.0, keys.run.1));
                if run || keep {
                    if hits.is_empty() {
                        search.request_focus();
                    } else {
                        let index = self.anvil_selected.min(hits.len() - 1);
                        chosen = Some((hits[index].kind.clone(), keep));
                    }
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        if hits.is_empty() {
                            ui.label(RichText::new(self.text(empty_key)).color(muted(&tokens)));
                        }
                        for (row, hit) in hits.iter().enumerate() {
                            let response = anvil_row(ui, &tokens, hit, row == self.anvil_selected);
                            if response.clicked() {
                                self.anvil_selected = row;
                                chosen = Some((hit.kind.clone(), false));
                            }
                        }
                    });
                if !self.anvil_views.is_empty()
                    && self
                        .anvil_views
                        .iter()
                        .any(|view| !view.view.provides_anvil_hits())
                {
                    ui.separator();
                    for view in self.anvil_views.iter_mut() {
                        if view.view.provides_anvil_hits() {
                            continue;
                        }
                        let module = view.module.clone();
                        let mut cx = self.runtime.ctx(&module);
                        ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                    }
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(footer)
                        .color(muted(&tokens))
                        .size(tokens.metrics().caption),
                );
            });
        if let Some(modal) = modal {
            self.slot_rects.insert("anvil".into(), modal.response.rect);
        }
        if cycle != 0 {
            let count = 5i32;
            self.anvil_group = (self.anvil_group as i32 + cycle).rem_euclid(count) as usize;
        }
        if let Some((kind, keep)) = chosen {
            if !keep {
                restore_focus(ctx);
            }
            self.run_hit(kind, keep);
        }
    }

    fn active_group(&self) -> usize {
        if self.anvil_query.trim_start().starts_with('>') {
            1
        } else {
            self.anvil_group
        }
    }

    fn empty_key(&self) -> &'static str {
        match self.active_group() {
            2 if self.runtime.registry().command("bench.list").is_none() => {
                "shell.anvil.benches-missing"
            }
            3 if self.runtime.registry().command("file.search").is_none()
                && self.runtime.registry().command("file.list").is_none() =>
            {
                "shell.anvil.files-missing"
            }
            _ => "shell.anvil.empty",
        }
    }

    fn collect_hits(&mut self) -> Vec<Hit> {
        let raw = self
            .anvil_query
            .trim()
            .trim_start_matches('>')
            .trim()
            .to_owned();
        let query = raw.to_lowercase();
        self.runtime.set_anvil_query(&raw);
        let group = self.active_group();
        let mut hits = Vec::new();
        if group == 0 || group == 1 {
            self.push_commands(&mut hits, &query);
        }
        if group == 0 || group == 2 {
            self.push_benches(&mut hits, &query);
        }
        if group == 0 || group == 3 {
            self.push_files(&mut hits, &query);
            self.push_source_hits(&mut hits, &raw);
        }
        if group == 0 || group == 4 {
            self.push_settings(&mut hits, &query);
        }
        let show_recent = self.setting_bool("anvil.history.show_recent", true);
        apply_history_boost(&mut hits, &self.anvil_history, &query, show_recent);
        hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        hits.truncate(40);
        if self.anvil_selected >= hits.len() {
            self.anvil_selected = hits.len().saturating_sub(1);
        }
        hits
    }

    fn push_commands(&self, hits: &mut Vec<Hit>, query: &str) {
        for command in self.runtime.available_commands(Default::default()) {
            let title = self.text(&command.title_key);
            let id = command.id.as_str();
            let Some(score) = best_score(query, &title, id) else {
                continue;
            };
            hits.push(Hit {
                kind: HitKind::Command(id.to_owned()),
                title,
                detail: id.to_owned(),
                icon: None,
                chord: self.chord_for(id),
                score,
            });
        }
    }

    fn push_benches(&mut self, hits: &mut Vec<Hit>, query: &str) {
        self.refresh_benches();
        for (name, id) in &self.bench_cache {
            let Some(score) = best_score(query, name, id) else {
                continue;
            };
            hits.push(Hit {
                kind: HitKind::Bench(id.clone()),
                title: name.clone(),
                detail: id.clone(),
                icon: None,
                chord: String::new(),
                score,
            });
        }
    }

    fn push_files(&mut self, hits: &mut Vec<Hit>, query: &str) {
        self.refresh_files(query);
        for (name, path) in &self.file_cache {
            let Some(score) = best_score(query, name, path) else {
                continue;
            };
            hits.push(Hit {
                kind: HitKind::File(path.clone()),
                title: name.clone(),
                detail: path.clone(),
                icon: None,
                chord: String::new(),
                score,
            });
        }
    }

    fn push_source_hits(&mut self, hits: &mut Vec<Hit>, query: &str) {
        let mut rows = Vec::new();
        for view in self.anvil_views.iter_mut() {
            if !view.view.provides_anvil_hits() {
                continue;
            }
            let module = view.module.clone();
            let mut cx = self.runtime.ctx(&module);
            rows.extend(view.view.anvil_hits(query, &mut cx));
        }
        let lowered = query.to_lowercase();
        for item in rows {
            if item.command.is_empty() {
                continue;
            }
            let score = if lowered.is_empty() {
                item.score.clamp(1, 12)
            } else {
                let Some(fuzzy) = best_score(&lowered, &item.title, &item.detail) else {
                    continue;
                };
                fuzzy + item.score.clamp(0, 400) / 40
            };
            let chord = self.chord_for(&item.command);
            hits.push(Hit {
                kind: HitKind::Action {
                    command: item.command,
                    args: item.args,
                },
                title: item.title,
                detail: item.detail,
                icon: item.icon_glyph,
                chord,
                score,
            });
        }
    }

    fn push_settings(&self, hits: &mut Vec<Hit>, query: &str) {
        for spec in self.runtime.registry().settings() {
            let label = self.text(&spec.label_key);
            let Some(score) = best_score(query, &label, &spec.key) else {
                continue;
            };
            hits.push(Hit {
                kind: HitKind::Setting(spec.key.clone()),
                title: label,
                detail: spec.key.clone(),
                icon: None,
                chord: String::new(),
                score,
            });
        }
    }

    fn refresh_benches(&mut self) {
        let stale = self
            .bench_cached_at
            .map(|at| at.elapsed() > Duration::from_secs(2))
            .unwrap_or(true);
        if !stale {
            return;
        }
        self.bench_cached_at = Some(Instant::now());
        self.bench_cache.clear();
        if self.runtime.registry().command("bench.list").is_none() {
            return;
        }
        if let Ok(value) = self.runtime.run_command("bench.list", Args::new()) {
            self.bench_cache = named_list(&value, "benches", "name", "bench_id");
        }
    }

    fn refresh_files(&mut self, query: &str) {
        if self.file_cache_query == query {
            return;
        }
        self.file_cache_query = query.to_owned();
        self.file_cache.clear();
        if query.len() < 2 {
            return;
        }
        let mut args = Args::new();
        args.insert("query", Value::String(query.to_owned()));
        if self.runtime.registry().command("file.search").is_some() {
            if let Ok(value) = self.runtime.run_command("file.search", args) {
                self.file_cache = file_list(&value);
            }
        } else if self.runtime.registry().command("file.list").is_some() {
            if let Ok(value) = self.runtime.run_command("file.list", Args::new()) {
                self.file_cache = file_list(&value);
            }
        }
    }

    pub(crate) fn chord_for(&self, command: &str) -> String {
        self.runtime
            .keymap()
            .bindings()
            .iter()
            .find(|binding| {
                binding.binding.command.as_str() == command
                    && binding.binding.scope == Scope::Global
            })
            .map(|binding| binding.binding.chord.to_string())
            .unwrap_or_default()
    }

    fn run_hit(&mut self, kind: HitKind, keep_open: bool) {
        let id = history_id(&kind);
        self.remember(&id);
        if !keep_open {
            self.anvil_open = false;
        }
        match kind {
            HitKind::Command(command) => self.run_command(&command),
            HitKind::Bench(bench_id) => {
                let mut args = Args::new();
                args.insert("bench_id", Value::String(bench_id));
                let _ = self.runtime.run_command("bench.activate", args);
            }
            HitKind::File(path) => {
                let mut args = Args::new();
                args.insert("path", Value::String(path));
                let _ = self.runtime.run_command("file.open", args);
            }
            HitKind::Setting(key) => {
                self.settings_open = true;
                self.settings_query = key;
            }
            HitKind::Action { command, args } => {
                let _ = self.runtime.run_command(&command, Args::from_map(args));
            }
        }
    }

    fn remember(&mut self, id: &str) {
        self.anvil_history.retain(|item| item != id);
        self.anvil_history.insert(0, id.to_owned());
        let limit = clamp_history_limit(self.setting_number("anvil.history.limit", 100.0));
        self.anvil_history.truncate(limit);
        let path = self.runtime.store().path_for("shell");
        let bytes = serde_json::to_vec(&serde_json::json!({
            "anvil_history": self.anvil_history
        }))
        .unwrap_or_default();
        self.runtime.tasks().spawn(
            "shell",
            move || {
                let _ = forja_kernel::store::write_atomic_bytes(&path, &bytes);
            },
            |_, _| {},
        );
    }
}

fn restore_focus(ctx: &egui::Context) {
    let focused: Option<egui::Id> = ctx.data_mut(|data| {
        data.remove_temp::<Option<egui::Id>>(egui::Id::new("forja.anvil.return_focus"))
            .flatten()
    });
    if let Some(focused) = focused {
        ctx.memory_mut(|memory| memory.request_focus(focused));
    }
}

pub(crate) fn load_history(value: &Value) -> Vec<String> {
    let array = value
        .get("anvil_history")
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    array
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn history_id(kind: &HitKind) -> String {
    match kind {
        HitKind::Command(id) => format!("command:{id}"),
        HitKind::Bench(id) => format!("bench:{id}"),
        HitKind::File(path) => format!("file:{path}"),
        HitKind::Setting(key) => format!("setting:{key}"),
        HitKind::Action { command, args } => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            format!("action:{command}:{path}")
        }
    }
}

fn best_score(query: &str, title: &str, detail: &str) -> Option<i32> {
    let title_score = fuzzy_score(query, title).map(|score| score + 5);
    let detail_score = fuzzy_score(query, detail);
    match (title_score, detail_score) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

pub(crate) fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(1);
    }
    let query: Vec<char> = query.chars().flat_map(|ch| ch.to_lowercase()).collect();
    let chars: Vec<char> = text.chars().flat_map(|ch| ch.to_lowercase()).collect();
    let mut query_index = 0;
    let mut score = 0;
    let mut previous: Option<usize> = None;
    for (index, ch) in chars.iter().enumerate() {
        if query_index < query.len() && *ch == query[query_index] {
            score += 10;
            if previous == Some(index.wrapping_sub(1)) {
                score += 8;
            }
            if index == 0 || !chars[index - 1].is_alphanumeric() {
                score += 6;
            }
            previous = Some(index);
            query_index += 1;
        }
    }
    if query_index == query.len() {
        Some(score)
    } else {
        None
    }
}

fn named_list(value: &Value, field: &str, name_key: &str, id_key: &str) -> Vec<(String, String)> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get(name_key).and_then(Value::as_str)?;
                    let id = item.get(id_key).and_then(Value::as_str)?;
                    Some((name.to_owned(), id.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn file_list(value: &Value) -> Vec<(String, String)> {
    let array = value
        .get("files")
        .or_else(|| value.get("paths"))
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    let Some(array) = array else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|item| {
            if let Some(path) = item.as_str() {
                let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
                return Some((name.to_owned(), path.to_owned()));
            }
            let path = item
                .get("path")
                .and_then(Value::as_str)
                .or_else(|| item.get("file").and_then(Value::as_str))?;
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| path.rsplit(['/', '\\']).next().unwrap_or(path));
            Some((name.to_owned(), path.to_owned()))
        })
        .collect()
}

fn anvil_row(ui: &mut egui::Ui, tokens: &Tokens, hit: &Hit, selected: bool) -> egui::Response {
    let metrics = tokens.metrics();
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, metrics.row), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &hit.title)
    });
    let fill = if selected {
        paint(tokens.color.accent_soft)
    } else if response.hovered() {
        paint(tokens.color.surface_alt)
    } else {
        egui::Color32::TRANSPARENT
    };
    if fill != egui::Color32::TRANSPARENT {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
    }
    let font = FontId::new(metrics.body, egui::FontFamily::Proportional);
    let text = rect.shrink2(egui::vec2(8.0, 0.0));
    let mut title_pos = text.left_center();
    if let Some(icon) = &hit.icon {
        ui.painter().text(
            title_pos,
            Align2::LEFT_CENTER,
            icon,
            font.clone(),
            paint(tokens.color.text),
        );
        title_pos.x += 22.0;
    }
    ui.painter().text(
        title_pos,
        Align2::LEFT_CENTER,
        &hit.title,
        font.clone(),
        paint(tokens.color.text),
    );
    let right = if hit.chord.is_empty() {
        hit.detail.clone()
    } else {
        hit.chord.clone()
    };
    ui.painter().text(
        text.right_center(),
        Align2::RIGHT_CENTER,
        right,
        font,
        paint(tokens.color.text_muted),
    );
    response
}

fn muted(tokens: &Tokens) -> egui::Color32 {
    paint(tokens.color.text_muted)
}

fn apply_history_boost(hits: &mut [Hit], history: &[String], query: &str, show_recent: bool) {
    if !show_recent {
        return;
    }
    for hit in hits {
        if history.iter().any(|id| id == &history_id(&hit.kind)) {
            hit.score += if query.is_empty() { 200 } else { 15 };
        }
    }
}

fn clamp_history_limit(value: f64) -> usize {
    value.round().clamp(10.0, 1000.0) as usize
}

fn paint(color: Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_prefers_a_word_start_and_rejects_missing_letters() {
        assert!(fuzzy_score("greet", "hello.greet").is_some());
        assert!(fuzzy_score("xyz", "hello.greet").is_none());
        let word = fuzzy_score("an", "Open the Anvil").unwrap();
        let mid = fuzzy_score("en", "Open the Anvil").unwrap();
        assert!(word >= mid);
        assert_eq!(fuzzy_score("", "anything"), Some(1));
    }

    #[test]
    fn history_reads_the_shell_object() {
        let value = serde_json::json!({ "anvil_history": ["command:anvil.open"] });
        assert_eq!(load_history(&value), vec!["command:anvil.open".to_owned()]);
    }

    #[test]
    fn history_boost_and_limit_respect_settings() {
        let history = vec!["command:anvil.open".to_owned()];
        let mut hits = vec![Hit {
            kind: HitKind::Command("anvil.open".to_owned()),
            title: "Open".to_owned(),
            detail: "anvil.open".to_owned(),
            chord: String::new(),
            icon: None,
            score: 10,
        }];
        apply_history_boost(&mut hits, &history, "", false);
        assert_eq!(hits[0].score, 10);
        apply_history_boost(&mut hits, &history, "", true);
        assert_eq!(hits[0].score, 210);

        assert_eq!(clamp_history_limit(5.0), 10);
        assert_eq!(clamp_history_limit(100.0), 100);
        assert_eq!(clamp_history_limit(2000.0), 1000);
    }
}
