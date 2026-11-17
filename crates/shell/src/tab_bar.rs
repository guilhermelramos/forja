//! Shell tab strip, icon mapping, side panel icon rows, and tool-tab launcher ("+").

use crate::app::{ForjaApp, ViewInstance};
use eframe::egui;
use forja_kernel::{settings::SettingValue, Args, Runtime};
use forja_ui_kit::{MenuItem, PanelIconTab, TabItem};
use serde_json::Value;

/// Maps each bench tab or side-panel view ID to its UI glyph (easily swappable in Item 7).
pub(crate) fn tab_icon(view_id: &str) -> forja_ui_kit::Icon {
    match view_id {
        "terminal.tab" => forja_ui_kit::TERMINAL,
        "chat.tab" => forja_ui_kit::CHAT,
        "diff.tab" => forja_ui_kit::DIFF,
        "editor.tab" => forja_ui_kit::CODE,
        "browser.tab" => forja_ui_kit::GLOBE,
        "viewers.tab" => forja_ui_kit::PREVIEW,
        "bench.detail" => forja_ui_kit::LAYERS,
        "review.tab" => forja_ui_kit::REVIEW,
        "git.history" => forja_ui_kit::HISTORY,
        "extension.panels.bench" => forja_ui_kit::Icon::ExtensionBench,
        "extension.panels.side" => forja_ui_kit::Icon::ExtensionSide,
        "extension.panels.right" => forja_ui_kit::Icon::ExtensionRight,
        "project.sidebar" => forja_ui_kit::PROJECTS,
        "board.sidebar" => forja_ui_kit::Icon::Benches,
        "explorer.panel" => forja_ui_kit::Icon::Files,
        "git.changes" => forja_ui_kit::CHANGES,
        "search.panel" => forja_ui_kit::Icon::SearchResults,
        "ports.panel" => forja_ui_kit::PORTS,
        "board.page" => forja_ui_kit::Icon::Board,
        "helper.tab" => forja_ui_kit::Icon::Helpers,
        "request.page" => forja_ui_kit::Icon::Requests,
        "routine.page" => forja_ui_kit::Icon::Routines,
        "muster.page" => forja_ui_kit::Icon::Muster,
        "archive.tab" => forja_ui_kit::Icon::Archive,
        "skill.page" => forja_ui_kit::Icon::Skills,
        "usage.page" => forja_ui_kit::Icon::Usage,
        "extension.page" => forja_ui_kit::Icon::Extensions,
        _ => forja_ui_kit::Icon::Info,
    }
}

/// Resolves a `bench.layout.tool_tabs` setting entry to a `Slot::BenchTab` view ID.
pub(crate) fn resolve_tool_tab_setting_id(entry: &str) -> Option<&'static str> {
    match entry.trim().to_ascii_lowercase().as_str() {
        "terminal" | "terminal.tab" => Some("terminal.tab"),
        "chat" | "chat.tab" | "helper" => Some("chat.tab"),
        "diff" | "diff.tab" => Some("diff.tab"),
        "editor" | "editor.tab" => Some("editor.tab"),
        "browser" | "browser.tab" => Some("browser.tab"),
        "viewer" | "viewers" | "viewers.tab" => Some("viewers.tab"),
        "bench" | "benches" | "bench.detail" => Some("bench.detail"),
        "review" | "review.tab" => Some("review.tab"),
        "history" | "git.history" => Some("git.history"),
        "extensions" | "extension" | "extension.panels.bench" => Some("extension.panels.bench"),
        _ => None,
    }
}

pub(crate) fn view_icon(view: &ViewInstance) -> forja_ui_kit::Icon {
    view.icon_glyph
        .as_deref()
        .and_then(forja_ui_kit::Icon::from_id)
        .unwrap_or_else(|| tab_icon(&view.id))
}

fn view_shortcut_command(view_id: &str) -> Option<&'static str> {
    match view_id {
        "search.panel" => Some("find.in_files"),
        "diff.tab" => Some("diff.open"),
        "chat.tab" => Some("helper.focus"),
        "terminal.tab" => Some("terminal.toggle"),
        _ => None,
    }
}

