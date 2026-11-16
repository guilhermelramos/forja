//! Corpo do terminal: barras, teclado, menu de contexto e ponteiro.

use super::chrome::c32;
use super::hub_ui::Control;
use super::input::{key_bytes, paste_bytes, wheel_bytes};
use super::render;
use super::{UiText, ViewAction};
use crate::core::PendingPaste;
use crate::links::Link;
use crate::panes::tree::{PaneId, SplitDir, TerminalId};
use crate::settings::TermSettings;
use crate::terminal::Terminal;
use crate::theme::Palette;
use forja_kernel::ui_contract::egui::{self, FontId, Rect, RichText};
use forja_ui_kit::applied_tokens;
use std::collections::HashMap;

/// Tique enquanto uma seleção é arrastada (rolagem automática na borda).
const SELECTION_TICK: std::time::Duration = std::time::Duration::from_millis(30);

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_terminal(
    ui: &mut egui::Ui,
    terminal: &mut Terminal,
    pane: PaneId,
    settings: &TermSettings,
    texts: &UiText,
    palette: &Palette,
    textures: &mut HashMap<u32, egui::TextureHandle>,
    body: Rect,
    focused_pane: bool,
    actions: &mut Vec<ViewAction>,
    controls: &mut Vec<Control>,
    pending_link: &mut Option<(TerminalId, Link)>,
    pending_paste: &mut Option<PendingPaste>,
) {
    if terminal.search.open && ui.available_height() >= 26.0 {
        draw_search(ui, terminal, texts);
    }
    if pending_link
        .as_ref()
        .is_some_and(|(owner, _)| *owner == terminal.id)
    {
        draw_link_bar(ui, terminal, texts, actions, pending_link);
    }
    if pending_paste
        .as_ref()
        .is_some_and(|pending| pending.terminal == terminal.id)
    {
        draw_paste_bar(ui, terminal, texts, pending_paste);
    }
    if let Some(error) = terminal.error.clone() {
        draw_error_banner(ui, terminal, &error, texts, actions);
    }
    let Some(body) = body_after(ui, body) else {
        return;
    };
    let focus_id = ui.id().with(("forja-terminal-body", terminal.id));
    terminal.focus_id = Some(focus_id);
    terminal.drawn_frame = focused_pane.then_some(ui.ctx().cumulative_frame_nr());
    let keyboard_focus = ui.memory(|memory| memory.has_focus(focus_id));
    let grid = render::draw_grid(
        ui,
        terminal,
        settings,
        palette,
        textures,
        body,
        focused_pane && keyboard_focus && ui.input(|input| input.focused),
    );
    let response = grid.response;
    let metrics = grid.metrics;
    let focus_id = response.id;
    if terminal.focus_request {
        terminal.focus_request = false;
        ui.memory_mut(|memory| memory.request_focus(focus_id));
    }
    if response.drag_started() {
        if let Some(position) = ui.input(|input| input.pointer.press_origin()) {
            let anchor = metrics.hit(terminal, position);
            terminal.selection = Some(crate::terminal::Selection {
                anchor,
                head: anchor,
                generation: terminal.surface.generation,
            });
            terminal.follow = false;
            response.request_focus();
        }
    }
    if response.dragged() {
        if let Some(position) = response.interact_pointer_pos() {
            let head = metrics.hit(terminal, position);
            if let Some(selection) = &mut terminal.selection {
                selection.head = head;
            }
            if position.y < body.top() {
                terminal.scroll_by(-1);
            } else if position.y > body.bottom() {
                terminal.scroll_by(1);
            }
            response.request_focus();
        }
        // Arrastar para fora do corpo rola sozinho: o ponteiro parado não gera
        // eventos, então um tique rápido mantém a rolagem só durante o arraste.
        forja_ui_kit::request_repaint_after_active(ui.ctx(), SELECTION_TICK);
    }
    if response.drag_stopped() && settings.copy_on_select {
        if let Some(text) = terminal.selected_text(settings) {
            ui.ctx().copy_text(text);
        }
    }
    if response.clicked() && !response.double_clicked() {
        if ui.input(|input| input.modifiers.ctrl) {
            if let Some(position) = response.interact_pointer_pos() {
                let (row, col) = metrics.hit(terminal, position);
                if let Some(link) = terminal.link_at(row, col) {
                    open_or_confirm(terminal, link, settings, actions, pending_link);
                }
            }
        } else {
            terminal.selection = None;
        }
        response.request_focus();
    }
    if response.double_clicked() {
        if let Some(position) = response.interact_pointer_pos() {
            let (row, col) = metrics.hit(terminal, position);
            if let Some((start, end)) = terminal.word_at(row, col) {
                terminal.selection = Some(crate::terminal::Selection {
                    anchor: (row, start),
                    head: (row, end),
                    generation: terminal.surface.generation,
                });
                if settings.copy_on_select {
                    if let Some(text) = terminal.selected_text(settings) {
                        ui.ctx().copy_text(text);
                    }
                }
            }
        }
        response.request_focus();
    }
    if response.hovered() {
        if ui.input(|input| input.modifiers.ctrl) {
            if let Some(position) = ui.input(|input| input.pointer.hover_pos()) {
                let (row, col) = metrics.hit(terminal, position);
                if terminal.link_at(row, col).is_some() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        }
        let delta = ui.input_mut(|input| {
            let delta = input.raw_scroll_delta.y;
            input.raw_scroll_delta.y = 0.0;
            delta
        });
        if delta.abs() > 0.0 {
            if terminal.surface.is_alternate() && terminal.surface.mouse_tracking {
                if let Some(position) = ui.input(|input| input.pointer.hover_pos()) {
                    let (row, col) = metrics.hit(terminal, position);
                    let relative = row.saturating_sub(terminal.surface.history_start);
                    for _ in 0..settings.wheel_lines.clamp(1, 10) {
                        let bytes = wheel_bytes(
                            delta > 0.0,
                            col,
                            relative,
                            true,
                            terminal.surface.mouse_sgr,
                        );
                        terminal.write(&bytes);
                    }
                }
            } else if !terminal.surface.is_alternate() {
                let lines = (delta / metrics.line_height * settings.wheel_notch()) as isize;
                if lines != 0 {
                    terminal.scroll_by(-lines);
                }
            }
        }
    }
    context_menu(terminal, &response, settings, texts, controls);
    let has_focus = ui.memory(|memory| memory.has_focus(focus_id));
    if has_focus {
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                focus_id,
                egui::EventFilter {
                    tab: true,
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    escape: true,
                },
            );
        });
        handle_keyboard(ui, terminal, pane, settings, controls, pending_paste);
    }
    if !terminal.search.open && !has_focus {
        ui.painter().text(
            body.right_bottom() - egui::vec2(8.0, 6.0),
            egui::Align2::RIGHT_BOTTOM,
            &texts.click_to_type,
            FontId::proportional(11.0),
            c32(applied_tokens(ui.ctx()).color.text_muted),
        );
    }
}

