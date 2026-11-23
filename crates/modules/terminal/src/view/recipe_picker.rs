//! Seletor de receitas do terminal: filtro, grupos e prévia do comando.
//!
//! As receitas que combinam com os arquivos da bancada ficam em primeiro
//! plano; as demais vivem na seção "Outras", recolhida. A navegação é por
//! setas e Enter, e a prévia mostra o comando exato e a pasta de execução.

use super::chrome::c32;
use super::UiText;
use crate::core::Hub;
use crate::project::{self, ProjectKind};
use crate::recipes::Recipe;
use crate::settings::TermSettings;
use forja_kernel::ui_contract::egui::{self, Align2, FontId, Rect, RichText, Sense};
use forja_ui_kit::{applied_tokens, ButtonKind};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Desenha o seletor quando ele está aberto.
pub(super) fn draw(
    hub: &mut Hub,
    ui: &mut egui::Ui,
    texts: &UiText,
    settings: &TermSettings,
    run_recipe: &mut Option<Recipe>,
    toolbar_bottom: f32,
) {
    if !hub.recipe_picker.open {
        return;
    }
    let recipes: Vec<Recipe> = hub
        .recipes
        .iter()
        .filter(|recipe| settings.recipe_listed(&recipe.scope))
        .cloned()
        .collect();
    let dirs: BTreeMap<String, PathBuf> = recipes
        .iter()
        .map(|recipe| (recipe.id.clone(), hub.recipe_target_dir(recipe, settings)))
        .collect();
    let profile = hub.project_profile.clone();
    let picker = &mut hub.recipe_picker;
    let opened_this_frame = picker.focus_query;
    let layer = egui::Id::new("forja-terminal-recipe-picker.keyboard");
    if forja_ui_kit::consume_layer_escape(ui.ctx(), layer) {
        picker.open = false;
        return;
    }
    let mut chosen = None;
    let mut bounds = ui.ctx().content_rect();
    bounds.min.y = (toolbar_bottom + forja_ui_kit::Spacing::SM).min(bounds.bottom());
    let window = egui::Window::new(texts.recipes.clone())
        .id(egui::Id::new("forja-terminal-recipe-picker"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .constrain_to(bounds)
        .show(ui.ctx(), |ui| {
            forja_ui_kit::register_keyboard_layer(ui, layer);
            ui.set_min_width(620.0);
            ui.set_max_width(760.0);
            let (applicable, others) = project::arrange(&recipes, &profile, &picker.query);
            let empty_message = if recipes.is_empty() {
                &texts.recipe_empty
            } else {
                &texts.recipe_picker_empty
            };
            let mut show_others = picker.others_open;
            let total = applicable.len() + if show_others { others.len() } else { 0 };
            picker.selected = picker.selected.min(total.saturating_sub(1));
            let first_frame = std::mem::take(&mut picker.focus_query);
            let mut moved = first_frame;
            let down = ui
                .input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
            let up =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
            let home =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Home));
            let end =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::End));
            if down {
                if !picker.others_open
                    && !others.is_empty()
                    && (applicable.is_empty() || picker.selected + 1 >= applicable.len())
                {
                    picker.others_open = true;
                    picker.selected = applicable.len();
                    moved = true;
                } else if total > 0 {
                    picker.selected = (picker.selected + 1).min(total - 1);
                    moved = true;
                }
            }
            if up && total > 0 {
                picker.selected = picker.selected.saturating_sub(1);
                moved = true;
            }
            if home && total > 0 {
                picker.selected = 0;
                moved = true;
            }
            if end && (!applicable.is_empty() || !others.is_empty()) {
                if !others.is_empty() {
                    picker.others_open = true;
                }
                picker.selected = applicable.len() + others.len() - 1;
                moved = true;
            }

            let mut refocus_search = false;
            ui.horizontal(|ui| {
                if !applicable.is_empty() {
                    let header = if profile.is_empty() {
                        format!("{} ({})", texts.recipe_picker_project, applicable.len())
                    } else {
                        format!(
                            "{} · {} ({})",
                            texts.recipe_picker_project,
                            profile
                                .kinds()
                                .map(|kind| texts.recipe_kind(kind))
                                .collect::<Vec<_>>()
                                .join(", "),
                            applicable.len()
                        )
                    };
                    ui.label(RichText::new(header).strong());
                }
                if !others.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let chevron = if picker.others_open {
                            forja_ui_kit::Icon::ChevronDown
                        } else {
                            forja_ui_kit::Icon::ChevronRight
                        };
                        let label = format!("{} ({})", texts.recipe_picker_others, others.len());
                        if forja_ui_kit::button_with_icon(ui, chevron, &label, ButtonKind::Ghost)
                            .clicked()
                        {
                            picker.others_open = !picker.others_open;
                            refocus_search = true;
                            if !picker.others_open {
                                picker.selected =
                                    picker.selected.min(applicable.len().saturating_sub(1));
                            }
                        }
                    });
                }
            });
            show_others = picker.others_open;
            picker.selected = picker.selected.min(
                (applicable.len() + usize::from(show_others) * others.len()).saturating_sub(1),
            );
            let visible_recipe = |index: usize| -> Option<&Recipe> {
                if index < applicable.len() {
                    applicable.get(index).copied()
                } else if show_others {
                    others.get(index - applicable.len()).copied()
                } else {
                    None
                }
            };
            let enter =
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if enter {
                chosen = visible_recipe(picker.selected).cloned();
            }

            let search = ui.add(
                egui::TextEdit::singleline(&mut picker.query)
                    .hint_text(texts.recipe_picker_filter.clone())
                    .desired_width(f32::INFINITY),
            );
            if first_frame || refocus_search {
                search.request_focus();
            }
            if search.changed() {
                picker.selected = 0;
            }
            ui.add_space(4.0);

            let mut hovered = None;
            let mut clicked = None;
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if applicable.is_empty() && others.is_empty() {
                        ui.label(RichText::new(empty_message).weak());
                        return;
                    }
                    if !applicable.is_empty() {
                        let hit = draw_group(ui, texts, &applicable, 0, picker.selected, moved);
                        hovered = hit.hovered;
                        clicked = hit.clicked;
                    }
                    if show_others && !others.is_empty() {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.label(RichText::new(&texts.recipe_picker_others).small().strong());
                        let hit = draw_group(
                            ui,
                            texts,
                            &others,
                            applicable.len(),
                            picker.selected,
                            moved,
                        );
                        hovered = hovered.or(hit.hovered);
                        clicked = clicked.or(hit.clicked);
                    }
                });
            if let Some(index) = hovered {
                picker.selected = index;
            }
            if let Some(index) = clicked {
                picker.selected = index;
            }

            ui.separator();
            if let Some(recipe) = visible_recipe(picker.selected) {
                let dir = dirs
                    .get(&recipe.id)
                    .map(|path| path.display().to_string())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{}: {}",
                                texts.recipe_picker_command, recipe.command
                            ))
                            .monospace(),
                        );
                        ui.label(
                            RichText::new(format!("{}: {dir}", texts.recipe_directory))
                                .small()
                                .weak(),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if forja_ui_kit::button(ui, ButtonKind::Primary, &texts.recipe_picker_run)
                            .clicked()
                        {
                            chosen = Some(recipe.clone());
                        }
                    });
                });
            } else if applicable.is_empty() && others.is_empty() {
                ui.label(RichText::new(empty_message).weak());
            }
            ui.label(RichText::new(&texts.recipe_picker_hint).small().weak());
        });
    let clicked_outside = !opened_this_frame
        && window.as_ref().is_some_and(|window| {
            ui.ctx().input(|input| {
                input.pointer.any_click()
                    && input
                        .pointer
                        .interact_pos()
                        .is_some_and(|position| !window.response.rect.contains(position))
            })
        });
    picker.open = chosen.is_none() && !clicked_outside;
    if let Some(recipe) = chosen {
        *run_recipe = Some(recipe);
    }
}