impl ForjaApp {
    pub(crate) fn query_initial_tool_tabs(
        runtime: &mut Runtime,
        bench_id: Option<&str>,
    ) -> Vec<String> {
        if runtime.registry().command("bench.tool_tabs").is_some() {
            let mut args = Args::new();
            if let Some(id) = bench_id {
                args.insert("bench_id", Value::String(id.to_owned()));
            }
            if let Ok(value) = runtime.run_command("bench.tool_tabs", args) {
                if let Some(items) = value.get("tool_tabs").and_then(Value::as_array) {
                    return items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect();
                }
            }
        }
        let configured = runtime
            .settings()
            .values()
            .get("bench.layout.tool_tabs")
            .and_then(SettingValue::as_list)
            .map(|list| list.to_vec())
            .unwrap_or_else(|| {
                ["terminal", "chat", "diff", "editor"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            });
        configured
            .iter()
            .filter_map(|entry| resolve_tool_tab_setting_id(entry))
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn initial_open_views(
        runtime: &mut Runtime,
        bench_views: &[ViewInstance],
        bench_id: Option<&str>,
    ) -> Vec<String> {
        let mut open = Self::query_initial_tool_tabs(runtime, bench_id);
        for view in bench_views {
            if view.view.empty_by_nature()
                && view.view.has_content()
                && !open.iter().any(|id| id == &view.id)
            {
                open.push(view.id.clone());
            }
        }
        open
    }

    pub(crate) fn view_tooltip(&self, view_id: &str, title: &str) -> String {
        let Some(command_id) = view_shortcut_command(view_id) else {
            return title.to_owned();
        };
        let chord = self
            .runtime
            .keymap()
            .bindings()
            .iter()
            .find(|resolved| resolved.binding.command.as_str() == command_id)
            .map(|resolved| resolved.binding.chord.to_string())
            .filter(|text| !text.is_empty());
        match chord {
            Some(shortcut) => format!("{title} ({shortcut})"),
            None => title.to_owned(),
        }
    }

    pub(crate) fn sync_dynamic_tabs(&mut self) {
        for views in [
            &mut self.bench_views,
            &mut self.sidebar_views,
            &mut self.right_views,
        ] {
            for view in views.iter_mut() {
                if view.view.empty_by_nature() {
                    let mut cx = self.runtime.ctx(&view.module);
                    view.view.poll(&mut cx);
                }
            }
        }
        let total = self.bench_views.len();
        for (index, view) in self.bench_views.iter().enumerate() {
            if !view.view.empty_by_nature() {
                continue;
            }
            let has = view.view.has_content();
            let prev = self
                .known_tab_content
                .insert(view.id.clone(), has)
                .unwrap_or(false);
            if has && !prev && !self.tabs.was_closed(index) {
                self.tabs.show(index, total);
            }
        }
    }

    /// Renders icon tabs for a side panel (`sidebar` or `right`), hiding empty-by-nature
    /// views that currently have no content. Returns the active full-slot index.
    pub(crate) fn panel_icon_strip(&mut self, ui: &mut egui::Ui, sidebar: bool) -> usize {
        let (views, selected_slot, id_salt) = if sidebar {
            (&self.sidebar_views, self.sidebar_tab, "forja.sidebar.tabs")
        } else {
            (&self.right_views, self.right_tab, "forja.right.tabs")
        };

        let shown_indices: Vec<usize> = views
            .iter()
            .enumerate()
            .filter(|(_, view)| !view.view.empty_by_nature() || view.view.has_content())
            .map(|(index, _)| index)
            .collect();
        if shown_indices.is_empty() {
            return selected_slot;
        }

        let titles: Vec<String> = shown_indices
            .iter()
            .map(|&idx| self.text(&views[idx].title_key))
            .collect();
        let tooltips: Vec<String> = shown_indices
            .iter()
            .zip(&titles)
            .map(|(&idx, title)| self.view_tooltip(&views[idx].id, title))
            .collect();
        let tabs: Vec<PanelIconTab<'_>> = shown_indices
            .iter()
            .enumerate()
            .map(|(pos, &idx)| PanelIconTab {
                icon: view_icon(&views[idx]),
                label: &titles[pos],
                tooltip: &tooltips[pos],
            })
            .collect();

        let mut chosen = shown_indices
            .iter()
            .position(|&idx| idx == selected_slot)
            .unwrap_or(0);
        let runtime = &self.runtime;
        forja_ui_kit::panel_icon_tabs(ui, id_salt, &mut chosen, &tabs, |count| {
            let count_str = count.to_string();
            runtime
                .i18n()
                .t_args("shell.tabs.overflow", &[("count", &count_str)])
        });
        ui.add_space(6.0);
        shown_indices
            .get(chosen)
            .copied()
            .unwrap_or(shown_indices[0])
    }

    /// Renders the central bench tab bar (`tab_strip` + tool-tab `+` menu).
    pub(crate) fn bench_tab_strip(&mut self, ui: &mut egui::Ui, focus_active: bool) {
        let total = self.bench_views.len();
        let visible = self.tabs.visible(total);
        let titles: Vec<String> = visible
            .iter()
            .map(|&index| self.text(&self.bench_views[index].title_key))
            .collect();
        let tooltips: Vec<String> = visible
            .iter()
            .zip(&titles)
            .map(|(&index, title)| self.view_tooltip(&self.bench_views[index].id, title))
            .collect();
        let close_labels: Vec<String> = titles
            .iter()
            .map(|title| self.text_args("shell.tabs.close", &[("title", title)]))
            .collect();

        let items: Vec<TabItem<'_>> = visible
            .iter()
            .enumerate()
            .map(|(pos, &index)| TabItem {
                icon: view_icon(&self.bench_views[index]),
                label: &titles[pos],
                close_label: Some(&close_labels[pos]),
                tooltip: Some(&tooltips[pos]),
            })
            .collect();

        // Available tool tabs listed by "+": all currently hidden bench views, or all
        // views if every tab is already open so clicking "+" still lets the user jump.
        let hidden_views: Vec<usize> = (0..total)
            .filter(|index| !self.tabs.is_visible(*index, total))
            .collect();
        let add_targets = if hidden_views.is_empty() {
            (0..total).collect()
        } else {
            hidden_views
        };
        let add_titles: Vec<String> = add_targets
            .iter()
            .map(|&idx| self.text(&self.bench_views[idx].title_key))
            .collect();
        let add_items: Vec<MenuItem<'_>> = add_titles
            .iter()
            .map(|label| MenuItem {
                label,
                danger: false,
            })
            .collect();

        let active_pos = visible
            .iter()
            .position(|&index| index == self.active_tab)
            .unwrap_or(0);
        let middle_closes = self.setting_bool("appearance.tabs.middle_click", true);
        let add_label = self.text("shell.tabs.add");
        let runtime = &self.runtime;

        let outcome = forja_ui_kit::tab_strip(
            ui,
            "forja.bench.tabs",
            &items,
            active_pos,
            middle_closes,
            |count| {
                let count_str = count.to_string();
                runtime
                    .i18n()
                    .t_args("shell.tabs.overflow", &[("count", &count_str)])
            },
            &add_label,
            &add_items,
            focus_active,
        );

        if let Some(pos) = outcome.selected.or(outcome.overflow_clicked) {
            if let Some(&view_idx) = visible.get(pos) {
                self.active_tab = view_idx;
            }
        }
        if let Some(pos) = outcome.closed {
            if let Some(&closed_idx) = visible.get(pos) {
                let next = self.tabs.close(closed_idx, total);
                if closed_idx == self.active_tab {
                    if let Some(next_idx) = next {
                        self.active_tab = next_idx;
                    }
                }
            }
        }
        if let Some(menu_idx) = outcome.add_clicked {
            if let Some(&view_idx) = add_targets.get(menu_idx) {
                self.tabs.show(view_idx, total);
                self.active_tab = view_idx;
            }
        }
        if let Some(next) = self.tabs.clamp(self.active_tab, total) {
            self.active_tab = next;
        }
    }
}
