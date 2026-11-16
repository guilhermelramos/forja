//! Faixa de abas com ícones, medição antes do desenho, fechamento e fileira de ícones laterais.

use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, Response, Sense, Stroke, Ui};

use crate::apply::radius_u8;
use crate::paint::{self, body_font, caption_font, painter_for};
use crate::tokens::{Metrics, Tokens};

use super::menu::{popup_menu, MenuItem};
use super::tabs_fit::{measure_overflow_width, measure_tab_width, plan_tab_fit, CLOSE_RESERVE};

const TAB_GAP: f32 = 4.0;
const CLOSE_SIZE: f32 = 14.0;

/// Entrada de uma aba na faixa principal com ícone e botão de fechar opcional.
pub struct TabItem<'a> {
    pub icon: &'a str,
    pub label: &'a str,
    pub close_label: Option<&'a str>,
    pub tooltip: Option<&'a str>,
}

/// Entrada de uma aba de ícone nos painéis laterais.
pub struct PanelIconTab<'a> {
    pub icon: &'a str,
    pub label: &'a str,
    pub tooltip: &'a str,
}

/// Resultado da interação em [`tab_strip`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabStripOutcome {
    pub selected: Option<usize>,
    pub closed: Option<usize>,
    pub overflow_clicked: Option<usize>,
    pub add_clicked: Option<usize>,
    pub active_tab_id: Option<egui::Id>,
}

/// Desenha a faixa de abas central com ícones, fechar no hover, excedente à direita e botão "+".
#[allow(clippy::too_many_arguments)]
pub fn tab_strip(
    ui: &mut Ui,
    id_salt: &str,
    items: &[TabItem<'_>],
    active: usize,
    middle_click_closes: bool,
    overflow_label_for: impl Fn(usize) -> String,
    add_label: &str,
    add_items: &[MenuItem<'_>],
    focus_active: bool,
) -> TabStripOutcome {
    let mut outcome = TabStripOutcome::default();
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let row_h = metrics.control;
    let add_w = metrics.control;
    let avail = ui.available_width().max(add_w);

    let measured: Vec<f32> = items
        .iter()
        .map(|item| {
            measure_tab_width(
                ui,
                item.icon,
                item.label,
                item.close_label.is_some(),
                &metrics,
            )
        })
        .collect();
    let plan = plan_tab_fit(
        &measured,
        active,
        avail,
        TAB_GAP,
        |hidden_count| measure_overflow_width(ui, hidden_count, &metrics),
        add_w,
    );

    let trailing_needed = add_w
        + if plan.hidden.is_empty() {
            0.0
        } else {
            TAB_GAP + measure_overflow_width(ui, plan.hidden.len(), &metrics)
        };
    let tabs_budget = (avail - trailing_needed - TAB_GAP).max(metrics.control);

    ui.push_id(id_salt, |ui| {
        draw_strip_baseline(ui, avail, row_h, &tokens);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TAB_GAP;
            for &index in &plan.shown {
                let item = &items[index];
                let width = if plan.shown.len() == 1 {
                    measured[index].min(tabs_budget).max(metrics.control)
                } else {
                    measured[index]
                };
                let is_active = index == active;
                let (tab_resp, close_clicked, middle_clicked) =
                    draw_tab_chip(ui, &tokens, &metrics, item, index, width, row_h, is_active);
                if is_active {
                    outcome.active_tab_id = Some(tab_resp.id);
                    if focus_active {
                        ui.ctx().memory_mut(|mem| mem.request_focus(tab_resp.id));
                    }
                }
                let key_activate = tab_resp.has_focus()
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                    });
                if close_clicked || (middle_click_closes && middle_clicked) {
                    outcome.closed = Some(index);
                } else if tab_resp.clicked() || key_activate {
                    outcome.selected = Some(index);
                    tab_resp.request_focus();
                }
            }

            outcome.overflow_clicked = draw_strip_overflow(
                ui,
                &tokens,
                &metrics,
                &plan.hidden,
                |idx| items[idx].label,
                &overflow_label_for,
            );

            let add_resp = draw_add_button(ui, &tokens, &metrics, add_label, !add_items.is_empty());
            if !add_items.is_empty() {
                if let Some(menu_idx) = popup_menu(&add_resp, add_items) {
                    outcome.add_clicked = Some(menu_idx);
                }
            }
        });
    });
    outcome
}

fn draw_strip_baseline(ui: &Ui, avail: f32, height: f32, tokens: &Tokens) {
    let strip_rect = Rect::from_min_size(ui.cursor().min, egui::vec2(avail, height));
    painter_for(ui).hline(
        strip_rect.x_range(),
        strip_rect.bottom(),
        Stroke::new(1.0, tokens.color.border.to_color32()),
    );
}