fn body_after(ui: &egui::Ui, body: Rect) -> Option<Rect> {
    let available = ui.available_rect_before_wrap().intersect(ui.clip_rect());
    let body = body.intersect(available);
    (body.width() > 2.0 && body.height() > 2.0).then_some(body)
}

fn draw_search(ui: &mut egui::Ui, terminal: &mut Terminal, texts: &UiText) {
    let single_row_only = ui.available_height() < 54.0;
    let field_width = (ui.available_width() - 210.0).clamp(60.0, 220.0);
    let mut contents = |ui: &mut egui::Ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut terminal.search.query)
                .hint_text(texts.search_hint.clone())
                .desired_width(field_width),
        );
        if terminal.search.focus_requested {
            terminal.search.focus_requested = false;
            response.request_focus();
        }
        terminal.search.editing = response.has_focus();
        if response.changed() {
            terminal.search.revision = u64::MAX;
            terminal.search.current = 0;
        }
        if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            let previous = ui.input(|input| input.modifiers.shift);
            terminal.search_step(previous);
        }
        if response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            terminal.search.open = false;
            terminal.search.editing = false;
            terminal.focus_request = true;
        }
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &texts.previous).clicked() {
            terminal.search_step(true);
        }
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &texts.next).clicked() {
            terminal.search_step(false);
        }
        let count = terminal.search.hits.len();
        let label = if count == 1 {
            texts.match_single.clone()
        } else {
            format!("{count} {}", texts.match_plural)
        };
        ui.label(RichText::new(label).weak());
        let close_btn = forja_ui_kit::icon_button(ui, forja_ui_kit::CLOSE);
        let close_label = texts.close.clone();
        close_btn.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, close_label.clone())
        });
        if close_btn.on_hover_text(texts.search.clone()).clicked() {
            terminal.search.open = false;
            terminal.search.editing = false;
            terminal.focus_request = true;
        }
    };
    if single_row_only {
        ui.horizontal(|ui| {
            contents(ui);
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            contents(ui);
        });
    }
}

