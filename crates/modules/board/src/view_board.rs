use crate::state::{Shared, StageColumn};
use crate::view_shared::{
    apply_stage_move, c32, create_dialog, dot_color, helper_status_key, move_card,
    render_helper_offer, save_prefs, stage_menu, stage_parent, visible_cards, CardOptions,
};
use forja_contracts::BenchCard;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::{json, Map};

pub(crate) fn render_board(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let mut prefs = state.prefs();
    let cards = state.cards();
    let stages = state.stages();
    let names = state.names();
    let error = state.error();
    let dragging = state.dragging();
    let pending = state.pending();
    let helper_offer = state.helper_offer();

    let wip_limit = cx
        .setting("bench.stage.wip_limit")
        .and_then(SettingValue::as_i64)
        .unwrap_or(0);
    let wip = cards
        .iter()
        .filter(|card| !card.archived && stage_parent(&stages, &card.stage) == "forging")
        .count();
    let wip_note = (wip_limit > 0 && wip as i64 > wip_limit).then(|| {
        cx.t_args(
            "board.column.wip",
            &[
                ("count", &wip.to_string()),
                ("limit", &wip_limit.to_string()),
            ],
        )
    });
    let title = cx.t("board.page.title");
    let refresh = cx.t("board.page-state.refresh");
    let create = cx.t("board.action.create");
    let mut spec = forja_ui_kit::PageSpec::new("board.page", &title)
        .wide()
        .primary(&create)
        .refresh(&refresh)
        .footer();
    spec.subtitle = wip_note.as_deref();
    let mut page = forja_ui_kit::page(ui, spec);
    if page.response.refresh_clicked {
        state.request_refresh();
    }
    if page.response.primary_clicked {
        create_dialog(cx, state.active_project().as_deref());
    }
    page.filters(|ui| {
        ui.horizontal_wrapped(|ui| {
            let width = ui.available_width().min(240.0);
            let mut filter = prefs.filter.clone();
            ui.scope(|ui| {
                ui.set_max_width(width);
                forja_ui_kit::text_field(ui, &mut filter, &cx.t("board.filter.hint"));
            });
            if filter != prefs.filter {
                let new_filter = filter.clone();
                prefs.filter = new_filter.clone();
                save_prefs(cx, state, move |prefs| prefs.filter = new_filter);
            }
            let mut archived = prefs.show_archived;
            if forja_ui_kit::toggle(ui, &mut archived, &cx.t("board.show-archived")).changed() {
                prefs.show_archived = archived;
                save_prefs(cx, state, |prefs| prefs.show_archived = archived);
            }
        });
    });
    page.body(|ui| {
        if let Some(error) = &error {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Error,
                &cx.t("board.page-state.error"),
                Some(error),
            );
        }
        if let Some(pending) = &pending {
            let frame = egui::Frame::new()
                .fill(c32(tokens.color.surface_alt))
                .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.spark)))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(8, 6));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    forja_ui_kit::text(ui, 
                        egui::RichText::new(cx.t_args(
                            "board.drag.backward",
                            &[("name", &pending.bench_name), ("stage", &pending.to_label)],
                        ))
                        .color(c32(tokens.color.text)),
                    );
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Primary,
                        &cx.t("board.drag.confirm"),
                    )
                    .clicked()
                    {
                        if let Err(error) = apply_stage_move(
                            cx,
                            state,
                            &pending.bench_id,
                            &pending.bench_name,
                            &pending.bench_path,
                            &pending.to,
                            &stages,
                            true,
                        ) {
                            state.set_error(Some(error));
                        }
                        state.set_pending(None);
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("board.drag.cancel"),
                    )
                    .clicked()
                    {
                        state.set_pending(None);
                    }
                });
            });
        }
        if let Some(offer) = &helper_offer {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !egui::Popup::is_any_open(ui.ctx())
            {
                state.set_helper_offer(None);
            } else {
                render_helper_offer(ui, cx, state, &tokens, offer);
            }
        }

        let visible = visible_cards(&cards, &prefs, &names);
        if !stages.is_empty() && cards.is_empty() {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Empty,
                &cx.t("board.page-state.empty"),
                Some(&cx.t("board.empty")),
            );
            ui.add_space(forja_ui_kit::Spacing::SM);
        }
        let mut column_rects: Vec<(String, egui::Rect)> = Vec::new();
        let options = CardOptions::from_ctx(cx);
        egui::ScrollArea::horizontal()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    for stage in &stages {
                        let cards: Vec<&BenchCard> = visible
                            .iter()
                            .copied()
                            .filter(|card| card.stage == stage.id)
                            .collect();
                        let height = ui.available_height().max(160.0);
                        let inner = ui.allocate_ui_with_layout(
                            egui::vec2(176.0, height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(176.0);
                                render_column(
                                    ui,
                                    cx,
                                    state,
                                    stage,
                                    &cards,
                                    &tokens,
                                    &options,
                                    dragging.as_deref(),
                                    &stages,
                                );
                            },
                        );
                        column_rects.push((stage.id.clone(), inner.response.rect));
                    }
                });
            });

        if let Some(bench_id) = dragging {
            let released = ui.ctx().input(|input| input.pointer.any_released());
            if released {
                let pointer = ui.ctx().input(|input| input.pointer.interact_pos());
                let target = pointer.and_then(|pointer| {
                    column_rects
                        .iter()
                        .find(|(_, rect)| rect.contains(pointer))
                        .map(|(stage, _)| stage.clone())
                });
                if let Some(target) = target {
                    if let Some(card) = cards.iter().find(|card| card.bench.bench_id == bench_id) {
                        move_card(cx, state, card, &target, &stages);
                    }
                }
                state.set_dragging(None);
            }
        }
    });
    let count = cx.t_args(
        "board.count",
        &[(
            "count",
            &visible_cards(&cards, &prefs, &names).len().to_string(),
        )],
    );
    page.footer(Some(&count), |_| {});
}

