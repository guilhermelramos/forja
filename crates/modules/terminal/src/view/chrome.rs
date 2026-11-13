//! Abas, menus de receita/shell e botões do chrome.

use super::UiText;
use crate::panes::tree::TerminalId;
use crate::recipes::Recipe;
use forja_kernel::ui_contract::egui::{self, Color32, FontId, Rect, Sense};
use forja_ui_kit::{Color, MenuItem, Tokens};

pub(super) fn tab_chip(
    ui: &mut egui::Ui,
    tokens: &Tokens,
    title: &str,
    alive: bool,
    selected: bool,
    terminal: TerminalId,
    texts: &UiText,
) -> (egui::Response, bool) {
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
    ui.painter().circle_filled(
        egui::pos2(rect.left() + 8.0, rect.center().y),
        3.0,
        if alive {
            c32(tokens.color.ok)
        } else {
            c32(tokens.color.text_muted)
        },
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
    let tooltip = if alive {
        texts.running.clone()
    } else {
        texts.exited.clone()
    };
    (response.on_hover_text(tooltip), close.clicked())
}

/// Menu "Receitas" com o componente de menu do ui-kit. Escolher uma receita a marca para rodar.
pub(super) fn recipes_menu(
    ui: &mut egui::Ui,
    texts: &UiText,
    recipes: &[Recipe],
    run_recipe: &mut Option<Recipe>,
) {
    let labels: Vec<String> = if recipes.is_empty() {
        vec![texts.recipe_empty.clone()]
    } else {
        recipes
            .iter()
            .map(|recipe| {
                let scope = if recipe.scope == "project" {
                    &texts.recipe_scope_project
                } else {
                    &texts.recipe_scope_global
                };
                format!("{}  ·  {scope}", recipe.title(&texts.locale))
            })
            .collect()
    };
    let items: Vec<MenuItem<'_>> = labels
        .iter()
        .map(|label| MenuItem {
            label: label.as_str(),
            danger: false,
        })
        .collect();
    if let Some(index) = forja_ui_kit::context_menu(ui, &texts.recipes, &items) {
        if let Some(recipe) = recipes.get(index) {
            *run_recipe = Some(recipe.clone());
        }
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
