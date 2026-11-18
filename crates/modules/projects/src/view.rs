//! Sidebar for registered projects.

use crate::state::{CloneForm, Dialog, EditForm, ProjectForm, Shared};
use crate::view_dialogs::render_dialog;
use forja_contracts::BenchRef;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map};

pub struct ProjectsView {
    state: Shared,
}

impl ProjectsView {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

/// Project dialogs live in the modal slot so they open from any screen,
/// including the board's "Add folder" action.
pub struct ProjectsDialogs {
    state: Shared,
}

impl ProjectsDialogs {
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for ProjectsDialogs {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        if matches!(self.state.dialog(), Dialog::None) {
            cx.close_modal("project.dialogs");
        } else {
            cx.open_modal("project.dialogs");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let mut dialog = self.state.dialog();
        if matches!(dialog, Dialog::None) {
            ui.ctx().data_mut(|data| {
                data.remove_temp::<bool>(egui::Id::new("forja.projects.add.focus"));
                data.remove_temp::<bool>(egui::Id::new("forja.projects.create.focus"));
            });
            return;
        }
        let mut next = render_dialog(ui.ctx(), &mut dialog, cx, &self.state);
        std::mem::swap(&mut dialog, &mut next);
        self.state.set_dialog(dialog);
    }
}

impl View for ProjectsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let projects = self.state.projects();
        let mut dialog = self.state.dialog();

        let add_labels = [
            cx.t("project.sidebar.add"),
            cx.t("project.sidebar.create"),
            cx.t("project.sidebar.clone"),
        ];
        let add_items: Vec<_> = add_labels
            .iter()
            .map(|label| forja_ui_kit::MenuItem {
                label,
                danger: false,
            })
            .collect();
        match forja_ui_kit::icon_menu(
            ui,
            forja_ui_kit::Icon::Add,
            &cx.t("project.sidebar.add-menu"),
            &add_items,
        ) {
            Some(0) => dialog = Dialog::Add(ProjectForm::default()),
            Some(1) => dialog = Dialog::Create(ProjectForm::default()),
            Some(2) => dialog = Dialog::Clone(CloneForm::default()),
            _ => {}
        }
        ui.add_space(forja_ui_kit::Spacing::XS);

        if let Some(error) = self.state.error() {
            let human = humanize_error_detail(cx, &error);
            let message = cx.t_args("project.error.prefix", &[("message", &human.summary)]);
            if forja_ui_kit::error_notice(
                ui,
                &message,
                human.detail.as_deref(),
                &cx.t("kernel.error.details"),
                Some(&cx.t("project.error.dismiss")),
            ) {
                self.state.set_error(None);
            }
        }
        if let Some(status) = self.state.status() {
            forja_ui_kit::text(ui, 
                egui::RichText::new(status)
                    .small()
                    .color(tokens.color.info.forja_color()),
            );
        }

        if projects.is_empty() {
            forja_ui_kit::empty_state(
                ui,
                forja_ui_kit::ARCHIVE,
                &cx.t("project.sidebar.empty"),
                None,
            );
        }

