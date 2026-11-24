//! Bigorna: fuzzy search across commands, benches, files and settings.

use crate::anvil_ask::{Ask, AskEmptyAction};
use crate::anvil_keys::AnvilKeys;
use crate::app::ForjaApp;
use eframe::egui::{self, Align2, FontId, RichText, Sense};
use forja_kernel::keymap::Scope;
use forja_kernel::Args;
use forja_ui_kit::{text_field_labeled, Color, Tokens};
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnvilHelperNotice {
    pub(crate) profile_name: Option<String>,
    pub(crate) install_command: Option<String>,
    pub(crate) any_installed: bool,
    pub(crate) copied: bool,
}

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
    /// One option of the value the Anvil is currently asking for.
    Ask(usize),
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
        let ask = self.anvil_ask.clone();
        let search_id = egui::Id::new("forja.anvil.search_id");
        let search_widget = ctx.data(|data| data.get_temp::<egui::Id>(search_id));
        let search_focused =
            search_widget.is_some_and(|id| ctx.memory(|memory| memory.has_focus(id)));
        refocus_search_for_text(ctx, active, search_widget);
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
            // Escape first backs out of the current question, keeping the
            // Anvil open on the command list.
            if self.anvil_ask.take().is_some() {
                self.anvil_query.clear();
                self.anvil_query_seen.clear();
                self.anvil_selected = 0;
                self.anvil_selection_explicit = false;
                self.anvil_selection_id = None;
                self.anvil_selection_missing = false;
                self.focus_anvil = true;
                return;
            }
            self.anvil_open = false;
            self.anvil_helper_notice = None;
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
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
            self.anvil_helper_notice = None;
        }
        let mut hits = self.collect_hits();
        let mut chosen: Option<(HitKind, bool)> = None;
        let mut chosen_empty_action: Option<AskEmptyAction> = None;
        let mut ask_submit: Option<String> = None;
        let mut cycle = 0i32;
        let title = self.text("shell.anvil.title");
        let footer = self.text(if ask.is_some() {
            "shell.anvil.ask-footer"
        } else {
            "shell.anvil.footer"
        });
        let empty_key = self.empty_key();
        let ask_empty_action = ask.as_ref().and_then(Ask::empty_action);
        let ask_prompt = ask
            .as_ref()
            .and_then(Ask::field)
            .map(|field| self.text(field.label_key));
        let ask_is_text = ask.as_ref().is_some_and(Ask::is_text);
        let command_has_inline_args = ask.is_none()
            && !self.anvil_rail_select
            && !self.anvil_muster_title
            && split_command_query_with_arguments(&self.anvil_query).is_some();
        let group_labels = [
            self.text("shell.anvil.group.all"),
            self.text("shell.anvil.group.commands"),
            self.text("shell.anvil.group.benches"),
            self.text("shell.anvil.group.files"),
            self.text("shell.anvil.group.settings"),
        ];
        let tokens = self.tokens.clone();
        let results_height = (ctx.content_rect().height() - 360.0).clamp(80.0, 240.0);
        let mut open_helpers = false;
        let mut notice_copied = self
            .anvil_helper_notice
            .as_ref()
            .is_some_and(|notice| notice.copied);
        let notice_view = self.anvil_helper_notice.as_ref().map(|notice| {
            let headline = match (notice.any_installed, &notice.profile_name) {
                (true, Some(name)) => {
                    format!("{name} · {}", self.text("shell.anvil.helper-not-installed"))
                }
                _ => self.text("shell.anvil.helper-none-installed"),
            };
            let body = self.text("shell.anvil.helper-install-guide");
            let copy_label = self.text(if notice.copied {
                "shell.anvil.helper-copied"
            } else {
                "shell.anvil.helper-copy-install"
            });
            let open_label = self.text("shell.anvil.helper-open-helpers");
            (
                headline,
                body,
                notice.install_command.clone(),
                copy_label,
                open_label,
            )
        });
        let entry = forja_ui_kit::entry_motion(ctx, layer.with("entry"), forja_ui_kit::Motion::Base);
        let veil_layer = egui::LayerId::new(egui::Order::Middle, layer.with("veil"));
        ctx.layer_painter(veil_layer).rect_filled(ctx.content_rect(), 0.0, egui::Color32::from_black_alpha((90.0 * entry) as u8));
        let modal = egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .default_width(620.0)
            .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
            .show(ctx, |ui| {
                ui.set_opacity(entry);
                forja_ui_kit::register_keyboard_layer(ui, layer);
                let hint = self.text(if self.anvil_muster_title {
                    "shell.anvil.muster-title"
                } else if ask.is_some() {
                    "shell.anvil.ask-search"
                } else {
                    "shell.anvil.search"
                });
                let search = text_field_labeled(ui, &mut self.anvil_query, &hint, &hint);
                ui.ctx()
                    .data_mut(|data| data.insert_temp(search_id, search.id));
                if search.changed() {
                    self.anvil_query_seen.clone_from(&self.anvil_query);
                    self.anvil_selected = 0;
                    self.anvil_selection_explicit = false;
                    self.anvil_selection_id = None;
                    self.anvil_selection_missing = false;
                    self.anvil_helper_notice = None;
                    hits = self.collect_hits();
                }
                if self.focus_anvil || reactivated {
                    search.request_focus();
                    self.focus_anvil = false;
                }
                ui.add_space(6.0);
                if let Some(prompt) = &ask_prompt {
                    let metrics = tokens.metrics();
                    forja_ui_kit::wrapped_label(
                        ui,
                        prompt,
                        FontId::proportional(metrics.body),
                        paint(tokens.color.text),
                    );
                }
                if self.anvil_rail_select {
                    ui.label(self.text("shell.anvil.choose-rail"));
                }
                if self.anvil_muster_title {
                    ui.label(self.text("shell.anvil.muster-title"));
                }
                if !self.anvil_rail_select && !self.anvil_muster_title && ask.is_none() {
                    let mut index = self.active_group();
                    let refs: Vec<&str> = group_labels.iter().map(String::as_str).collect();
                    forja_ui_kit::segmented(ui, &mut index, &refs);
                    if index != self.anvil_group || self.anvil_query.trim_start().starts_with('>') {
                        if index != self.anvil_group {
                            self.anvil_selected = 0;
                            self.anvil_selection_explicit = false;
                            self.anvil_selection_id = None;
                            self.anvil_selection_missing = false;
                        }
                        self.anvil_group = index;
                        if let Some(stripped) = self.anvil_query.trim_start().strip_prefix('>') {
                            self.anvil_query = stripped.trim_start().to_owned();
                        }
                    }
                }
                if active
                    && ui.input_mut(|input| input.consume_key(keys.next.0, keys.next.1))
                    && !hits.is_empty()
                {
                    self.anvil_selected = (self.anvil_selected + 1).min(hits.len() - 1);
                    self.anvil_selection_explicit = true;
                    self.anvil_selection_id = Some(history_id(&hits[self.anvil_selected].kind));
                    self.anvil_selection_missing = false;
                }
                if active
                    && !hits.is_empty()
                    && ui.input_mut(|input| input.consume_key(keys.previous.0, keys.previous.1))
                {
                    self.anvil_selected = self.anvil_selected.saturating_sub(1);
                    self.anvil_selection_explicit = true;
                    self.anvil_selection_id = Some(history_id(&hits[self.anvil_selected].kind));
                    self.anvil_selection_missing = false;
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
                    if ask.is_some() {
                        if ask_is_text {
                            let value = self.anvil_query.trim().to_owned();
                            if !value.is_empty() {
                                ask_submit = Some(value);
                            }
                            search.request_focus();
                        } else if hits.is_empty() {
                            if self.anvil_query.trim().is_empty() {
                                chosen_empty_action = ask_empty_action;
                            }
                            search.request_focus();
                        } else {
                            let index = self.anvil_selected.min(hits.len() - 1);
                            chosen = Some((hits[index].kind.clone(), keep));
                        }
                    } else if self.anvil_muster_title {
                        if !self.anvil_query.trim().is_empty() {
                            let mut args = serde_json::Map::new();
                            args.insert(
                                "title".into(),
                                Value::String(self.anvil_query.trim().to_owned()),
                            );
                            chosen = Some((
                                HitKind::Action {
                                    command: "muster.run".into(),
                                    args,
                                },
                                false,
                            ));
                        }
                        search.request_focus();
                    } else if hits.is_empty() {
                        search.request_focus();
                    } else {
                        let index = self.anvil_selected.min(hits.len() - 1);
                        let raw_query = self
                            .anvil_query
                            .trim()
                            .trim_start_matches('>')
                            .trim()
                            .to_owned();
                        let query = split_command_query_with_arguments(&raw_query)
                            .map(|(command, _)| command.to_lowercase())
                            .unwrap_or_else(|| raw_query.to_lowercase());
                        if !self.anvil_selection_missing
                            && can_run_selection(
                                &query,
                                &hits[index],
                                self.anvil_selection_explicit,
                            )
                        {
                            chosen = Some((hits[index].kind.clone(), keep));
                        }
                    }
                }
                ui.add_space(4.0);
                if command_has_inline_args {
                    ui.label(
                        RichText::new(self.text("shell.anvil.command-arguments"))
                            .small()
                            .color(muted(&tokens)),
                    );
                }
                if ask_is_text {
                    ui.label(
                        RichText::new(self.text("shell.anvil.ask-text-hint")).color(muted(&tokens)),
                    );
                } else if !self.anvil_muster_title {
                    egui::ScrollArea::vertical()
                        .max_height(results_height)
                        .show(ui, |ui| {
                            if hits.is_empty() {
                                ui.label(RichText::new(self.text(empty_key)).color(muted(&tokens)));
                                if let Some(action) = ask_empty_action {
                                    ui.add_space(6.0);
                                    if forja_ui_kit::button(
                                        ui,
                                        forja_ui_kit::ButtonKind::Primary,
                                        &self.text(action.label_key()),
                                    )
                                    .clicked()
                                    {
                                        chosen_empty_action = Some(action);
                                    }
                                }
                            }
                            for (row, hit) in hits.iter().enumerate() {
                                let response =
                                    anvil_row(ui, &tokens, hit, row == self.anvil_selected);
                                if response.clicked() {
                                    self.anvil_selected = row;
                                    chosen = Some((hit.kind.clone(), false));
                                }
                            }
                        });
                }
                if ask.is_none()
                    && !self.anvil_muster_title
                    && !self.anvil_rail_select
                    && !self.anvil_views.is_empty()
                    && self
                        .anvil_views
                        .iter()
                        .any(|view| !view.view.provides_anvil_hits())
                {
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("forja.anvil.sources")
                        .max_height(60.0)
                        .show(ui, |ui| {
                            for view in self.anvil_views.iter_mut() {
                                if view.view.provides_anvil_hits() {
                                    continue;
                                }
                                let module = view.module.clone();
                                let mut cx = self.runtime.ctx(&module);
                                ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
                            }
                        });
                }
                if let Some((headline, body, command, copy_label, open_label)) = &notice_view {
                    ui.add_space(6.0);
                    let guide = forja_ui_kit::HelperMissingGuide {
                        headline,
                        body,
                        install_command: command.as_deref(),
                        copy_label,
                        open_helpers_label: Some(open_label.as_str()),
                    };
                    if forja_ui_kit::helper_missing_guide(ui, &guide, &mut notice_copied) {
                        open_helpers = true;
                    }
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(footer)
                        .color(muted(&tokens))
                        .size(tokens.metrics().caption),
                );
            });
        if let Some(modal) = &modal {
            forja_ui_kit::scale_layer(
                ctx,
                modal.response.layer_id,
                modal.response.rect.center(),
                egui::lerp(0.98..=1.0, entry),
            );
        }
        if let Some(notice) = self.anvil_helper_notice.as_mut() {
            notice.copied = notice_copied;
        }
        if let Some(modal) = modal {
            self.slot_rects.insert("anvil".into(), modal.response.rect);
        }
        if cycle != 0 {
            let count = 5i32;
            self.anvil_group = (self.anvil_group as i32 + cycle).rem_euclid(count) as usize;
            self.anvil_selected = 0;
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
            self.anvil_helper_notice = None;
        }
        if open_helpers {
            self.anvil_open = false;
            self.anvil_helper_notice = None;
            crate::actions::push(crate::actions::Action::OpenRail("helper.tab".into()));
            restore_focus(ctx);
        } else if let Some(action) = chosen_empty_action {
            self.run_ask_empty_action(action);
        } else if let Some((kind, keep)) = chosen {
            if let HitKind::Ask(index) = kind {
                self.apply_ask_choice(index);
            } else {
                self.run_hit(kind, keep);
                if !keep && !self.anvil_open {
                    restore_focus(ctx);
                }
            }
        }
        if let Some(value) = ask_submit {
            self.apply_ask_value(value);
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
        if self.anvil_ask.is_some() {
            return "shell.anvil.ask-empty";
        }
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
        if self.anvil_ask.is_some() {
            return self.ask_hits(&query);
        }
        if self.anvil_rail_select {
            return self
                .rail_views
                .iter()
                .filter_map(|view| {
                    let title = self.text(&view.title_key);
                    let score = best_score(&query, &title, &view.id)?;
                    Some(Hit {
                        kind: rail_hit(&view.id),
                        title,
                        detail: view.id.clone(),
                        icon: view.icon_glyph.clone(),
                        chord: String::new(),
                        score,
                    })
                })
                .collect();
        }
        self.runtime.set_anvil_query(&raw);
        let group = self.active_group();
        let command_query =
            split_command_query_with_arguments(&raw).map(|(command, _)| command.to_lowercase());
        let mut hits = Vec::new();
        if group == 0 || group == 1 {
            self.push_commands(&mut hits, command_query.as_deref().unwrap_or(&query));
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
        apply_history_boost(
            &mut hits,
            &self.shell_data.anvil_history,
            &query,
            show_recent,
        );
        let sort_query = if group == 0 || group == 1 {
            command_query.as_deref().unwrap_or(&query)
        } else {
            &query
        };
        sort_hits(&mut hits, sort_query);
        hits.truncate(40);
        self.reconcile_selection(&hits);
        hits
    }

    /// Options of the current question. An empty query keeps the source order
    /// (pinned and recent benches first); typing filters by both label and detail.
    fn ask_hits(&mut self, query: &str) -> Vec<Hit> {
        let Some(ask) = &self.anvil_ask else {
            return Vec::new();
        };
        if ask.is_text() {
            return Vec::new();
        }
        let mut hits: Vec<Hit> = ask
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                let score = best_score(query, &option.label, &option.detail)?;
                Some(Hit {
                    kind: HitKind::Ask(index),
                    title: option.label.clone(),
                    detail: option.detail.clone(),
                    icon: None,
                    chord: String::new(),
                    score,
                })
            })
            .collect();
        if !query.is_empty() {
            hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        }
        hits.truncate(40);
        self.reconcile_selection(&hits);
        hits
    }

    fn reconcile_selection(&mut self, hits: &[Hit]) {
        if let Some(id) = self.anvil_selection_id.as_deref() {
            if let Some(index) = selected_hit_index(hits, id) {
                self.anvil_selected = index;
                self.anvil_selection_missing = false;
            } else {
                self.anvil_selected = 0;
                self.anvil_selection_missing = true;
            }
        } else if self.anvil_selected >= hits.len() {
            self.anvil_selected = hits.len().saturating_sub(1);
        }
    }

    fn push_commands(&self, hits: &mut Vec<Hit>, query: &str) {
        for command in self.runtime.available_commands(Default::default()) {
            let title = self.text(&command.title_key);
            let id = command.id.as_str();
            if id == "terminal.recipe.run" {
                continue;
            }
            let Some(score) = best_score(query, &title, id) else {
                continue;
            };
            hits.push(Hit {
                kind: HitKind::Command(id.to_owned()),
                title,
                detail: id.to_owned(),
                icon: None,
                chord: self.chord_for(id),
                score: score
                    + if matches!(id, "appearance.theme.light" | "appearance.theme.dark") {
                        30
                    } else {
                        0
                    },
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
            .filter(|binding| binding.binding.command.as_str() == command)
            .min_by_key(|binding| binding.binding.scope != Scope::Global)
            .map(|binding| binding.binding.chord.to_string())
            .unwrap_or_default()
    }

    pub(crate) fn run_hit(&mut self, kind: HitKind, keep_open: bool) {
        if let Some(notice) = self.missing_helper_for_hit(&kind) {
            self.anvil_helper_notice = Some(notice);
            self.anvil_open = true;
            return;
        }
        self.anvil_helper_notice = None;
        if matches!(&kind, HitKind::Command(command) if command == "muster.run") {
            self.anvil_muster_title = true;
            self.anvil_query.clear();
            self.anvil_query_seen.clear();
            self.anvil_selected = 0;
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
            self.focus_anvil = true;
            return;
        }
        if matches!(&kind, HitKind::Command(command) if command == "shell.rail.open") {
            self.anvil_rail_select = true;
            self.anvil_query.clear();
            self.anvil_query_seen.clear();
            self.anvil_selected = 0;
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
            self.focus_anvil = true;
            return;
        }
        // A command that needs context asks for it in the Anvil instead of
        // failing with a missing-argument error.
        match &kind {
            HitKind::Command(command) if self.start_ask(command) => return,
            HitKind::Action { command, args }
                if args.is_empty()
                    && self
                        .runtime
                        .registry()
                        .command(command)
                        .is_some_and(|spec| !spec.fields.is_empty()) =>
            {
                self.start_ask(command);
                return;
            }
            _ => {}
        }
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
                let label = self
                    .runtime
                    .registry()
                    .settings()
                    .iter()
                    .find(|spec| spec.key.as_str() == key)
                    .map(|spec| self.text(&spec.label_key));
                self.settings_query = setting_search_label(&key, label.as_deref());
            }
            HitKind::Ask(_) => {}
            HitKind::Action { command, args } => {
                match self.runtime.run_command(&command, Args::from_map(args)) {
                    Ok(_) if command == "muster.run" => {
                        crate::actions::push(crate::actions::Action::OpenRail("muster.page".into()))
                    }
                    Ok(_) if command == "terminal.recipe.run" => crate::actions::push(
                        crate::actions::Action::ShowBenchTab("terminal.tab".into()),
                    ),
                    Ok(_) => {}
                    Err(error) => {
                        self.set_command_error(&command, &error);
                    }
                }
            }
        }
    }

    fn missing_helper_for_hit(&mut self, kind: &HitKind) -> Option<AnvilHelperNotice> {
        let (command, args) = match kind {
            HitKind::Command(command) => (command.as_str(), None),
            HitKind::Action { command, args } => (command.as_str(), Some(args)),
            _ => return None,
        };
        let requested_profile_id = self.requested_helper_for_command(command, args)?;
        self.runtime.registry().command("helper.list")?;
        let Ok(list) = self.runtime.run_command("helper.list", Args::new()) else {
            return None;
        };
        let profiles = list.get("profiles").and_then(Value::as_array)?;
        evaluate_missing_helper(profiles, requested_profile_id.as_deref())
    }

    fn requested_helper_for_command(
        &mut self,
        command: &str,
        args: Option<&serde_json::Map<String, Value>>,
    ) -> Option<Option<String>> {
        if matches!(
            command,
            "helper.launch"
                | "helper.launch.default"
                | "helper.launch.profile"
                | "chat.open"
                | "chat.send"
        ) {
            let profile_id = args
                .and_then(|map| map.get("profile_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned);
            return Some(profile_id);
        }
        if command == "content.run" {
            let content_id = args.and_then(|map| map.get("id")).and_then(Value::as_str)?;
            let list = self
                .runtime
                .run_command("content.commands.list", Args::new())
                .ok()?;
            let entry = list
                .get("commands")
                .and_then(Value::as_array)?
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(content_id))?;
            if entry.get("run").and_then(Value::as_str) == Some("helper") {
                let helper = entry
                    .get("helper")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned);
                return Some(helper);
            }
        }
        None
    }

    fn remember(&mut self, id: &str) {
        self.shell_data.anvil_history.retain(|item| item != id);
        self.shell_data.anvil_history.insert(0, id.to_owned());
        let limit = clamp_history_limit(self.setting_number("anvil.history.limit", 100.0));
        self.shell_data.anvil_history.truncate(limit);
        self.save_shell_data();
    }
}