struct Hit {
    hovered: Option<usize>,
    clicked: Option<usize>,
}

fn draw_group(
    ui: &mut egui::Ui,
    texts: &UiText,
    recipes: &[&Recipe],
    offset: usize,
    selected: usize,
    scroll: bool,
) -> Hit {
    let mut hit = Hit {
        hovered: None,
        clicked: None,
    };
    let mut last: Option<Option<ProjectKind>> = None;
    for (index, recipe) in (offset..).zip(recipes.iter().copied()) {
        let kind = ProjectKind::of(recipe);
        if last != Some(kind) {
            let label = match kind {
                Some(kind) => texts.recipe_kind(kind),
                None => texts.recipe_picker_untyped.as_str(),
            };
            ui.add_space(2.0);
            ui.label(RichText::new(label).small().weak());
            last = Some(kind);
        }
        let response = draw_row(ui, texts, recipe, index == selected, scroll);
        if response.hovered() {
            hit.hovered = Some(index);
        }
        if response.clicked() {
            hit.clicked = Some(index);
        }
    }
    hit
}

fn draw_row(
    ui: &mut egui::Ui,
    texts: &UiText,
    recipe: &Recipe,
    selected: bool,
    scroll: bool,
) -> egui::Response {
    let tokens = applied_tokens(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), Sense::click());
    let scope = if recipe.scope == "project" {
        &texts.recipe_scope_project
    } else {
        &texts.recipe_scope_global
    };
    let label = format!(
        "{}  ·  {scope}  ·  {}",
        recipe.title(&texts.locale),
        recipe.command
    );
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone()));
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(egui::vec2(2.0, 1.0)),
            egui::CornerRadius::same(5),
            if selected {
                c32(tokens.color.accent_soft)
            } else {
                c32(tokens.color.surface_alt)
            },
        );
    }
    if scroll && selected {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    let left = rect.left() + 10.0;
    let right = rect.right() - 78.0;
    let title_rect = Rect::from_min_max(
        egui::pos2(left, rect.top() + 3.0),
        egui::pos2(right, rect.top() + 23.0),
    );
    ui.painter().with_clip_rect(title_rect).text(
        egui::pos2(left, rect.top() + 4.0),
        Align2::LEFT_TOP,
        recipe.title(&texts.locale),
        forja_ui_kit::TextRole::Body.font(),
        c32(tokens.color.text),
    );
    let command_rect = Rect::from_min_max(
        egui::pos2(left, rect.top() + 23.0),
        egui::pos2(right, rect.bottom()),
    );
    ui.painter().with_clip_rect(command_rect).text(
        egui::pos2(left, rect.top() + 24.0),
        Align2::LEFT_TOP,
        &recipe.command,
        FontId::monospace(forja_ui_kit::TextRole::Caption.size()),
        c32(tokens.color.text_muted),
    );
    ui.painter().text(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        scope,
        forja_ui_kit::TextRole::Caption.font(),
        c32(tokens.color.text_muted),
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}
