//! Start page owned by projects; optional services are accessed through commands.
use crate::state::{CloneForm, Dialog, ProjectForm, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::Value;
use std::time::{Duration, Instant};

pub struct HomeView {
    state: Shared,
    helpers: Value,
    checked: Option<Instant>,
    copied: Option<String>,
}
impl HomeView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            helpers: Value::Null,
            checked: None,
            copied: None,
        }
    }
    fn helpers(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if self
            .checked
            .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
            || self.helpers["detecting"] == true
        {
            self.checked = Some(Instant::now());
            self.helpers = cx
                .run_command("helper.list", Args::new())
                .unwrap_or(Value::Null);
        }
        let profiles = self
            .helpers
            .get("profiles")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let installed: Vec<_> = profiles
            .iter()
            .filter(|item| item["installed"] == true)
            .collect();
        forja_ui_kit::card(ui, Some(&cx.t("project.home.helpers")), |ui| {
            if installed.is_empty() {
                ui.label(cx.t("project.home.helpers-none"));
            } else {
                for item in installed {
                    ui.label(item["name"].as_str().unwrap_or_default());
                }
            }
            if self.helpers["detecting"] == true {
                ui.label(cx.t("project.home.detecting"));
                ui.ctx().request_repaint_after(Duration::from_millis(500));
            }
            let missing: Vec<_> = profiles
                .iter()
                .filter(|item| {
                    item["installed"] != true
                        && item["install_hint"]
                            .as_str()
                            .is_some_and(|text| !text.is_empty())
                })
                .collect();
            for item in missing.iter().take(2) {
                self.install_row(ui, cx, item);
            }
            if missing.len() > 2 {
                ui.collapsing(cx.t("project.home.more-install"), |ui| {
                    for item in missing.iter().skip(2) {
                        self.install_row(ui, cx, item);
                    }
                });
            }
        });
    }
    fn install_row(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>, item: &Value) {
        let command = item["install_hint"].as_str().unwrap_or_default();
        let id = item["id"].as_str().unwrap_or_default();
        ui.label(egui::RichText::new(item["name"].as_str().unwrap_or_default()).strong());
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 90.0).max(80.0);
            ui.add_sized(
                [width, 24.0],
                egui::Label::new(egui::RichText::new(command).monospace().small()).truncate(),
            )
            .on_hover_text(command);
            let label = cx.t(if self.copied.as_deref() == Some(id) {
                "project.home.copied"
            } else {
                "project.home.copy"
            });
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &label).clicked() {
                ui.ctx().copy_text(command.to_owned());
                self.copied = Some(id.to_owned());
            }
        });
        ui.add_space(6.0);
    }
    fn recent(&self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        forja_ui_kit::card(ui, Some(&cx.t("project.home.recent")), |ui| {
            let recent = self.state.recent();
            if recent.is_empty() {
                ui.label(cx.t("project.home.recent-empty"));
            }
            egui::ScrollArea::vertical()
                .id_salt("home.recent")
                .max_height(195.0)
                .show(ui, |ui| {
                    for project in recent {
                        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &project.name)
                            .clicked()
                        {
                            let result = if self
                                .state
                                .projects()
                                .iter()
                                .any(|item| item.id == project.id)
                            {
                                self.state.activate(cx, &project.id).map(|_| ())
                            } else {
                                self.state
                                    .add(
                                        cx,
                                        &crate::model::NewProject {
                                            path: project.path.clone(),
                                            name: project.name.clone(),
                                            icon: project.icon.clone(),
                                            color: project.color.clone(),
                                        },
                                    )
                                    .map(|_| ())
                            };
                            if let Err(error) = result {
                                self.state.set_error(Some(error));
                            }
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(short_path(&project.path)).small(),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&project.path);
                    }
                });
        });
    }
}
impl View for HomeView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.state.ensure_loaded(cx);
        egui::ScrollArea::vertical()
            .id_salt("project.home")
            .show(ui, |ui| {
                ui.add_space(20.0);
                forja_ui_kit::page_heading(
                    ui,
                    &cx.t("project.home.title"),
                    Some(&cx.t("project.home.lead")),
                    |_| {},
                );
                ui.add_space(20.0);
                let width = ((ui.available_width() - 24.0) / 3.0).max(180.0);
                ui.horizontal(|ui| {
                    if forja_ui_kit::action_tile(
                        ui,
                        forja_ui_kit::ARCHIVE,
                        &cx.t("project.home.open"),
                        width,
                        true,
                    )
                    .clicked()
                    {
                        self.state.set_dialog(Dialog::Add(ProjectForm::default()));
                    }
                    if forja_ui_kit::action_tile(
                        ui,
                        forja_ui_kit::SPLIT_RIGHT,
                        &cx.t("project.home.clone"),
                        width,
                        false,
                    )
                    .clicked()
                    {
                        self.state.set_dialog(Dialog::Clone(CloneForm::default()));
                    }
                    if forja_ui_kit::action_tile(
                        ui,
                        forja_ui_kit::BOARD,
                        &cx.t("project.sidebar.create"),
                        width,
                        false,
                    )
                    .clicked()
                    {
                        self.state
                            .set_dialog(Dialog::Create(ProjectForm::default()));
                    }
                });
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    for (command, key) in [
                        ("anvil.open", "project.home.shortcut-anvil"),
                        ("find.file", "project.home.shortcut-find"),
                        ("bench.create", "project.home.shortcut-bench"),
                    ] {
                        let chord = cx
                            .keymap()
                            .bindings()
                            .iter()
                            .filter(|item| item.binding.command.as_str() == command)
                            .min_by_key(|item| {
                                item.binding.scope != forja_kernel::keymap::Scope::Global
                            })
                            .map(|item| item.binding.chord.to_string())
                            .unwrap_or_default();
                        ui.label(format!("{}  {}", chord, cx.t(key)));
                        ui.add_space(12.0);
                    }
                });
                ui.add_space(20.0);
                if let Some(error) = self.state.error() {
                    let human = super::view::humanize_error_detail(cx, &error);
                    if forja_ui_kit::error_notice(
                        ui,
                        &human.summary,
                        human.detail.as_deref(),
                        &cx.t("kernel.error.details"),
                        Some(&cx.t("project.error.dismiss")),
                    ) {
                        self.state.set_error(None);
                    }
                }
                ui.columns(2, |columns| {
                    self.recent(&mut columns[0], cx);
                    self.helpers(&mut columns[1], cx);
                });
            });
    }
}
fn short_path(path: &str) -> String {
    let parts: Vec<_> = path
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .take(2)
        .collect();
    format!(
        "…\\{}",
        parts.into_iter().rev().collect::<Vec<_>>().join("\\")
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn recent_paths_are_abbreviated_without_losing_the_leaf_folder() {
        assert_eq!(
            super::short_path("C:/work/team/project"),
            "…\\team\\project"
        );
        assert_eq!(super::short_path(r"D:\team\project\"), "…\\team\\project");
    }
}