fn rail_hit(view_id: &str) -> HitKind {
    let mut args = serde_json::Map::new();
    args.insert("view_id".into(), Value::String(view_id.to_owned()));
    HitKind::Action {
        command: "shell.rail.open".into(),
        args,
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
            let path = args
                .get("path")
                .or_else(|| args.get("recipe_id"))
                .and_then(Value::as_str)
                .unwrap_or("");
            format!("action:{command}:{path}")
        }
        HitKind::Ask(index) => format!("ask:{index}"),
    }
}

fn setting_search_label(key: &str, label: Option<&str>) -> String {
    label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or(key)
        .to_owned()
}

fn selected_hit_index(hits: &[Hit], id: &str) -> Option<usize> {
    hits.iter().position(|hit| history_id(&hit.kind) == id)
}

fn has_printable_text(events: &[egui::Event]) -> bool {
    events.iter().any(
        |event| matches!(event, egui::Event::Text(text) if text.chars().any(|ch| !ch.is_control())),
    )
}

fn refocus_search_for_text(ctx: &egui::Context, active: bool, search: Option<egui::Id>) {
    if !active
        || egui::Popup::is_any_open(ctx)
        || !ctx.input(|input| has_printable_text(&input.events))
    {
        return;
    }
    if let Some(id) = search {
        ctx.memory_mut(|memory| memory.request_focus(id));
    }
}

