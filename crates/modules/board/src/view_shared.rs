use crate::state::{PendingMove, Prefs, Shared, StageColumn};
use forja_contracts::BenchCard;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map};
use std::collections::BTreeMap;
pub(crate) fn visible_cards<'a>(
    cards: &'a [BenchCard],
    prefs: &Prefs,
    names: &BTreeMap<String, String>,
) -> Vec<&'a BenchCard> {
    let filter = prefs.filter.trim().to_lowercase();
    let mut selected: Vec<&BenchCard> = cards
        .iter()
        .filter(|card| prefs.show_archived || !card.archived)
        .filter(|card| {
            if filter.is_empty() {
                return true;
            }
            let project = names
                .get(&card.bench.project_id)
                .map(String::as_str)
                .unwrap_or("");
            card.bench.name.to_lowercase().contains(&filter)
                || card
                    .bench
                    .branch
                    .as_deref()
                    .map(|branch| branch.to_lowercase().contains(&filter))
                    .unwrap_or(false)
                || project.to_lowercase().contains(&filter)
        })
        .collect();
    selected.sort_by(|a, b| {
        if prefs.pinned_first && a.pinned != b.pinned {
            return b.pinned.cmp(&a.pinned);
        }
        match prefs.sort.as_str() {
            "name" => a
                .bench
                .name
                .to_lowercase()
                .cmp(&b.bench.name.to_lowercase()),
            "stage" => a.stage.cmp(&b.stage).then_with(|| {
                a.bench
                    .name
                    .to_lowercase()
                    .cmp(&b.bench.name.to_lowercase())
            }),
            _ => b.activated_at.cmp(&a.activated_at),
        }
    });
    selected
}

pub(crate) fn grouped<'a>(
    cards: &[&'a BenchCard],
    prefs: &Prefs,
    stages: &[StageColumn],
    names: &BTreeMap<String, String>,
) -> Vec<(String, Vec<&'a BenchCard>)> {
    match prefs.group.as_str() {
        "stage" => {
            let mut groups: Vec<(String, Vec<&BenchCard>)> = Vec::new();
            for stage in stages {
                let cards: Vec<&BenchCard> = cards
                    .iter()
                    .copied()
                    .filter(|card| card.stage == stage.id)
                    .collect();
                if !cards.is_empty() {
                    groups.push((stage.label.clone(), cards));
                }
            }
            groups
        }
        "none" => vec![(String::new(), cards.to_vec())],
        _ => {
            let mut map: BTreeMap<String, Vec<&BenchCard>> = BTreeMap::new();
            for card in cards {
                let name = names
                    .get(&card.bench.project_id)
                    .cloned()
                    .unwrap_or_else(|| card.bench.project_id.clone());
                map.entry(name).or_default().push(*card);
            }
            map.into_iter().collect()
        }
    }
}

pub(crate) fn set_stage(
    cx: &mut Ctx<'_>,
    bench_id: &str,
    stage: &str,
    confirm: bool,
) -> Result<(), String> {
    let mut args = Map::new();
    args.insert("bench_id".to_owned(), json!(bench_id));
    args.insert("stage".to_owned(), json!(stage));
    args.insert("confirm_backward".to_owned(), json!(confirm));
    cx.run_command("bench.stage.set", Args::from_map(args))
        .map(|_| ())
        .map_err(|error| match error {
            forja_kernel::KernelError::Command { message, .. } => message,
            other => other.to_string(),
        })
}