        let active_id = self.state.active_id();
        let mut remove_request = None;
        let mut edit_request = None;
        let mut project_switch = None;
        let mut bench_activation = None;
        let mut close_request = false;
        for project in &projects {
            let expanded = self.state.is_expanded(&project.id);
            let active = active_id.as_deref() == Some(project.id.as_str());
            let fill = if active {
                tokens.color.accent_soft.forja_color()
            } else {
                tokens.color.surface.forja_color()
            };
            let stroke = if active {
                egui::Stroke::new(1.5_f32, tokens.color.accent.forja_color())
            } else {
                egui::Stroke::new(1.0_f32, tokens.color.border.forja_color())
            };
            let frame = egui::Frame::new()
                .fill(fill)
                .stroke(stroke)
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(8, 6));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let color =
                        parse_color(&project.color).unwrap_or(tokens.color.info.forja_color());
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 4.0, color);
                    if forja_ui_kit::icon_button(
                        ui,
                        if expanded {
                            forja_ui_kit::Icon::ChevronDown
                        } else {
                            forja_ui_kit::Icon::ChevronRight
                        },
                        &cx.t(if expanded {
                            "project.tree.collapse"
                        } else {
                            "project.tree.expand"
                        }),
                    )
                    .clicked()
                    {
                        self.state.toggle_expanded(&project.id);
                    }
                    if project.icon == crate::model::DEFAULT_ICON {
                        forja_ui_kit::icon(ui, forja_ui_kit::Icon::Projects, 16.0);
                    } else {
                        forja_ui_kit::text(ui, 
                            egui::RichText::new(&project.icon)
                                .size(forja_ui_kit::TextRole::Section.size()),
                        );
                    }
                    let name = ui.add_sized(
                        egui::vec2(
                            (ui.available_width()
                                - tokens.metrics().control
                                - tokens.metrics().gap)
                                .max(40.0),
                            tokens.metrics().control,
                        ),
                        egui::Label::new(
                            egui::RichText::new(&project.name)
                                .strong()
                                .color(tokens.color.text.forja_color()),
                        )
                        .truncate()
                        .sense(egui::Sense::click()),
                    );
                    name.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Button,
                            ui.is_enabled(),
                            active,
                            &project.name,
                        )
                    });
                    if name.clicked() {
                        if !expanded {
                            self.state.toggle_expanded(&project.id);
                        }
                        project_switch = Some(project.id.clone());
                    }
                    let action_labels = [
                        cx.t("project.sidebar.edit"),
                        cx.t("project.sidebar.remove"),
                        cx.t("project.command.close"),
                    ];
                    let items: Vec<_> = action_labels
                        .iter()
                        .take(if active { 3 } else { 2 })
                        .map(|label| forja_ui_kit::MenuItem {
                            label,
                            danger: false,
                        })
                        .collect();
                    let mut chosen = None;
                    name.context_menu(|ui| {
                        chosen = forja_ui_kit::menu_panel(ui, &items);
                    });
                    let actions = cx.t_args("project.sidebar.actions", &[("name", &project.name)]);
                    chosen =
                        forja_ui_kit::icon_menu(ui, forja_ui_kit::Icon::More, &actions, &items)
                            .or(chosen);
                    match chosen {
                        Some(0) => edit_request = Some(project.clone()),
                        Some(1) => {
                            remove_request = Some((project.id.clone(), project.name.clone()))
                        }
                        Some(2) => close_request = true,
                        _ => {}
                    }
                });
                forja_ui_kit::text(ui, 
                    egui::RichText::new(&project.path)
                        .small()
                        .color(tokens.color.text_muted.forja_color()),
                );
                if expanded {
                    ui.add_space(2.0);
                    match benches_for(cx, &project.id) {
                        Ok(benches) => {
                            self.state.mark_benches_missing(false);
                            if benches.is_empty() {
                                forja_ui_kit::text(ui, 
                                    egui::RichText::new(cx.t("project.sidebar.no-benches"))
                                        .small()
                                        .color(tokens.color.text_muted.forja_color()),
                                );
                            }
                            for bench in benches {
                                let label = match &bench.branch {
                                    Some(branch) => format!("{}  \u{00b7}  {}", bench.name, branch),
                                    None => bench.name.clone(),
                                };
                                let row = ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(label)
                                            .small()
                                            .color(tokens.color.text.forja_color()),
                                    )
                                    .truncate()
                                    .sense(egui::Sense::click()),
                                );
                                if row.clicked() {
                                    bench_activation = Some(bench.bench_id.clone());
                                }
                            }
                        }
                        Err(_) => {
                            if !self.state.benches_missing() {
                                self.state.mark_benches_missing(true);
                            }
                            forja_ui_kit::text(ui, 
                                egui::RichText::new(cx.t("project.sidebar.benches-missing"))
                                    .small()
                                    .color(tokens.color.text_muted.forja_color()),
                            );
                        }
                    }
                }
            });
            ui.add_space(forja_ui_kit::Spacing::XS);
        }

        if close_request {
            if let Err(error) = self.state.close(cx) {
                self.state.set_error(Some(error));
            }
        }
        if let Some((project_id, name)) = remove_request {
            dialog = Dialog::Remove { project_id, name };
        }
        if let Some(project) = edit_request {
            dialog = Dialog::Edit(EditForm {
                project_id: project.id,
                name: project.name,
                icon: project.icon,
                color: project.color,
                error: None,
            });
        }
        if let Some(project_id) = project_switch {
            let mut args = Map::new();
            args.insert("project_id".to_owned(), json!(project_id));
            match cx.run_command("project.switch", Args::from_map(args)) {
                Ok(_) => self.state.set_error(None),
                Err(error) => self.state.set_error(Some(error.to_string())),
            }
        }
        if let Some(bench_id) = bench_activation {
            let mut args = Map::new();
            args.insert("bench_id".to_owned(), json!(bench_id));
            match cx.run_command("bench.activate", Args::from_map(args)) {
                Ok(_) => self.state.set_error(None),
                Err(error) => self.state.set_error(Some(error.to_string())),
            }
        }

        self.state.set_dialog(dialog);
    }
}