fn match_quality(query: &str, title: &str, detail: &str) -> u8 {
    if query.is_empty() {
        return 0;
    }
    let title = title.to_lowercase();
    if title == query {
        return 5;
    }
    if title.starts_with(query) {
        return 4;
    }
    if word_starts_with(&title, query) {
        return 3;
    }
    let detail = detail.to_lowercase();
    if detail == query {
        return 2;
    }
    if word_starts_with(&detail, query) {
        return 1;
    }
    0
}

fn word_starts_with(text: &str, query: &str) -> bool {
    text.match_indices(query).any(|(index, _)| {
        text[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !ch.is_alphanumeric())
    })
}

fn sort_hits(hits: &mut [Hit], query: &str) {
    hits.sort_by(|a, b| {
        match_quality(query, &b.title, &b.detail)
            .cmp(&match_quality(query, &a.title, &a.detail))
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| {
                if query.is_empty() {
                    a.title.cmp(&b.title)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
    });
}

fn can_run_selection(query: &str, hit: &Hit, explicitly_selected: bool) -> bool {
    query.is_empty() || explicitly_selected || match_quality(query, &hit.title, &hit.detail) > 0
}

/// Splits `<command> key=value ...` queries typed in the Anvil so the command
/// stays selectable and the UI can explain that arguments are asked next.
pub(crate) fn split_command_query_with_arguments(query: &str) -> Option<(&str, &str)> {
    let trimmed = query.trim().trim_start_matches('>').trim();
    let split = trimmed.find(char::is_whitespace)?;
    let (head, rest) = trimmed.split_at(split);
    let rest = rest.trim();
    if head.is_empty() || !rest.contains('=') {
        return None;
    }
    Some((head, rest))
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
    if let Some(id) = hit.icon.as_deref().and_then(forja_ui_kit::Icon::from_id) {
        forja_ui_kit::paint_icon(
            ui.painter(),
            id,
            egui::Rect::from_center_size(title_pos + egui::vec2(8.0, 0.0), egui::vec2(16.0, 16.0)),
            paint(tokens.color.text),
        );
        title_pos.x += 22.0;
    } else if let Some(icon) = &hit.icon {
        ui.painter().text(
            title_pos,
            Align2::LEFT_CENTER,
            icon,
            font.clone(),
            paint(tokens.color.text),
        );
        title_pos.x += 22.0;
    }
    let right = right_label(hit);
    let right_width = if right.is_empty() {
        0.0
    } else {
        ui.painter()
            .layout_no_wrap(
                right.to_owned(),
                font.clone(),
                paint(tokens.color.text_muted),
            )
            .size()
            .x
            + 8.0
    };
    let title_max_width = (text.right() - title_pos.x - right_width).max(24.0);
    let mut title_job = egui::text::LayoutJob::single_section(
        hit.title.clone(),
        egui::TextFormat {
            font_id: font.clone(),
            color: paint(tokens.color.text),
            ..Default::default()
        },
    );
    title_job.wrap = egui::text::TextWrapping {
        max_width: title_max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let title_galley = ui.painter().layout_job(title_job);
    let title_elided = title_galley.elided;
    let draw_pos = egui::pos2(title_pos.x, title_pos.y - title_galley.size().y * 0.5);
    ui.painter()
        .galley(draw_pos, title_galley, paint(tokens.color.text));
    if !right.is_empty() {
        let detail_rect = egui::Rect::from_min_max(
            egui::pos2(text.right() - right_width + 8.0, text.top()),
            text.right_bottom(),
        );
        ui.put(
            detail_rect,
            egui::Label::new(
                RichText::new(right)
                    .font(font)
                    .color(paint(tokens.color.text_muted)),
            )
            .truncate(),
        );
    }
    if title_elided {
        forja_ui_kit::tooltip(response, &hit.title)
    } else {
        response
    }
}

/// Column on the right shows a real shortcut chord when bound, or human file/action context.
/// Never leaks internal command, bench, or setting identifiers.
fn right_label(hit: &Hit) -> &str {
    if !hit.chord.is_empty() {
        return &hit.chord;
    }
    match hit.kind {
        HitKind::File(_) | HitKind::Action { .. } | HitKind::Ask(_) => &hit.detail,
        HitKind::Command(_) | HitKind::Bench(_) | HitKind::Setting(_) => "",
    }
}

fn muted(tokens: &Tokens) -> egui::Color32 {
    paint(tokens.color.text_muted)
}

fn apply_history_boost(hits: &mut [Hit], history: &[String], query: &str, show_recent: bool) {
    if !show_recent || !query.is_empty() {
        return;
    }
    for hit in hits {
        if history.iter().any(|id| id == &history_id(&hit.kind)) {
            hit.score += 200;
        }
    }
}

fn clamp_history_limit(value: f64) -> usize {
    value.round().clamp(10.0, 1000.0) as usize
}

fn paint(color: Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn evaluate_missing_helper(
    profiles: &[Value],
    requested_profile_id: Option<&str>,
) -> Option<AnvilHelperNotice> {
    let any_installed = profiles
        .iter()
        .any(|item| item.get("installed").and_then(Value::as_bool) == Some(true));
    let default_hint = profiles
        .iter()
        .find(|item| item.get("default").and_then(Value::as_bool) == Some(true))
        .and_then(|item| item.get("install_hint").and_then(Value::as_str))
        .or_else(|| {
            profiles
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some("claude-code"))
                .and_then(|item| item.get("install_hint").and_then(Value::as_str))
        })
        .or_else(|| {
            profiles
                .iter()
                .find_map(|item| item.get("install_hint").and_then(Value::as_str))
        })
        .map(str::to_owned);
    if let Some(id) = requested_profile_id {
        let target = profiles
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id));
        let is_installed =
            target.is_some_and(|item| item.get("installed").and_then(Value::as_bool) == Some(true));
        if is_installed {
            return None;
        }
        return Some(AnvilHelperNotice {
            profile_name: target
                .and_then(|item| item.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .or_else(|| Some(id.to_owned())),
            install_command: target
                .and_then(|item| item.get("install_hint").and_then(Value::as_str))
                .map(str::to_owned)
                .or(default_hint),
            any_installed,
            copied: false,
        });
    }
    if any_installed {
        None
    } else {
        Some(AnvilHelperNotice {
            profile_name: None,
            install_command: default_hint,
            any_installed: false,
            copied: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_matches_beat_recent_fuzzy_hits_and_fuzzy_needs_selection() {
        let make_hit = |title: &str, score| Hit {
            kind: HitKind::Command(title.to_owned()),
            title: title.to_owned(),
            detail: String::new(),
            icon: None,
            chord: String::new(),
            score,
        };
        let mut hits = vec![
            make_hit("Compartilhar com ajudante", 1000),
            make_hit("Compilar pacotes Go", 1),
        ];
        sort_hits(&mut hits, "compilar");
        assert_eq!(hits[0].title, "Compilar pacotes Go");
        assert!(can_run_selection("compilar", &hits[0], false));
        assert!(!can_run_selection("compilar", &hits[1], false));
        assert!(can_run_selection("compilar", &hits[1], true));
    }

    #[test]
    fn history_does_not_reorder_equal_prefixes_and_recipes_have_distinct_ids() {
        let recipe = |id: &str, title: &str| {
            let mut args = serde_json::Map::new();
            args.insert("recipe_id".into(), Value::String(id.to_owned()));
            Hit {
                kind: HitKind::Action {
                    command: "terminal.recipe.run".into(),
                    args,
                },
                title: title.into(),
                detail: "projeto".into(),
                icon: None,
                chord: String::new(),
                score: 100,
            }
        };
        let mut hits = vec![
            recipe("go-build", "Compilar pacotes Go"),
            recipe("python-build", "Compilar pacote Python"),
        ];
        let recent = vec![history_id(&hits[1].kind)];
        assert_ne!(history_id(&hits[0].kind), recent[0]);
        apply_history_boost(&mut hits, &recent, "compilar", true);
        sort_hits(&mut hits, "compilar");
        assert_eq!(hits[0].title, "Compilar pacotes Go");
        let chosen = history_id(&hits[1].kind);
        hits.swap(0, 1);
        assert_eq!(selected_hit_index(&hits, &chosen), Some(0));
        hits.remove(0);
        assert_eq!(selected_hit_index(&hits, &chosen), None);
    }

    #[test]
    fn only_printable_text_reclaims_anvil_search_focus() {
        assert!(has_printable_text(&[egui::Event::Text("banc".into())]));
        assert!(!has_printable_text(&[egui::Event::Text("\n".into())]));
        assert!(!has_printable_text(&[]));
    }

    #[test]
    fn typing_after_focusing_a_filter_reaches_the_search() {
        let ctx = egui::Context::default();
        let search = egui::Id::new("search");
        let filter = egui::Id::new("filter");
        let mut query = String::new();
        for frame in 0..3 {
            let mut input = egui::RawInput::default();
            if frame == 2 {
                input.events.push(egui::Event::Text("banc".into()));
            }
            let _ = ctx.run(input, |ctx| {
                refocus_search_for_text(ctx, true, Some(search));
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut query).id(search));
                    let response = ui.interact(
                        ui.available_rect_before_wrap(),
                        filter,
                        egui::Sense::click(),
                    );
                    if frame == 1 {
                        response.request_focus();
                    }
                });
            });
        }
        assert_eq!(query, "banc");
    }

    #[test]
    fn rail_choice_supplies_the_selected_view() {
        let HitKind::Action { command, args } = rail_hit("helper.page") else {
            panic!("expected rail action");
        };
        assert_eq!(command, "shell.rail.open");
        assert_eq!(
            args.get("view_id"),
            Some(&Value::String("helper.page".into()))
        );
    }

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
    fn settings_hit_uses_its_human_label_as_the_search_query() {
        assert_eq!(
            setting_search_label("archive.sessions.index", Some("Indexar sessões no Acervo")),
            "Indexar sessões no Acervo"
        );
        assert_eq!(
            setting_search_label("archive.sessions.index", None),
            "archive.sessions.index"
        );
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

    #[test]
    fn right_label_hides_internal_ids_without_shortcut() {
        let cmd = Hit {
            kind: HitKind::Command("terminal.open".into()),
            title: "Abrir terminal".into(),
            detail: "terminal.open".into(),
            chord: String::new(),
            icon: None,
            score: 1,
        };
        assert_eq!(right_label(&cmd), "");
        let setting = Hit {
            kind: HitKind::Setting("appearance.theme.light_name".into()),
            title: "Tema claro".into(),
            detail: "appearance.theme.light_name".into(),
            chord: String::new(),
            icon: None,
            score: 1,
        };
        assert_eq!(right_label(&setting), "");
        let bound = Hit {
            kind: HitKind::Command("anvil.open".into()),
            title: "Abrir Bigorna".into(),
            detail: "anvil.open".into(),
            chord: "Ctrl+K".into(),
            icon: None,
            score: 1,
        };
        assert_eq!(right_label(&bound), "Ctrl+K");
    }

    #[test]
    fn inline_key_value_arguments_keep_command_selectable() {
        let split =
            split_command_query_with_arguments("> bench.stage.set bench_id=exemplo stage=forjando");
        assert_eq!(
            split,
            Some(("bench.stage.set", "bench_id=exemplo stage=forjando"))
        );
        assert_eq!(split_command_query_with_arguments("Mudar etapa"), None);
        let hit = Hit {
            kind: HitKind::Command("bench.stage.set".into()),
            title: "Mudar etapa".into(),
            detail: "bench.stage.set".into(),
            chord: String::new(),
            icon: None,
            score: 1,
        };
        let command = split.unwrap().0;
        assert!(can_run_selection(command, &hit, false));
    }

    #[test]
    fn missing_helper_notice_handles_none_specific_and_installed_cases() {
        let none_profiles = vec![serde_json::json!({
            "id": "claude-code",
            "name": "Claude Code",
            "installed": false,
            "default": true,
            "install_hint": "npm install -g @anthropic-ai/claude-code"
        })];
        let notice =
            evaluate_missing_helper(&none_profiles, None).expect("notice when none installed");
        assert!(!notice.any_installed);
        assert_eq!(notice.profile_name, None);
        assert_eq!(
            notice.install_command.as_deref(),
            Some("npm install -g @anthropic-ai/claude-code")
        );

        let mixed_profiles = vec![
            serde_json::json!({
                "id": "claude-code",
                "name": "Claude Code",
                "installed": true,
                "default": true,
                "install_hint": "npm install -g @anthropic-ai/claude-code"
            }),
            serde_json::json!({
                "id": "codex",
                "name": "OpenAI Codex CLI",
                "installed": false,
                "default": false,
                "install_hint": "npm install -g @openai/codex"
            }),
        ];
        assert_eq!(evaluate_missing_helper(&mixed_profiles, None), None);
        assert_eq!(
            evaluate_missing_helper(&mixed_profiles, Some("claude-code")),
            None
        );
        let codex_notice = evaluate_missing_helper(&mixed_profiles, Some("codex"))
            .expect("missing requested codex");
        assert!(codex_notice.any_installed);
        assert_eq!(
            codex_notice.profile_name.as_deref(),
            Some("OpenAI Codex CLI")
        );
        assert_eq!(
            codex_notice.install_command.as_deref(),
            Some("npm install -g @openai/codex")
        );
    }
}
