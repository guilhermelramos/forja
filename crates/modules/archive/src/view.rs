//! Página "Acervo": busca, filtros, retomar, bifurcar e excluir.

use crate::index::{Filters, Hit};
use crate::parse::ArchiveSession;
use crate::state::{self, ArchiveState, ScanOptions};
use crate::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Aba do Acervo.
pub struct ArchiveView {
    core: Shared,
    query: String,
    profile_filter: Option<String>,
    project_filter: String,
    date_filter: String,
    pending_delete: Option<PathBuf>,
    notice: Option<String>,
    action_error: Option<String>,
    searched: bool,
}

impl ArchiveView {
    /// Cria a view ligada ao estado.
    pub fn new(core: Shared) -> Self {
        Self {
            core,
            query: String::new(),
            profile_filter: None,
            project_filter: String::new(),
            date_filter: "all".to_owned(),
            pending_delete: None,
            notice: None,
            action_error: None,
            searched: false,
        }
    }

    fn seed_preview(&self, state: &mut ArchiveState) {
        if std::env::var_os("FORJA_ARCHIVE_PREVIEW").is_none() || state.preview_seeded {
            return;
        }
        state.preview_seeded = true;
        let now = chrono::Utc::now().timestamp_millis();
        let sessions = vec![
            ArchiveSession {
                session_id: "preview-claude".to_owned(),
                helper_id: "claude-code".to_owned(),
                project: "C:/forja/pedido-123".to_owned(),
                path: "C:/forja/.sessions/preview-claude.jsonl".to_owned(),
                started_ms: now - 1000 * 60 * 20,
                updated_ms: now - 1000 * 60 * 4,
                title: "Corrigir o parser do Acervo".to_owned(),
                text: String::new(),
            },
            ArchiveSession {
                session_id: "preview-codex".to_owned(),
                helper_id: "codex".to_owned(),
                project: "C:/forja/pedido-123".to_owned(),
                path: "C:/forja/.sessions/preview-codex.jsonl".to_owned(),
                started_ms: now - 1000 * 60 * 120,
                updated_ms: now - 1000 * 60 * 60,
                title: "Ajustar o painel de resultados".to_owned(),
                text: String::new(),
            },
            ArchiveSession {
                session_id: "preview-opencode".to_owned(),
                helper_id: "opencode".to_owned(),
                project: "C:/forja/fatura".to_owned(),
                path: "C:/forja/.sessions/preview-opencode.json".to_owned(),
                started_ms: now - 1000 * 60 * 60 * 26,
                updated_ms: now - 1000 * 60 * 60 * 25,
                title: "Revisar a tabela e os trechos".to_owned(),
                text: String::new(),
            },
        ];
        state.results = sessions
            .iter()
            .map(|session| Hit {
                session_id: session.session_id.clone(),
                profile_id: session.helper_id.clone(),
                project: session.project.clone(),
                path: session.path.clone(),
                started_ms: session.started_ms,
                updated_ms: session.updated_ms,
                title: session.title.clone(),
                snippet:
                    "…o parser do Acervo precisa aceitar a transcrição e o filtro por ajudante…"
                        .to_owned(),
                score: 1,
            })
            .collect();
        state.sessions = sessions;
        state.report.discovered = 3;
        state.report.parsed = 3;
        state.last_scan = Some(std::time::Instant::now());
    }

    fn maybe_scan(&mut self, cx: &mut Ctx<'_>, state: &mut ArchiveState, index_enabled: bool) {
        if std::env::var_os("FORJA_ARCHIVE_PREVIEW").is_some() {
            self.seed_preview(state);
            self.searched = true;
            return;
        }
        if !index_enabled || !state.needs_scan() {
            return;
        }
        let cache = state.begin_scan();
        let profiles = state.profiles.clone();
        let index_dir = state::index_dir(&cx.store().path());
        let options = ScanOptions {
            keep_text: crate::keep_transcripts(cx),
            retention_days: cx
                .setting("archive.sessions.retention_days")
                .and_then(|value| value.as_i64())
                .unwrap_or(365),
            max_bytes: (cx
                .setting("archive.sessions.max_gb")
                .and_then(|value| value.as_i64())
                .unwrap_or(10)
                .max(1) as u64)
                * 1024
                * 1024
                * 1024,
        };
        let core = Arc::clone(&self.core);
        cx.spawn(
            move || state::scan_task(profiles, index_dir, cache, options),
            move |_cx, result| {
                if let Ok(mut state) = core.state.lock() {
                    match result {
                        Ok((cache, sessions, report)) => {
                            state.finish(cache, sessions, report);
                        }
                        Err(error) => {
                            let cache = std::mem::take(&mut state.cache);
                            state.fail(cache, error);
                        }
                    }
                }
            },
        );
    }

