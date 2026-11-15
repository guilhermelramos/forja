//! Quadro (página do trilho) e a lateral de bancadas.

use crate::state::{Shared, StageColumn};
use crate::view_board::render_board;
use crate::view_shared::c32;
use crate::view_sidebar::render_sidebar;
use forja_contracts::BenchCard;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub struct BoardSidebar {
    state: Shared,
}

impl BoardSidebar {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for BoardSidebar {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        refresh_if_needed(cx, &self.state);
        render_sidebar(ui, cx, &self.state);
    }
}

pub struct BoardRail {
    state: Shared,
}

impl BoardRail {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for BoardRail {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if ui.available_width() >= 120.0 {
            refresh_if_needed(cx, &self.state);
            render_board(ui, cx, &self.state);
            return;
        }
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let unread = self
            .state
            .cards()
            .iter()
            .filter(|card| card.unread && !card.archived)
            .count();
        let label = if unread > 0 {
            format!("\u{25a6}\u{00b7}{unread}")
        } else {
            "\u{25a6}".to_owned()
        };
        let response = ui.add(
            egui::Button::new(egui::RichText::new(label).color(c32(tokens.color.text)))
                .frame(false),
        );
        if response.on_hover_text(cx.t("board.page.title")).clicked() {
            self.state.request_refresh();
        }
    }
}

fn refresh_if_needed(cx: &mut Ctx<'_>, state: &Shared) {
    state.ensure_loaded(cx);
    let interval = cx
        .setting("board.refresh.interval_s")
        .and_then(SettingValue::as_i64)
        .unwrap_or(15);
    if state.needs_refresh(interval) {
        refresh(cx, state);
    }
}

pub(crate) fn refresh(cx: &mut Ctx<'_>, state: &Shared) {
    let project_id = active_project_id(cx);
    state.set_active_project(project_id.clone());
    let mut args = Map::new();
    if let Some(ref id) = project_id {
        args.insert("project_id".to_owned(), json!(id));
    }
    args.insert("include_archived".to_owned(), json!(true));
    let mut cards = match cx.run_command("bench.list_cards", Args::from_map(args)) {
        Ok(value) => serde_json::from_value::<Vec<BenchCard>>(
            value.get("benches").cloned().unwrap_or(Value::Null),
        )
        .unwrap_or_default(),
        Err(_) => {
            state.set_error(Some(cx.t("board.error.benches")));
            state.set_data(Vec::new(), Vec::new(), BTreeMap::new(), BTreeMap::new());
            return;
        }
    };
    if let Some(ref id) = project_id {
        cards.retain(|card| card.bench.project_id == *id);
    }
    let names = project_names(cx);
    let stages = stage_columns(cx, &cards, project_id.as_deref());
    let helpers = helper_statuses(cx, &cards);
    state.set_data(cards, stages, names, helpers);
}