#[allow(clippy::too_many_arguments)]
fn render_column(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    stage: &StageColumn,
    cards: &[&BenchCard],
    tokens: &forja_ui_kit::Tokens,
    options: &CardOptions,
    dragging: Option<&str>,
    stages: &[StageColumn],
) {
    let bounds = egui::Rect::from_min_size(ui.cursor().min, ui.available_size());
    ui.set_min_height(bounds.height());
    ui.painter()
        .rect_filled(bounds, 6, c32(tokens.color.surface_alt));
    let header_fill = if stage.parent == "forging" {
        c32(tokens.color.accent_soft)
    } else {
        c32(tokens.color.surface_alt)
    };
    let header = egui::Frame::new()
        .fill(header_fill)
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(8, 6));
    header.show(ui, |ui| {
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(dot.center(), 4.0, dot_color(tokens, &stage.dot));
            forja_ui_kit::text(ui, 
                egui::RichText::new(&stage.label)
                    .strong()
                    .color(c32(tokens.color.text)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cards.len().to_string())
                        .small()
                        .color(c32(tokens.color.text_muted)),
                );
            });
        });
    });
    ui.add_space(forja_ui_kit::Spacing::XS);
    egui::ScrollArea::vertical()
        .id_salt(("board.column", &stage.id))
        .auto_shrink([false, false])
        .max_height(ui.available_height())
        .show(ui, |ui| {
            for card in cards {
                render_card(
                    ui,
                    cx,
                    state,
                    card,
                    &stage.dot,
                    &stage.label,
                    tokens,
                    options,
                    dragging == Some(card.bench.bench_id.as_str()),
                    stages,
                );
                ui.add_space(forja_ui_kit::Spacing::XS);
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn render_card(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    card: &BenchCard,
    stage_dot: &str,
    stage_label: &str,
    tokens: &forja_ui_kit::Tokens,
    options: &CardOptions,
    dragging: bool,
    stages: &[StageColumn],
) {
    let fill = if dragging {
        c32(tokens.color.accent_soft)
    } else {
        c32(tokens.color.surface)
    };
    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, c32(tokens.color.border)))
        .corner_radius(8)
        .inner_margin(egui::Margin::symmetric(8, 6));
    let mut body_top = 0.0;
    let inner = frame.show(ui, |ui| {
        ui.set_width(154.0);
        ui.horizontal(|ui| {
            if card.pinned {
                forja_ui_kit::icon(ui, forja_ui_kit::Icon::Pin, 16.0);
            }
            let title = ui.add_sized(
                egui::vec2(
                    (ui.available_width() - tokens.metrics().control - tokens.metrics().gap)
                        .max(32.0),
                    tokens.metrics().control,
                ),
                egui::Label::new(
                    egui::RichText::new(&card.bench.name)
                        .strong()
                        .color(c32(tokens.color.text)),
                )
                .truncate()
                .sense(egui::Sense::click()),
            );
            if title.on_hover_text(cx.t("board.action.open")).clicked() {
                let mut args = Map::new();
                args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
                if let Err(error) = cx.run_command("bench.activate", Args::from_map(args)) {
                    state.set_error(Some(error.to_string()));
                } else {
                    state.request_refresh();
                }
            }
            let actions = cx.t_args("board.card.actions", &[("name", &card.bench.name)]);
            let more = forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::More, &actions);
            egui::Popup::from_response(&more)
                .kind(egui::PopupKind::Menu)
                .layout(egui::Layout::top_down(egui::Align::Min))
                .open_memory(more.clicked().then_some(egui::SetOpenCommand::Toggle))
                .show(|ui| render_card_actions(ui, cx, state, card, stages));
        });
        body_top = ui.cursor().min.y;
        forja_ui_kit::text(ui, 
            forja_ui_kit::TextRole::Caption
                .rich(stage_label)
                .color(c32(tokens.color.text_muted)),
        );
        if !options.compact {
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                ui.painter()
                    .circle_filled(dot.center(), 3.0, dot_color(tokens, stage_dot));
                let mut detail = String::new();
                if options.show_branch {
                    if let Some(branch) = &card.bench.branch {
                        detail.push_str(branch);
                    }
                }
                if let Some(base) = &card.base_branch {
                    if !detail.is_empty() {
                        detail.push_str(" \u{00b7} ");
                    }
                    detail.push_str("base ");
                    detail.push_str(base);
                }
                if !detail.is_empty() {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(detail)
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        )
                        .truncate(),
                    );
                }
            });
            let helper = options
                .show_helper
                .then(|| state.helper_status(&card.bench.bench_id))
                .flatten();
            let request = options.show_request.then(|| card.request.clone()).flatten();
            if helper.is_some() || request.is_some() {
                ui.horizontal(|ui| {
                    if let Some(status) = &helper {
                        let text = cx.t(helper_status_key(status));
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(text)
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        );
                    }
                    if let Some(key) = &request {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(key)
                                .small()
                                .color(c32(tokens.color.text_muted)),
                        );
                    }
                });
            }
            if let Some(project) = card_project(cx, state, &card.bench.project_id) {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(project)
                            .small()
                            .color(c32(tokens.color.text_muted)),
                    )
                    .truncate(),
                );
            }
        }
    });
    let rect = inner.response.rect;
    if card.unread {
        ui.painter().circle_filled(
            egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
            3.0,
            c32(tokens.color.accent),
        );
    }
    if card.archived {
        let box_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 30.0, rect.top() + 7.0),
            egui::vec2(8.0, 8.0),
        );
        ui.painter().rect_stroke(
            box_rect,
            1,
            egui::Stroke::new(1.0_f32, c32(tokens.color.text_muted)),
            egui::StrokeKind::Inside,
        );
    }
    if card.sleeping {
        ui.painter().text(
            egui::pos2(rect.right() - 44.0, rect.top() + 4.0),
            egui::Align2::LEFT_TOP,
            "z",
            forja_ui_kit::TextRole::Caption.font(),
            c32(tokens.color.info),
        );
    }
    // The card body is draggable/clickable; never overlay the title's actions.
    let body_rect = egui::Rect::from_min_max(egui::pos2(rect.left(), body_top), rect.max);
    let response = ui.interact(
        body_rect,
        ui.id().with(("board.card", card.bench.bench_id.as_str())),
        egui::Sense::click_and_drag(),
    );
    // The stage color marks the card even when the helper status is unknown.
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 1.0, rect.top() + 6.0),
        egui::vec2(3.0, rect.height() - 12.0),
    );
    ui.painter()
        .rect_filled(bar, 2, dot_color(tokens, stage_dot));
    if response.hovered() {
        ui.painter().rect_stroke(
            rect,
            8,
            egui::Stroke::new(1.0_f32, c32(tokens.color.accent)),
            egui::StrokeKind::Inside,
        );
    }
    if response.clicked() {
        let mut args = Map::new();
        args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
        if let Err(error) = cx.run_command("bench.activate", Args::from_map(args)) {
            state.set_error(Some(error.to_string()));
        } else {
            state.request_refresh();
        }
    }
    if response.drag_started() {
        state.set_dragging(Some(card.bench.bench_id.clone()));
    }
    response.context_menu(|ui| render_card_actions(ui, cx, state, card, stages));
}

