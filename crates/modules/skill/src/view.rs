//! Página "Skills" (aba de bancada): skills agrupadas por dono e escopo, busca, o `SKILL.md` do
//! item escolhido e as ações de instalar, copiar e remover.
//!
//! Como nas outras telas: copia o estado, solta o lock e só então chama comandos. Toda operação
//! que mexe em disco passa por uma confirmação. Fora da Forja, a confirmação mostra o caminho e
//! a escrita registra o consentimento.

use crate::actions;
use crate::model::{RootInfo, Scope, Skill, FORJA_OWNER};
use crate::state::{InstallKind, Notice, Pending, Shared};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use forja_ui_kit::{button, segmented, select_labeled, text_field_labeled, ButtonKind};
use std::path::PathBuf;

/// Chaves que a tela monta em tempo de execução.
pub const USED_KEYS: &[&str] = &[
    "skill.tab.title",
    "skill.intro",
    "skill.action.install-folder",
    "skill.action.install-zip",
    "skill.action.refresh",
    "skill.target.label",
    "skill.copy.label",
    "skill.copy.action",
    "skill.remove.action",
    "skill.scope.user",
    "skill.scope.project",
    "skill.search.hint",
    "skill.search.help",
    "skill.install.owner-label",
    "skill.copy.owner-label",
    "skill.list.loading",
    "skill.list.empty",
    "skill.detail.empty",
    "skill.detail.loading",
    "skill.detail.path",
    "skill.hint.no-project",
    "skill.confirm.install.title",
    "skill.confirm.install.body",
    "skill.confirm.copy.title",
    "skill.confirm.copy.body",
    "skill.confirm.remove.title",
    "skill.confirm.remove.body",
    "skill.confirm.outside",
    "skill.confirm.ok",
    "skill.dialog.cancel",
    "skill.conflict.title",
    "skill.conflict.body",
    "skill.conflict.replace",
];

/// Cópia do estado que a tela usa num quadro. Solta o lock antes de desenhar.
#[derive(Clone, Default)]
struct Snap {
    project: Option<String>,
    scanned: bool,
    scanning: bool,
    roots: Vec<RootInfo>,
    skills: Vec<Skill>,
    notice: Option<Notice>,
    pending: Option<Pending>,
    conflict: Option<String>,
    selected: Option<String>,
    body: Option<(String, String)>,
}

/// Escolhas da tela: destino das instalações, e o destino da cópia do item selecionado.
pub struct SkillView {
    shared: Shared,
    query: String,
    started: bool,
    install_owner: usize,
    install_scope: usize,
    copy_owner: usize,
    copy_scope: usize,
}

