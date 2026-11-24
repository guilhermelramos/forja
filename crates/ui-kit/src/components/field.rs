//! Campo de texto e seletor.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, EventFilter, Key, Margin, Modifiers, Popup, PopupKind,
    Response, Sense, SetOpenCommand, Stroke, StrokeKind, TextEdit, Ui,
};

use crate::paint::{self, body_font, chevron, hover_fill, paint_text, painter_for};

/// Último recurso diagnóstico quando nem rótulo nem dica foram passados.
/// Não é texto de interface: só existe para nenhum controle ficar sem nome.
const FALLBACK_NAME: &str = "control";

/// Nome acessível: o rótulo explícito, ou a dica quando o rótulo está vazio.
/// Um rótulo vazio é erro de contrato em depuração, mas nunca derruba o app.
pub(crate) fn accessible_name<'a>(label: &'a str, hint: &'a str) -> &'a str {
    let label = label.trim();
    if !label.is_empty() {
        return label;
    }
    debug_assert!(false, "ui-kit controls require a non-empty accessible label");
    let hint = hint.trim();
    if !hint.is_empty() {
        return hint;
    }
    debug_assert!(false, "ui-kit controls require a label or a hint");
    FALLBACK_NAME
}

/// Campo de uma linha, com dica quando está vazio.
/// Registra a dica como nome acessível quando há dica (API legada).
pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str) -> Response {
    draw_single_line(ui, text, hint, hint.trim(), false)
}

/// Campo de uma linha com rótulo acessível obrigatório (i18n de quem chama).
pub fn text_field_labeled(ui: &mut Ui, text: &mut String, label: &str, hint: &str) -> Response {
    let name = accessible_name(label, hint);
    let response = draw_single_line(ui, text, hint, name, false);
    crate::tooltip(response, name.to_owned())
}

/// Campo de senha/token com rótulo acessível obrigatório.
pub fn secret_field(ui: &mut Ui, text: &mut String, label: &str, hint: &str) -> Response {
    let name = accessible_name(label, hint);
    let response = draw_single_line(ui, text, hint, name, true);
    crate::tooltip(response, name.to_owned())
}

fn draw_single_line(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    name: &str,
    password: bool,
) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let width = ui.available_width().max(metrics.control * 4.0);
    let predicted = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, metrics.control));
    ui.painter().rect(
        predicted,
        CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
        tokens.color.surface.to_color32(),
        Stroke::new(1.0, tokens.color.border.to_color32()),
        StrokeKind::Inside,
    );
    let mut edit = TextEdit::singleline(text)
        .hint_text(hint)
        .desired_width(width)
        .min_size(egui::vec2(width, metrics.control))
        .frame(false)
        .margin(Margin::symmetric(
            crate::apply::pad_i8(metrics.pad_x),
            crate::apply::pad_i8(4.0),
        ))
        .font(body_font(&metrics))
        .text_color(tokens.color.text.to_color32());
    if password {
        edit = edit.password(true);
    }
    let output = edit.show(ui);
    if !name.is_empty() {
        let name = name.to_owned();
        output.response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), name.clone())
        });
    }
    if output.response.has_focus() && ui.is_enabled() {
        paint::focus_ring(
            ui,
            output.response.rect,
            metrics.radius_control,
            tokens.color.accent.to_color32(),
        );
    }
    output.response
}

/// Área de texto multilinha com rótulo acessível obrigatório.
pub fn text_area(
    ui: &mut Ui,
    text: &mut String,
    label: &str,
    hint: &str,
    rows: usize,
) -> Response {
    let name = accessible_name(label, hint);
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let width = ui.available_width().max(metrics.control * 4.0);
    let output = TextEdit::multiline(text)
        .hint_text(hint)
        .desired_width(width)
        .desired_rows(rows.max(1))
        .frame(false)
        .margin(Margin::symmetric(
            crate::apply::pad_i8(metrics.pad_x),
            crate::apply::pad_i8(4.0),
        ))
        .font(body_font(&metrics))
        .text_color(tokens.color.text.to_color32())
        .show(ui);
    let name_owned = name.to_owned();
    output.response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), name_owned.clone())
    });
    if output.response.has_focus() && ui.is_enabled() {
        paint::focus_ring(
            ui,
            output.response.rect,
            metrics.radius_control,
            tokens.color.accent.to_color32(),
        );
    }
    crate::tooltip(output.response, name.to_owned())
}

/// Seletor legado: expõe a opção atual como nome (API sem rótulo).
pub fn select(ui: &mut Ui, selected: &mut usize, options: &[&str]) -> Response {
    select_inner(ui, selected, options, None)
}

/// Seletor com rótulo acessível obrigatório (i18n de quem chama).
pub fn select_labeled(
    ui: &mut Ui,
    selected: &mut usize,
    label: &str,
    options: &[&str],
) -> Response {
    let name = accessible_name(label, "");
    let response = select_inner(ui, selected, options, Some(name));
    crate::tooltip(response, name.to_owned())
}