fn render_card_actions(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    card: &BenchCard,
    stages: &[StageColumn],
) {
    if forja_ui_kit::button(
        ui,
        forja_ui_kit::ButtonKind::Ghost,
        &cx.t("board.action.open"),
    )
    .clicked()
    {
        let mut args = Map::new();
        args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
        if let Err(error) = cx.run_command("bench.activate", Args::from_map(args)) {
            state.set_error(Some(error.to_string()));
        } else {
            state.request_refresh();
        }
        ui.close();
    }
    if forja_ui_kit::button(
        ui,
        forja_ui_kit::ButtonKind::Ghost,
        &cx.t("board.action.pin"),
    )
    .clicked()
    {
        let mut args = Map::new();
        args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
        let _ = cx.run_command("bench.pin", Args::from_map(args));
        state.request_refresh();
        ui.close();
    }
    if forja_ui_kit::button(
        ui,
        forja_ui_kit::ButtonKind::Ghost,
        &cx.t(if card.sleeping {
            "board.action.wake"
        } else {
            "board.action.sleep"
        }),
    )
    .clicked()
    {
        let mut args = Map::new();
        args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
        let _ = cx.run_command(
            if card.sleeping {
                "bench.wake"
            } else {
                "bench.sleep"
            },
            Args::from_map(args),
        );
        state.request_refresh();
        ui.close();
    }
    if !card.archived
        && forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Ghost,
            &cx.t("board.action.store"),
        )
        .clicked()
    {
        let mut args = Map::new();
        args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
        let _ = cx.run_command("bench.store_dialog", Args::from_map(args));
        ui.close();
    }
    ui.separator();
    stage_menu(ui, cx, state, card, stages);
}

fn card_project(cx: &mut Ctx<'_>, state: &Shared, project_id: &str) -> Option<String> {
    let _ = cx;
    state.names().get(project_id).cloned()
}
