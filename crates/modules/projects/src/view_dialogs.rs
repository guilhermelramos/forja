use crate::state::{self, CloneForm, Dialog, ProjectForm, Shared};
use crate::view::{humanize_error, parse_color, ColorExt};
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;
use std::path::PathBuf;

fn render_dialog_error(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    tokens: &forja_ui_kit::Tokens,
    error: &mut Option<String>,
) {
    let Some(raw) = error.as_deref() else {
        return;
    };
    let message = humanize_error(cx, raw);
    let mut dismissed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(message)
                .small()
                .color(tokens.color.danger.forja_color()),
        );
        if forja_ui_kit::icon_button(ui, forja_ui_kit::CLOSE)
            .on_hover_text(cx.t("project.error.dismiss"))
            .clicked()
        {
            dismissed = true;
        }
    });
    if dismissed {
        *error = None;
    }
}
pub(crate) fn render_dialog(
    ctx: &egui::Context,
    dialog: &mut Dialog,
    cx: &mut Ctx<'_>,
    shared: &Shared,
) -> Dialog {
    let tokens = forja_ui_kit::applied_tokens(ctx);
    let creating_form = matches!(dialog, Dialog::Create(_));
    // Closing replaces the dialog; otherwise the edited form (typing, picked folder) is kept.
    let mut next: Option<Dialog> = None;
    match dialog {
        Dialog::None => {}
        Dialog::Remove { project_id, name } => {
            let mut close = false;
            let mut remove = false;
            egui::Window::new(cx.t("project.dialog.remove.title"))
                .id(egui::Id::new("forja.projects.remove"))
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .show(ctx, |ui| {
                    ui.label(cx.t_args("project.dialog.remove.body", &[("name", name)]));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Danger,
                            &cx.t("project.action.remove"),
                        )
                        .clicked()
                        {
                            remove = true;
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("project.action.cancel"),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });
            if remove {
                match shared.remove(cx, project_id) {
                    Ok(()) => {
                        shared.set_error(None);
                        next = Some(Dialog::None);
                    }
                    Err(error) => shared.set_error(Some(error)),
                }
            } else if close {
                next = Some(Dialog::None);
            }
        }
        Dialog::Add(form) | Dialog::Create(form) => {
            let creating = creating_form;
            let layer = egui::Id::new(if creating {
                "forja.projects.create.keyboard"
            } else {
                "forja.projects.add.keyboard"
            });
            if forja_ui_kit::consume_layer_escape(ctx, layer) {
                return Dialog::None;
            }
            let title = if creating {
                cx.t("project.dialog.create.title")
            } else {
                cx.t("project.dialog.add.title")
            };
            let mut close = false;
            let mut submit = false;
            egui::Window::new(title)
                .id(egui::Id::new(if creating {
                    "forja.projects.create"
                } else {
                    "forja.projects.add"
                }))
                .collapsible(false)
                .resizable(false)
                .default_width(420.0)
                .show(ctx, |ui| {
                    forja_ui_kit::register_keyboard_layer(ui, layer);
                    ui.label(
                        egui::RichText::new(cx.t(if creating {
                            "project.field.new-path"
                        } else {
                            "project.field.path"
                        }))
                        .small()
                        .color(tokens.color.text_muted.forja_color()),
                    );
                    forja_ui_kit::text_field(ui, &mut form.path, "C:\\dev\\projeto");
                    if ui
                        .add(egui::Button::new(cx.t("project.action.choose")).frame(false))
                        .clicked()
                    {
                        if let Some(path) = pick_folder(form.path.trim()) {
                            form.path = path.to_string_lossy().into_owned();
                        }
                    }
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(cx.t("project.field.name"))
                            .small()
                            .color(tokens.color.text_muted.forja_color()),
                    );
                    forja_ui_kit::text_field(ui, &mut form.name, &cx.t("project.field.name-hint"));
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(cx.t("project.field.icon"))
                                    .small()
                                    .color(tokens.color.text_muted.forja_color()),
                            );
                            ui.add_sized([52.0, 28.0], egui::TextEdit::singleline(&mut form.icon));
                        });
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(cx.t("project.field.color"))
                                    .small()
                                    .color(tokens.color.text_muted.forja_color()),
                            );
                            ui.horizontal(|ui| {
                                ui.add_sized(
                                    [120.0, 28.0],
                                    egui::TextEdit::singleline(&mut form.color),
                                );
                                let color = parse_color(&form.color)
                                    .unwrap_or(tokens.color.text_muted.forja_color());
                                let (swatch, _) = ui.allocate_exact_size(
                                    egui::vec2(24.0, 24.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(swatch, 4, color);
                            });
                        });
                    });
                    render_dialog_error(ui, cx, &tokens, &mut form.error);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let submit_label = if creating {
                            cx.t("project.action.create")
                        } else {
                            cx.t("project.action.add")
                        };
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Primary,
                            &submit_label,
                        )
                        .clicked()
                        {
                            submit = true;
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("project.action.cancel"),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });
            if submit {
                let result = if creating {
                    start_create(cx, shared, form)
                } else {
                    let new = crate::model::NewProject {
                        path: form.path.clone(),
                        name: form.name.clone(),
                        icon: form.icon.clone(),
                        color: form.color.clone(),
                    };
                    shared.add(cx, &new).map(|_| ())
                };
                match result {
                    Ok(()) => next = Some(Dialog::None),
                    Err(error) => form.error = Some(error),
                }
            } else if close {
                next = Some(Dialog::None);
            }
        }
        Dialog::Clone(clone) => {
            let layer = egui::Id::new("forja.projects.clone.keyboard");
            if forja_ui_kit::consume_layer_escape(ctx, layer) {
                return Dialog::None;
            }
            let mut close = false;
            let mut submit = false;
            egui::Window::new(cx.t("project.dialog.clone.title"))
                .id(egui::Id::new("forja.projects.clone"))
                .collapsible(false)
                .resizable(false)
                .default_width(440.0)
                .show(ctx, |ui| {
                    forja_ui_kit::register_keyboard_layer(ui, layer);
                    ui.label(
                        egui::RichText::new(cx.t("project.field.url"))
                            .small()
                            .color(tokens.color.text_muted.forja_color()),
                    );
                    forja_ui_kit::text_field(ui, &mut clone.url, "https://host/owner/repo.git");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(cx.t("project.field.destination"))
                            .small()
                            .color(tokens.color.text_muted.forja_color()),
                    );
                    forja_ui_kit::text_field(ui, &mut clone.form.path, "C:\\dev\\repo");
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(cx.t("project.field.name"))
                            .small()
                            .color(tokens.color.text_muted.forja_color()),
                    );
                    forja_ui_kit::text_field(
                        ui,
                        &mut clone.form.name,
                        &cx.t("project.field.name-hint"),
                    );
                    render_dialog_error(ui, cx, &tokens, &mut clone.form.error);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Primary,
                            &cx.t("project.action.clone"),
                        )
                        .clicked()
                        {
                            submit = true;
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("project.action.cancel"),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });
            if submit {
                match start_clone(cx, shared, clone) {
                    Ok(()) => next = Some(Dialog::None),
                    Err(error) => clone.form.error = Some(error),
                }
            } else if close {
                next = Some(Dialog::None);
            }
        }
        Dialog::Edit(form) => {
            let mut close = false;
            let mut submit = false;
            egui::Window::new(cx.t("project.dialog.edit.title"))
                .id(egui::Id::new("forja.projects.edit"))
                .collapsible(false)
                .resizable(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    forja_ui_kit::text_field(ui, &mut form.name, &cx.t("project.field.name-hint"));
                    ui.horizontal(|ui| {
                        ui.add_sized([52.0, 28.0], egui::TextEdit::singleline(&mut form.icon));
                        ui.add_sized([120.0, 28.0], egui::TextEdit::singleline(&mut form.color));
                        let color = parse_color(&form.color)
                            .unwrap_or(tokens.color.text_muted.forja_color());
                        let (swatch, _) =
                            ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
                        ui.painter().rect_filled(swatch, 4, color);
                    });
                    render_dialog_error(ui, cx, &tokens, &mut form.error);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Primary,
                            &cx.t("project.action.save"),
                        )
                        .clicked()
                        {
                            submit = true;
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("project.action.cancel"),
                        )
                        .clicked()
                        {
                            close = true;
                        }
                    });
                });
            if submit {
                match shared.update(
                    cx,
                    &form.project_id,
                    Some(&form.name),
                    Some(&form.icon),
                    Some(&form.color),
                ) {
                    Ok(_) => {
                        shared.set_error(None);
                        next = Some(Dialog::None);
                    }
                    Err(error) => form.error = Some(error),
                }
            } else if close {
                next = Some(Dialog::None);
            }
        }
    }
    next.unwrap_or_else(|| dialog.clone())
}

