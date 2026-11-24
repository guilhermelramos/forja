//! Seletor de arranjos, projeção no chrome legado e gravação pelo catálogo.
//!
//! Toda leitura vem do estado já publicado pelo módulo `content` (comando
//! `content.layouts.list`); nenhuma função chamada por `ui()` toca disco. A
//! gravação passa por `content.layouts.save`, que é atômica e confinada.

use crate::actions::{self, Action};
use crate::app::ForjaApp;
use crate::layout_settings::{
    apply_rail_edit, export, new_spec, order_side_tabs, project, resolve_selection, CatalogSnapshot,
    LayoutCandidate, LayoutLayer, LayoutSelection, Projection, RailEdit,
};
use eframe::egui;
use forja_kernel::{Args, Locale, SettingValue};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};


/// Estado do seletor, vivo só na memória do app.
#[derive(Default)]
pub struct LayoutState {
    pub catalog: CatalogSnapshot,
    pub checked: Option<Instant>,
    pub selection: Option<LayoutSelection>,
    pub applied: bool,
    pub projection: Projection,
    pub rail_registration: Vec<String>,
    pub picker_open: bool,
    pub save_dialog: SaveDialog,
    /// `(id, revisão do catálogo na fila)`: só muda o padrão após a gravação.
    pub pending_default: Option<(String, u64)>,
}

#[derive(Default, Clone, Debug)]
pub struct SaveDialog {
    pub open: bool,
    pub name: String,
    pub layer: LayoutLayer,
    pub make_default: bool,
    pub error: Option<String>,
}

pub(crate) const FACTORY_IDS: [&str; 4] = ["oficina", "foco", "revisao", "mutirao"];

impl ForjaApp {
    /// Chamado em `update()`, nunca em `ui()`. O comando devolve o snapshot já
    /// publicado pelo catálogo; a leitura de arquivos acontece em `Tasks`.
    pub(crate) fn poll_layout_catalog(&mut self) {
        let due = self
            .layout_state
            .checked
            .is_none_or(|at| at.elapsed() >= Duration::from_millis(200));
        if !due {
            return;
        }
        self.layout_state.checked = Some(Instant::now());
        let Ok(value) = self.runtime.run_command("content.layouts.list", Args::new()) else {
            return;
        };
        let catalog = CatalogSnapshot::from_value(&value);
        if catalog == self.layout_state.catalog && self.layout_state.applied {
            return;
        }
        // Catálogo ainda vazio: não descarta a escolha salva enquanto a leitura
        // em `Tasks` não publicou as entradas efetivas.
        if catalog.candidates.is_empty() {
            return;
        }
        self.layout_state.catalog = catalog.clone();
        let user_default = self
            .runtime
            .settings()
            .values()
            .get("appearance.layout.default")
            .and_then(SettingValue::as_str)
            .map(str::to_owned);
        let default_id = catalog
            .default_id
            .as_deref()
            .or(user_default.as_deref());
        let changed = resolve_selection(
            self.layout_state.selection.as_ref(),
            default_id,
            &catalog.candidates,
        );
        if self.layout_state.selection != changed || !self.layout_state.applied {
            self.layout_state.selection = changed;
            self.apply_layout_projection(false);
        }
        if let Some((id, revision)) = self.layout_state.pending_default.clone() {
            if catalog.revision > revision {
                self.runtime
                    .set_setting("appearance.layout.default", SettingValue::Text(id))
                    .ok();
                self.layout_state.pending_default = None;
            }
        }
    }

    fn current_layout_name(&self) -> String {
        let pt = self.runtime.i18n().locale() == Locale::PtBr;
        self.layout_state
            .selection
            .as_ref()
            .and_then(|selection| self.layout_state.catalog.effective_for(&selection.id))
            .map(|candidate| candidate.name(pt).to_owned())
            .unwrap_or_else(|| self.text("shell.layout.fallback"))
    }

    pub(crate) fn layout_label(&self) -> String {
        self.text_args("shell.layout.label", &[("name", &self.current_layout_name())])
    }