impl SkillView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            query: String::new(),
            started: false,
            install_owner: 0,
            install_scope: 0,
            copy_owner: 0,
            copy_scope: 0,
        }
    }

    fn snapshot(&self) -> Snap {
        self.shared
            .with(|state| Snap {
                project: state.listing.project.clone(),
                scanned: state.scanned,
                scanning: state.scanning,
                roots: state.listing.roots.clone(),
                skills: state.listing.skills.clone(),
                notice: state.notice.clone(),
                pending: state.pending.clone(),
                conflict: state.conflict.clone(),
                selected: state.selected.clone(),
                body: state.body.clone(),
            })
            .unwrap_or_default()
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        let owners = owners(&snap.roots);
        let owner_names: Vec<&str> = owners.iter().map(|(_, name)| name.as_str()).collect();
        let scope_labels = [cx.t("skill.scope.user"), cx.t("skill.scope.project")];
        let scope_refs: Vec<&str> = scope_labels.iter().map(String::as_str).collect();

        ui.horizontal_wrapped(|ui| {
            segmented(ui, &mut self.install_scope, &scope_refs);
            forja_ui_kit::text(ui, cx.t("skill.target.label"));
            if !owner_names.is_empty() {
                let mut index = self.install_owner.min(owners.len().saturating_sub(1));
                select_labeled(ui, &mut index, &cx.t("skill.install.owner-label"), &owner_names);
                self.install_owner = index;
            }
        });
        if snap.project.is_none() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("skill.hint.no-project")).weak());
        }
    }

    fn pick(&self, cx: &mut Ctx<'_>, kind: InstallKind, owners: &[(String, String)], index: usize) {
        let Some((owner, owner_name)) = owners.get(index).cloned() else {
            return;
        };
        let scope = scope_of(self.install_scope);
        let shared = self.shared.clone();
        cx.spawn(
            move || match kind {
                InstallKind::Folder => forja_kernel::test_pick::folder()
                    .unwrap_or_else(|| rfd::FileDialog::new().pick_folder()),
                InstallKind::Zip => forja_kernel::test_pick::file().unwrap_or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("zip", &["zip"])
                        .pick_file()
                }),
            },
            move |_cx, picked: Option<PathBuf>| {
                if let Some(source) = picked {
                    shared.with(|state| {
                        state.conflict = None;
                        state.pending = Some(Pending::Install {
                            kind,
                            source,
                            owner,
                            owner_name,
                            scope,
                        });
                    });
                }
            },
        );
    }

    fn notice(&self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        if let Some(notice) = &snap.notice {
            let text = cx.t_args(&notice.key, &[("detail", notice.detail.as_str())]);
            let failed = notice.key.starts_with("skill-error");
            if failed {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Error,
                    &cx.t("skill.page-state.error"),
                    Some(&text),
                );
            } else {
                forja_ui_kit::text(ui, text);
            }
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        if !snap.scanned {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Loading,
                &cx.t("skill.page-state.loading"),
                None,
            );
            return;
        }
        if snap.scanning {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Loading,
                &cx.t("skill.page-state.loading"),
                None,
            );
        }
        let query = self.query.trim().to_lowercase();
        let visible: Vec<&Skill> = snap
            .skills
            .iter()
            .filter(|skill| query.is_empty() || matches_query(skill, &query))
            .collect();
        if visible.is_empty() {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Empty,
                &cx.t("skill.page-state.empty"),
                Some(&cx.t("skill.list.empty")),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("skill-list")
            .max_height(440.0)
            .show(ui, |ui| {
                for (owner, owner_name) in owners(&snap.roots) {
                    let items: Vec<&&Skill> = visible.iter().filter(|s| s.owner == owner).collect();
                    if items.is_empty() {
                        continue;
                    }
                    egui::CollapsingHeader::new(owner_name)
                        .id_salt(format!("owner-{owner}"))
                        .default_open(true)
                        .show(ui, |ui| {
                            for scope in [Scope::User, Scope::Project] {
                                let rows: Vec<&&Skill> =
                                    items.iter().filter(|s| s.scope == scope).copied().collect();
                                if rows.is_empty() {
                                    continue;
                                }
                                forja_ui_kit::text(ui, egui::RichText::new(scope_label(cx, scope)).strong());
                                for skill in rows {
                                    self.row(ui, cx, snap, skill);
                                }
                            }
                        });
                }
            });
    }

    fn row(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap, skill: &Skill) {
        let selected = snap.selected.as_deref() == Some(skill.path.as_str());
        ui.vertical(|ui| {
            let title = skill.name.clone().unwrap_or_else(|| skill.folder.clone());
            if ui.selectable_label(selected, title).clicked() && !selected {
                let path = skill.path.clone();
                self.shared.with(|state| {
                    state.selected = Some(path.clone());
                    state.body = None;
                });
                actions::read_body(cx, &self.shared, PathBuf::from(&skill.path));
            }
            if let Some(description) = &skill.description {
                forja_ui_kit::text(ui, egui::RichText::new(description).weak().small());
            }
            if let Some(error) = &skill.error {
                forja_ui_kit::text(ui,
                    egui::RichText::new(cx.t(error))
                        .color(ui.visuals().error_fg_color)
                        .small(),
                );
            }
        });
    }

    fn detail(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        let Some(path) = snap.selected.clone() else {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Empty,
                &cx.t("skill.page-state.empty"),
                Some(&cx.t("skill.detail.empty")),
            );
            return;
        };
        let Some(skill) = snap.skills.iter().find(|skill| skill.path == path) else {
            return;
        };
        ui.heading(skill.name.clone().unwrap_or_else(|| skill.folder.clone()));
        if let Some(description) = &skill.description {
            forja_ui_kit::text(ui, description);
        }
        forja_ui_kit::text(ui,
            egui::RichText::new(cx.t_args("skill.detail.path", &[("path", skill.path.as_str())]))
                .weak()
                .small(),
        );
        if let Some(error) = &skill.error {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t(error)).color(ui.visuals().error_fg_color));
        }
        match &snap.body {
            Some((body_path, text)) if body_path == &path => {
                egui::ScrollArea::vertical()
                    .id_salt("skill-body")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        forja_ui_kit::text(ui, egui::RichText::new(text).monospace());
                    });
            }
            _ => {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Loading,
                    &cx.t("skill.page-state.loading"),
                    None,
                );
            }
        }
        ui.separator();
        self.actions(ui, cx, snap, skill);
    }

    fn actions(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap, skill: &Skill) {
        let owners = owners(&snap.roots);
        let owner_names: Vec<&str> = owners.iter().map(|(_, name)| name.as_str()).collect();
        let scope_labels = [cx.t("skill.scope.user"), cx.t("skill.scope.project")];
        let scope_refs: Vec<&str> = scope_labels.iter().map(String::as_str).collect();
        ui.horizontal_wrapped(|ui| {
            forja_ui_kit::text(ui, cx.t("skill.copy.label"));
            if !owner_names.is_empty() {
                let mut index = self.copy_owner.min(owners.len() - 1);
                select_labeled(ui, &mut index, &cx.t("skill.copy.owner-label"), &owner_names);
                self.copy_owner = index;
            }
            segmented(ui, &mut self.copy_scope, &scope_refs);
            if button(ui, ButtonKind::Secondary, &cx.t("skill.copy.action")).clicked() {
                if let Some((owner, owner_name)) = owners
                    .get(self.copy_owner.min(owners.len().saturating_sub(1)))
                    .cloned()
                {
                    let pending = Pending::Copy {
                        from: PathBuf::from(&skill.path),
                        owner,
                        owner_name,
                        scope: scope_of(self.copy_scope),
                    };
                    self.shared.with(|state| state.pending = Some(pending));
                }
            }
            if button(ui, ButtonKind::Danger, &cx.t("skill.remove.action")).clicked() {
                let pending = Pending::Remove {
                    path: PathBuf::from(&skill.path),
                    folder: skill.folder.clone(),
                    owner: skill.owner.clone(),
                    owner_name: skill.owner_name.clone(),
                    scope: skill.scope,
                };
                self.shared.with(|state| state.pending = Some(pending));
            }
        });
    }

    /// Confirmações: a de sobrescrita (quando a instalação encontrou uma skill igual) e a da
    /// operação pendente. Fora da Forja, a confirmação mostra o caminho e registra o consentimento.
    fn dialogs(&mut self, ctx: &egui::Context, cx: &mut Ctx<'_>, snap: &Snap) {
        if let Some(existing) = &snap.conflict {
            let external = snap.pending.as_ref().is_some_and(is_external);
            let mut replace = false;
            let mut cancel = false;
            egui::Window::new(cx.t("skill.conflict.title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    forja_ui_kit::text(ui, cx.t_args("skill.conflict.body", &[("path", existing.as_str())]));
                    ui.horizontal(|ui| {
                        replace = button(
                            ui,
                            ButtonKind::DangerConfirm,
                            &cx.t("skill.conflict.replace"),
                        )
                        .clicked();
                        cancel =
                            button(ui, ButtonKind::Ghost, &cx.t("skill.dialog.cancel")).clicked();
                    });
                });
            if replace {
                actions::run_pending(cx, &self.shared, true, external);
            } else if cancel {
                self.shared.with(|state| {
                    state.conflict = None;
                    state.pending = None;
                });
            }
            return;
        }
        let Some(pending) = &snap.pending else {
            return;
        };
        let external = is_external(pending);
        let (title, body) = describe(cx, pending);
        let outside = external.then(|| {
            let path = root_path(pending, &snap.roots);
            cx.t_args("skill.confirm.outside", &[("path", path.as_str())])
        });
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                forja_ui_kit::text(ui, body);
                if let Some(outside) = &outside {
                    forja_ui_kit::text(ui, egui::RichText::new(outside).color(ui.visuals().warn_fg_color));
                }
                ui.horizontal(|ui| {
                    confirm = button(ui, ButtonKind::Primary, &cx.t("skill.confirm.ok")).clicked();
                    cancel = button(ui, ButtonKind::Ghost, &cx.t("skill.dialog.cancel")).clicked();
                });
            });
        if confirm {
            actions::run_pending(cx, &self.shared, false, external);
        } else if cancel {
            self.shared.with(|state| state.pending = None);
        }
    }
}