fn draw_strip_overflow<'a>(
    ui: &mut Ui,
    tokens: &Tokens,
    metrics: &Metrics,
    hidden: &[usize],
    label_for: impl Fn(usize) -> &'a str,
    overflow_label_for: impl Fn(usize) -> String,
) -> Option<usize> {
    if hidden.is_empty() {
        return None;
    }
    let ov_label = overflow_label_for(hidden.len());
    let ov_resp = draw_overflow_button(ui, tokens, metrics, hidden.len(), &ov_label);
    let menu_items: Vec<MenuItem<'_>> = hidden
        .iter()
        .map(|&idx| MenuItem {
            label: label_for(idx),
            danger: false,
        })
        .collect();
    popup_menu(&ov_resp, &menu_items).and_then(|menu_idx| hidden.get(menu_idx).copied())
}

#[allow(clippy::too_many_arguments)]
fn draw_tab_chip(
    ui: &mut Ui,
    tokens: &Tokens,
    metrics: &Metrics,
    item: &TabItem<'_>,
    index: usize,
    width: f32,
    height: f32,
    active: bool,
) -> (Response, bool, bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let closable = item.close_label.is_some();
    let body_right = if closable {
        (rect.right() - CLOSE_RESERVE).max(rect.left() + 18.0)
    } else {
        rect.right()
    };
    let body_rect = Rect::from_min_max(rect.min, egui::pos2(body_right, rect.bottom()));
    let body_resp = ui.interact(body_rect, ui.id().with(("tab-body", index)), Sense::click());
    body_resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, ui.is_enabled(), active, item.label)
    });

    let close_rect = Rect::from_center_size(
        egui::pos2((rect.right() - 10.0).max(body_right + 10.0), rect.center().y),
        egui::vec2(CLOSE_SIZE, CLOSE_SIZE),
    );
    let close_resp = item.close_label.map(|close_label| {
        let resp = ui.interact(close_rect, ui.id().with(("tab-close", index)), Sense::CLICK);
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), close_label)
        });
        resp
    });

    let hovered = body_resp.hovered() || close_resp.as_ref().is_some_and(Response::hovered);
    let close_hovered = close_resp.as_ref().is_some_and(Response::hovered);
    let painter = painter_for(ui);

    if hovered && !active {
        painter.rect_filled(
            rect.shrink2(egui::vec2(0.0, 2.0)),
            CornerRadius::same(radius_u8(metrics.radius_control)),
            tokens.color.surface_alt.to_color32(),
        );
    }

    let fg = if active || hovered {
        tokens.color.text.to_color32()
    } else {
        tokens.color.text_muted.to_color32()
    };

    let mut text_left = rect.left() + 6.0;
    if !item.icon.is_empty() {
        painter.text(
            egui::pos2(rect.left() + 13.0, rect.center().y),
            Align2::CENTER_CENTER,
            item.icon,
            crate::icon::font_id(13.0),
            fg,
        );
        text_left += 16.0;
    }

    let label_right = if closable {
        rect.right() - CLOSE_RESERVE
    } else {
        rect.right() - 6.0
    };
    if label_right > text_left + 4.0 {
        let text_clip = Rect::from_min_max(
            egui::pos2(text_left, rect.top()),
            egui::pos2(label_right, rect.bottom()),
        );
        painter.with_clip_rect(text_clip.intersect(ui.clip_rect())).text(
            egui::pos2(text_clip.left(), text_clip.center().y),
            Align2::LEFT_CENTER,
            item.label,
            body_font(metrics),
            fg,
        );
    }

    if let Some(_close_resp) = &close_resp {
        if hovered {
            if close_hovered {
                painter.rect_filled(
                    close_rect,
                    CornerRadius::same(4),
                    tokens.color.surface_alt.to_color32(),
                );
            }
            painter.text(
                close_rect.center(),
                Align2::CENTER_CENTER,
                "×",
                FontId::proportional(12.0),
                if close_hovered {
                    tokens.color.text.to_color32()
                } else {
                    tokens.color.text_muted.to_color32()
                },
            );
        }
    }

    if active {
        painter.hline(
            rect.x_range(),
            rect.bottom(),
            Stroke::new(2.0, tokens.color.accent.to_color32()),
        );
    }
    paint::paint_focus(ui, &body_resp, rect, 4.0, tokens.color.accent.to_color32());

    let tip = item.tooltip.unwrap_or(item.label);
    let body_with_tip = body_resp.on_hover_text(tip.to_owned());
    let close_clicked = close_resp.as_ref().is_some_and(Response::clicked);
    let middle_clicked = body_with_tip.middle_clicked()
        || close_resp.as_ref().is_some_and(Response::middle_clicked);
    (body_with_tip, close_clicked, middle_clicked)
}

