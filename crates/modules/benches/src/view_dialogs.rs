use crate::service;
use crate::state::{Dialog, Shared};
use crate::view::{c32, field_label};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
pub struct DialogsView {
    state: Shared,
}

impl DialogsView {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for DialogsView {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        if matches!(self.state.dialog(), Dialog::None) {
            cx.close_modal("bench.dialogs");
        } else {
            cx.open_modal("bench.dialogs");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ui;
        self.state.ensure_loaded(cx);
        let dialog = self.state.dialog();
        if matches!(dialog, Dialog::None) {
            return;
        }
        let projects = service::projects(cx);
        let mut next = dialog;
        match &mut next {
            Dialog::None => {}
            Dialog::Create(form) => {
                if form.project_id.is_empty() {
                    if let Some(project) = projects.first() {
                        form.project_id = project.id.clone();
                    }
                }
                let mut submit = false;
                let mut cancel = false;
                let mut add_folder = false;
                let title = cx.t("bench.dialog.create.title");
                egui::Window::new(title)
                    .id(egui::Id::new("forja.bench.create"))
                    .collapsible(false)
                    .resizable(false)
                    .default_width(440.0)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_width(440.0);
                        if projects.is_empty() {
                            ui.label(egui::RichText::new(cx.t("bench.notice.no-projects")).color(
                                c32(forja_ui_kit::applied_tokens(ui.ctx()).color.text_muted),
                            ));
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Primary,
                                    &cx.t("bench.dialog.create.add-folder"),
                                )
                                .clicked()
                                {
                                    add_folder = true;
                                }
                                if forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Ghost,
                                    &cx.t("bench.dialog.cancel"),
                                )
                                .clicked()
                                {
                                    cancel = true;
                                }
                            });
                            return;
                        }
                        ui.label(egui::RichText::new(cx.t("bench.dialog.create.project")).small());
                        let names: Vec<&str> = projects
                            .iter()
                            .map(|project| project.name.as_str())
                            .collect();
                        let mut selected = projects
                            .iter()
                            .position(|project| project.id == form.project_id)
                            .unwrap_or(0);
                        forja_ui_kit::select(ui, &mut selected, &names);
                        form.project_id = projects[selected].id.clone();
                        ui.add_space(4.0);
                        field_label(ui, &cx.t("bench.dialog.create.name"));
                        forja_ui_kit::text_field(
                            ui,
                            &mut form.name,
                            &cx.t("bench.dialog.create.name-hint"),
                        );
                        ui.add_space(4.0);
                        field_label(ui, &cx.t("bench.dialog.create.base"));
                        forja_ui_kit::text_field(
                            ui,
                            &mut form.base,
                            &cx.t("bench.dialog.create.base-hint"),
                        );
                        ui.add_space(4.0);
                        let mut fetch = form.fetch.unwrap_or_else(|| {
                            cx.setting("bench.worktree.fetch_before")
                                .and_then(forja_kernel::SettingValue::as_bool)
                                .unwrap_or(true)
                        });
                        forja_ui_kit::toggle(ui, &mut fetch, &cx.t("bench.dialog.create.fetch"));
                        form.fetch = Some(fetch);
                        if let Some(error) = &form.error {
                            ui.label(
                                egui::RichText::new(error).small().color(c32(
                                    forja_ui_kit::applied_tokens(ui.ctx()).color.danger,
                                )),
                            );
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Primary,
                                &cx.t("bench.dialog.create.submit"),
                            )
                            .clicked()
                            {
                                submit = true;
                            }
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Ghost,
                                &cx.t("bench.dialog.cancel"),
                            )
                            .clicked()
                            {
                                cancel = true;
                            }
                        });
                    });
                if add_folder {
                    self.state.set_dialog(Dialog::None);
                    if let Err(error) = cx.run_command("project.add_dialog", Args::new()) {
                        self.state.set_error(Some(error.to_string()));
                    }
                } else if submit {
                    match service::create(
                        cx,
                        &self.state,
                        &form.project_id,
                        &form.name,
                        Some(form.base.as_str()).filter(|value| !value.trim().is_empty()),
                        None,
                        form.fetch,
                    ) {
                        Ok(_) => {
                            self.state.set_dialog(Dialog::None);
                            // The card must show up without waiting for the board timer.
                            let _ = cx.run_command("board.refresh", Args::new());
                        }
                        Err(error) => form.error = Some(error),
                    }
                } else if cancel {
                    self.state.set_dialog(Dialog::None);
                } else {
                    let next_dialog = Dialog::Create(form.clone());
                    self.state.set_dialog(next_dialog);
                }
            }
            Dialog::Store(form) => {
                let mut submit = false;
                let mut cancel = false;
                let title = cx.t("bench.dialog.store.title");
                let mut open = true;
                forja_ui_kit::modal_explicit_close(ui.ctx(), &mut open, &title, |ui| {
                    ui.set_min_width(420.0);
                    ui.label(cx.t_args(
                        "bench.dialog.store.body",
                        &[
                            ("name", form.name.as_str()),
                            ("branch", form.branch.as_deref().unwrap_or("\u{2014}")),
                        ],
                    ));
                    ui.add_space(6.0);
                    forja_ui_kit::toggle(
                        ui,
                        &mut form.delete_branch,
                        &cx.t("bench.dialog.store.delete-branch"),
                    );
                    forja_ui_kit::toggle(
                        ui,
                        &mut form.force,
                        &cx.t("bench.dialog.store.force"),
                    );
                    if let Some(error) = &form.error {
                        ui.label(
                            egui::RichText::new(error).small().color(c32(
                                forja_ui_kit::applied_tokens(ui.ctx()).color.danger,
                            )),
                        );
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Danger,
                            &cx.t("bench.dialog.store.submit"),
                        )
                        .clicked()
                        {
                            submit = true;
                        }
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::Ghost,
                            &cx.t("bench.dialog.cancel"),
                        )
                        .clicked()
                        {
                            cancel = true;
                        }
                    });
                });
                if submit {
                    match service::store(
                        cx,
                        &self.state,
                        &form.bench_id,
                        Some(form.delete_branch),
                        Some(form.force),
                    ) {
                        Ok(()) => self.state.set_dialog(Dialog::None),
                        Err(error) => form.error = Some(error),
                    }
                } else if cancel || !open {
                    self.state.set_dialog(Dialog::None);
                } else {
                    self.state.set_dialog(Dialog::Store(form.clone()));
                }
            }
            Dialog::Approve(form) => {
                let mut submit = false;
                let mut cancel = false;
                let title = cx.t("bench.dialog.approve.title");
                egui::Window::new(title)
                    .id(egui::Id::new("forja.bench.approve"))
                    .collapsible(false)
                    .resizable(false)
                    .default_width(480.0)
                    .show(ui.ctx(), |ui| {
                        ui.set_min_width(480.0);
                        field_label(ui, &cx.t("bench.dialog.approve.setup"));
                        if form.setup.is_empty() {
                            ui.label(cx.t("bench.dialog.approve.none"));
                        } else {
                            ui.label(egui::RichText::new(&form.setup).monospace());
                        }
                        field_label(ui, &cx.t("bench.dialog.approve.teardown"));
                        if form.teardown.is_empty() {
                            ui.label(cx.t("bench.dialog.approve.none"));
                        } else {
                            ui.label(egui::RichText::new(&form.teardown).monospace());
                        }
                        if let Some(hash) = &form.setup_hash {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {}",
                                    cx.t("bench.dialog.approve.hash"),
                                    crate::config::short_hash(hash)
                                ))
                                .small()
                                .weak(),
                            );
                        }
                        if let Some(hash) = &form.teardown_hash {
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {}",
                                    cx.t("bench.dialog.approve.hash"),
                                    crate::config::short_hash(hash)
                                ))
                                .small()
                                .weak(),
                            );
                        }
                        if let Some(error) = &form.error {
                            ui.label(
                                egui::RichText::new(error).small().color(c32(
                                    forja_ui_kit::applied_tokens(ui.ctx()).color.danger,
                                )),
                            );
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Primary,
                                &cx.t("bench.dialog.approve.submit"),
                            )
                            .clicked()
                            {
                                submit = true;
                            }
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Ghost,
                                &cx.t("bench.dialog.cancel"),
                            )
                            .clicked()
                            {
                                cancel = true;
                            }
                        });
                    });
                if submit {
                    match service::approve(cx, &self.state, &form.project_id, Some("both")) {
                        Ok(_) => self.state.set_dialog(Dialog::None),
                        Err(error) => form.error = Some(error),
                    }
                } else if cancel {
                    self.state.set_dialog(Dialog::None);
                } else {
                    self.state.set_dialog(Dialog::Approve(form.clone()));
                }
            }
        }
    }
}