impl View for SkillView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.started {
            self.started = true;
            actions::refresh(cx, &self.shared);
        }
        let snap = self.snapshot();
        let title = cx.t("skill.tab.title");
        let subtitle = cx.t("skill.intro");
        let refresh = cx.t("skill.page-state.refresh");
        let install = cx.t("skill.action.install-folder");
        let zip = cx.t("skill.action.install-zip");
        let icons = [forja_ui_kit::PageIconAction {
            icon: forja_ui_kit::Icon::Attach,
            label: &zip,
        }];
        let mut page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("skill.page", &title)
                .subtitle(&subtitle)
                .primary(&install)
                .refresh(&refresh)
                .icons(&icons),
        );
        let owners = owners(&snap.roots);
        let index = self.install_owner.min(owners.len().saturating_sub(1));
        if page.response.primary_clicked {
            self.pick(cx, InstallKind::Folder, &owners, index);
        }
        if page.response.icon_clicked == Some(0) {
            self.pick(cx, InstallKind::Zip, &owners, index);
        }
        if page.response.refresh_clicked {
            actions::refresh(cx, &self.shared);
        }
        page.filters(|ui| {
            text_field_labeled(
                ui,
                &mut self.query,
                &cx.t("skill.search.hint"),
                &cx.t("skill.search.help"),
            );
            self.toolbar(ui, cx, &snap);
        });
        page.body(|ui| {
            self.notice(ui, cx, &snap);
            ui.columns(2, |columns| {
                let (left, right) = columns.split_at_mut(1);
                self.list(&mut left[0], cx, &snap);
                self.detail(&mut right[0], cx, &snap);
            });
        });
        self.dialogs(ui.ctx(), cx, &snap);
    }
}