    /// Aplica a projeção no chrome legado. `reset_tabs` só é usado na troca
    /// explícita: na restauração, abas e estado salvos continuam abrindo.
    pub(crate) fn apply_layout_projection(&mut self, reset_tabs: bool) {
        let Some(selection) = self.layout_state.selection.clone() else {
            return;
        };
        let Some(spec) = self.layout_state.catalog.spec_for(&selection.id) else {
            return;
        };
        let mut projection = project(
            &spec,
            &self.layout_state.catalog.known_views,
            &self.layout_state.rail_registration,
        );
        if projection.effective_rail_order.is_empty() {
            projection.effective_rail_order = self.layout_state.rail_registration.clone();
        }
        if reset_tabs {
            let visible: Vec<String> = projection
                .bench_tabs
                .iter()
                .filter(|id| self.bench_views.iter().any(|view| &view.id == *id))
                .cloned()
                .collect();
            if !visible.is_empty() {
                let total = self.bench_views.len();
                let hidden = (0..total).filter(|index| {
                    self.bench_views
                        .get(*index)
                        .is_none_or(|view| !visible.contains(&view.id))
                });
                let closed: Vec<usize> = self
                    .tabs
                    .closed_indexes()
                    .iter()
                    .copied()
                    .filter(|index| {
                        self.bench_views
                            .get(*index)
                            .is_none_or(|view| !visible.contains(&view.id))
                    })
                    .collect();
                self.tabs = crate::tabs::TabSet::from_hidden_and_closed(hidden, closed, total);
                if let Some(first) = visible.first() {
                    if let Some(index) = self.bench_views.iter().position(|view| &view.id == first)
                    {
                        self.active_tab = index;
                    }
                }
            }
        }
        let order = projection.effective_rail_order.clone();
        self.rail_views.sort_by_key(|view| {
            order
                .iter()
                .position(|id| id == &view.id)
                .unwrap_or(order.len())
        });
        // O arranjo também declara as abas das laterais; aplicar só `bench_tabs`
        // deixaria `sidebar_tabs` e `right_tabs` calculados e nunca usados.
        self.apply_side_panel(true, &projection.sidebar_tabs, reset_tabs);
        self.apply_side_panel(false, &projection.right_tabs, reset_tabs);
        self.layout_state.projection = projection;
        self.layout_state.applied = true;
    }

    /// Reordena a lateral conforme a projeção e(ao trocar de arranjo) escolhe a
    /// primeira aba declarada. Nas restaurações preserva a aba que já estava ativa.
    fn apply_side_panel(&mut self, sidebar: bool, declared: &[String], select_first: bool) {
        let active_id = if sidebar {
            self.sidebar_views.get(self.sidebar_tab).map(|view| view.id.clone())
        } else {
            self.right_views.get(self.right_tab).map(|view| view.id.clone())
        };
        let (views, tab) = if sidebar {
            (&mut self.sidebar_views, &mut self.sidebar_tab)
        } else {
            (&mut self.right_views, &mut self.right_tab)
        };
        let ids: Vec<String> = views.iter().map(|view| view.id.clone()).collect();
        let (order, first) = order_side_tabs(&ids, declared);
        views.sort_by_key(|view| order.iter().position(|id| id == &view.id).unwrap_or(order.len()));
        let wanted = if select_first { first.or(active_id) } else { active_id };
        if let Some(id) = wanted {
            if let Some(index) = views.iter().position(|view| view.id == id) {
                *tab = index;
            }
        }
    }

    pub(crate) fn layout_rail_visible(&self) -> bool {
        self.layout_state
            .projection
            .rail_visible
            .unwrap_or_else(|| self.setting_bool("appearance.rail.visible", true))
    }

    pub(crate) fn layout_status_bar_visible(&self) -> bool {
        self.layout_state
            .projection
            .status_bar_visible
            .unwrap_or_else(|| self.setting_bool("appearance.status_bar.visible", true))
    }

    pub(crate) fn visible_rail_pages(&self, pages: Vec<crate::layout::RailPage>) -> Vec<crate::layout::RailPage> {
        let hidden: BTreeSet<&str> = self
            .layout_state
            .projection
            .rail_hidden
            .iter()
            .map(String::as_str)
            .collect();
        let order = &self.layout_state.projection.effective_rail_order;
        let mut pages: Vec<_> = pages
            .into_iter()
            .filter(|page| !hidden.contains(page.id.as_str()))
            .collect();
        pages.sort_by_key(|page| order.iter().position(|id| id == &page.id).unwrap_or(order.len()));
        pages
    }

