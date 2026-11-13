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
            let detail = if blocked {
                format!("{} · trust", command.layer.label())
            } else {
                format!("{} · {}", command.layer.label(), command.run)
            };
            hits.push(AnvilHit {
                title,
                detail,
                icon_glyph: Some("⌘".to_owned()),
                score: 30,
                command: "content.run".to_owned(),
                args: json!({ "id": command.id }).as_object().cloned().unwrap_or_default(),
            });
        }
        hits
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        let watch = cx
            .setting("content.reload.watch")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if watch {
            cx.request_repaint();
        }
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
        let (revision, diagnostics, commands, themes, templates, recipes) = self
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
        forja_ui_kit::page_heading(
            ui,
            &cx.t("content.settings.title"),
            Some(&format!("rev {revision}")),
            |_| {},
        );
        ui.label(
            egui::RichText::new(format!(
                "{} · {} · {} · {}",
                cx.t_args("content.settings.commands", &[("count", &commands.to_string())]),
                cx.t_args("content.settings.themes", &[("count", &themes.to_string())]),
                cx.t_args("content.settings.templates", &[("count", &templates.to_string())]),
                cx.t_args("content.settings.recipes", &[("count", &recipes.to_string())]),
            ))
            .weak(),
        );
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &cx.t("content.settings.reload"))
            .clicked()
        {
            let project = state::current_project(cx);
            state::load(&self.core, cx, project);
        }
        if diagnostics.is_empty() {
            ui.label(egui::RichText::new(cx.t("content.settings.no-diagnostics")).weak());
            return;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(cx.t("content.settings.diagnostics")).strong());
        egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
            for diagnostic in diagnostics {
                ui.label(
                    egui::RichText::new(format!(
                        "{} · {}",
                        diagnostic.layer,
                        diagnostic.path
                    ))
                    .weak(),
                );
                ui.label(diagnostic.message);
            }
        });
    }
}
