//! A página de Pedidos: filtros, busca, lista, gaveta de detalhes e pedidos locais.
//!
//! Regra de ouro: a tela copia o que precisa do estado e solta o lock antes de chamar
//! qualquer comando, porque os comandos também travam o estado.

use crate::commands::{self, with_state};
use crate::forge_preview::ForgePreview;
use crate::state::{self, Filter, Notice, NoticeKind, Shared, State};
use forja_contracts::{RequestItem, RequestKind, RequestState};
use forja_counter_kit::error_key;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::Ctx;
use forja_ui_kit::{button, segmented, text_field, wrapped_label, ButtonKind};
use std::time::SystemTime;

/// Cópia do estado usada por um quadro. Não guarda lock.
struct Frame {
    filter: Filter,
    source: Option<String>,
    project_known: bool,
    loading: bool,
    local_loaded: bool,
    items: Vec<RequestItem>,
    detail: Option<RequestItem>,
    selected: Option<(String, String)>,
    notice: Option<Notice>,
    errors: Vec<(String, String)>,
}

pub struct RequestsView {
    shared: Shared,
    started: bool,
    /// Em modo fixture (captura de tela), abre o primeiro pedido uma vez para mostrar a gaveta.
    auto_selected: bool,
    query_draft: String,
    comment_draft: String,
    local_open: bool,
    local_title: String,
    local_body: String,
    forge_preview: ForgePreview,
}