fn draw_overflow_button(
    ui: &mut Ui,
    tokens: &Tokens,
    metrics: &Metrics,
    hidden_count: usize,
    label: &str,
) -> Response {
    let w = measure_overflow_width(ui, hidden_count, metrics);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, metrics.control), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let fill = paint::hover_fill(tokens.color.surface_alt.to_color32(), &response, tokens);
    let painter = painter_for(ui);
    painter.rect_filled(
        rect.shrink2(egui::vec2(0.0, 2.0)),
        CornerRadius::same(radius_u8(metrics.radius_control)),
        fill,
    );
    let fg = tokens.color.text.to_color32();
    painter.text(
        egui::pos2(rect.left() + 12.0, rect.center().y),
        Align2::CENTER_CENTER,
        crate::icon::CHEVRON_DOWN,
        crate::icon::font_id(11.0),
        fg,
    );
    let count_text = format!("+{hidden_count}");
    painter.text(
        egui::pos2(rect.left() + 21.0, rect.center().y),
        Align2::LEFT_CENTER,
        &count_text,
        caption_font(metrics),
        fg,
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );
    response.on_hover_text(label.to_owned())
}

fn draw_add_button(
    ui: &mut Ui,
    tokens: &Tokens,
    metrics: &Metrics,
    label: &str,
    enabled: bool,
) -> Response {
    let size = egui::vec2(metrics.control, metrics.control);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && enabled, label)
    });
    let painter = painter_for(ui);
    if response.hovered() || response.is_pointer_button_down_on() {
        let fill = paint::hover_fill(tokens.color.surface_alt.to_color32(), &response, tokens);
        painter.rect_filled(
            rect.shrink(2.0),
            CornerRadius::same(radius_u8(metrics.radius_control)),
            fill,
        );
    }
    let fg = if enabled {
        if response.hovered() {
            tokens.color.text.to_color32()
        } else {
            tokens.color.text_muted.to_color32()
        }
    } else {
        tokens.color.text_muted.to_color32().gamma_multiply(0.4)
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        crate::icon::ADD,
        crate::icon::font_id(14.0),
        fg,
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );
    response.on_hover_text(label.to_owned())
}

/// Desenha fileira de ícones nos painéis laterais (cabe 4 ícones confortavelmente em 180 px).
pub fn panel_icon_tabs(
    ui: &mut Ui,
    id_salt: &str,
    selected: &mut usize,
    items: &[PanelIconTab<'_>],
    overflow_label_for: impl Fn(usize) -> String,
) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    if items.is_empty() {
        return ui.allocate_response(egui::Vec2::ZERO, Sense::hover());
    }
    if *selected >= items.len() {
        *selected = 0;
    }
    let btn_w = metrics.control;
    let avail = ui.available_width().max(btn_w);
    let widths = vec![btn_w; items.len()];
    let plan = plan_tab_fit(
        &widths,
        *selected,
        avail,
        TAB_GAP,
        |hidden| measure_overflow_width(ui, hidden, &metrics),
        0.0,
    );

    let inner = ui.push_id(id_salt, |ui| {
        draw_strip_baseline(ui, avail, btn_w, &tokens);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TAB_GAP;
            for &idx in &plan.shown {
                draw_panel_icon_button(ui, &tokens, &metrics, btn_w, selected, idx, &items[idx]);
            }
            if let Some(chosen) = draw_strip_overflow(
                ui,
                &tokens,
                &metrics,
                &plan.hidden,
                |idx| items[idx].label,
                overflow_label_for,
            ) {
                *selected = chosen;
            }
        });
    });
    inner.response
}

fn draw_panel_icon_button(
    ui: &mut Ui,
    tokens: &Tokens,
    metrics: &Metrics,
    btn_w: f32,
    selected: &mut usize,
    idx: usize,
    item: &PanelIconTab<'_>,
) {
    let active = idx == *selected;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(btn_w, btn_w), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            active,
            item.label,
        )
    });
    let key_activate = response.has_focus()
        && ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
        });
    if response.clicked() || key_activate {
        *selected = idx;
        response.request_focus();
    }
    let active_now = idx == *selected;
    let painter = painter_for(ui);
    let bg = if active_now {
        Some(tokens.color.accent_soft.to_color32())
    } else if response.hovered() {
        Some(tokens.color.surface_alt.to_color32())
    } else {
        None
    };
    if let Some(fill) = bg {
        painter.rect_filled(
            rect.shrink(2.0),
            CornerRadius::same(radius_u8(metrics.radius_control)),
            fill,
        );
    }
    let fg = if active_now || response.hovered() {
        tokens.color.text.to_color32()
    } else {
        tokens.color.text_muted.to_color32()
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        item.icon,
        crate::icon::font_id(15.0),
        fg,
    );
    if active_now {
        painter.hline(
            rect.x_range(),
            rect.bottom(),
            Stroke::new(2.0, tokens.color.accent.to_color32()),
        );
    }
    paint::paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );
    response.on_hover_text(item.tooltip.to_owned());
}