    fn filters(&self) -> Filters {
        let since_ms = match self.date_filter.as_str() {
            "7" => Some(chrono::Utc::now().timestamp_millis() - 7 * 24 * 60 * 60 * 1000),
            "30" => Some(chrono::Utc::now().timestamp_millis() - 30 * 24 * 60 * 60 * 1000),
            "365" => Some(chrono::Utc::now().timestamp_millis() - 365 * 24 * 60 * 60 * 1000),
            _ => None,
        };
        Filters {
            profile_id: self.profile_filter.clone(),
            project: (!self.project_filter.trim().is_empty())
                .then(|| self.project_filter.trim().to_owned()),
            since_ms,
            until_ms: None,
        }
    }

    fn search(&mut self, cx: &mut Ctx<'_>, state: &mut ArchiveState) {
        let index_dir = state::index_dir(&cx.store().path());
        match state::search_task(&index_dir, &self.query, &self.filters(), 100) {
            Ok(hits) => {
                state.results = hits;
                self.notice = None;
                self.action_error = None;
                self.searched = true;
            }
            Err(error) => {
                self.notice = None;
                self.action_error = Some(error);
                state.results.clear();
            }
        }
    }

    fn draw_filters(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>, state: &ArchiveState) {
        ui.horizontal_wrapped(|ui| {
            ui.scope(|ui| {
                ui.set_max_width(180.0);
                forja_ui_kit::text_field(ui, &mut self.query, &cx.t("archive.view.search-hint"));
            });
            ui.label(cx.t("archive.view.project"));
            ui.scope(|ui| {
                ui.set_max_width(100.0);
                forja_ui_kit::text_field(
                    ui,
                    &mut self.project_filter,
                    &cx.t("archive.view.project-hint"),
                );
            });
            // `select` não marca `changed()`: comparamos o índice antes e depois.
            let mut helpers: Vec<(Option<String>, String)> =
                vec![(None, cx.t("archive.view.all-helpers"))];
            for profile in &state.profiles {
                helpers.push((Some(profile.id.clone()), profile.name.clone()));
            }
            let helper_labels: Vec<&str> =
                helpers.iter().map(|(_, label)| label.as_str()).collect();
            let current = helpers
                .iter()
                .position(|(id, _)| *id == self.profile_filter)
                .unwrap_or(0);
            let mut index = current;
            if forja_ui_kit::select(ui, &mut index, &helper_labels).changed() {
                self.profile_filter = helpers[index].0.clone();
            }
            let dates = ["all", "7", "30", "365"];
            let date_texts: Vec<String> = dates.iter().map(|value| date_label(cx, value)).collect();
            let date_labels: Vec<&str> = date_texts.iter().map(String::as_str).collect();
            let current = dates
                .iter()
                .position(|value| *value == self.date_filter)
                .unwrap_or(0);
            let mut index = current;
            if forja_ui_kit::select(ui, &mut index, &date_labels).changed() {
                self.date_filter = dates[index].to_owned();
            }
        });
    }

