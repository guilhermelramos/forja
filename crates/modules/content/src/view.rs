//! Fonte da Bigorna e seção de Ajustes do conteúdo.

use crate::commands;
use crate::state::{self, ContentCore};
use forja_kernel::ui::AnvilHit;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::sync::Arc;

/// Fonte da Bigorna: uma linha por comando de `commands/*.md`.
pub struct ContentAnvilView {
    core: Arc<ContentCore>,
    last_poll: Option<std::time::Instant>,
}

impl ContentAnvilView {
    pub fn new(core: Arc<ContentCore>) -> Self {
        Self {
            core,
            last_poll: None,
        }
    }
}

impl View for ContentAnvilView {
    fn provides_anvil_hits(&self) -> bool {
        true
    }

    fn anvil_hits(&mut self, query: &str, cx: &mut Ctx<'_>) -> Vec<AnvilHit> {
        state::ensure(&self.core, cx, false);
        let locale = cx.i18n().locale().id().to_owned();
        let commands = self
            .core
            .with(|state| state.commands.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let query = query.trim().to_ascii_lowercase();
        let blocked_ids: std::collections::BTreeSet<String> =
            commands::commands_list(&self.core, cx, &Args::new())
                .ok()
                .and_then(|value| {
                    value
                        .get("commands")
                        .and_then(serde_json::Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter(|item| {
                                    item.get("blocked").and_then(serde_json::Value::as_bool)
                                        == Some(true)
                                })
                                .filter_map(|item| {
                                    item.get("id")
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::to_owned)
                                })
                                .collect()
                        })
                })
                .unwrap_or_default();
        let mut hits = Vec::new();
        for command in commands {
            if !command.scope.iter().any(|scope| scope == "anvil") {
                continue;
            }
            let title = command.title.resolve(&locale).to_owned();
            let search = format!("{} {} {}", title, command.name, command.id);
            if !query.is_empty() && !search.to_ascii_lowercase().contains(&query) {
                continue;
            }
            let blocked = blocked_ids.contains(&command.id);
            let category = cx.t(if blocked {
                "content.anvil.approval"
            } else {
                match command.run.as_str() {
                    "helper" => "content.anvil.helper",
                    "terminal" => "content.anvil.terminal",
                    _ => "content.anvil.command",
                }
            });
            let detail = match command.shortcut.as_deref() {
                Some(shortcut) => format!("{category} · {shortcut}"),
                None => category,
            };
            hits.push(AnvilHit {
                title,
                detail,
                icon_glyph: Some(forja_ui_kit::Icon::Configure.id().to_owned()),
                score: 30,
                command: "content.run".to_owned(),
                args: json!({ "id": command.id })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        hits
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        let due = self
            .last_poll
            .is_none_or(|at| at.elapsed() >= std::time::Duration::from_millis(500));
        if !due {
            return;
        }
        self.last_poll = Some(std::time::Instant::now());
        state::ensure(&self.core, cx, false);
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _cx: &mut Ctx<'_>) {}
}

/// Seção de Ajustes com a revisão e os diagnósticos.
pub struct ContentSettingsView {
    core: Arc<ContentCore>,
}

impl ContentSettingsView {
    pub fn new(core: Arc<ContentCore>) -> Self {
        Self { core }
    }
}

impl View for ContentSettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx, false);
        let (_revision, mut diagnostics, commands, themes, templates, recipes) = self
            .core
            .with(|state| {
                (
                    state.revision,
                    state.diagnostics.clone(),
                    state.commands.len(),
                    state.themes.len(),
                    state.templates.len(),
                    state.recipes.len(),
                )
            })
            .unwrap_or((0, Vec::new(), 0, 0, 0, 0));
        let layout_count = self
            .core
            .layouts
            .with(|state| {
                diagnostics.extend(state.catalog.diagnostics.iter().cloned());
                state.catalog.entries.len()
            })
            .unwrap_or(0);
        forja_ui_kit::page_heading(ui, &cx.t("content.settings.title"), None, |_| {});
        ui.label(
            egui::RichText::new(format!(
                "{} · {} · {} · {} · {}",
                cx.t_args(
                    "content.settings.commands",
                    &[("count", &commands.to_string())]
                ),
                cx.t_args("content.settings.themes", &[("count", &themes.to_string())]),
                cx.t_args(
                    "content.settings.templates",
                    &[("count", &templates.to_string())]
                ),
                cx.t_args(
                    "content.settings.recipes",
                    &[("count", &recipes.to_string())]
                ),
                cx.t_args(
                    "content.settings.layouts",
                    &[("count", &layout_count.to_string())]
                ),
            ))
            .weak(),
        );
        if forja_ui_kit::button(
            ui,
            forja_ui_kit::ButtonKind::Secondary,
            &cx.t("content.settings.reload"),
        )
        .clicked()
        {
            let project = state::current_project(cx);
            state::load(&self.core, cx, project);
            self.core.layouts.request_reload(cx);
        }
        if diagnostics.is_empty() {
            ui.label(egui::RichText::new(cx.t("content.settings.no-diagnostics")).weak());
            return;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(cx.t("content.settings.diagnostics")).strong());
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for diagnostic in diagnostics {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} · {}",
                            cx.t(&format!("content.layer.{}", diagnostic.layer)),
                            diagnostic_path(
                                &diagnostic.path,
                                state::current_project(cx).as_deref(),
                                cx.user_dir()
                            )
                        ))
                        .weak(),
                    );
                    ui.label(cx.t(if diagnostic.message.contains("missing `+++`") {
                        "content.error.header-missing"
                    } else if diagnostic.message.contains("closing `+++`") {
                        "content.error.header-unclosed"
                    } else {
                        "content.error.invalid-file"
                    }));
                    ui.collapsing(cx.t("content.settings.details"), |ui| {
                        ui.label(&diagnostic.message);
                    });
                }
            });
    }
}

fn diagnostic_path(
    path: &str,
    project: Option<&std::path::Path>,
    user: &std::path::Path,
) -> String {
    let normalized = std::path::PathBuf::from(path).components().fold(
        std::path::PathBuf::new(),
        |mut out, part| {
            match part {
                std::path::Component::ParentDir => {
                    out.pop();
                }
                std::path::Component::CurDir => {}
                part => out.push(part.as_os_str()),
            };
            out
        },
    );
    for root in project.into_iter().chain(std::iter::once(user)) {
        if let Ok(relative) = normalized.strip_prefix(root) {
            return relative.display().to_string();
        }
    }
    normalized
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    #[test]
    fn diagnostics_use_project_relative_paths_without_parent_segments() {
        let root = std::path::Path::new("C:/sample");
        assert_eq!(
            diagnostic_path(
                "C:/sample/.forja/commands/../commands/bad.md",
                Some(root),
                root
            ),
            ".forja\\commands\\bad.md"
        );
        assert_eq!(
            diagnostic_path("C:/elsewhere/../../../bad.md", Some(root), root),
            "bad.md"
        );
    }
}