impl RequestsView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            started: false,
            auto_selected: false,
            query_draft: String::new(),
            comment_draft: String::new(),
            local_open: false,
            local_title: String::new(),
            local_body: String::new(),
            forge_preview: ForgePreview::default(),
        }
    }

    fn frame(&self) -> Frame {
        with_state(&self.shared, |state: &mut State| Frame {
            filter: state.filter,
            source: state.source.clone(),
            project_known: state.project.is_some(),
            loading: !state.pending.is_empty(),
            local_loaded: state.local_loaded,
            items: state::visible(state, SystemTime::now()),
            detail: state.detail.clone(),
            selected: state.selected.clone(),
            notice: state.notice.clone(),
            errors: state
                .errors
                .iter()
                .map(|(counter, error)| (counter.clone(), error_key("request", error)))
                .collect(),
        })
        .unwrap_or_else(|| Frame {
            filter: Filter::All,
            source: None,
            project_known: false,
            loading: false,
            local_loaded: false,
            items: Vec::new(),
            detail: None,
            selected: None,
            notice: None,
            errors: Vec::new(),
        })
    }

    fn set_filter(&self, filter: Filter) {
        with_state(&self.shared, |state| state.filter = filter);
    }

    fn set_query(&self, query: &str) {
        with_state(&self.shared, |state| state.query = query.to_owned());
    }

    /// Primeiro quadro: carrega os pedidos locais e pede a lista dos balcões.
    fn start(&mut self, cx: &mut Ctx<'_>) {
        if self.started {
            return;
        }
        self.started = true;
        self.query_draft =
            with_state(&self.shared, |state| state.query.clone()).unwrap_or_default();
        commands::ensure_local(cx, &self.shared);
        let _ = commands::refresh(cx, &self.shared);
    }

    /// Atualização automática pelo intervalo da configuração, ou logo após conectar.
    fn maybe_refresh(&self, cx: &mut Ctx<'_>, ui: &egui::Ui) {
        let interval = cx
            .setting("request.refresh.interval_s")
            .and_then(forja_kernel::SettingValue::as_i64)
            .unwrap_or(60);
        let Some(every) = state::refresh_interval(interval) else {
            return;
        };
        forja_ui_kit::request_repaint_after_active(ui.ctx(), every);
        let due = with_state(&self.shared, |state| {
            state.needs_refresh
                || state
                    .last_refresh
                    .map(|last| last.elapsed().map(|age| age >= every).unwrap_or(true))
                    .unwrap_or(true)
        })
        .unwrap_or(false);
        if due {
            let _ = commands::refresh(cx, &self.shared);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) -> forja_ui_kit::Page {
        let title = cx.t("request-name");
        let refresh = cx.t("request.page-state.refresh");
        let create = cx.t("request-local-create");
        let page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("request.page", &title)
                .wide()
                .primary(&create)
                .refresh(&refresh),
        );
        if page.response.refresh_clicked {
            let _ = commands::refresh(cx, &self.shared);
        }
        if page.response.primary_clicked {
            self.local_open = !self.local_open;
        }
        page
    }

    fn filters(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, frame: &Frame) {
        let width = ui.available_width();
        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.set_width((width * 0.25).clamp(100.0, 240.0));
                if text_field(ui, &mut self.query_draft, &cx.t("request-search-hint")).changed() {
                    self.set_query(&self.query_draft.clone());
                }
            });
            ui.scope(|ui| {
                ui.set_width(
                    (width - (width * 0.25).clamp(100.0, 240.0) - 130.0 - 16.0).max(160.0),
                );
                let labels: Vec<String> = Filter::ALL.iter().map(|f| cx.t(f.label_key())).collect();
                let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
                let mut selected = Filter::ALL
                    .iter()
                    .position(|filter| *filter == frame.filter)
                    .unwrap_or(4);
                if segmented(ui, &mut selected, &refs).changed() {
                    if let Some(filter) = Filter::ALL.get(selected).copied() {
                        self.set_filter(filter);
                        let _ = commands::refresh(cx, &self.shared);
                    }
                }
            });
            ui.scope(|ui| {
                ui.set_width(130.0);
                self.source_row(ui, cx, frame);
            });
        });
    }

    /// Balcão mostrado: local, cada balcão instalado e todos.
    fn source_row(&self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, frame: &Frame) {
        let mut options: Vec<Option<String>> = vec![Some(crate::local::COUNTER.to_owned())];
        options.extend(
            commands::sources(cx)
                .into_iter()
                .map(|(counter, _)| Some(counter)),
        );
        options.push(None);
        let labels: Vec<String> = options
            .iter()
            .map(|option| match option {
                Some(counter) => counter_label(cx, counter),
                None => cx.t(Filter::All.label_key()),
            })
            .collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut selected = options
            .iter()
            .position(|option| *option == frame.source)
            .unwrap_or(options.len() - 1);
        if forja_ui_kit::select(ui, &mut selected, &refs).changed() {
            if let Some(source) = options.get(selected).cloned() {
                commands::set_source(&self.shared, source);
                let _ = commands::refresh(cx, &self.shared);
            }
        }
    }

    fn notices(&self, ui: &mut egui::Ui, cx: &Ctx<'_>, frame: &Frame) {
        if !frame.project_known && !forja_counter_kit::fixtures_enabled() {
            ui.label(egui::RichText::new(cx.t("request-no-project")).weak());
        }
        if let Some(notice) = &frame.notice {
            let color = match notice.kind {
                NoticeKind::Ok => egui::Color32::from_rgb(0x6C, 0xC4, 0x8A),
                NoticeKind::Warn => egui::Color32::from_rgb(0xFF, 0xC1, 0x4D),
                NoticeKind::Error => egui::Color32::from_rgb(0xE5, 0x53, 0x4B),
                NoticeKind::Info => ui.visuals().weak_text_color(),
            };
            ui.colored_label(color, cx.t(&notice.key));
        }
        for (counter, key) in &frame.errors {
            let counter_name = counter_label(cx, counter);
            let error_text = cx.t(key);
            let line = cx.t_args(
                "request-error-line",
                &[
                    ("counter", counter_name.as_str()),
                    ("error", error_text.as_str()),
                ],
            );
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Error,
                &cx.t("request.page-state.error"),
                Some(&line),
            );
        }
    }

    fn local_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, frame: &Frame) {
        if !self.local_open {
            return;
        }
        if !frame.local_loaded {
            ui.label(cx.t("request-local-loading"));
            return;
        }
        let mut submit = false;
        ui.group(|ui| {
            ui.label(
                egui::RichText::new(cx.t("request-local-title-label"))
                    .small()
                    .strong(),
            );
            let title_hint = cx.t("request-local-title-hint");
            let title = text_field(ui, &mut self.local_title, &title_hint);
            title.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), &title_hint)
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(cx.t("request-local-body-label"))
                    .small()
                    .strong(),
            );
            let body_hint = cx.t("request-local-body-hint");
            let body = text_field(ui, &mut self.local_body, &body_hint);
            body.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), &body_hint)
            });
            let enter_pressed = ui.input(|input| input.key_pressed(egui::Key::Enter));
            let title_active = title.has_focus() || title.lost_focus();
            let body_active = body.has_focus() || body.lost_focus();
            if enter_pressed && title_active && !body.has_focus() {
                body.request_focus();
            } else if enter_pressed && body_active {
                submit = true;
            }
            if button(ui, ButtonKind::Primary, &cx.t("request-local-create")).clicked() {
                submit = true;
            }
        });
        if submit {
            let body = self.local_body.trim().to_owned();
            let body = (!body.is_empty()).then_some(body);
            let title = self.local_title.clone();
            if commands::create_local(cx, &self.shared, &title, body.as_deref()).is_ok() {
                self.local_title.clear();
                self.local_body.clear();
                self.local_open = false;
            }
        }
    }

    fn list(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, frame: &Frame) {
        if frame.items.is_empty() {
            forja_ui_kit::page_state(
                ui,
                if frame.loading {
                    forja_ui_kit::PageState::Loading
                } else {
                    forja_ui_kit::PageState::Empty
                },
                &cx.t(if frame.loading {
                    "request.page-state.loading"
                } else {
                    "request.page-state.empty"
                }),
                Some(&cx.t("request-empty")),
            );
            return;
        }
        egui::ScrollArea::vertical()
            .id_salt("request.list")
            .show(ui, |ui| {
                for item in &frame.items {
                    let selected = frame
                        .selected
                        .as_ref()
                        .is_some_and(|(counter, key)| *counter == item.counter && *key == item.key);
                    let title = format!("{}  {}", item.key, item.title);
                    if ui.selectable_label(selected, title).clicked() {
                        let _ = commands::open(cx, &self.shared, &item.counter, &item.key);
                    }
                    ui.label(egui::RichText::new(row_meta(cx, item)).small().weak());
                    ui.add_space(4.0);
                }
            });
    }

    fn drawer(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, frame: &Frame) {
        let Some(item) = frame.detail.clone() else {
            return;
        };
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let metrics = tokens.metrics();
        let text_color = tokens.color.text.to_color32();
        let muted_color = tokens.color.text_muted.to_color32();
        let body_font = egui::FontId::proportional(metrics.body);
        let caption_font = egui::FontId::proportional(metrics.caption);

        egui::ScrollArea::vertical()
            .id_salt("request.drawer.scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let max_w = ui.available_width().max(1.0);
                ui.set_max_width(max_w);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    if button(ui, ButtonKind::Ghost, &cx.t("request-detail-close")).clicked() {
                        with_state(&self.shared, |state| {
                            state.selected = None;
                            state.detail = None;
                        });
                    }
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        let title_w = ui.available_width().max(1.0);
                        ui.set_max_width(title_w);
                        wrapped_label(
                            ui,
                            &format!("{}  {}", item.key, item.title),
                            body_font.clone(),
                            text_color,
                        );
                    });
                });
                wrapped_label(ui, &row_meta(cx, &item), caption_font.clone(), muted_color);
                if let Some(url) = &item.url {
                    wrapped_label(ui, url, caption_font.clone(), muted_color);
                }
                ui.add_space(6.0);
                match item
                    .body
                    .as_deref()
                    .map(str::trim)
                    .filter(|body| !body.is_empty())
                {
                    Some(body) => {
                        wrapped_label(ui, body, body_font.clone(), text_color);
                    }
                    None => {
                        wrapped_label(
                            ui,
                            &cx.t("request-detail-empty-body"),
                            body_font.clone(),
                            muted_color,
                        );
                    }
                }
                ui.add_space(8.0);
                self.actions(ui, cx, &item, frame.project_known);
                ui.add_space(8.0);
                ui.label(egui::RichText::new(cx.t("request-detail-comments")).strong());
                if item.comments.is_empty() {
                    ui.label(egui::RichText::new(cx.t("request-detail-no-comments")).weak());
                }
                for comment in &item.comments {
                    let who = if comment.author.is_empty() {
                        cx.t("request-local-tag")
                    } else {
                        comment.author.clone()
                    };
                    wrapped_label(
                        ui,
                        &format!("{who}  {}", comment.created_at.clone().unwrap_or_default()),
                        caption_font.clone(),
                        muted_color,
                    );
                    wrapped_label(ui, comment.body.as_str(), body_font.clone(), text_color);
                    ui.add_space(4.0);
                }
                ui.add(
                    egui::TextEdit::multiline(&mut self.comment_draft)
                        .hint_text(cx.t("request-comment-hint"))
                        .desired_width(f32::INFINITY)
                        .desired_rows(2),
                );
                if button(ui, ButtonKind::Secondary, &cx.t("request-action-comment")).clicked()
                    && !self.comment_draft.trim().is_empty()
                {
                    let body = self.comment_draft.clone();
                    if commands::comment(cx, &self.shared, &item.counter, &item.key, &body).is_ok()
                    {
                        self.comment_draft.clear();
                    }
                }
            });
    }

    fn actions(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        item: &RequestItem,
        project_known: bool,
    ) {
        ui.horizontal_wrapped(|ui| {
            let can_forge = project_known || forja_counter_kit::fixtures_enabled();
            ui.add_enabled_ui(can_forge, |ui| {
                if button(ui, ButtonKind::Primary, &cx.t("request-action-forge")).clicked() {
                    self.forge_preview.begin(cx, &self.shared, item);
                }
            });
            let (label, to) = if item.is_open() {
                (cx.t("request-action-close"), RequestState::Closed)
            } else {
                (cx.t("request-action-reopen"), RequestState::Open)
            };
            if button(ui, ButtonKind::Secondary, &label).clicked() {
                let _ = commands::transition(cx, &self.shared, &item.counter, &item.key, to);
            }
        });
    }
}

