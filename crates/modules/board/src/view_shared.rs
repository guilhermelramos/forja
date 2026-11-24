use crate::state::{HelperCandidate, HelperOffer, PendingMove, Prefs, Shared, StageColumn};
use forja_contracts::BenchCard;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map, Value};
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
            bench_path: card.bench.path.clone(),
            to: target.to_owned(),
            to_label: label,
        }));
        return;
    }
    if let Err(error) = apply_stage_move(
        cx,
        state,
        &card.bench.bench_id,
        &card.bench.name,
        &card.bench.path,
        target,
        stages,
        false,
    ) {
        state.set_error(Some(error));
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_stage_move(
    cx: &mut Ctx<'_>,
    state: &Shared,
    bench_id: &str,
    bench_name: &str,
    bench_path: &str,
    target: &str,
    stages: &[StageColumn],
    allow_backward: bool,
) -> Result<(), String> {
    set_stage(cx, bench_id, target, allow_backward)?;
    maybe_offer_helper_on_forging(cx, state, bench_id, bench_name, bench_path, target, stages);
    state.request_refresh();
    Ok(())
}

pub(crate) fn is_forging_stage(stages: &[StageColumn], target: &str) -> bool {
    target == "forging" || stage_parent(stages, target) == "forging"
}

fn maybe_offer_helper_on_forging(
    cx: &mut Ctx<'_>,
    state: &Shared,
    bench_id: &str,
    bench_name: &str,
    bench_path: &str,
    target: &str,
    stages: &[StageColumn],
) {
    if !is_forging_stage(stages, target) || !flag(cx, "board.stage.offer_helper", true) {
        state.set_helper_offer(None);
        return;
    }
    if matches!(
        state.helper_status(bench_id).as_deref(),
        Some("working" | "needs_you")
    ) {
        state.set_helper_offer(None);
        return;
    }
    let Ok(list) = cx.run_command("helper.list", Args::new()) else {
        state.set_helper_offer(None);
        return;
    };
    state.set_helper_offer(Some(offer_from_helper_list(
        bench_id, bench_name, bench_path, &list,
    )));
}

pub(crate) fn offer_from_helper_list(
    bench_id: &str,
    bench_name: &str,
    bench_path: &str,
    list: &Value,
) -> HelperOffer {
    let profiles = list
        .get("profiles")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let installed = profiles
        .iter()
        .filter(|profile| profile.get("installed").and_then(Value::as_bool).unwrap_or(false))
        .filter_map(|profile| {
            let id = profile.get("id").and_then(Value::as_str)?.trim();
            if id.is_empty() {
                return None;
            }
            let name = profile
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(id)
                .to_owned();
            let is_default = profile
                .get("default")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Some((is_default, HelperCandidate { id: id.to_owned(), name }))
        });

    let mut fallback_installed = None;
    let mut default_profile = None;
    for (is_default, candidate) in installed {
        if is_default {
            default_profile = Some(candidate);
            break;
        }
        if fallback_installed.is_none() {
            fallback_installed = Some(candidate);
        }
    }
    let default_profile = default_profile.or(fallback_installed);

    let install_hint = if default_profile.is_none() {
        profiles
            .iter()
            .filter(|profile| {
                profile
                    .get("default")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .chain(profiles.iter())
            .filter_map(|profile| profile.get("install_hint").and_then(Value::as_str))
            .map(str::trim)
            .find(|hint| !hint.is_empty())
            .map(str::to_owned)
    } else {
        None
    };

    HelperOffer {
        bench_id: bench_id.to_owned(),
        bench_name: bench_name.to_owned(),
        bench_path: bench_path.to_owned(),
        default_profile,
        install_hint,
        error: None,
    }
}

pub(crate) fn launch_offered_helper(
    cx: &mut Ctx<'_>,
    state: &Shared,
    offer: &HelperOffer,
) {
    let Some(profile) = offer.default_profile.as_ref() else {
        return;
    };
    let mut args = Args::new();
    args.insert("profile_id", json!(profile.id));
    args.insert("bench_path", json!(offer.bench_path));
    match cx.run_command("helper.launch", args) {
        Ok(_) => {
            state.set_helper_offer(None);
            state.request_refresh();
        }
        Err(error) => {
            let detail = match error {
                forja_kernel::KernelError::Command { message, .. } => message,
                other => other.to_string(),
            };
            state.set_helper_offer_error(Some(cx.t_args(
                "board.forging-offer.error",
                &[("helper", &profile.name), ("detail", &detail)],
            )));
        }
    }
}

pub(crate) fn open_helpers_tab(cx: &mut Ctx<'_>, state: &Shared) {
    let mut args = Args::new();
    args.insert("view_id", json!("helper.tab"));
    let _ = cx.run_command("shell.rail.open", args);
    state.set_helper_offer(None);
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
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &label).clicked() {
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
    color.to_color32()
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
        "error" => "board.card.helper.error",
        _ => "board.card.helper.unknown",
    }
}

pub(crate) fn render_helper_offer(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    tokens: &forja_ui_kit::Tokens,
    offer: &HelperOffer,
) {
    let frame = egui::Frame::new()
        .fill(c32(tokens.color.surface_alt))
        .stroke(egui::Stroke::new(
            1.0_f32,
            c32(if offer.default_profile.is_some() {
                tokens.color.accent
            } else {
                tokens.color.spark
            }),
        ))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6));

    frame.show(ui, |ui| {
        if let Some(profile) = &offer.default_profile {
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t_args(
                        "board.forging-offer.body",
                        &[("bench", &offer.bench_name), ("helper", &profile.name)],
                    ))
                    .color(c32(tokens.color.text)),
                );
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Primary,
                    &cx.t_args("board.forging-offer.start", &[("helper", &profile.name)]),
                )
                .clicked()
                {
                    launch_offered_helper(cx, state, offer);
                }
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &cx.t("board.forging-offer.dismiss"),
                )
                .clicked()
                {
                    state.set_helper_offer(None);
                }
            });
            if let Some(error) = &offer.error {
                forja_ui_kit::text(ui,
                    egui::RichText::new(error)
                        .small()
                        .color(c32(tokens.color.danger)),
                );
            }
        } else {
            let headline = cx.t_args(
                "board.forging-offer.missing-title",
                &[("bench", &offer.bench_name)],
            );
            let body = cx.t("board.forging-offer.missing-body");
            let mut copied = state.helper_missing_copied();
            let copy_label = if copied {
                cx.t("board.forging-offer.copied")
            } else {
                cx.t("board.forging-offer.copy")
            };
            let open_helpers_label = cx
                .registry()
                .command("shell.rail.open")
                .is_some()
                .then(|| cx.t("board.forging-offer.open-helpers"));
            let guide = forja_ui_kit::HelperMissingGuide {
                headline: &headline,
                body: &body,
                install_command: offer.install_hint.as_deref(),
                copy_label: &copy_label,
                open_helpers_label: open_helpers_label.as_deref(),
            };
            let open_clicked = forja_ui_kit::helper_missing_guide(ui, &guide, &mut copied);
            state.set_helper_missing_copied(copied);
            if open_clicked {
                open_helpers_tab(cx, state);
            }
            ui.add_space(forja_ui_kit::Spacing::XS);
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("board.forging-offer.close"),
            )
            .clicked()
            {
                state.set_helper_offer(None);
            }
        }
    });
}