    /// Botão no topo, à direita da busca e antes do tema.
    pub(crate) fn layout_selector(&mut self, ui: &mut egui::Ui) {
        let labels = self.layout_menu_labels();
        let items: Vec<forja_ui_kit::MenuItem<'_>> = labels
            .iter()
            .map(|label| forja_ui_kit::MenuItem {
                label,
                danger: false,
            })
            .collect();
        let label = self.layout_label();
        if let Some(index) = forja_ui_kit::context_menu(ui, &label, &items) {
            self.choose_layout_menu(index);
        }
    }

    fn layout_menu_labels(&self) -> Vec<String> {
        let pt = self.runtime.i18n().locale() == Locale::PtBr;
        let mut candidates: Vec<&LayoutCandidate> = self
            .layout_state
            .catalog
            .candidates
            .iter()
            .filter(|candidate| FACTORY_IDS.contains(&candidate.id.as_str()))
            .collect();
        let rest: Vec<&LayoutCandidate> = self
            .layout_state
            .catalog
            .candidates
            .iter()
            .filter(|candidate| !FACTORY_IDS.contains(&candidate.id.as_str()))
            .collect();
        candidates.extend(rest);
        let mut labels: Vec<String> = candidates
            .iter()
            .map(|candidate| {
                let name = candidate.name(pt);
                match candidate.layer {
                    LayoutLayer::Builtin => name.to_owned(),
                    other => self.text_args(
                        "shell.layout.with-origin",
                        &[("name", name), ("origin", &self.text(other.label_key()))],
                    ),
                }
            })
            .collect();
        if labels.is_empty() {
            labels.push(self.text("shell.layout.fallback"));
        }
        labels.push(self.text("shell.layout.save"));
        labels.push(self.text("shell.layout.restore"));
        labels
    }

    fn choose_layout_menu(&mut self, index: usize) {
        let pt = self.runtime.i18n().locale() == Locale::PtBr;
        let mut candidates: Vec<LayoutCandidate> = self
            .layout_state
            .catalog
            .candidates
            .iter()
            .filter(|candidate| FACTORY_IDS.contains(&candidate.id.as_str()))
            .cloned()
            .collect();
        let mut rest: Vec<LayoutCandidate> = self
            .layout_state
            .catalog
            .candidates
            .iter()
            .filter(|candidate| !FACTORY_IDS.contains(&candidate.id.as_str()))
            .cloned()
            .collect();
        candidates.append(&mut rest);
        if index < candidates.len() {
            let candidate = &candidates[index];
            actions::push(Action::SwitchLayout {
                id: candidate.id.clone(),
                layer: candidate.layer,
                name: candidate.name(pt).to_owned(),
            });
        } else if index == candidates.len() {
            self.open_save_dialog();
        } else {
            self.restore_layout_defaults();
        }
    }

    fn open_save_dialog(&mut self) {
        let mut dialog = SaveDialog {
            open: true,
            name: String::new(),
            layer: LayoutLayer::User,
            ..Default::default()
        };
        if let Some(selection) = &self.layout_state.selection {
            dialog.layer = if selection.layer == LayoutLayer::Builtin {
                LayoutLayer::User
            } else {
                selection.layer
            };
            dialog.name = self
                .layout_state
                .catalog
                .effective_for(&selection.id)
                .map(|candidate| candidate.name(true).to_owned())
                .unwrap_or_default();
        }
        self.layout_state.save_dialog = dialog;
    }

    fn restore_layout_defaults(&mut self) {
        let Some(selection) = self.layout_state.selection.clone() else {
            return;
        };
        let Some(spec) = self.layout_state.catalog.spec_for(&selection.id) else {
            return;
        };
        let projection = project(
            &spec,
            &self.layout_state.catalog.known_views,
            &self.layout_state.rail_registration,
        );
        self.layout_state.projection = projection;
        self.layout_state.applied = true;
    }

