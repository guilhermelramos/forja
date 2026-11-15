//! Chrome da aba de terminal: abas, divisores e confirmações.

use super::chrome::{c32, icon_button, recipes_menu, shell_menu, tab_chip, TabChip};
use super::terminal_ui::draw_terminal;
use super::{UiText, ViewAction};
use crate::core::{CloseTarget, Hub, PendingPaste};
use crate::links::Link;
use crate::panes::tree::{first_pane, PaneId, PaneNode, SplitDir, TerminalId};
use crate::recipes::Recipe;
use crate::settings::TermSettings;
use crate::terminal::Terminal;
use crate::theme::Palette;
use forja_kernel::ui_contract::egui::{
    self, Rect, RichText, Sense, Stroke, UiBuilder,
};
use forja_ui_kit::{applied_tokens, Tokens};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Controle que só o hub pode executar depois de desenhar a árvore.
pub(super) enum Control {
    Split(SplitDir),
    NextPane,
    PreviousPane,
    ClosePane,
    CloseTab(PaneId, TerminalId),
}

/// Desenha todos os painéis.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    hub: &mut Hub,
    ui: &mut egui::Ui,
    settings: &TermSettings,
    texts: &UiText,
    wsl: &[String],
    textures: &mut HashMap<u32, egui::TextureHandle>,
    approve_recipe: &mut dyn FnMut(&Recipe) -> Result<(), String>,
    actions: &mut Vec<ViewAction>,
) {
    let tokens = applied_tokens(ui.ctx());
    let palette = hub
        .themes
        .palette(
            &settings.theme_name,
            settings.theme_follow_app,
            tokens.mode == forja_ui_kit::Mode::Dark,
        )
        .with_opacity(settings.background_opacity());
    let recipes: Vec<Recipe> = hub
        .recipes
        .iter()
        .filter(|recipe| settings.recipe_listed(&recipe.scope))
        .cloned()
        .collect();
    if let Some(notice) = hub.notice.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(notice).color(c32(tokens.color.spark)));
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &texts.notice_dismiss).clicked() {
                hub.notice = None;
            }
        });
    }
    if hub.root.is_none() {
        forja_ui_kit::page_heading(ui, &texts.empty_title, None, |_| {});
        let open = forja_ui_kit::empty_state(
            ui,
            forja_ui_kit::TERMINAL,
            &texts.empty_hint,
            Some(&texts.open_terminal),
        )
        .is_some_and(|response| response.clicked());
        if open {
            if let Err(error) = hub.open(settings, crate::core::OpenRequest::default()) {
                hub.notice = Some(error);
            }
        }
        return;
    }
    let mut root = hub.root.take();
    let mut focused = hub.focused;
    let mut controls = Vec::new();
    let mut close_targets = Vec::new();
    let mut new_tab = false;
    let mut new_tab_shell = None;
    let mut run_recipe = None;
    if let Some(node) = &mut root {
        let rect = ui.available_rect_before_wrap();
        draw_node(
            node,
            &mut hub.terminals,
            &mut focused,
            ui,
            rect,
            &tokens,
            &palette,
            settings,
            texts,
            textures,
            actions,
            &mut controls,
            &mut close_targets,
            &mut new_tab,
            &mut new_tab_shell,
            &mut run_recipe,
            wsl,
            &recipes,
            &hub.helper_terminals,
            &mut hub.pending_link,
            &mut hub.pending_paste,
        );
    }
    hub.root = root;
    if hub.focused != focused {
        hub.focused = focused;
        if let Some(terminal) = hub.focused_terminal_mut() {
            terminal.focus_request = true;
        }
    }
    for target in close_targets {
        hub.request_close(target, settings);
    }
    if new_tab {
        if let Err(error) = hub.new_tab(settings, new_tab_shell.as_deref()) {
            hub.notice = Some(error);
        }
    }
    if let Some(recipe) = run_recipe {
        match approve_recipe(&recipe) {
            Ok(()) if settings.recipe_confirm => hub.pending_recipe = Some(recipe),
            Ok(()) => {
                if let Err(error) = hub.run_recipe(&recipe, settings) {
                    hub.notice = Some(error);
                }
            }
            Err(error) => {
                if recipe.scope == "project" {
                    hub.pending_trust_recipe = Some(recipe);
                }
                hub.notice = Some(error);
            }
        }
    }
    for control in controls {
        match control {
            Control::Split(direction) => {
                if hub.pane_count() >= crate::panes::tree::MAX_PANES {
                    hub.notice = Some(texts.max_panes.clone());
                } else if let Err(error) = hub.split(direction, settings) {
                    hub.notice = Some(error);
                }
            }
            Control::NextPane => hub.focus_next(),
            Control::PreviousPane => hub.focus_previous(),
            Control::ClosePane => {
                let pane = hub.focused;
                hub.request_close(CloseTarget::Pane(pane), settings);
            }
            Control::CloseTab(pane, terminal) => {
                hub.request_close(CloseTarget::Tab(pane, terminal), settings);
            }
        }
    }
    if hub.pending_close.is_some() {
        draw_close_modal(hub, ui.ctx(), &tokens, texts);
    }
    if hub.pending_recipe.is_some() {
        draw_recipe_modal(hub, ui.ctx(), &tokens, texts, settings);
    }
}