pub(crate) fn humanize_error_detail(cx: &Ctx<'_>, raw: &str) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    if trimmed == "path is empty" {
        return forja_kernel::HumanError::simple(cx.t("project.error.empty-path"));
    }
    if let Some(detail) = trimmed.strip_prefix("folder does not exist:") {
        let detail = detail.trim();
        return forja_kernel::HumanError::with_detail(
            cx.t("project.error.not-found"),
            if detail.is_empty() { trimmed } else { detail },
        );
    }
    if trimmed == "path is not a folder" {
        return forja_kernel::HumanError::simple(cx.t("project.error.not-folder"));
    }
    if trimmed == "this folder is already registered" {
        return forja_kernel::HumanError::simple(cx.t("project.error.already-registered"));
    }
    if trimmed == "the folder is not empty" {
        return forja_kernel::HumanError::simple(cx.t("project.error.not-empty"));
    }
    if trimmed == "project not found" {
        return forja_kernel::HumanError::simple(cx.t("project.error.not-found-simple"));
    }
    if trimmed == "color must be #RRGGBB" {
        return forja_kernel::HumanError::simple(cx.t("project.error.color-format"));
    }
    if trimmed == "repository URL is required" {
        return forja_kernel::HumanError::simple(cx.t("project.error.missing-url"));
    }
    if trimmed == "destination is required" {
        return forja_kernel::HumanError::simple(cx.t("project.error.missing-destination"));
    }
    for (prefix, key) in [
        (
            "could not create the folder:",
            "project.error.create-folder",
        ),
        (
            "could not create the destination:",
            "project.error.create-folder",
        ),
        ("git init failed:", "project.error.git-init"),
        ("could not run git init:", "project.error.git-init"),
        ("git clone failed:", "project.error.git-clone"),
        ("could not run git clone:", "project.error.git-clone"),
        ("could not resolve the clone:", "project.error.git-clone"),
        ("could not save projects:", "project.error.save"),
    ] {
        if trimmed.starts_with(prefix) {
            return forja_kernel::HumanError::with_detail(cx.t(key), trimmed);
        }
    }
    forja_kernel::human_error::humanize_ctx(cx, trimmed)
}

/// Bench that represents the checkout of the project itself.
fn benches_for(cx: &mut Ctx<'_>, project_id: &str) -> Result<Vec<BenchRef>, String> {
    let mut args = Map::new();
    args.insert("project_id".to_owned(), json!(project_id));
    let value = cx
        .run_command("bench.list", Args::from_map(args))
        .map_err(|error| error.to_string())?;
    serde_json::from_value(value["benches"].clone()).map_err(|error| error.to_string())
}

pub(crate) fn parse_color(value: &str) -> Option<egui::Color32> {
    let value = crate::model::normalize_color(value)?;
    let r = u8::from_str_radix(&value[1..3], 16).ok()?;
    let g = u8::from_str_radix(&value[3..5], 16).ok()?;
    let b = u8::from_str_radix(&value[5..7], 16).ok()?;
    // ui-lint: allow project color chosen by the user, not interface chrome
    Some(egui::Color32::from_rgb(r, g, b))
}

/// Project color adapter delegates to the public Brasa color conversion.
pub(crate) trait ColorExt {
    fn forja_color(self) -> egui::Color32;
}

impl ColorExt for forja_ui_kit::Color {
    fn forja_color(self) -> egui::Color32 {
        self.to_color32()
    }
}
