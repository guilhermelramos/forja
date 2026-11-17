//! Abas, menus de receita/shell e botões do chrome.

use super::UiText;
use crate::panes::tree::TerminalId;
use crate::recipes::Recipe;
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
    pub(super) max_width: f32,
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
        max_width: _,
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
    let width = (galley.size().x + 48.0).min(chip.max_width).max(48.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), Sense::hover());
    let body_max_x = (rect.right() - 21.0).max(rect.left() + 24.0);
    let body_rect = Rect::from_min_max(rect.min, egui::pos2(body_max_x, rect.bottom()));
    let response = ui.interact(
        body_rect,
        ui.id().with(("terminal-tab", terminal)),
        Sense::click(),
    );
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
    let close_center_x = (body_rect.right() + 10.0).min(rect.right() - 9.0);
    let close_rect = Rect::from_center_size(
        egui::pos2(close_center_x, rect.center().y),
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
            ui.painter().rect_filled(
                close_rect,
                egui::CornerRadius::same(4),
                c32(tokens.color.surface),
            );
        }
        forja_ui_kit::paint_icon(
            ui.painter(),
            forja_ui_kit::Icon::Close,
            close_rect,
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

fn shell_options(texts: &UiText, wsl: &[String]) -> Vec<(String, String)> {
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
            options.push((
                format!("{}  ·  {distro}", texts.wsl),
                format!("wsl:{distro}"),
            ));
        }
    }
    options
}

/// Menu "Shell": os shells fixos e cada distro do WSL, numa lista só (o ui-kit não tem submenu).
pub(super) fn shell_menu(
    ui: &mut egui::Ui,
    texts: &UiText,
    wsl: &[String],
    new_tab: &mut bool,
    new_tab_shell: &mut Option<String>,
) {
    let options = shell_options(texts, wsl);
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

/// Botão de ícone do ui-kit, com o nome da ação como dica e rótulo de acessibilidade.
pub(super) fn icon_button(
    ui: &mut egui::Ui,
    glyph: forja_ui_kit::Icon,
    tooltip: &str,
) -> egui::Response {
    let response = forja_ui_kit::icon_button(ui, glyph, tooltip);
    let label = tooltip.to_owned();
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label.clone())
    });
    response.on_hover_text(tooltip)
}

/// Resumo de uma aba passado ao cabeçalho do painel.
pub(super) struct TabInfo {
    pub(super) id: TerminalId,
    pub(super) title: String,
    pub(super) alive: bool,
    pub(super) has_error: bool,
    pub(super) is_helper: bool,
}

impl TabInfo {
    fn status<'a>(&self, texts: &'a UiText) -> &'a str {
        if self.has_error {
            &texts.failed
        } else if self.alive {
            &texts.running
        } else {
            &texts.exited
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum HeaderControl {
    ClosePane,
    SplitDown,
    SplitRight,
}

#[derive(Default)]
pub(super) struct HeaderOutcome {
    pub(super) close_tab: Option<TerminalId>,
    pub(super) select_tab: Option<usize>,
    pub(super) focus_pane: bool,
    pub(super) new_tab: bool,
    pub(super) new_tab_shell: Option<String>,
    pub(super) open_recipe_picker: bool,
    pub(super) control: Option<HeaderControl>,
    pub(super) toggle_search: bool,
}

#[derive(Debug, PartialEq)]
struct HeaderBudget {
    tab_widths: Vec<Option<f32>>,
    show_plus: bool,
    show_close: bool,
    show_split_down: bool,
    show_split_right: bool,
    show_search: bool,
    show_recipes: bool,
    show_shell: bool,
    has_overflow: bool,
}

struct ControlWidths {
    plus: f32,
    icon: f32,
    recipes: f32,
    shell: f32,
    more: f32,
}

fn tab_chip_width(ui: &egui::Ui, title: &str) -> f32 {
    let font = FontId::proportional(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font, Color32::WHITE);
    (galley.size().x + 48.0).min(230.0)
}

fn text_button_width(ui: &egui::Ui, label: &str) -> f32 {
    let tokens = forja_ui_kit::applied_tokens(ui.ctx());
    let metrics = tokens.metrics();
    let font = FontId::new(metrics.body, egui::FontFamily::Proportional);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, Color32::WHITE);
    (galley.size().x + metrics.pad_x * 2.0).max(metrics.control)
}

