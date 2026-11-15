//! Abas, menus de receita/shell e botões do chrome.

use super::UiText;
use crate::panes::tree::TerminalId;
use forja_kernel::ui_contract::egui::{self, Color32, FontId, Rect, Sense};
use forja_ui_kit::{Color, MenuItem, Tokens};

pub(super) struct TabChip<'a> {
    pub(super) tokens: &'a Tokens,
    pub(super) title: &'a str,
    pub(super) alive: bool,
    pub(super) has_error: bool,
    pub(super) is_helper: bool,
    pub(super) selected: bool,
    pub(super) terminal: TerminalId,
    pub(super) texts: &'a UiText,
}

pub(super) fn tab_chip(ui: &mut egui::Ui, chip: &TabChip<'_>) -> (egui::Response, bool) {
    let TabChip {
        tokens,
        title,
        alive,
        has_error,
        is_helper,
        selected,
        terminal,
        texts,
    } = *chip;
    let font = FontId::proportional(12.5);
    let text_color = if selected {
        c32(tokens.color.text)
    } else {
        c32(tokens.color.text_muted)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font.clone(), text_color);
    let width = (galley.size().x + 48.0).min(230.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 22.0), Sense::click());
    let status_word = if has_error {
        &texts.failed
    } else if alive {
        &texts.running
    } else {
        &texts.exited
    };
    let chip_label = format!("{title} · {status_word}");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, chip_label.clone())
    });
    let hovered = response.hovered();
    if selected || hovered {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(5),
            if selected {
                c32(tokens.color.surface_alt)
            } else {
                c32(tokens.color.accent_soft)
            },
        );
    }
    let dot_color = if has_error {
        c32(tokens.color.danger)
    } else if alive {
        if is_helper {
            c32(tokens.color.accent)
        } else {
            c32(tokens.color.ok)
        }
    } else {
        c32(tokens.color.text_muted)
    };
    ui.painter().circle_filled(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        3.0,
        dot_color,
    );
    let text_rect = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top()),
        egui::pos2(rect.right() - 20.0, rect.bottom()),
    );
    ui.painter().with_clip_rect(text_rect).text(
        egui::pos2(text_rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        font,
        text_color,
    );
    let close_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 11.0, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    let close = ui.interact(
        close_rect,
        ui.id().with(("terminal-tab-close", terminal)),
        Sense::click(),
    );
    let close_label = format!("{} {title}", texts.close);
    close.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, close_label.clone())
    });
    if hovered || close.hovered() {
        if close.hovered() {
            ui.painter()
                .rect_filled(close_rect, egui::CornerRadius::same(4), c32(tokens.color.surface));
        }
        ui.painter().text(
            close_rect.center(),
            egui::Align2::CENTER_CENTER,
            "×",
            FontId::proportional(11.0),
            c32(tokens.color.text_muted),
        );
    }
    (response.on_hover_text(chip_label), close.clicked())
}

/// Botão "Receitas" que abre o seletor com filtro, grupos e prévia.
pub(super) fn recipes_button(ui: &mut egui::Ui, texts: &UiText, open_picker: &mut bool) {
    if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &texts.recipes).clicked() {
        *open_picker = true;
    }
}

/// Menu "Shell": os shells fixos e cada distro do WSL, numa lista só (o ui-kit não tem submenu).
pub(super) fn shell_menu(
    ui: &mut egui::Ui,
    texts: &UiText,
    wsl: &[String],
    new_tab: &mut bool,
    new_tab_shell: &mut Option<String>,
) {
    let mut options: Vec<(String, String)> = vec![
        (texts.pwsh.clone(), "pwsh".to_owned()),
        (texts.powershell.clone(), "powershell".to_owned()),
        (texts.cmd.clone(), "cmd".to_owned()),
        (texts.git_bash.clone(), "git-bash".to_owned()),
    ];
    if wsl.is_empty() {
        options.push((
            format!("{}  ·  {}", texts.wsl, texts.wsl_default),
            "wsl".to_owned(),
        ));
    } else {
        for distro in wsl {
            options.push((format!("{}  ·  {distro}", texts.wsl), format!("wsl:{distro}")));
        }
    }
    let items: Vec<MenuItem<'_>> = options
        .iter()
        .map(|(label, _)| MenuItem {
            label: label.as_str(),
            danger: false,
        })
        .collect();
    if let Some(index) = forja_ui_kit::context_menu(ui, &texts.shell, &items) {
        if let Some((_, id)) = options.get(index) {
            *new_tab = true;
            *new_tab_shell = Some(id.clone());
        }
    }
}

/// Botão de ícone do ui-kit, com o nome da ação como dica.
pub(super) fn icon_button(ui: &mut egui::Ui, glyph: &str, tooltip: &str) -> egui::Response {
    forja_ui_kit::icon_button(ui, glyph).on_hover_text(tooltip)
}

pub(super) fn c32(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}