    pub(crate) fn handle_layout_action(&mut self, action: Action) {
        match action {
            Action::OpenLayoutPicker => self.layout_state.picker_open = true,
            Action::SwitchLayout { id, layer, .. } => {
                let hash = self
                    .layout_state
                    .catalog
                    .candidates
                    .iter()
                    .find(|candidate| candidate.id == id && candidate.layer == layer)
                    .map(|candidate| candidate.hash.clone())
                    .unwrap_or_default();
                self.layout_state.selection = Some(LayoutSelection { id, layer, hash });
                self.layout_state.picker_open = false;
                self.apply_layout_projection(true);
            }
            Action::SaveLayout => self.open_save_dialog(),
            Action::ResetLayout => self.restore_layout_defaults(),
            Action::ManageLayouts => {
                self.settings_open = true;
                self.settings_section = Some("appearance".to_owned());
            }
            _ => {}
        }
    }

    /// Modal do seletor, também aberto por `Ctrl+Alt+L`.
    pub(crate) fn layout_picker_window(&mut self, ctx: &egui::Context) {
        if !self.layout_state.picker_open {
            return;
        }
        let labels = self.layout_menu_labels();
        let mut open = true;
        let mut chosen = None;
        egui::Window::new(self.text("shell.layout.picker-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let items: Vec<forja_ui_kit::MenuItem<'_>> = labels
                    .iter()
                    .map(|label| forja_ui_kit::MenuItem {
                        label,
                        danger: false,
                    })
                    .collect();
                chosen = forja_ui_kit::menu_panel(ui, &items);
            });
        self.layout_state.picker_open = open;
        if let Some(index) = chosen {
            self.choose_layout_menu(index);
        }
    }
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
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), &name_label)
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

    /// Ajustes → Layout: edita o chrome do arranjo atual. As mudanças ficam na
    /// projeção em memória até "Salvar layout atual…" gravar pelo catálogo.
    pub(crate) fn layout_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.text("shell.layout.settings-title"));
        let current = self.current_layout_name();
        ui.label(self.text_args("shell.layout.settings-current", &[("name", &current)]));
        let mut rail = self.layout_rail_visible();
        if ui
            .checkbox(&mut rail, self.text("shell.layout.settings-rail"))
            .changed()
        {
            self.layout_state.projection.rail_visible = Some(rail);
        }
        let mut status = self.layout_status_bar_visible();
        if ui
            .checkbox(&mut status, self.text("shell.layout.settings-status"))
            .changed()
        {
            self.layout_state.projection.status_bar_visible = Some(status);
        }
        ui.separator();
        ui.label(self.text("shell.layout.settings-rail-order"));
        let order = self.layout_state.projection.effective_rail_order.clone();
        let hidden = self.layout_state.projection.rail_hidden.clone();
        let mut edit = None;
        for (index, id) in order.iter().enumerate() {
            let name = self
                .rail_views
                .iter()
                .find(|view| view.id == *id)
                .map(|view| self.text(&view.title_key))
                .unwrap_or_else(|| id.clone());
            ui.horizontal(|ui| {
                ui.label(&name);
                if index > 0
                    && forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &self.text("shell.layout.up"),
                    )
                    .clicked()
                {
                    edit = Some(RailEdit::Move(id.clone(), -1));
                }
                if index + 1 < order.len()
                    && forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &self.text("shell.layout.down"),
                    )
                    .clicked()
                {
                    edit = Some(RailEdit::Move(id.clone(), 1));
                }
                if forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Ghost,
                    &self.text("shell.layout.hide"),
                )
                .clicked()
                {
                    edit = Some(RailEdit::Hide(id.clone()));
                }
            });
        }
        for id in &hidden {
            let label = self.text_args("shell.layout.show", &[("id", id)]);
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &label).clicked() {
                edit = Some(RailEdit::Show(id.clone()));
            }
        }
        if let Some(edit) = edit {
            apply_rail_edit(&mut self.layout_state.projection, edit);
        }
        let restore = self.text("shell.layout.restore");
        if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &restore).clicked() {
            self.restore_layout_defaults();
        }
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