fn draw_link_bar(
    ui: &mut egui::Ui,
    terminal: &Terminal,
    texts: &UiText,
    actions: &mut Vec<ViewAction>,
    pending_link: &mut Option<(TerminalId, Link)>,
) {
    let Some((_, link)) = pending_link.clone() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(texts.link_title.clone()).strong());
        ui.label(RichText::new(link.display()).color(c32(applied_tokens(ui.ctx()).color.info)));
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &texts.open).clicked() {
            actions.push(ViewAction::OpenLink(terminal.resolve_link(&link)));
            *pending_link = None;
        }
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &texts.copy).clicked() {
            ui.ctx().copy_text(link.display());
            *pending_link = None;
        }
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &texts.dismiss).clicked() {
            *pending_link = None;
        }
    });
}

fn draw_paste_bar(
    ui: &mut egui::Ui,
    terminal: &mut Terminal,
    texts: &UiText,
    pending_paste: &mut Option<PendingPaste>,
) {
    let Some(pending) = pending_paste.clone() else {
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(texts.paste_title.clone()).strong());
        ui.label(RichText::new(texts.paste_body.clone()).weak());
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &texts.paste_accept).clicked() {
            let bytes = paste_bytes(&pending.text, true);
            terminal.write(&bytes);
            *pending_paste = None;
        }
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &texts.cancel).clicked() {
            *pending_paste = None;
        }
    });
}