/// Moves a card to another stage. Going back asks for confirmation first.
pub(crate) fn move_card(
    cx: &mut Ctx<'_>,
    state: &Shared,
    card: &BenchCard,
    target: &str,
    stages: &[StageColumn],
) {
    if target == card.stage {
        return;
    }
    let from_order = stage_order(stages, &card.stage);
    let to_order = stage_order(stages, target);
    if to_order < from_order {
        let label = stages
            .iter()
            .find(|stage| stage.id == target)
            .map(|stage| stage.label.clone())
            .unwrap_or_else(|| target.to_owned());
        state.set_pending(Some(PendingMove {
            bench_id: card.bench.bench_id.clone(),
            bench_name: card.bench.name.clone(),
            to: target.to_owned(),
            to_label: label,
        }));
        return;
    }
    match set_stage(cx, &card.bench.bench_id, target, false) {
        Ok(()) => state.request_refresh(),
        Err(error) => state.set_error(Some(error)),
    }
}

/// Menu items for every other stage of the card.
pub(crate) fn stage_menu(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    card: &BenchCard,
    stages: &[StageColumn],
) {
    let current = stage_order(stages, &card.stage);
    let mut chosen = None;
    for stage in stages.iter().filter(|stage| stage.id != card.stage) {
        let key = if stage.order < current {
            "board.action.move-back"
        } else {
            "board.action.move-to"
        };
        let label = cx.t_args(key, &[("stage", &stage.label)]);
        if ui.button(label).clicked() {
            chosen = Some(stage.id.clone());
            ui.close();
        }
    }
    if let Some(target) = chosen {
        move_card(cx, state, card, &target, stages);
    }
}

pub(crate) fn create_dialog(cx: &mut Ctx<'_>, project_id: Option<&str>) {
    let mut args = Map::new();
    if let Some(project_id) = project_id {
        args.insert("project_id".to_owned(), json!(project_id));
    }
    let _ = cx.run_command("bench.create_dialog", Args::from_map(args));
}

pub(crate) fn save_prefs(cx: &Ctx<'_>, state: &Shared, change: impl FnOnce(&mut Prefs)) {
    if let Err(error) = state.update_prefs(cx, change) {
        state.set_error(Some(error));
    }
    state.request_refresh();
}

pub(crate) fn flag(cx: &Ctx<'_>, key: &str, fallback: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(fallback)
}

pub(crate) fn stage_parent(stages: &[StageColumn], id: &str) -> String {
    stages
        .iter()
        .find(|stage| stage.id == id)
        .map(|stage| stage.parent.clone())
        .unwrap_or_else(|| id.to_owned())
}

pub(crate) fn stage_order(stages: &[StageColumn], id: &str) -> i32 {
    stages
        .iter()
        .find(|stage| stage.id == id)
        .map(|stage| stage.order)
        .unwrap_or(0)
}

pub(crate) fn dot_color(tokens: &forja_ui_kit::Tokens, dot: &str) -> egui::Color32 {
    c32(match dot {
        "accent" => tokens.color.accent,
        "spark" => tokens.color.spark,
        "ok" => tokens.color.ok,
        "info" => tokens.color.info,
        "danger" => tokens.color.danger,
        _ => tokens.color.text_muted,
    })
}

pub(crate) fn c32(color: forja_ui_kit::Color) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

/// Display switches of a board card, read from the `board.cards.*` settings.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CardOptions {
    pub show_branch: bool,
    pub compact: bool,
    pub show_helper: bool,
    pub show_request: bool,
}

impl CardOptions {
    pub(crate) fn from_ctx(cx: &Ctx<'_>) -> Self {
        Self {
            show_branch: flag(cx, "board.cards.show_branch", true),
            compact: flag(cx, "board.cards.compact", false),
            show_helper: flag(cx, "board.cards.show_helper", true),
            show_request: flag(cx, "board.cards.show_request", true),
        }
    }
}

/// i18n key of the helper state shown on a card. `status` is the snake_case
/// `HelperStatus` value that `helper.status` reports.
pub(crate) fn helper_status_key(status: &str) -> &'static str {
    match status {
        "needs_you" => "board.card.helper.needs-you",
        "working" => "board.card.helper.working",
        "idle" => "board.card.helper.idle",
        "finished" => "board.card.helper.finished",
        _ => "board.card.helper.unknown",
    }
}
