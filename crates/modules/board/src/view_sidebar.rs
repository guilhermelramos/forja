use crate::state::{Shared, StageColumn};
use crate::view_shared::{
    c32, create_dialog, dot_color, grouped, save_prefs, stage_menu, visible_cards,
};
use forja_contracts::BenchCard;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map};
pub(crate) fn render_sidebar(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let prefs = state.prefs();
    let cards = state.cards();
    let stages = state.stages();
    let names = state.names();
    let error = state.error();

    // A aba "Bancadas" da barra lateral já diz o nome da lista: só as ações ficam aqui.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::Add, &cx.t("board.action.create"))
                .on_hover_text(cx.t("board.action.create"))
                .clicked()
            {
                create_dialog(cx, state.active_project().as_deref());
            }
            if forja_ui_kit::icon_button(
                ui,
                forja_ui_kit::Icon::Refresh,
                &cx.t("board.action.refresh"),
            )
            .on_hover_text(cx.t("board.action.refresh"))
            .clicked()
            {
                state.request_refresh();
            }
        });
    });
    ui.add_space(2.0);

    let mut filter = prefs.filter.clone();
    forja_ui_kit::text_field(ui, &mut filter, &cx.t("board.filter.hint"));
    if filter != prefs.filter {
        save_prefs(cx, state, |prefs| prefs.filter = filter);
    }

    ui.horizontal(|ui| {
        let mut show_archived = prefs.show_archived;
        if forja_ui_kit::toggle(ui, &mut show_archived, &cx.t("board.show-archived")).changed() {
            save_prefs(cx, state, |prefs| prefs.show_archived = show_archived);
        }
    });
    ui.horizontal(|ui| {
        let mut pinned = prefs.pinned_first;
        if forja_ui_kit::toggle(ui, &mut pinned, &cx.t("board.pinned-first")).changed() {
            save_prefs(cx, state, |prefs| prefs.pinned_first = pinned);
        }
    });
    ui.horizontal(|ui| {
        let groups = [
            ("project", cx.t("board.group.project")),
            ("stage", cx.t("board.group.stage")),
            ("none", cx.t("board.group.none")),
        ];
        let selected = groups
            .iter()
            .position(|(id, _)| *id == prefs.group)
            .unwrap_or(0);
        let labels: Vec<&str> = groups.iter().map(|(_, label)| label.as_str()).collect();
        // `select` não marca `changed()`: comparamos o índice antes e depois.
        let mut index = selected;
        if forja_ui_kit::select(ui, &mut index, &labels).changed() {
            let id = groups[index].0;
            save_prefs(cx, state, |prefs| prefs.group = id.to_owned());
        }
        let sorts = [
            ("recent", cx.t("board.sort.recent")),
            ("name", cx.t("board.sort.name")),
            ("stage", cx.t("board.sort.stage")),
        ];
        let selected = sorts
            .iter()
            .position(|(id, _)| *id == prefs.sort)
            .unwrap_or(0);
        let labels: Vec<&str> = sorts.iter().map(|(_, label)| label.as_str()).collect();
        let mut index = selected;
        if forja_ui_kit::select(ui, &mut index, &labels).changed() {
            let id = sorts[index].0;
            save_prefs(cx, state, |prefs| prefs.sort = id.to_owned());
        }
    });
    ui.separator();

    if let Some(error) = &error {
        ui.label(
            egui::RichText::new(error)
                .small()
                .color(c32(tokens.color.danger)),
        );
    }

    let visible = visible_cards(&cards, &prefs, &names);
    if visible.is_empty() {
        forja_ui_kit::empty_state(ui, forja_ui_kit::BOARD, &cx.t("board.sidebar.empty"), None);
        return;
    }
    let groups = grouped(&visible, &prefs, &stages, &names);
    for (group, cards) in groups {
        if !group.is_empty() {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(group)
                    .small()
                    .color(c32(tokens.color.text_muted)),
            );
        }
        for card in cards {
            render_sidebar_row(ui, cx, state, card, &tokens, &stages);
        }
    }
}

fn render_sidebar_row(
    ui: &mut egui::Ui,
    cx: &mut Ctx<'_>,
    state: &Shared,
    card: &BenchCard,
    tokens: &forja_ui_kit::Tokens,
    stages: &[StageColumn],
) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 34.0), egui::Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, 4, c32(tokens.color.surface_alt));
    }
    let mut x = rect.left() + 6.0;
    if card.pinned {
        forja_ui_kit::paint_icon(
            painter,
            forja_ui_kit::Icon::Pin,
            egui::Rect::from_min_size(egui::pos2(x, rect.top() + 4.0), egui::vec2(12.0, 12.0)),
            c32(tokens.color.spark),
        );
    }
    x += 12.0;
    let stage = stages.iter().find(|stage| stage.id == card.stage);
    painter.circle_filled(
        egui::pos2(x + 4.0, rect.center().y),
        4.0,
        dot_color(
            tokens,
            stage
                .map(|stage| stage.dot.as_str())
                .unwrap_or("text_muted"),
        ),
    );
    x += 16.0;
    let clip = egui::Rect::from_min_max(
        egui::pos2(x, rect.top() + 2.0),
        egui::pos2(rect.right() - 24.0, rect.bottom() - 2.0),
    );
    let text_painter = painter.with_clip_rect(clip);
    text_painter.text(
        egui::pos2(x, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        &card.bench.name,
        egui::FontId::proportional(12.5),
        c32(tokens.color.text),
    );
    if let Some(branch) = &card.bench.branch {
        text_painter.text(
            egui::pos2(x, rect.top() + 17.0),
            egui::Align2::LEFT_TOP,
            branch,
            egui::FontId::proportional(10.5),
            c32(tokens.color.text_muted),
        );
    }
    let mut mark_x = rect.right() - 8.0;
    if card.unread {
        painter.circle_filled(
            egui::pos2(mark_x, rect.center().y),
            3.0,
            c32(tokens.color.accent),
        );
    }
    mark_x -= 14.0;
    if card.sleeping {
        painter.text(
            egui::pos2(mark_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            "z",
            egui::FontId::proportional(11.0),
            c32(tokens.color.info),
        );
    }
    if card.archived {
        let box_rect = egui::Rect::from_center_size(
            egui::pos2(mark_x - 14.0, rect.center().y),
            egui::vec2(8.0, 8.0),
        );
        painter.rect_stroke(
            box_rect,
            1,
            egui::Stroke::new(1.0_f32, c32(tokens.color.text_muted)),
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
    response.context_menu(|ui| {
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
            &cx.t("board.action.read"),
        )
        .clicked()
        {
            let mut args = Map::new();
            args.insert("bench_id".to_owned(), json!(card.bench.bench_id));
            args.insert("unread".to_owned(), json!(!card.unread));
            let _ = cx.run_command("bench.read", Args::from_map(args));
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
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Ghost,
            &cx.t("board.action.approve"),
        )
        .clicked()
        {
            let mut args = Map::new();
            args.insert("project_id".to_owned(), json!(card.bench.project_id));
            let _ = cx.run_command("bench.approve_dialog", Args::from_map(args));
            ui.close();
        }
        ui.separator();
        stage_menu(ui, cx, state, card, stages);
    });
}
