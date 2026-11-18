use crate::service;
use crate::state::{Dialog, Shared};
use crate::view::{c32, field_label};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
pub struct DialogsView {
    state: Shared,
    create_focused: bool,
}

impl DialogsView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            create_focused: false,
        }
    }
}

impl View for DialogsView {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        let dialog = self.state.dialog();
        if !matches!(dialog, Dialog::Create(_)) {
            self.create_focused = false;
        }
        if matches!(dialog, Dialog::None) {
            cx.close_modal("bench.dialogs");
        } else {
            cx.open_modal("bench.dialogs");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let _ = ui;
        self.state.ensure_loaded(cx);
        let dialog = self.state.dialog();
        if !matches!(dialog, Dialog::Create(_)) {
            self.create_focused = false;
        }
        if matches!(dialog, Dialog::None) {
            return;
        }
        let projects = service::projects(cx);
        let mut next = dialog;
        match &mut next {
            Dialog::None => {}
            Dialog::Create(form) => {
                let layer = egui::Id::new("forja.bench.create.keyboard");
                if !form.creating && forja_ui_kit::consume_layer_escape(ui.ctx(), layer) {
                    self.create_focused = false;
                    self.state.set_dialog(Dialog::None);
                    return;
                }
                if form.project_id.is_empty() {
                    if let Some(project) = projects.first() {
                        form.project_id = project.id.clone();
                    }
                }
                // The Name field takes focus as soon as the dialog opens so
                // typing and Tab never leak to the main window behind it.
                let focus_name = !self.create_focused;
                let mut focused_now = false;
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
                        forja_ui_kit::register_keyboard_layer(ui, layer);
                        ui.set_min_width(440.0);
                        if projects.is_empty() {
                            forja_ui_kit::text(ui, egui::RichText::new(cx.t("bench.notice.no-projects")).color(
                                c32(forja_ui_kit::applied_tokens(ui.ctx()).color.text_muted),
                            ));
                            ui.add_space(forja_ui_kit::Spacing::SM);
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
                        forja_ui_kit::text(ui, egui::RichText::new(cx.t("bench.dialog.create.project")).small());
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
                        ui.add_space(forja_ui_kit::Spacing::XS);
                        field_label(ui, &cx.t("bench.dialog.create.name"));
                        let name_field = forja_ui_kit::text_field(
                            ui,
                            &mut form.name,
                            &cx.t("bench.dialog.create.name-hint"),
                        );
                        if focus_name {
                            name_field.request_focus();
                            focused_now = true;
                        }
                        ui.add_space(forja_ui_kit::Spacing::XS);
                        field_label(ui, &cx.t("bench.dialog.create.base"));
                        forja_ui_kit::text_field(
                            ui,
                            &mut form.base,
                            &cx.t("bench.dialog.create.base-hint"),
                        );
                        ui.add_space(forja_ui_kit::Spacing::XS);
                        let mut fetch = form.fetch.unwrap_or_else(|| {
                            cx.setting("bench.worktree.fetch_before")
                                .and_then(forja_kernel::SettingValue::as_bool)
                                .unwrap_or(true)
                        });
                        forja_ui_kit::toggle(ui, &mut fetch, &cx.t("bench.dialog.create.fetch"));
                        form.fetch = Some(fetch);
                        if form.creating {
                            ui.horizontal(|ui| {
                                forja_ui_kit::spinner_if_active(ui);
                                forja_ui_kit::text(ui, 
                                    egui::RichText::new(cx.t("bench.dialog.create.creating"))
                                        .small(),
                                );
                            });
                        }
                        if let Some(error) = &form.error {
                            let human = crate::create::humanize_error(cx, error);
                            forja_ui_kit::error_banner(
                                ui,
                                &human.summary,
                                human.detail.as_deref(),
                                &cx.t("kernel.error.details"),
                            );
                        }
                        ui.add_space(forja_ui_kit::Spacing::SM);
                        ui.horizontal(|ui| {
                            if !form.creating
                                && forja_ui_kit::button(
                                    ui,
                                    forja_ui_kit::ButtonKind::Primary,
                                    &cx.t("bench.dialog.create.submit"),
                                )
                                .clicked()
                            {
                                submit = true;
                            }
                            if !form.creating
                                && forja_ui_kit::button(
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
                if focused_now {
                    self.create_focused = true;
                }
                let mut save_form = true;
                if add_folder {
                    save_form = false;
                    self.create_focused = false;
                    self.state.set_dialog(Dialog::None);
                    if let Err(error) = cx.run_command("project.add_dialog", Args::new()) {
                        self.state.set_error(Some(error.to_string()));
                    }
                } else if submit {
                    form.error = None;
                    if form.name.trim().is_empty() {
                        form.error = Some(cx.t("bench.error.name-required"));
                    } else {
                        form.creating = true;
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
                                // The worker closes the dialog on success; the
                                // board refresh shows the card as soon as it exists.
                                let _ = cx.run_command("board.refresh", Args::new());
                            }
                            Err(error) => {
                                form.creating = false;
                                form.error = Some(error);
                            }
                        }
                    }
                } else if cancel {
                    save_form = false;
                    self.create_focused = false;
                    self.state.set_dialog(Dialog::None);
                }
                if save_form {
                    self.state.set_dialog(Dialog::Create(form.clone()));
                }
            }
            Dialog::Store(form) => {
                let mut submit = false;
                let mut cancel = false;
                let title = cx.t("bench.dialog.store.title");
                let mut open = true;
                forja_ui_kit::modal_explicit_close(ui.ctx(), &mut open, &title, |ui| {
                    ui.set_min_width(420.0);
                    forja_ui_kit::text(ui, cx.t_args(
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
                    forja_ui_kit::toggle(ui, &mut form.force, &cx.t("bench.dialog.store.force"));
                    if let Some(error) = &form.error {
                        let human = crate::create::humanize_error(cx, error);
                        forja_ui_kit::error_banner(
                            ui,
                            &human.summary,
                            human.detail.as_deref(),
                            &cx.t("kernel.error.details"),
                        );
                    }
                    ui.add_space(forja_ui_kit::Spacing::SM);
                    ui.horizontal(|ui| {
                        if forja_ui_kit::button(
                            ui,
                            forja_ui_kit::ButtonKind::DangerConfirm,
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
                let mut keep = true;
                if submit {
                    match service::store(
                        cx,
                        &self.state,
                        &form.bench_id,
                        Some(form.delete_branch),
                        Some(form.force),
                    ) {
                        Ok(()) => {
                            keep = false;
                            self.state.set_dialog(Dialog::None);
                        }
                        Err(error) => form.error = Some(error),
                    }
                } else if cancel || !open {
                    keep = false;
                    self.state.set_dialog(Dialog::None);
                }
                if keep {
                    self.state.set_dialog(Dialog::Store(form.clone()));
                }
            }
            Dialog::Approve(form) => {
                let layer = egui::Id::new("forja.bench.approve.keyboard");
                if forja_ui_kit::consume_layer_escape(ui.ctx(), layer) {
                    self.state.set_dialog(Dialog::None);
                    return;
                }
                let mut submit = false;
                let mut cancel = false;
                let title = cx.t("bench.dialog.approve.title");
                egui::Window::new(title)
                    .id(egui::Id::new("forja.bench.approve"))
                    .collapsible(false)
                    .resizable(false)
                    .default_width(480.0)
                    .show(ui.ctx(), |ui| {
                        forja_ui_kit::register_keyboard_layer(ui, layer);
                        ui.set_min_width(480.0);
                        field_label(ui, &cx.t("bench.dialog.approve.setup"));
                        if form.setup.is_empty() {
                            forja_ui_kit::text(ui, cx.t("bench.dialog.approve.none"));
                        } else {
                            forja_ui_kit::text(ui, egui::RichText::new(&form.setup).monospace());
                        }
                        field_label(ui, &cx.t("bench.dialog.approve.teardown"));
                        if form.teardown.is_empty() {
                            forja_ui_kit::text(ui, cx.t("bench.dialog.approve.none"));
                        } else {
                            forja_ui_kit::text(ui, egui::RichText::new(&form.teardown).monospace());
                        }
                        if let Some(hash) = &form.setup_hash {
                            forja_ui_kit::text(ui, 
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
                            forja_ui_kit::text(ui, 
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
                            let human = crate::create::humanize_error(cx, error);
                            forja_ui_kit::error_banner(
                                ui,
                                &human.summary,
                                human.detail.as_deref(),
                                &cx.t("kernel.error.details"),
                            );
                        }
                        ui.add_space(forja_ui_kit::Spacing::SM);
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
                let mut keep = true;
                if submit {
                    match service::approve(cx, &self.state, &form.project_id, Some("both")) {
                        Ok(_) => {
                            keep = false;
                            self.state.set_dialog(Dialog::None);
                        }
                        Err(error) => form.error = Some(error),
                    }
                } else if cancel {
                    keep = false;
                    self.state.set_dialog(Dialog::None);
                }
                if keep {
                    self.state.set_dialog(Dialog::Approve(form.clone()));
                }
            }
        }
    }
}