    fn draw_results(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &mut ArchiveState) {
        let results = state.results.clone();
        if results.is_empty() {
            let has_filter = !self.query.trim().is_empty()
                || self.profile_filter.is_some()
                || !self.project_filter.trim().is_empty()
                || self.date_filter != "all";
            let key = if state.scanning {
                "archive.view.indexing"
            } else if state.sessions.is_empty() && !has_filter {
                "archive.view.no-sessions"
            } else {
                "archive.view.empty"
            };
            forja_ui_kit::page_state(
                ui,
                if state.scanning {
                    forja_ui_kit::PageState::Loading
                } else {
                    forja_ui_kit::PageState::Empty
                },
                &cx.t(if state.scanning {
                    "archive.page-state.loading"
                } else {
                    "archive.page-state.empty"
                }),
                Some(&cx.t(key)),
            );
            return;
        }
        let mut resume = None;
        let mut fork = None;
        let mut delete = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 20.0)
            .show(ui, |ui| {
                for hit in &results {
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.set_max_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&hit.title).strong());
                            ui.label(
                                egui::RichText::new(helper_name(state, &hit.profile_id)).weak(),
                            );
                            ui.label(egui::RichText::new(format_date(hit.updated_ms)).weak());
                        });
                        if !hit.snippet.is_empty() {
                            ui.label(egui::RichText::new(&hit.snippet).italics());
                        }
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&hit.project).monospace().weak());
                            ui.add_space(12.0);
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Secondary,
                                &cx.t("archive.view.resume"),
                            )
                            .clicked()
                            {
                                resume = Some((
                                    hit.profile_id.clone(),
                                    hit.session_id.clone(),
                                    hit.project.clone(),
                                ));
                            }
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Secondary,
                                &cx.t("archive.view.fork"),
                            )
                            .clicked()
                            {
                                fork = Some((
                                    hit.profile_id.clone(),
                                    hit.session_id.clone(),
                                    hit.project.clone(),
                                ));
                            }
                            if forja_ui_kit::button(
                                ui,
                                forja_ui_kit::ButtonKind::Danger,
                                &cx.t("archive.view.delete"),
                            )
                            .clicked()
                            {
                                delete = Some(PathBuf::from(&hit.path));
                            }
                        });
                    });
                }
            });
        if let Some((profile_id, session_id, project)) = resume {
            self.open_session(cx, profile_id, session_id, Some(project), false);
        }
        if let Some((profile_id, session_id, project)) = fork {
            self.open_session(cx, profile_id, session_id, Some(project), true);
        }
        if let Some(path) = delete {
            self.pending_delete = Some(path);
        }
    }

    fn open_session(
        &mut self,
        cx: &mut Ctx<'_>,
        profile_id: String,
        session_id: String,
        bench_path: Option<String>,
        fork: bool,
    ) {
        let mut args = Args::new();
        args.insert("profile_id", json!(profile_id));
        args.insert("session_id", json!(session_id));
        if let Some(bench) = bench_path {
            args.insert("bench_path", json!(bench));
        }
        args.insert("fork", json!(fork));
        match cx.run_command("archive.resume", args) {
            Ok(value) => {
                let line = value
                    .get("command_line")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                self.action_error = None;
                self.notice = Some(format!("{}: {line}", cx.t("archive.view.opened")));
            }
            Err(error) => {
                let message = match error {
                    forja_kernel::KernelError::Command { message, .. } => message,
                    other => other.to_string(),
                };
                self.notice = None;
                self.action_error = Some(message);
            }
        }
    }

    fn draw_delete_modal(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let Some(path) = self.pending_delete.clone() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(cx.t("archive.view.delete-title"))
            .id(egui::Id::new("forja-archive-delete"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(cx.t("archive.view.delete-body"));
                ui.label(egui::RichText::new(path.display().to_string()).monospace());
                ui.horizontal(|ui| {
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Danger,
                        &cx.t("archive.view.delete-confirm"),
                    )
                    .clicked()
                    {
                        confirm = true;
                    }
                    if forja_ui_kit::button(
                        ui,
                        forja_ui_kit::ButtonKind::Ghost,
                        &cx.t("archive.view.cancel"),
                    )
                    .clicked()
                    {
                        cancel = true;
                    }
                });
            });
        if cancel {
            self.pending_delete = None;
            return;
        }
        if !confirm {
            return;
        }
        let mut args = Args::new();
        args.insert("path", json!(path.display().to_string()));
        args.insert("confirm", json!(true));
        match cx.run_command("archive.delete", args) {
            Ok(_) => {
                self.action_error = None;
                self.notice = Some(cx.t("archive.view.deleted"));
                if let Ok(mut state) = self.core.state.lock() {
                    let text = path.display().to_string();
                    state.results.retain(|hit| hit.path != text);
                    state.cache.forget(&path);
                    state.sessions.retain(|session| session.path != text);
                }
            }
            Err(error) => {
                let message = match error {
                    forja_kernel::KernelError::Command { message, .. } => message,
                    other => other.to_string(),
                };
                self.notice = None;
                self.action_error = Some(message);
            }
        }
        self.pending_delete = None;
    }
}