/// Donos na ordem das raízes, sem repetir: (id, nome visível).
fn owners(roots: &[RootInfo]) -> Vec<(String, String)> {
    let mut owners: Vec<(String, String)> = Vec::new();
    for root in roots {
        if !owners.iter().any(|(id, _)| id == &root.owner) {
            owners.push((root.owner.clone(), root.owner_name.clone()));
        }
    }
    if let Some(index) = owners.iter().position(|(id, _)| id == FORJA_OWNER) {
        let forja = owners.remove(index);
        owners.insert(0, forja);
    }
    owners
}

fn scope_of(index: usize) -> Scope {
    if index == 1 {
        Scope::Project
    } else {
        Scope::User
    }
}

fn scope_label(cx: &Ctx<'_>, scope: Scope) -> String {
    match scope {
        Scope::User => cx.t("skill.scope.user"),
        Scope::Project => cx.t("skill.scope.project"),
    }
}

fn matches_query(skill: &Skill, query: &str) -> bool {
    skill.folder.to_lowercase().contains(query)
        || skill
            .name
            .as_deref()
            .is_some_and(|name| name.to_lowercase().contains(query))
        || skill
            .description
            .as_deref()
            .is_some_and(|description| description.to_lowercase().contains(query))
}

/// Dono e escopo de uma operação pendente.
fn target_of(pending: &Pending) -> (&str, Scope) {
    match pending {
        Pending::Install { owner, scope, .. } | Pending::Copy { owner, scope, .. } => {
            (owner.as_str(), *scope)
        }
        Pending::Remove { owner, scope, .. } => (owner.as_str(), *scope),
    }
}

/// A operação escreve numa raiz que não é da Forja?
fn is_external(pending: &Pending) -> bool {
    target_of(pending).0 != FORJA_OWNER
}

/// O caminho da raiz de destino, para mostrar na confirmação.
fn root_path(pending: &Pending, roots: &[RootInfo]) -> String {
    let (owner, scope) = target_of(pending);
    roots
        .iter()
        .find(|root| root.owner == owner && root.scope == scope)
        .map(|root| root.path.clone())
        .unwrap_or_default()
}

/// Título e texto da confirmação de uma operação pendente.
fn describe(cx: &Ctx<'_>, pending: &Pending) -> (String, String) {
    match pending {
        Pending::Install {
            source,
            owner_name,
            scope,
            ..
        } => {
            let source = source.display().to_string();
            let scope = scope_label(cx, *scope);
            (
                cx.t("skill.confirm.install.title"),
                cx.t_args(
                    "skill.confirm.install.body",
                    &[
                        ("source", source.as_str()),
                        ("owner", owner_name.as_str()),
                        ("scope", scope.as_str()),
                    ],
                ),
            )
        }
        Pending::Copy {
            from,
            owner_name,
            scope,
            ..
        } => {
            let folder = from
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let scope = scope_label(cx, *scope);
            (
                cx.t("skill.confirm.copy.title"),
                cx.t_args(
                    "skill.confirm.copy.body",
                    &[
                        ("folder", folder.as_str()),
                        ("owner", owner_name.as_str()),
                        ("scope", scope.as_str()),
                    ],
                ),
            )
        }
        Pending::Remove {
            folder, owner_name, ..
        } => (
            cx.t("skill.confirm.remove.title"),
            cx.t_args(
                "skill.confirm.remove.body",
                &[("folder", folder.as_str()), ("owner", owner_name.as_str())],
            ),
        ),
    }
}
