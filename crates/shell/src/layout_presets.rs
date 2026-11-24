//! Seletor de arranjos, projeção no chrome legado e gravação pelo catálogo.
//!
//! Toda leitura vem do estado já publicado pelo módulo `content` (comando
//! `content.layouts.list`); nenhuma função chamada por `ui()` toca disco. A
//! gravação passa por `content.layouts.save`, que é atômica e confinada.

use crate::actions::{self, Action};
use crate::app::ForjaApp;
use crate::layout_projection::{apply_rail_edit, order_side_tabs, project, Projection, RailEdit};
use crate::layout_save::SaveDialog;
use crate::layout_settings::{
    resolve_selection, CatalogSnapshot, LayoutCandidate, LayoutLayer, LayoutSelection,
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
    /// Último conjunto de refs indisponíveis avisado, para não repetir o toast.
    pub last_notified_refs: Vec<String>,
}

pub(crate) const FACTORY_IDS: [&str; 4] = ["oficina", "foco", "revisao", "mutirao"];

impl LayoutState {
    /// Estado inicial: honra a escolha salva da bancada e a ordem de registro
    /// do trilho sem espalhar essa montagem pela construção da janela.
    pub(crate) fn initial(
        shell_data: &crate::app::shell_store::ShellData,
        bench_id: Option<&str>,
        rail_views: &[crate::app::ViewInstance],
    ) -> Self {
        let selection = bench_id
            .and_then(|id| shell_data.layout.get(id))
            .and_then(|layout| layout.layout.clone());
        let rail_registration = rail_views.iter().map(|view| view.id.clone()).collect();
        Self {
            selection,
            rail_registration,
            ..Default::default()
        }
    }
}

/// Ações do seletor de arranjos, despachadas pelo shell numa única braça.
pub(crate) fn is_layout_action(action: &Action) -> bool {
    matches!(
        action,
        Action::OpenLayoutPicker
            | Action::SwitchLayout { .. }
            | Action::SaveLayout
            | Action::ResetLayout
            | Action::ManageLayouts
    )
}

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
        self.notify_unavailable_refs();
    }

    /// Refs que a projeção não consegue mostrar viram diagnóstico no canal dos
    /// avisos (toast `attention`), em pt-BR/en, sem descartar a ref do arquivo.
    pub(crate) fn notify_unavailable_refs(&mut self) {
        let refs = self.layout_state.projection.unavailable_refs.clone();
        if refs.is_empty() || refs == self.layout_state.last_notified_refs {
            return;
        }
        self.layout_state.last_notified_refs = refs.clone();
        let title = self.text("shell.layout.unavailable-ref-title");
        let body = refs
            .iter()
            .map(|id| self.text_args("shell.layout.unavailable-ref", &[("id", id)]))
            .collect::<Vec<_>>()
            .join("\n");
        let mut args = Args::new();
        args.insert("title", serde_json::Value::String(title));
        args.insert("body", serde_json::Value::String(body));
        args.insert("level", serde_json::Value::String("attention".into()));
        let _ = self.runtime.run_command("notify.show", args);
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

    /// Abre as duas janelas do seletor numa única chamada do loop do app.
    pub(crate) fn layout_windows(&mut self, ctx: &egui::Context) {
        self.layout_picker_window(ctx);
        self.layout_save_window(ctx);
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