/// Limites da proporção de uma divisão, dadas as medidas mínimas de cada lado
/// e a extensão disponível (já sem o divisor).
///
/// No tamanho mínimo exato, `min_a / available` e `1.0 - min_b / available`
/// são o mesmo número, mas o arredondamento pode deixar o segundo um ULP
/// menor. `f32::clamp` entra em pânico quando `min > max`, então o limite
/// superior nunca pode ficar abaixo do inferior. Refs #70, #71.
fn split_bounds(min_a: f32, min_b: f32, available: f32) -> (f32, f32) {
    if available > 0.0 && available >= min_a + min_b {
        let lower = (min_a / available).clamp(0.0, 1.0);
        let upper = (1.0 - min_b / available).max(lower).min(1.0);
        (lower, upper)
    } else {
        (0.5, 0.5)
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    node: &mut PaneNode,
    terminals: &mut BTreeMap<TerminalId, Terminal>,
    focused: &mut PaneId,
    ui: &mut egui::Ui,
    rect: Rect,
    tokens: &Tokens,
    palette: &Palette,
    settings: &TermSettings,
    texts: &UiText,
    textures: &mut HashMap<u32, egui::TextureHandle>,
    actions: &mut Vec<ViewAction>,
    controls: &mut Vec<Control>,
    close_targets: &mut Vec<CloseTarget>,
    new_tab: &mut bool,
    new_tab_shell: &mut Option<String>,
    run_recipe: &mut Option<Recipe>,
    wsl: &[String],
    recipes: &[Recipe],
    helper_terminals: &BTreeSet<TerminalId>,
    pending_link: &mut Option<(TerminalId, Link)>,
    pending_paste: &mut Option<PendingPaste>,
) {
    match node {
        PaneNode::Split {
            dir,
            ratio,
            first,
            second,
        } => {
            let horizontal = *dir == SplitDir::Columns;
            let extent = if horizontal { rect.width() } else { rect.height() };
            let thickness = 5.0;
            let available = (extent - thickness).max(0.0);
            let a = first.minimum_size();
            let b = second.minimum_size();
            let min_a = if horizontal { a.x } else { a.y };
            let min_b = if horizontal { b.x } else { b.y };
            let (lower, upper) = split_bounds(min_a, min_b, available);
            let current = if ratio.is_finite() { *ratio } else { lower };
            *ratio = current.clamp(lower, upper);
            let size = available * *ratio;
            let mut divider = rect;
            if horizontal {
                divider.min.x += size;
                divider.max.x = divider.min.x + thickness;
            } else {
                divider.min.y += size;
                divider.max.y = divider.min.y + thickness;
            }
            let response = ui.interact(
                divider,
                ui.id().with(("terminal-divider", first_pane(first), first_pane(second))),
                Sense::drag(),
            );
            if response.dragged() && available > 0.0 {
                let delta = response.drag_delta();
                *ratio = (*ratio + (if horizontal { delta.x } else { delta.y }) / available)
                    .clamp(lower, upper);
                ui.ctx().request_repaint();
            }
            let response = response.on_hover_cursor(if horizontal {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::ResizeVertical
            });
            let line = if horizontal {
                Rect::from_center_size(divider.center(), egui::vec2(thickness, divider.height()))
            } else {
                Rect::from_center_size(divider.center(), egui::vec2(divider.width(), thickness))
            };
            ui.painter().rect_filled(
                line,
                0.0,
                if response.hovered() || response.dragged() {
                    c32(tokens.color.accent)
                } else {
                    c32(tokens.color.border)
                },
            );
            let mut left = rect;
            let mut right = rect;
            if horizontal {
                left.max.x = divider.min.x;
                right.min.x = divider.max.x;
            } else {
                left.max.y = divider.min.y;
                right.min.y = divider.max.y;
            }
            draw_node(
                first,
                terminals,
                focused,
                ui,
                left,
                tokens,
                palette,
                settings,
                texts,
                textures,
                actions,
                controls,
                close_targets,
                new_tab,
                new_tab_shell,
                run_recipe,
                wsl,
                recipes,
                helper_terminals,
                pending_link,
                pending_paste,
            );
            draw_node(
                second,
                terminals,
                focused,
                ui,
                right,
                tokens,
                palette,
                settings,
                texts,
                textures,
                actions,
                controls,
                close_targets,
                new_tab,
                new_tab_shell,
                run_recipe,
                wsl,
                recipes,
                helper_terminals,
                pending_link,
                pending_paste,
            );
        }
        PaneNode::Leaf { pane, tabs, active } => {
            if ui.rect_contains_pointer(rect)
                && ui.input(|input| input.pointer.any_pressed())
            {
                *focused = *pane;
            }
            ui.painter().rect(
                rect.shrink(0.5),
                egui::CornerRadius::same(8),
                c32(tokens.color.surface),
                Stroke::new(1.0_f32, c32(tokens.color.border)),
                egui::StrokeKind::Inside,
            );
            let inner = rect.shrink(4.0);
            ui.scope_builder(
                UiBuilder::new()
                    .id_salt(("terminal-pane", *pane))
                    .max_rect(inner),
                |ui| {
                    ui.set_clip_rect(inner.intersect(ui.clip_rect()));
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 3.0);
                    let mut close_tab = None;
                    // Quebra de linha: com o painel estreito, o grupo de ícones e menus desce em vez de sair da área.
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        for (index, terminal_id) in tabs.iter().enumerate() {
                            let Some(terminal) = terminals.get(terminal_id) else {
                                continue;
                            };
                            let selected = index == *active;
                            let (response, close) = tab_chip(
                                ui,
                                &TabChip {
                                    tokens,
                                    title: &terminal.title(),
                                    alive: terminal.alive(),
                                    has_error: terminal.error.is_some(),
                                    is_helper: helper_terminals.contains(terminal_id),
                                    selected,
                                    terminal: *terminal_id,
                                    texts,
                                },
                            );
                            if close || response.middle_clicked() {
                                close_tab = Some(*terminal_id);
                            } else if response.clicked() {
                                *active = index;
                                *focused = *pane;
                                if let Some(terminal) = terminals.get_mut(terminal_id) {
                                    terminal.focus_request = true;
                                }
                            }
                        }
                        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, "+")
                            .on_hover_text(texts.new_tab.clone())
                            .clicked()
                        {
                            *new_tab = true;
                            *new_tab_shell = None;
                        }
                        ui.horizontal(|ui| {
                            if icon_button(ui, forja_ui_kit::CLOSE, &texts.close_pane).clicked() {
                                *focused = *pane;
                                controls.push(Control::ClosePane);
                            }
                            if icon_button(ui, forja_ui_kit::SPLIT_DOWN, &texts.split_down).clicked()
                            {
                                *focused = *pane;
                                controls.push(Control::Split(SplitDir::Rows));
                            }
                            if icon_button(ui, forja_ui_kit::SPLIT_RIGHT, &texts.split_right)
                                .clicked()
                            {
                                *focused = *pane;
                                controls.push(Control::Split(SplitDir::Columns));
                            }
                            if icon_button(ui, forja_ui_kit::SEARCH, &texts.search).clicked() {
                                if let Some(terminal) = tabs
                                    .get(*active)
                                    .and_then(|id| terminals.get_mut(id))
                                {
                                    terminal.toggle_search();
                                }
                            }
                            recipes_menu(ui, texts, recipes, run_recipe);
                            shell_menu(
                                ui,
                                texts,
                                wsl,
                                new_tab,
                                new_tab_shell,
                            );
                        });
                    });
                    if let Some(terminal_id) = close_tab {
                        close_targets.push(CloseTarget::Tab(*pane, terminal_id));
                    }
                    ui.add_space(2.0);
                    let body = ui.available_rect_before_wrap().intersect(ui.clip_rect());
                    if let Some(terminal) = tabs
                        .get(*active)
                        .and_then(|id| terminals.get_mut(id))
                    {
                        draw_terminal(
                            ui,
                            terminal,
                            *pane,
                            settings,
                            texts,
                            palette,
                            textures,
                            body,
                            *focused == *pane,
                            actions,
                            controls,
                            pending_link,
                            pending_paste,
                        );
                    } else {
                        ui.label(RichText::new(texts.empty_title.clone()).weak());
                    }
                },
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Confirmação antes de rodar uma receita (`terminal.recipes.confirm`).
fn draw_recipe_modal(
    hub: &mut Hub,
    ctx: &egui::Context,
    tokens: &Tokens,
    texts: &UiText,
    settings: &TermSettings,
) {
    let Some(recipe) = hub.pending_recipe.clone() else {
        return;
    };
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new(texts.recipe_confirm_title.clone())
        .id(egui::Id::new("forja-terminal-recipe-confirm"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(texts.recipe_confirm_body.clone());
            ui.label(
                RichText::new(&recipe.command)
                    .monospace()
                    .color(c32(tokens.color.text_muted)),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &texts.recipe_confirm_accept).clicked() {
                    accept = true;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &texts.cancel).clicked() {
                    cancel = true;
                }
            });
        });
    if accept {
        hub.pending_recipe = None;
        if let Err(error) = hub.run_recipe(&recipe, settings) {
            hub.notice = Some(error);
        }
    } else if cancel {
        hub.pending_recipe = None;
    }
}