fn open_or_confirm(
    terminal: &Terminal,
    link: Link,
    settings: &TermSettings,
    actions: &mut Vec<ViewAction>,
    pending_link: &mut Option<(TerminalId, Link)>,
) {
    if settings.links_confirm {
        *pending_link = Some((terminal.id, link));
    } else {
        actions.push(ViewAction::OpenLink(terminal.resolve_link(&link)));
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_keyboard(
    ui: &mut egui::Ui,
    terminal: &mut Terminal,
    pane: PaneId,
    settings: &TermSettings,
    controls: &mut Vec<Control>,
    pending_paste: &mut Option<PendingPaste>,
) {
    if terminal.search.editing {
        return;
    }
    if terminal.paste_request {
        terminal.paste_request = false;
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }
    let events = ui.input(|input| input.events.clone());
    let has_paste = events
        .iter()
        .any(|event| matches!(event, egui::Event::Paste(_)));
    let has_copy = events
        .iter()
        .any(|event| matches!(event, egui::Event::Copy));
    let modifiers = ui.input(|input| input.modifiers);
    for event in events {
        match event {
            egui::Event::Text(text) => {
                if modifiers.alt {
                    terminal.write(b"\x1b");
                }
                terminal.write(text.as_bytes());
            }
            egui::Event::Paste(text) => {
                if settings.paste_warn_multiline && (text.contains('\n') || text.contains('\r')) {
                    *pending_paste = Some(PendingPaste {
                        terminal: terminal.id,
                        text,
                    });
                } else {
                    let bytes = paste_bytes(&text, settings.paste_bracketed);
                    terminal.write(&bytes);
                }
            }
            egui::Event::Copy => {
                if let Some(text) = terminal.selected_text(settings) {
                    ui.ctx().copy_text(text);
                }
            }
            egui::Event::Cut => terminal.write(&[24]),
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                if modifiers.ctrl && modifiers.shift && key == egui::Key::C {
                    if let Some(text) = terminal.selected_text(settings) {
                        ui.ctx().copy_text(text);
                    }
                    continue;
                }
                if modifiers.ctrl && modifiers.shift && key == egui::Key::F {
                    terminal.toggle_search();
                    continue;
                }
                if modifiers.ctrl && modifiers.shift && key == egui::Key::K {
                    terminal.clear();
                    continue;
                }
                if modifiers.alt && modifiers.shift && key == egui::Key::R {
                    controls.push(Control::Split(SplitDir::Columns));
                    continue;
                }
                if modifiers.alt && modifiers.shift && key == egui::Key::D {
                    controls.push(Control::Split(SplitDir::Rows));
                    continue;
                }
                if modifiers.alt && modifiers.shift && key == egui::Key::PageDown {
                    controls.push(Control::NextPane);
                    continue;
                }
                if modifiers.alt && modifiers.shift && key == egui::Key::PageUp {
                    controls.push(Control::PreviousPane);
                    continue;
                }
                if modifiers.alt && modifiers.shift && key == egui::Key::W {
                    controls.push(Control::ClosePane);
                    continue;
                }
                if modifiers.ctrl && modifiers.shift && key == egui::Key::W {
                    controls.push(Control::CloseTab(pane, terminal.id));
                    continue;
                }
                if modifiers.ctrl && key == egui::Key::V && !has_paste {
                    terminal.paste_request = true;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    continue;
                }
                if modifiers.ctrl && key == egui::Key::C {
                    if !has_copy {
                        terminal.write(&[3]);
                    }
                    continue;
                }
                if modifiers.ctrl && key == egui::Key::Home {
                    terminal.scroll_top();
                    continue;
                }
                if modifiers.ctrl && key == egui::Key::End {
                    terminal.scroll_bottom();
                    continue;
                }
                if !modifiers.ctrl && !modifiers.alt && key == egui::Key::PageUp {
                    terminal.scroll_page(false);
                    continue;
                }
                if !modifiers.ctrl && !modifiers.alt && key == egui::Key::PageDown {
                    terminal.scroll_page(true);
                    continue;
                }
                if terminal.search.open && key == egui::Key::Escape {
                    terminal.search.open = false;
                    terminal.focus_request = true;
                    continue;
                }
                let bytes = key_bytes(key, modifiers, terminal.surface.application_cursor);
                terminal.write(&bytes);
            }
            _ => {}
        }
    }
}

fn context_menu(
    terminal: &mut Terminal,
    response: &egui::Response,
    settings: &TermSettings,
    texts: &UiText,
    controls: &mut Vec<Control>,
) {
    // Ordem dos itens = ordem dos índices em `match` abaixo.
    let labels = [
        texts.copy.as_str(),
        texts.copy_output.as_str(),
        texts.paste.as_str(),
        texts.clear.as_str(),
        texts.search.as_str(),
        texts.select_all.as_str(),
        texts.split_right.as_str(),
        texts.split_down.as_str(),
        texts.close_pane.as_str(),
    ];
    let items: Vec<forja_ui_kit::MenuItem<'_>> = labels
        .iter()
        .map(|label| forja_ui_kit::MenuItem {
            label,
            danger: false,
        })
        .collect();
    response.context_menu(|ui| {
        let Some(index) = forja_ui_kit::menu_panel(ui, &items) else {
            return;
        };
        match index {
            0 => {
                if let Some(text) = terminal.selected_text(settings) {
                    ui.ctx().copy_text(text);
                }
            }
            1 => ui.ctx().copy_text(terminal.surface.text()),
            2 => {
                terminal.paste_request = true;
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            3 => terminal.clear(),
            4 => terminal.toggle_search(),
            5 => terminal.select_all(),
            6 => controls.push(Control::Split(SplitDir::Columns)),
            7 => controls.push(Control::Split(SplitDir::Rows)),
            8 => controls.push(Control::ClosePane),
            _ => {}
        }
        ui.close();
    });
}

fn draw_error_banner(
    ui: &mut egui::Ui,
    terminal: &mut Terminal,
    raw_error: &str,
    texts: &UiText,
    actions: &mut Vec<ViewAction>,
) {
    let tokens = applied_tokens(ui.ctx());
    let lower = raw_error.to_ascii_lowercase();
    let not_found = lower.contains("os error 2")
        || lower.contains("cannot find the file")
        || lower.contains("não pode encontrar o arquivo")
        || lower.contains("not found");
    let (title, body) = if not_found {
        (
            texts.error_not_found_title.as_str(),
            texts
                .error_not_found_body
                .replace("{ $program }", &terminal.shell)
                .replace("{$program}", &terminal.shell),
        )
    } else {
        (texts.error_launch.as_str(), texts.error_launch_body.clone())
    };
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(title).strong().color(c32(tokens.color.danger)));
            ui.label(RichText::new(&body).weak());
        });
        ui.horizontal(|ui| {
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &texts.error_retry,
            )
            .clicked()
            {
                terminal.retry(Some(ui.ctx().clone()));
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &texts.error_settings,
            )
            .clicked()
            {
                actions.push(ViewAction::OpenSettings);
            }
        });
        ui.collapsing(&texts.error_details, |ui| {
            ui.label(RichText::new(raw_error).monospace().weak());
        });
    });
}