impl View for ArchiveView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let core = Arc::clone(&self.core);
        let bench = cx
            .run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| {
                value
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let index_enabled = cx
            .setting("archive.sessions.index")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        {
            let Ok(mut state) = core.state.lock() else {
                return;
            };
            state.ensure_profiles(Some(&bench));
            self.maybe_scan(cx, &mut state, index_enabled);
        }
        let (report, scanning, error, results_count, sessions_count) = {
            let Ok(state) = core.state.lock() else {
                return;
            };
            (
                state.report.clone(),
                state.scanning,
                state.error.clone(),
                state.results.len(),
                state.sessions.len(),
            )
        };
        let title = cx.t("archive.view.title");
        let refresh = cx.t("archive.page-state.refresh");
        let search = cx.t("archive.view.search-button");
        let mut page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("archive.tab", &title)
                .primary(&search)
                .refresh(&refresh)
                .footer(),
        );
        if page.response.primary_clicked {
            if let Ok(mut state) = core.state.lock() {
                self.search(cx, &mut state);
            }
        }
        if page.response.refresh_clicked {
            if let Ok(mut state) = core.state.lock() {
                state.last_scan = None;
            }
        }
        page.filters(|ui| {
            if let Ok(state) = core.state.lock() {
                self.draw_filters(ui, cx, &state);
            }
        });
        page.body(|ui| {
            if scanning && results_count > 0 {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Loading,
                    &cx.t("archive.page-state.loading"),
                    None,
                );
            }
            if let Some(notice) = &self.notice {
                ui.label(egui::RichText::new(notice).weak());
            }
            for raw in [self.action_error.as_ref(), error.as_ref()]
                .into_iter()
                .flatten()
            {
                let human = humanize_error(cx, raw);
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Error,
                    &cx.t("archive.page-state.error"),
                    Some(&human.summary),
                );
            }
            let auto_search = core
                .state
                .lock()
                .map(|state| !self.searched && !state.scanning && !state.sessions.is_empty())
                .unwrap_or(false);
            if let Ok(mut state) = core.state.lock() {
                if auto_search {
                    self.search(cx, &mut state);
                }
                self.draw_results(ui, cx, &mut state);
            }
        });
        let count = format!(
            "{} · {} · {} {} · {}: {}",
            cx.t("archive.view.sessions"),
            sessions_count,
            report.discovered,
            cx.t("archive.view.files"),
            cx.t("archive.view.results"),
            results_count
        );
        page.footer(Some(&count), |_| {});
        self.draw_delete_modal(ui, cx);
        if scanning {
            forja_ui_kit::request_repaint_after_active(ui.ctx(), Duration::from_millis(250));
        }
    }
}

fn helper_name(state: &ArchiveState, id: &str) -> String {
    state
        .profiles
        .iter()
        .find(|profile| profile.id == id)
        .map(|profile| profile.name.clone())
        .unwrap_or_else(|| id.to_owned())
}

fn date_label(cx: &Ctx<'_>, value: &str) -> String {
    cx.t(match value {
        "7" => "archive.view.date-7",
        "30" => "archive.view.date-30",
        "365" => "archive.view.date-365",
        _ => "archive.view.date-all",
    })
}

fn format_date(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|time| time.format("%d/%m %H:%M").to_string())
        .unwrap_or_default()
}

pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    if trimmed.starts_with("busca inválida:") || trimmed.starts_with("invalid query:") {
        return forja_kernel::HumanError::with_detail(cx.t("archive.error.invalid-query"), trimmed);
    }
    forja_kernel::human_error::humanize_ctx(cx, trimmed)
}