fn select_inner(
    ui: &mut Ui,
    selected: &mut usize,
    options: &[&str],
    accessible: Option<&str>,
) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let label = options.get(*selected).copied().unwrap_or("—");
    let name = accessible.unwrap_or(label);
    let font = body_font(&metrics);
    let text_color = tokens.color.text.to_color32();
    let width =
        (paint::text_size(ui, label, font.clone(), text_color).x + metrics.pad_x * 2.0 + 28.0)
            .max(metrics.control * 4.0)
            .min(ui.available_width().max(metrics.control));
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(width, metrics.control), Sense::click());
    let name = name.to_owned();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, ui.is_enabled(), name.clone())
    });
    let before = *selected;
    let fill = hover_fill(tokens.color.surface.to_color32(), &response, &tokens);
    let painter = painter_for(ui);
    painter.rect(
        rect,
        CornerRadius::same(crate::apply::radius_u8(metrics.radius_control)),
        fill,
        Stroke::new(1.0, tokens.color.border.to_color32()),
        StrokeKind::Inside,
    );
    let text_rect = rect.shrink2(egui::vec2(metrics.pad_x, 0.0));
    paint_text(
        &painter,
        text_rect,
        Align2::LEFT_CENTER,
        label,
        font,
        text_color,
    );
    chevron(
        &painter,
        egui::pos2(rect.right() - 14.0, rect.center().y),
        text_color,
    );
    paint::paint_focus(
        ui,
        &response,
        rect,
        metrics.radius_control,
        tokens.color.accent.to_color32(),
    );

    // Keyboard: click or Tab focuses the trigger; Space/Enter opens the list;
    // arrows move the highlight; Enter picks it; Escape closes the list.
    if response.clicked() {
        response.request_focus();
    }
    let popup_id = Popup::default_response_id(&response);
    let was_open = Popup::is_id_open(ui.ctx(), popup_id);
    let highlight_id = response.id.with("highlight");
    // `clicked()` also reports the keyboard's synthetic click on Space/Enter,
    // so only a real pointer click toggles the popup here.
    let pointer_click = response.clicked_by(egui::PointerButton::Primary);
    let mut open = pointer_click.then_some(SetOpenCommand::Toggle);
    if response.has_focus()
        && !was_open
        && open.is_none()
        && ui.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::Enter)
                || input.consume_key(Modifiers::NONE, Key::Space)
        })
    {
        open = Some(SetOpenCommand::Bool(true));
    }
    if was_open || open.is_some() {
        // Vertical arrows belong to the list, not to focus navigation.
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
    }
    if !was_open && open.is_some() {
        ui.ctx()
            .data_mut(|data| data.insert_temp(highlight_id, *selected));
    }
    let screen = ui.ctx().content_rect();
    let below = (screen.bottom() - rect.bottom() - 12.0).max(0.0);
    let above = (rect.top() - screen.top() - 12.0).max(0.0);
    let max_height = below.max(above).clamp(160.0, (screen.height() - 24.0).max(160.0));
    let row_height = paint::tokens(ui).metrics().row;
    Popup::from_response(&response)
        .kind(PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .width(rect.width())
        .gap(4.0)
        .open_memory(open)
        .show(|ui| {
            if options.is_empty() {
                return;
            }
            let mut highlight = ui
                .ctx()
                .data(|data| data.get_temp::<usize>(highlight_id).unwrap_or(*selected))
                .min(options.len() - 1);
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowDown)) {
                highlight = (highlight + 1).min(options.len() - 1);
            }
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::ArrowUp)) {
                highlight = highlight.saturating_sub(1);
            }
            let enter = ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
            let mut picked = None;
            egui::ScrollArea::vertical()
                .max_height(max_height)
                .show_rows(ui, row_height, options.len(), |ui, range| {
                    for index in range {
                        if menu_row(ui, options[index], index == highlight).clicked() {
                            picked = Some(index);
                        }
                    }
                });
            if enter {
                picked = Some(highlight);
            }
            if let Some(index) = picked {
                *selected = index;
                ui.ctx()
                    .data_mut(|data| data.insert_temp(highlight_id, index));
                Popup::close_id(ui.ctx(), popup_id);
            } else {
                ui.ctx()
                    .data_mut(|data| data.insert_temp(highlight_id, highlight));
            }
        });
    // A escolha só vem do popup: marcamos a resposta quando o índice muda.
    if *selected != before {
        response.mark_changed();
    }
    response
}

fn menu_row(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let width = ui.available_width().max(80.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, metrics.row), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    let fill = if selected {
        tokens.color.accent_soft.to_color32()
    } else if response.hovered() {
        tokens.color.surface_alt.to_color32()
    } else {
        Color32::TRANSPARENT
    };
    if fill != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
    }
    paint_text(
        ui.painter(),
        rect.shrink2(egui::vec2(8.0, 0.0)),
        Align2::LEFT_CENTER,
        label,
        body_font(&metrics),
        tokens.color.text.to_color32(),
    );
    response
}