fn draw_close_modal(hub: &mut Hub, ctx: &egui::Context, tokens: &Tokens, texts: &UiText) {
    let Some(pending) = hub.pending_close else {
        return;
    };
    let mut accept = false;
    let mut cancel = false;
    egui::Window::new(texts.confirm_close_title.clone())
        .id(egui::Id::new("forja-terminal-close-confirm"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(texts.confirm_close_body.clone());
            ui.label(
                RichText::new(format!("{}: {}", texts.running, pending.running))
                    .color(c32(tokens.color.spark)),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Danger, &texts.confirm_close_accept).clicked() {
                    accept = true;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &texts.cancel).clicked() {
                    cancel = true;
                }
            });
        });
    if accept {
        hub.confirm_close();
    } else if cancel {
        hub.cancel_close();
    }
}

#[cfg(test)]
mod tests {
    use super::split_bounds;

    #[test]
    fn bounds_never_invert_at_the_minimum_size() {
        // Values from the panic in #70/#71: 245 + 370 == 615, and the two
        // divisions differ by one ULP in f32.
        let (lower, upper) = split_bounds(245.0, 370.0, 615.0);
        assert!(lower <= upper, "lower {lower:?} > upper {upper:?}");
        assert!((lower - 0.398_374).abs() < 1e-5);
    }

    #[test]
    fn bounds_never_invert_for_the_second_panic_signature() {
        // Values from #71: 140 + 285 == 425, and `1.0 - 285/425` is one ULP
        // below `140/425`.
        let (lower, upper) = split_bounds(140.0, 285.0, 425.0);
        assert!(lower <= upper, "lower {lower:?} > upper {upper:?}");
        assert!((lower - 0.329_411_77).abs() < 1e-6);
    }

    #[test]
    fn bounds_stay_ordered_for_other_rounding_cases() {
        for (min_a, min_b) in [(1.0, 2.0), (100.0, 102.0), (140.0, 142.0)] {
            let available = min_a + min_b;
            let (lower, upper) = split_bounds(min_a, min_b, available);
            assert!(
                lower <= upper,
                "min_a {min_a} min_b {min_b}: {lower:?} > {upper:?}"
            );
        }
    }

    #[test]
    fn too_small_pane_keeps_the_default_ratio() {
        assert_eq!(split_bounds(140.0, 140.0, 200.0), (0.5, 0.5));
        assert_eq!(split_bounds(140.0, 140.0, 0.0), (0.5, 0.5));
    }
}