fn start_create(cx: &mut Ctx<'_>, shared: &Shared, form: &ProjectForm) -> Result<(), String> {
    if form.path.trim().is_empty() {
        return Err("path is empty".to_owned());
    }
    let path = form.path.clone();
    let form = form.clone();
    let state = shared.clone();
    shared.set_status(Some(cx.t("project.status.creating")));
    cx.spawn(
        move || state::create_folder(&path),
        move |cx, result| {
            state.set_status(None);
            match result {
                Ok(canonical) => {
                    let new = crate::model::NewProject {
                        path: canonical.to_string_lossy().into_owned(),
                        name: form.name.clone(),
                        icon: form.icon.clone(),
                        color: form.color.clone(),
                    };
                    match state.add(cx, &new) {
                        Ok(_) => state.set_dialog(Dialog::None),
                        Err(error) => state.set_error(Some(error)),
                    }
                }
                Err(error) => state.set_error(Some(error)),
            }
        },
    );
    Ok(())
}

fn start_clone(cx: &mut Ctx<'_>, shared: &Shared, clone: &CloneForm) -> Result<(), String> {
    if clone.url.trim().is_empty() {
        return Err("repository URL is required".to_owned());
    }
    if clone.form.path.trim().is_empty() {
        return Err("destination is required".to_owned());
    }
    let url = clone.url.clone();
    let path = clone.form.path.clone();
    let name = clone.form.name.clone();
    let icon = clone.form.icon.clone();
    let color = clone.form.color.clone();
    let state = shared.clone();
    shared.set_status(Some(cx.t("project.status.cloning")));
    cx.spawn(
        move || state::clone_repository(&url, &path),
        move |cx, result| {
            state.set_status(None);
            match result {
                Ok(canonical) => {
                    let new = crate::model::NewProject {
                        path: canonical.to_string_lossy().into_owned(),
                        name,
                        icon,
                        color,
                    };
                    match state.add(cx, &new) {
                        Ok(_) => state.set_dialog(Dialog::None),
                        Err(error) => state.set_error(Some(error)),
                    }
                }
                Err(error) => state.set_error(Some(error)),
            }
        },
    );
    Ok(())
}

fn pick_folder(current: &str) -> Option<PathBuf> {
    if let Some(answer) = forja_kernel::test_pick::folder() {
        return answer;
    }
    let mut dialog = rfd::FileDialog::new();
    let path = PathBuf::from(current);
    if path.is_dir() {
        dialog = dialog.set_directory(path);
    }
    dialog.pick_folder()
}