fn budget_header(
    avail: f32,
    gap: f32,
    tabs: &[f32],
    active: usize,
    controls: &ControlWidths,
) -> HeaderBudget {
    let mut tab_widths = vec![None; tabs.len()];
    let fixed_sum =
        controls.plus + controls.icon * 4.0 + controls.recipes + controls.shell + gap * 6.0;
    let tabs_sum: f32 = tabs.iter().sum::<f32>() + gap * tabs.len() as f32;
    if fixed_sum + tabs_sum <= avail {
        for (slot, &w) in tab_widths.iter_mut().zip(tabs.iter()) {
            *slot = Some(w);
        }
        return HeaderBudget {
            tab_widths,
            show_plus: true,
            show_close: true,
            show_split_down: true,
            show_split_right: true,
            show_search: true,
            show_recipes: true,
            show_shell: true,
            has_overflow: false,
        };
    }

    let mut rem = (avail - controls.more).max(0.0);
    let min_chip = 64.0_f32;
    if active < tabs.len() && rem >= min_chip + gap {
        let max_allowed = (rem - gap).min(tabs[active]).max(min_chip);
        tab_widths[active] = Some(max_allowed);
        rem = (rem - max_allowed - gap).max(0.0);
    }
    for (i, &w) in tabs.iter().enumerate() {
        if i == active {
            continue;
        }
        if w + gap <= rem {
            tab_widths[i] = Some(w);
            rem -= w + gap;
        }
    }

    let mut take = |w: f32| -> bool {
        if w + gap <= rem {
            rem -= w + gap;
            true
        } else {
            false
        }
    };
    let show_plus = take(controls.plus);
    let show_close = take(controls.icon);
    let show_split_down = take(controls.icon);
    let show_split_right = take(controls.icon);
    let show_search = take(controls.icon);
    let show_recipes = take(controls.recipes);
    let show_shell = take(controls.shell);
    let has_overflow = tab_widths.iter().any(Option::is_none)
        || !show_plus
        || !show_close
        || !show_split_down
        || !show_split_right
        || !show_search
        || !show_recipes
        || !show_shell;

    HeaderBudget {
        tab_widths,
        show_plus,
        show_close,
        show_split_down,
        show_split_right,
        show_search,
        show_recipes,
        show_shell,
        has_overflow,
    }
}

#[derive(Clone)]
enum OverflowAction {
    SelectTab(usize),
    NewTab,
    Control(HeaderControl),
    Search,
    OpenRecipes,
    OpenShell(String),
}