impl View for RequestsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.start(cx);
        self.maybe_refresh(cx, ui);
        let frame = self.frame();
        if !self.auto_selected && forja_counter_kit::fixtures_enabled() && frame.selected.is_none()
        {
            if let Some(first) = frame.items.first().cloned() {
                self.auto_selected = true;
                let _ = commands::open(cx, &self.shared, &first.counter, &first.key);
            }
        }
        let mut page = self.header(ui, cx);
        page.filters(|ui| self.filters(ui, cx, &frame));
        page.body(|ui| {
            self.notices(ui, cx, &frame);
            self.local_form(ui, cx, &frame);
            ui.separator();
            if frame.selected.is_some() {
                ui.columns(2, |columns| {
                    self.list(&mut columns[0], cx, &frame);
                    self.drawer(&mut columns[1], cx, &frame);
                });
            } else {
                self.list(ui, cx, &frame);
            }
        });
        self.forge_preview.ui(ui, cx, &self.shared);
    }
}

/// Nome do balcão para a tela: `GitHub`, `GitLab`, `Linear`, `Jira` ou `Local`.
fn counter_label(cx: &Ctx<'_>, counter: &str) -> String {
    if counter == crate::local::COUNTER {
        return cx.t("request-counter-local");
    }
    cx.t(&format!("counter_{counter}-name"))
}

/// Linha de apoio do pedido: balcão, tipo, estado, autor e rótulos.
fn row_meta(cx: &Ctx<'_>, item: &RequestItem) -> String {
    let mut parts = vec![counter_label(cx, &item.counter)];
    parts.push(match item.kind {
        RequestKind::Issue => cx.t("request-kind-issue"),
        RequestKind::Change => cx.t("request-kind-change"),
    });
    if item.draft {
        parts.push(cx.t("request-draft"));
    }
    parts.push(cx.t(state::state_key(item.state)));
    if let Some(author) = &item.author {
        parts.push(author.clone());
    }
    if !item.labels.is_empty() {
        parts.push(item.labels.join(", "));
    }
    parts.join(" · ")
}
