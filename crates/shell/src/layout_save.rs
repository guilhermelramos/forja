//! Modal "Salvar layout atual…" e gravação pelo catálogo do módulo `content`.
//!
//! O shell nunca escreve arquivos diretamente: monta o TOML a partir da
//! projeção e chama `content.layouts.save`, atômico e confinado.

use crate::app::ForjaApp;
use crate::layout_projection::{export, new_spec};
use crate::layout_settings::LayoutLayer;
use eframe::egui;
use forja_kernel::Args;

#[derive(Default, Clone, Debug)]
pub struct SaveDialog {
    pub open: bool,
    pub name: String,
    pub layer: LayoutLayer,
    pub make_default: bool,
    pub error: Option<String>,
}

impl ForjaApp {
    /// "Salvar layout atual…": grava pelo catálogo do módulo `content`, nunca
    /// escrevendo arquivos diretamente do shell.
    pub(crate) fn layout_save_window(&mut self, ctx: &egui::Context) {
        if !self.layout_state.save_dialog.open {
            return;
        }
        let mut dialog = self.layout_state.save_dialog.clone();
        let has_project = !self.no_project();
        let mut open = true;
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(self.text("shell.layout.save-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(self.text("shell.layout.save-hint"));
                let name_label = self.text("shell.layout.save-name");
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut dialog.name).hint_text(&name_label),
                );
                edit.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        ui.is_enabled(),
                        &name_label,
                    )
                });
                ui.radio_value(
                    &mut dialog.layer,
                    LayoutLayer::User,
                    self.text("shell.layout.origin-user"),
                );
                let project_scope = self.text("shell.layout.origin-project");
                ui.add_enabled_ui(has_project, |ui| {
                    ui.radio_value(&mut dialog.layer, LayoutLayer::Project, project_scope);
                });
                ui.checkbox(
                    &mut dialog.make_default,
                    self.text("shell.layout.save-default"),
                );
                if let Some(error) = &dialog.error {
                    ui.label(error);
                }
                ui.horizontal(|ui| {
                    confirm = forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Primary,
                        &self.text("shell.layout.save-confirm"),
                    )
                    .clicked();
                    cancel = forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &self.text("shell.layout.save-cancel"),
                    )
                    .clicked();
                });
            });
        dialog.open = open;
        if cancel {
            dialog.open = false;
        }
        self.layout_state.save_dialog = dialog;
        if confirm {
            self.queue_layout_save();
        }
    }

    fn queue_layout_save(&mut self) {
        let dialog = self.layout_state.save_dialog.clone();
        let id = slugify(&dialog.name);
        if let Err(error) = forja_kernel::layout_spec::validate_segment("layout.id", &id) {
            self.layout_state.save_dialog.error = Some(self.text_args(
                "shell.layout.save-invalid-name",
                &[("detail", &error.to_string())],
            ));
            return;
        }
        if dialog.layer == LayoutLayer::Project && self.no_project() {
            self.layout_state.save_dialog.error = Some(self.text("shell.layout.save-no-project"));
            return;
        }
        let text = self.layout_text_for_save(&id, &dialog.name);
        let expected = self
            .layout_state
            .catalog
            .candidates
            .iter()
            .find(|candidate| candidate.id == id && candidate.layer == dialog.layer)
            .map(|candidate| candidate.hash.clone())
            .filter(|hash| !hash.is_empty());
        let mut args = Args::new();
        args.insert("id", serde_json::json!(id));
        args.insert("layer", serde_json::json!(dialog.layer.as_str()));
        args.insert("text", serde_json::json!(text));
        if let Some(hash) = expected {
            args.insert("expected_hash", serde_json::json!(hash));
        }
        let revision = self.layout_state.catalog.revision;
        if let Err(error) = self.runtime.run_command("content.layouts.save", args) {
            self.layout_state.save_dialog.error = Some(error.to_string());
            return;
        }
        if dialog.make_default {
            self.layout_state.pending_default = Some((id, revision));
        }
        self.layout_state.save_dialog.open = false;
        self.layout_state.save_dialog.error = None;
    }

    fn layout_text_for_save(&self, id: &str, name: &str) -> String {
        let selection = self.layout_state.selection.as_ref();
        let Some(selection) = selection else {
            return new_spec(id, name, name, &[])
                .to_toml()
                .unwrap_or_default();
        };
        let mut spec = self
            .layout_state
            .catalog
            .spec_for(&selection.id)
            .unwrap_or_else(|| new_spec(id, name, name, &[]));
        spec.id = id.to_owned();
        spec.name.pt_br = name.to_owned();
        spec.name.en = name.to_owned();
        export(&spec, &self.layout_state.projection).unwrap_or_default()
    }
}

fn slugify(name: &str) -> String {
    let mut out: String = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').chars().take(64).collect()
}