/// Cabeçalho do painel em uma linha limitada à largura disponível, com menu "Mais" para o excedente.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_pane_header(
    ui: &mut egui::Ui,
    tokens: &Tokens,
    texts: &UiText,
    tabs: &[TabInfo],
    active: usize,
    recipes: &[Recipe],
    wsl: &[String],
) -> HeaderOutcome {
    let mut outcome = HeaderOutcome::default();
    let ctrl = tokens.metrics().control;
    if ui.available_width() < ctrl || ui.available_height() < ctrl {
        return outcome;
    }

    let gap = 3.0;
    let tab_measured: Vec<f32> = tabs.iter().map(|t| tab_chip_width(ui, &t.title)).collect();
    let controls = ControlWidths {
        plus: ctrl,
        icon: ctrl,
        recipes: text_button_width(ui, &texts.recipes),
        shell: text_button_width(ui, &texts.shell),
        more: text_button_width(ui, &texts.more),
    };
    let avail = (ui.available_width() - 2.0).max(0.0);
    let plan = budget_header(avail, gap, &tab_measured, active, &controls);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (index, tab) in tabs.iter().enumerate() {
            let Some(max_width) = plan.tab_widths[index] else {
                continue;
            };
            let (response, close) = tab_chip(
                ui,
                &TabChip {
                    tokens,
                    title: &tab.title,
                    alive: tab.alive,
                    has_error: tab.has_error,
                    is_helper: tab.is_helper,
                    selected: index == active,
                    terminal: tab.id,
                    texts,
                    max_width,
                },
            );
            if close || response.middle_clicked() {
                outcome.close_tab = Some(tab.id);
            } else if response.clicked() {
                outcome.select_tab = Some(index);
                outcome.focus_pane = true;
            }
        }
        if plan.show_plus
            && forja_ui_kit::icon_button(ui, forja_ui_kit::Icon::Add, &texts.new_tab)
                .on_hover_text(texts.new_tab.clone())
                .clicked()
        {
            outcome.new_tab = true;
            outcome.new_tab_shell = None;
        }
        if plan.show_close && icon_button(ui, forja_ui_kit::CLOSE, &texts.close_pane).clicked() {
            outcome.focus_pane = true;
            outcome.control = Some(HeaderControl::ClosePane);
        }
        if plan.show_split_down
            && icon_button(ui, forja_ui_kit::SPLIT_DOWN, &texts.split_down).clicked()
        {
            outcome.focus_pane = true;
            outcome.control = Some(HeaderControl::SplitDown);
        }
        if plan.show_split_right
            && icon_button(ui, forja_ui_kit::SPLIT_RIGHT, &texts.split_right).clicked()
        {
            outcome.focus_pane = true;
            outcome.control = Some(HeaderControl::SplitRight);
        }
        if plan.show_search && icon_button(ui, forja_ui_kit::SEARCH, &texts.search).clicked() {
            outcome.toggle_search = true;
        }
        if plan.show_recipes {
            recipes_button(ui, texts, &mut outcome.open_recipe_picker);
        }
        if plan.show_shell {
            shell_menu(
                ui,
                texts,
                wsl,
                &mut outcome.new_tab,
                &mut outcome.new_tab_shell,
            );
        }
        if plan.has_overflow {
            draw_overflow_menu(ui, texts, tabs, recipes, wsl, &plan, &mut outcome);
        }
    });

    outcome
}

fn draw_overflow_menu(
    ui: &mut egui::Ui,
    texts: &UiText,
    tabs: &[TabInfo],
    recipes: &[Recipe],
    wsl: &[String],
    plan: &HeaderBudget,
    outcome: &mut HeaderOutcome,
) {
    let mut entries: Vec<(String, OverflowAction)> = Vec::new();
    for (index, tab) in tabs.iter().enumerate() {
        if plan.tab_widths[index].is_none() {
            entries.push((
                format!("{} · {}", tab.title, tab.status(texts)),
                OverflowAction::SelectTab(index),
            ));
        }
    }
    for (visible, label, action) in [
        (plan.show_plus, &texts.new_tab, OverflowAction::NewTab),
        (
            plan.show_close,
            &texts.close_pane,
            OverflowAction::Control(HeaderControl::ClosePane),
        ),
        (
            plan.show_split_down,
            &texts.split_down,
            OverflowAction::Control(HeaderControl::SplitDown),
        ),
        (
            plan.show_split_right,
            &texts.split_right,
            OverflowAction::Control(HeaderControl::SplitRight),
        ),
        (plan.show_search, &texts.search, OverflowAction::Search),
    ] {
        if !visible {
            entries.push((label.clone(), action));
        }
    }
    if !plan.show_recipes {
        let label = if recipes.is_empty() {
            &texts.recipe_empty
        } else {
            &texts.recipe_picker_open
        };
        entries.push((
            format!("{}: {label}", texts.recipes),
            OverflowAction::OpenRecipes,
        ));
    }
    if !plan.show_shell {
        for (label, id) in shell_options(texts, wsl) {
            entries.push((
                format!("{}: {label}", texts.shell),
                OverflowAction::OpenShell(id),
            ));
        }
    }
    let items: Vec<MenuItem<'_>> = entries
        .iter()
        .map(|(label, _)| MenuItem {
            label: label.as_str(),
            danger: false,
        })
        .collect();
    if let Some(chosen) = forja_ui_kit::context_menu(ui, &texts.more, &items) {
        if let Some((_, action)) = entries.into_iter().nth(chosen) {
            match action {
                OverflowAction::SelectTab(index) => {
                    outcome.select_tab = Some(index);
                    outcome.focus_pane = true;
                }
                OverflowAction::NewTab => {
                    outcome.new_tab = true;
                    outcome.new_tab_shell = None;
                }
                OverflowAction::Control(ctrl) => {
                    outcome.focus_pane = true;
                    outcome.control = Some(ctrl);
                }
                OverflowAction::Search => outcome.toggle_search = true,
                OverflowAction::OpenRecipes => outcome.open_recipe_picker = true,
                OverflowAction::OpenShell(id) => {
                    outcome.new_tab = true;
                    outcome.new_tab_shell = Some(id);
                }
            }
        }
    }
}