fn active_project_id(cx: &mut Ctx<'_>) -> Option<String> {
    let value = cx.run_command("project.current", Args::new()).ok()?;
    value
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Helper status per bench, from the latest helper run of each bench folder.
fn helper_statuses(cx: &mut Ctx<'_>, cards: &[BenchCard]) -> BTreeMap<String, String> {
    let shown = cx
        .setting("board.cards.show_helper")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    if !shown {
        return BTreeMap::new();
    }
    let runs = cx
        .run_command("helper.status", Args::new())
        .ok()
        .and_then(|value| value.get("runs").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    card_helper_statuses(cards, &runs)
}

/// Pairs each card with the status of the most recent run in its folder.
fn card_helper_statuses(cards: &[BenchCard], runs: &[Value]) -> BTreeMap<String, String> {
    let mut latest: BTreeMap<String, (String, String)> = BTreeMap::new();
    for run in runs {
        let path = run.get("bench_path").and_then(Value::as_str);
        let status = run.get("status").and_then(Value::as_str);
        let (Some(path), Some(status)) = (path, status) else {
            continue;
        };
        let started = run
            .get("started_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let key = path.to_lowercase();
        if latest.get(&key).is_none_or(|(seen, _)| started >= *seen) {
            latest.insert(key, (started, status.to_owned()));
        }
    }
    cards
        .iter()
        .filter_map(|card| {
            latest
                .get(&card.bench.path.to_lowercase())
                .map(|(_, status)| (card.bench.bench_id.clone(), status.clone()))
        })
        .collect()
}

fn project_names(cx: &mut Ctx<'_>) -> BTreeMap<String, String> {
    let Ok(value) = cx.run_command("project.list", Args::new()) else {
        return BTreeMap::new();
    };
    let projects = value
        .get("projects")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    projects
        .into_iter()
        .filter_map(|project| {
            let id = project.get("id")?.as_str()?.to_owned();
            let name = project.get("name")?.as_str()?.to_owned();
            Some((id, name))
        })
        .collect()
}

fn stage_columns(
    cx: &mut Ctx<'_>,
    cards: &[BenchCard],
    active_project: Option<&str>,
) -> Vec<StageColumn> {
    let mut projects: Vec<String> = cards
        .iter()
        .map(|card| card.bench.project_id.clone())
        .collect();
    projects.sort();
    projects.dedup();
    let mut columns: BTreeMap<String, StageColumn> = BTreeMap::new();
    if projects.is_empty() {
        if let Some(stages) = command_stages(cx, active_project) {
            for stage in stages {
                columns.entry(stage.id.clone()).or_insert(stage);
            }
        }
    } else {
        for project in projects {
            if let Some(stages) = command_stages(cx, Some(&project)) {
                for stage in stages {
                    columns.entry(stage.id.clone()).or_insert(stage);
                }
            }
        }
    }
    let mut columns: Vec<StageColumn> = columns.into_values().collect();
    columns.sort_by_key(|column| column.order);
    columns
}

fn command_stages(cx: &mut Ctx<'_>, project_id: Option<&str>) -> Option<Vec<StageColumn>> {
    let mut args = Map::new();
    if let Some(project_id) = project_id {
        args.insert("project_id".to_owned(), json!(project_id));
    }
    let value = cx
        .run_command("bench.stage.list", Args::from_map(args))
        .ok()?;
    let stages = value.get("stages")?.as_array()?.clone();
    Some(
        stages
            .into_iter()
            .filter_map(|stage| {
                Some(StageColumn {
                    id: stage.get("id")?.as_str()?.to_owned(),
                    label: stage.get("label")?.as_str()?.to_owned(),
                    order: stage.get("order")?.as_i64()? as i32,
                    parent: stage.get("parent")?.as_str()?.to_owned(),
                    dot: stage
                        .get("dot")?
                        .as_str()
                        .unwrap_or("text_muted")
                        .to_owned(),
                })
            })
            .collect(),
    )
}

#[cfg(test)]
mod helper_status_tests {
    use super::*;
    use forja_contracts::BenchRef;

    fn card(id: &str, path: &str) -> BenchCard {
        BenchCard {
            bench: BenchRef {
                project_id: "p".to_owned(),
                bench_id: id.to_owned(),
                name: id.to_owned(),
                path: path.to_owned(),
                branch: None,
            },
            is_main: false,
            stage: "draft".to_owned(),
            archived: false,
            pinned: false,
            unread: false,
            sleeping: false,
            base_branch: None,
            created_at: 0,
            activated_at: 0,
            request: None,
        }
    }

    #[test]
    fn latest_run_of_a_folder_wins_and_matches_case_insensitively() {
        let cards = vec![card("a", "C:/bancadas/A"), card("b", "C:/bancadas/B")];
        let runs = vec![
            json!({"bench_path": "c:/bancadas/a", "status": "working", "started_at": "2026-11-13T10:00:00Z"}),
            json!({"bench_path": "C:/bancadas/a", "status": "needs_you", "started_at": "2026-11-13T11:00:00Z"}),
            json!({"bench_path": "C:/bancadas/x", "status": "idle", "started_at": "2026-11-13T12:00:00Z"}),
        ];
        let statuses = card_helper_statuses(&cards, &runs);
        assert_eq!(statuses.get("a").map(String::as_str), Some("needs_you"));
        assert!(!statuses.contains_key("b"));
        assert_eq!(statuses.len(), 1);
    }

    #[test]
    fn malformed_runs_are_ignored() {
        let cards = vec![card("a", "C:/bancadas/a")];
        let runs = vec![json!({"status": "working"}), json!({"bench_path": "C:/bancadas/a"})];
        assert!(card_helper_statuses(&cards, &runs).is_empty());
    }
}
