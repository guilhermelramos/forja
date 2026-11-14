//! Seção "Importar do Harbor" dos Ajustes. Mostra o estado e roda a importação
//! dos itens pendentes. A tela só lê o estado e chama comandos; nenhum arquivo
//! é lido aqui.

use crate::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::Value;

pub struct SettingsView {
    state: Shared,
}

impl SettingsView {
    /// Cria a seção ligada ao estado do módulo.
    pub fn new(state: Shared) -> Self {
        Self { state }
    }
}

impl View for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let status = self.state.status();
        if status.get("loaded") != Some(&Value::Bool(true))
            && status.get("busy") != Some(&Value::Bool(true))
        {
            let _ = cx.run_command("import.harbor.preview", Args::new());
        }
        forja_ui_kit::page_heading(ui, &cx.t("import.settings.title"), Some(&cx.t("import.settings.hint")), |_| {});

        if status.get("busy") == Some(&Value::Bool(true)) {
            ui.label(egui::RichText::new(cx.t("import.settings.loading")).weak());
            return;
        }
        if let Some(key) = status.get("error").and_then(Value::as_str) {
            ui.label(egui::RichText::new(cx.t(key)).color(egui::Color32::from_rgb(0xE5, 0x53, 0x4B)));
        }
        if status.get("available") != Some(&Value::Bool(true)) {
            ui.label(egui::RichText::new(cx.t("import.settings.none")).weak());
            return;
        }
        let items: Vec<&Value> = status
            .get("items")
            .and_then(Value::as_array)
            .map(|list| list.iter().collect())
            .unwrap_or_default();
        let pending = items
            .iter()
            .filter(|item| item.get("imported") != Some(&Value::Bool(true)))
            .count();
        match status.get("decided").and_then(Value::as_str) {
            Some("imported") => {
                ui.label(egui::RichText::new(cx.t("import.settings.imported")).weak());
            }
            Some("skipped") => {
                ui.label(egui::RichText::new(cx.t("import.settings.skipped")).weak());
            }
            _ => {}
        }
        for item in &items {
            let kind = item.get("kind").and_then(Value::as_str).unwrap_or("");
            let raw_label = item.get("label").and_then(Value::as_str).unwrap_or("");
            let label = if kind == "theme" {
                match raw_label {
                    "dark" => cx.t("import.theme.dark"),
                    "light" => cx.t("import.theme.light"),
                    other => other.to_owned(),
                }
            } else {
                raw_label.to_owned()
            };
            let detail = item.get("detail").and_then(Value::as_str).unwrap_or("");
            let done = item.get("imported") == Some(&Value::Bool(true));
            // Tema e fonte repetem o valor no rótulo; notas mostram só a primeira linha.
            let show_detail = !detail.is_empty()
                && detail != raw_label
                && !matches!(kind, "notes" | "theme" | "font");
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(cx.t(&format!("import.kind.{kind}"))).weak());
                ui.label(label);
                if show_detail {
                    ui.label(egui::RichText::new(detail).weak().monospace());
                }
                if done {
                    ui.label(egui::RichText::new("✓").weak());
                }
            });
        }
        if pending > 0 && ui.button(cx.t("import.settings.run")).clicked() {
            let _ = cx.run_command("import.harbor.run", Args::new());
        }
        if let Some(summary) = status.get("summary").filter(|value| value.is_object()) {
            let count = |name: &str| summary.get(name).and_then(Value::as_u64).unwrap_or(0).to_string();
            let line = cx.t_args(
                "import.result.line",
                &[
                    ("projects", &count("projects")),
                    ("benches", &count("benches")),
                    ("recipes", &count("recipes")),
                    ("notes", &count("notes")),
                    ("ssh", &count("ssh")),
                    ("settings", &count("settings")),
                ],
            );
            ui.label(line);
            if let Some(errors) = summary.get("errors").and_then(Value::as_array) {
                for error in errors.iter().filter_map(Value::as_str) {
                    ui.label(egui::RichText::new(error).weak());
                }
            }
        }
    }
}