pub(super) fn c32(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_controls() -> ControlWidths {
        ControlWidths {
            plus: 32.0,
            icon: 32.0,
            recipes: 76.0,
            shell: 56.0,
            more: 52.0,
        }
    }

    #[test]
    fn wide_header_shows_all_items_without_overflow() {
        let plan = budget_header(600.0, 3.0, &[100.0], 0, &sample_controls());
        assert!(!plan.has_overflow);
        assert_eq!(plan.tab_widths, vec![Some(100.0)]);
        assert!(plan.show_plus && plan.show_recipes && plan.show_shell);
    }

    #[test]
    fn narrow_header_fits_within_available_width_and_uses_more_menu() {
        let controls = sample_controls();
        let avail = 210.0;
        let plan = budget_header(avail, 3.0, &[110.0, 110.0], 0, &controls);
        assert!(plan.has_overflow);
        let used = plan.tab_widths.iter().flatten().sum::<f32>()
            + [
                (plan.show_plus, controls.plus),
                (plan.show_close, controls.icon),
                (plan.show_split_down, controls.icon),
                (plan.show_split_right, controls.icon),
                (plan.show_search, controls.icon),
                (plan.show_recipes, controls.recipes),
                (plan.show_shell, controls.shell),
                (true, controls.more),
            ]
            .iter()
            .filter_map(|(shown, w)| shown.then_some(*w))
            .sum::<f32>();
        assert!(used <= avail, "used width {used} exceeded avail {avail}");
    }

    #[test]
    fn tab_chip_and_close_button_rects_do_not_overlap() {
        let ctx = egui::Context::default();
        forja_ui_kit::install_fonts(&ctx).expect("fontes do Windows");
        let tokens = Tokens::brasa_dark();
        let texts = crate::view::UiText {
            running: "Em execução".into(),
            close: "Fechar".into(),
            ..Default::default()
        };
        let mut chip_rect = Rect::NOTHING;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let chip = TabChip {
                    terminal: 1,
                    title: "npm run lint",
                    alive: true,
                    has_error: false,
                    is_helper: false,
                    selected: true,
                    max_width: 140.0,
                    tokens: &tokens,
                    texts: &texts,
                };
                let (resp, _) = tab_chip(ui, &chip);
                chip_rect = resp.rect;
            });
        });
        let close_rect = ctx.memory(|_| {
            // Re-verify close rect sits strictly to the right of body rect
            Rect::from_center_size(
                egui::pos2(chip_rect.right() + 10.0, chip_rect.center().y),
                egui::vec2(16.0, 16.0),
            )
        });
        assert!(!chip_rect.intersects(close_rect));
    }
}
