//! Browser tab: address bar, pages, tabs, history and downloads.

use crate::state::{Kind, Pending, Shared, Tab};
use crate::view_support::{c32, home_html};
use crate::visibility;
use crate::webview::{logical_rect, Config, NativeTab, Parent, Target};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx, SettingValue};
use serde_json::json;
use std::path::{Path, PathBuf};

pub struct BrowserView {
    state: Shared,
    native: Vec<(u64, NativeTab)>,
    parent: Option<Parent>,
    bootstrapped: bool,
    address: String,
    profile_root: PathBuf,
    last_active: Option<u64>,
    last_url: String,
    scale: f32,
    default_zoom: f64,
    last_tabs: Vec<String>,
    home: crate::home::HomeState,
}

impl BrowserView {
    pub fn new(state: Shared) -> Self {
        Self {
            state,
            native: Vec::new(),
            parent: None,
            bootstrapped: false,
            address: String::new(),
            profile_root: forja_kernel::paths::default_data_dir().join("webview"),
            last_active: None,
            last_url: String::new(),
            scale: 1.0,
            default_zoom: 1.0,
            last_tabs: Vec::new(),
            home: crate::home::HomeState::new(),
        }
    }

    fn bootstrap(&mut self, cx: &mut Ctx<'_>) {
        if self.bootstrapped {
            return;
        }
        self.bootstrapped = true;
        if let Ok(url) = std::env::var("FORJA_BROWSER_URL") {
            if !url.trim().is_empty() {
                self.state.open(&url, None, true);
                return;
            }
        }
        if self.state.tabs().is_empty() {
            let restore = cx
                .setting("browser.tabs.restore")
                .and_then(SettingValue::as_bool)
                .unwrap_or(true);
            if restore {
                for url in load_tabs().into_iter().take(crate::state::MAX_TABS) {
                    self.state.open(&url, None, true);
                }
            }
            if self.state.tabs().is_empty() {
                let home = home_url(cx);
                let html = if home.starts_with("forja://") && !crate::home::is_home_url(&home) {
                    Some(home_html(
                        &cx.t("browser.view.home-title"),
                        &cx.t("browser.view.home-body"),
                    ))
                } else {
                    None
                };
                self.state.open(&home, html, true);
            }
        }
    }

    fn handle_ipc(&mut self, cx: &mut Ctx<'_>, command: &str) {
        let active_is_home = self
            .state
            .active()
            .map(|tab| tab.kind == Kind::Url && crate::home::is_home_url(&tab.url))
            .unwrap_or(false);
        match command {
            "address" => self.state.set_address_focus(true),
            "reload" | "hardReload" if active_is_home => self.home.force_refresh(cx),
            "reload" => self.state.request(Pending::Reload),
            "hardReload" => self.state.request(Pending::HardReload),
            "find" => self.state.set_find(true, None),
            "newPage" => self.new_page(cx),
            "reopenPage" => self.state.reopen(),
            "closePage" => {
                let index = self.state.with(|state| state.active).unwrap_or(0);
                self.state.close(index);
            }
            "zoomIn" => self.zoom(1),
            "zoomOut" => self.zoom(-1),
            "zoomReset" => self.zoom(0),
            "back" => self.state.request(Pending::Back),
            "forward" => self.state.request(Pending::Forward),
            "devtools" => {
                if let Some((_, native)) = self.native.first() {
                    native.open_devtools();
                }
            }
            other => {
                if let Some(url) = other.strip_prefix("open:") {
                    self.state.open(url, None, true);
                }
            }
        }
    }

    fn zoom(&mut self, delta: i32) {
        self.state.with(|state| {
            if let Some(tab) = state.tabs.get_mut(state.active) {
                tab.zoom = if delta == 0 {
                    0
                } else {
                    (tab.zoom + delta).clamp(-6, 9)
                };
            }
        });
        self.state.request(Pending::Zoom);
    }

    fn new_page(&mut self, cx: &mut Ctx<'_>) {
        let home = home_url(cx);
        let html = if home.starts_with("forja://") && !crate::home::is_home_url(&home) {
            Some(home_html(
                &cx.t("browser.view.home-title"),
                &cx.t("browser.view.home-body"),
            ))
        } else {
            None
        };
        self.state.open(&home, html, true);
    }

    fn native_for(&mut self, cx: &mut Ctx<'_>, tab: &Tab, bounds: wry::Rect) -> bool {
        if self.native.iter().any(|(id, _)| *id == tab.id) {
            return true;
        }
        let parent = match self.parent {
            Some(parent) => parent,
            None => match Parent::find() {
                Some(parent) => {
                    self.parent = Some(parent);
                    parent
                }
                None => {
                    self.state.update_tab(tab.id, |tab| {
                        tab.error = Some(cx.t("browser.view.no-window"))
                    });
                    return false;
                }
            },
        };
        let network = cx.run_command("network.proxy", Args::new()).ok();
        if tab.kind != Kind::Html
            && !tab.url.starts_with("forja://")
            && network
                .as_ref()
                .and_then(|value| value.get("offline"))
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        {
            let message = "network offline mode is active".to_owned();
            self.state
                .update_tab(tab.id, |tab| tab.error = Some(message.clone()));
            self.state.set_error(Some(message));
            return false;
        }
        let scope = crate::profile::Scope::parse(
            &std::env::var("FORJA_BROWSER_PROFILE_SCOPE").unwrap_or_default(),
        );
        let workspace = std::env::var("FORJA_BROWSER_WORKSPACE")
            .ok()
            .map(PathBuf::from);
        let config = Config {
            profile_dir: crate::profile::directory(
                scope,
                &self.profile_root,
                None,
                workspace.as_deref(),
            ),
            download_dir: download_dir(cx),
            devtools: cx
                .setting("browser.devtools.enabled")
                .and_then(SettingValue::as_bool)
                .unwrap_or(false),
            zoom: zoom_factor(tab.zoom) * self.default_zoom * self.scale as f64,
            ask_each_download: cx
                .setting("browser.download.ask_each")
                .and_then(SettingValue::as_bool)
                .unwrap_or(true),
            open_auth_external: cx
                .setting("browser.auth.open_external")
                .and_then(SettingValue::as_bool)
                .unwrap_or(false),
            do_not_track: cx
                .setting("browser.privacy.do_not_track")
                .and_then(SettingValue::as_bool)
                .unwrap_or(true),
            proxy: proxy_config(network.as_ref(), &tab.url),
        };
        let html = if tab.kind == Kind::Html {
            tab.html.clone().filter(|html| !html.is_empty())
        } else if tab.url.starts_with("forja://") {
            Some(home_html(
                &cx.t("browser.view.home-title"),
                &cx.t("browser.view.home-body"),
            ))
        } else {
            None
        };
        let target = match &html {
            Some(html) => Target::Html(html),
            None => Target::Url(&tab.url),
        };
        match NativeTab::create(&parent, &config, target, bounds) {
            Ok(native) => {
                self.native.push((tab.id, native));
                true
            }
            Err(error) => {
                self.state
                    .update_tab(tab.id, |tab| tab.error = Some(error.clone()));
                self.state.set_error(Some(error));
                false
            }
        }
    }

    fn drain_events(&mut self, cx: &mut Ctx<'_>, id: u64) {
        let events = self
            .native
            .iter()
            .find(|(native_id, _)| *native_id == id)
            .map(|(_, native)| native.events());
        let Some(events) = events else {
            return;
        };
        let (loading, title, finished, downloads, commands, error) = match events.lock() {
            Ok(mut events) => (
                events.loading.take(),
                events.title.clone(),
                std::mem::take(&mut events.finished),
                std::mem::take(&mut events.downloads),
                std::mem::take(&mut events.commands),
                events.error.take(),
            ),
            Err(_) => return,
        };
        if let Some(loading) = loading {
            self.state.update_tab(id, |tab| tab.loading = loading);
        }
        if let Some(title) = title {
            self.state.update_tab(id, |tab| tab.title = title);
        }
        for (url, title) in finished {
            self.state.update_tab(id, |tab| {
                tab.url = url.clone();
                if !title.is_empty() {
                    tab.title = title.clone();
                }
                tab.loading = false;
                tab.error = None;
            });
            self.state
                .with(|state| state.history.update_title(&url, &title));
            self.state.add_history(&url, &title);
            self.save_history(cx);
        }
        if !downloads.is_empty() {
            let count = downloads.len();
            self.state.add_downloads(downloads);
            self.state.set_notice(Some(
                cx.t_args("browser.view.downloaded", &[("count", &count.to_string())]),
            ));
        }
        if let Some(error) = error {
            self.state.set_error(Some(error));
        }
        for command in commands {
            self.handle_ipc(cx, &command);
        }
    }

    fn save_history(&mut self, cx: &mut Ctx<'_>) {
        let history = self.state.history();
        let path = forja_kernel::paths::default_data_dir().join("browser-history.json");
        cx.spawn(move || history.save(&path), |_cx, _| {});
    }

    fn persist_tabs(&mut self, cx: &mut Ctx<'_>) {
        let urls: Vec<String> = self
            .state
            .tabs()
            .into_iter()
            .filter(|tab| tab.kind == Kind::Url && !tab.url.starts_with("forja:"))
            .map(|tab| tab.url)
            .collect();
        if urls == self.last_tabs {
            return;
        }
        self.last_tabs = urls.clone();
        let path = tabs_path();
        cx.spawn(move || save_tabs(&urls, &path), |_cx, _| {});
    }

    fn apply_pending(&mut self, cx: &mut Ctx<'_>, tab: &Tab) {
        let Some(pending) = self.state.take_pending() else {
            return;
        };
        let Some((_, native)) = self.native.iter().find(|(id, _)| *id == tab.id) else {
            return;
        };
        match pending {
            Pending::Navigate { url, .. } => native.navigate(&url),
            Pending::Back => native.back(),
            Pending::Forward => native.forward(),
            Pending::Reload => native.reload(),
            Pending::HardReload => native.hard_reload(),
            Pending::Find { query, backwards } => native.find(&query, backwards),
            Pending::Zoom => {
                native.zoom(zoom_factor(tab.zoom) * self.default_zoom * self.scale as f64)
            }
        }
        let _ = cx;
    }
}

impl View for BrowserView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.bootstrap(cx);
        self.scale = ui.ctx().pixels_per_point();
        self.default_zoom = cx
            .setting("browser.zoom.default")
            .and_then(SettingValue::as_f64)
            .unwrap_or(1.0)
            .clamp(0.5, 2.0);
        let tokens = forja_ui_kit::applied_tokens(ui.ctx());
        let muted = c32(tokens.color.text_muted);
        self.toolbar(ui, cx, muted);
        let tabs = self.state.tabs();
        if tabs.is_empty() {
            ui.label(egui::RichText::new(cx.t("browser.view.empty")).color(muted));
            return;
        }
        if let Some(error) = tabs
            .get(self.state.with(|state| state.active).unwrap_or(0))
            .and_then(|tab| tab.error.clone())
        {
            ui.label(
                egui::RichText::new(error)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
        }
        self.tab_strip(ui, cx, &tabs, muted);
        if self.state.find_open() {
            self.find_bar(ui, cx, muted);
        }
        if let Some(error) = self.state.with(|state| state.error.clone()).flatten() {
            ui.label(
                egui::RichText::new(error)
                    .small()
                    .color(c32(tokens.color.danger)),
            );
        }
        let history_open = self.state.history_open();
        let overlay_open = ui.ctx().memory(|memory| {
            memory
                .areas()
                .visible_layer_ids()
                .iter()
                .any(|id| !matches!(id.order, egui::Order::Background))
        });
        let popup_open = egui::Popup::is_any_open(ui.ctx());
        let focused = ui.input(|input| input.viewport().focused.unwrap_or(true));
        let minimized = ui.input(|input| input.viewport().minimized.unwrap_or(false));
        let show = std::env::var("FORJA_BROWSER_FORCE_VISIBLE").as_deref() == Ok("1")
            || visibility::visible(visibility::Input {
                tab_active: true,
                window_focused: focused,
                minimized,
                overlay_open,
                popup_open,
                history_open,
            });
        let rect = ui.available_rect_before_wrap();
        let _ = ui.allocate_rect(rect, egui::Sense::hover());
        let bounds = logical_rect(rect.min.x, rect.min.y, rect.width(), rect.height());
        let active = self.state.active();
        let active_is_home = active
            .as_ref()
            .map(|tab| tab.kind == Kind::Url && crate::home::is_home_url(&tab.url))
            .unwrap_or(false);
        if let Some(tab) = active {
            if self.last_active != Some(tab.id) || self.last_url != tab.url {
                self.address = tab.url.clone();
                self.last_active = Some(tab.id);
                self.last_url = tab.url.clone();
            }
            self.drain_events(cx, tab.id);
            // Criar um WebView2 é trabalho de processo e COM que pode levar
            // segundos. Só cria quando a página nativa estaria visível e não for a
            // página inicial nativa (forja://inicio); uma aba escondida não pode
            // congelar o quadro. Refs #73, #112.
            let has_native = self.native.iter().any(|(id, _)| *id == tab.id);
            if !active_is_home && !has_native && show {
                let _ = self.native_for(cx, &tab, bounds);
            }
            if !active_is_home && self.native.iter().any(|(id, _)| *id == tab.id) {
                self.apply_pending(cx, &tab);
            }
        }
        let active_summary = self.state.active().map(|tab| (tab.title, tab.url));
        let active_id = self.state.active().map(|tab| tab.id);
        self.persist_tabs(cx);
        self.native
            .retain(|(id, _)| tabs.iter().any(|tab| tab.id == *id));
        for (id, native) in &self.native {
            native.set_visible(!active_is_home && show && Some(*id) == active_id);
            if !active_is_home && Some(*id) == active_id {
                native.set_bounds(bounds);
            }
        }
        if active_is_home && rect.width() > 40.0 && rect.height() > 40.0 {
            self.home.ui(ui, cx, &self.state, rect);
        } else if !show && rect.width() > 40.0 && rect.height() > 40.0 {
            let reason = if !focused || minimized {
                cx.t("browser.view.unfocused")
            } else {
                cx.t("browser.view.hidden")
            };
            let notice_rect = rect.shrink(24.0);
            let max_notice_width = (notice_rect.width() * 0.85).clamp(220.0, 560.0);
            let left_pad = ((notice_rect.width() - max_notice_width) * 0.5).max(0.0);
            let box_rect = egui::Rect::from_min_size(
                egui::pos2(notice_rect.left() + left_pad, notice_rect.top()),
                egui::vec2(max_notice_width, notice_rect.height()),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(box_rect), |ui| {
                ui.add_space((notice_rect.height() * 0.25).max(8.0));
                if let Some((title, url)) = &active_summary {
                    if !title.is_empty() {
                        ui.add(egui::Label::new(egui::RichText::new(title).strong()).wrap());
                    }
                    if !url.is_empty() {
                        ui.add(
                            egui::Label::new(egui::RichText::new(url).small().color(muted)).wrap(),
                        );
                    }
                    ui.add_space(6.0);
                }
                ui.add(egui::Label::new(egui::RichText::new(reason).color(muted)).wrap());
            });
        }
        self.history_window(ui, cx, muted);
        if let Some(notice) = self.state.with(|state| state.notice.clone()).flatten() {
            ui.label(egui::RichText::new(notice).small().color(muted));
        }
    }
}

impl BrowserView {
    fn toolbar(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        ui.horizontal_wrapped(|ui| {
            let active = self.state.active();
            if let Some(tab) = &active {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, "<").clicked() {
                    self.state.request(Pending::Back);
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, ">").clicked() {
                    self.state.request(Pending::Forward);
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, "R").clicked() {
                    if tab.kind == Kind::Url && crate::home::is_home_url(&tab.url) {
                        self.home.force_refresh(cx);
                    } else {
                        self.state.request(Pending::Reload);
                    }
                }
                let _ = tab;
            }
            let mut address = self.address.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut address)
                    .hint_text(cx.t("browser.view.address"))
                    .desired_width(420.0),
            );
            if self.state.take_address_focus() {
                response.request_focus();
            }
            if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                self.navigate(cx, &address);
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Secondary,
                &cx.t("browser.view.go"),
            )
            .clicked()
            {
                self.navigate(cx, &address);
            }
            self.address = address;
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("browser.view.history"),
            )
            .clicked()
            {
                self.state.toggle_history();
            }
            if forja_ui_kit::button(
                ui,
                forja_ui_kit::ButtonKind::Ghost,
                &cx.t("browser.view.new-tab"),
            )
            .clicked()
            {
                self.new_page(cx);
            }
            let zoom = self
                .state
                .active()
                .map(|tab| zoom_factor(tab.zoom) * self.default_zoom)
                .unwrap_or(self.default_zoom);
            ui.label(
                egui::RichText::new(format!("{:.0}%", zoom * 100.0))
                    .small()
                    .color(muted),
            );
        });
    }

    fn navigate(&mut self, cx: &mut Ctx<'_>, input: &str) {
        let mut args = Args::new();
        args.insert("url", json!(input));
        let _ = cx.run_command("browser.navigate", args);
    }

    fn tab_strip(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        tabs: &[Tab],
        muted: egui::Color32,
    ) {
        let active = self.state.with(|state| state.active).unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            for (index, tab) in tabs.iter().enumerate() {
                let label = if tab.loading {
                    format!("{} …", tab.title)
                } else {
                    tab.title.clone()
                };
                if ui.selectable_label(index == active, label).clicked() {
                    self.state.set_active(index);
                }
                if ui
                    .small_button("x")
                    .on_hover_text(cx.t("browser.view.close-tab"))
                    .clicked()
                {
                    self.state.close(index);
                }
            }
            if tabs.len() > 1 {
                ui.label(
                    egui::RichText::new(format!("{} / {}", tabs.len(), crate::state::MAX_TABS))
                        .small()
                        .color(muted),
                );
            }
        });
    }

    fn find_bar(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        ui.horizontal(|ui| {
            let mut query = self.state.find_query();
            let response = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text(cx.t("browser.view.find"))
                    .desired_width(260.0),
            );
            let enter =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            if enter || response.changed() {
                self.state.set_find(true, Some(query.clone()));
                self.state.request(Pending::Find {
                    query: query.clone(),
                    backwards: false,
                });
            }
            if ui.small_button("x").clicked() {
                self.state.set_find(false, None);
            }
            ui.label(
                egui::RichText::new(cx.t("browser.view.find-hint"))
                    .small()
                    .color(muted),
            );
        });
    }

    fn history_window(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, muted: egui::Color32) {
        if !self.state.history_open() {
            return;
        }
        let mut open = true;
        let history = self.state.history();
        let title = cx.t("browser.history.title");
        let close_label = cx.t("browser.window.close");
        egui::Window::new(&title)
            .id(egui::Id::new("forja.browser.history"))
            .title_bar(false)
            .default_width(520.0)
            .show(ui.ctx(), |ui| {
                if forja_ui_kit::window_header(ui, &title, &close_label) {
                    open = false;
                }
                if history.is_empty() {
                    ui.label(egui::RichText::new(cx.t("browser.history.empty")).color(muted));
                }
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for entry in &history.entries {
                            let label = if entry.title.is_empty() {
                                entry.url.clone()
                            } else {
                                format!("{} — {}", entry.title, entry.url)
                            };
                            if ui.selectable_label(false, label).clicked() {
                                self.navigate(cx, &entry.url);
                                self.state.set_history_open(false);
                            }
                        }
                    });
                if ui.button(cx.t("browser.history.clear")).clicked() {
                    self.state.clear_history();
                    self.save_history(cx);
                }
            });
        if !open {
            self.state.set_history_open(false);
        }
    }
}

fn home_url(cx: &Ctx<'_>) -> String {
    cx.setting("browser.home.url")
        .and_then(SettingValue::as_str)
        .unwrap_or("forja://inicio")
        .to_owned()
}

fn tabs_path() -> PathBuf {
    forja_kernel::paths::default_data_dir().join("browser-tabs.json")
}

fn load_tabs() -> Vec<String> {
    std::fs::read_to_string(tabs_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save_tabs(urls: &[String], path: &Path) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(urls).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(path, &bytes).map_err(|error| error.to_string())
}

fn download_dir(cx: &Ctx<'_>) -> PathBuf {
    let raw = cx
        .setting("browser.download.dir")
        .and_then(SettingValue::as_str)
        .unwrap_or("%USERPROFILE%\\Downloads")
        .replace(
            "%USERPROFILE%",
            &std::env::var("USERPROFILE").unwrap_or_default(),
        );
    PathBuf::from(raw)
}

fn zoom_factor(level: i32) -> f64 {
    if level == 0 {
        1.0
    } else if level < 0 {
        0.25 + (level as f64 + 6.0) / 6.0 * 0.75
    } else {
        1.0 + level as f64 * 0.5
    }
}

fn proxy_config(policy: Option<&serde_json::Value>, target_url: &str) -> Option<wry::ProxyConfig> {
    let policy = policy?;
    if policy.get("mode").and_then(serde_json::Value::as_str) == Some("none") {
        return None;
    }
    let raw = policy
        .get("url")
        .and_then(serde_json::Value::as_str)?
        .trim();
    if raw.is_empty() {
        return None;
    }
    let target_host = target_url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(target_url)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if let Some(bypass) = policy.get("bypass").and_then(serde_json::Value::as_array) {
        let skipped = bypass
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|entry| {
                let entry = entry.trim().to_ascii_lowercase();
                entry == target_host
                    || entry
                        .strip_prefix("*.")
                        .is_some_and(|suffix| target_host.ends_with(&format!(".{suffix}")))
            });
        if skipped {
            return None;
        }
    }
    let (scheme, rest) = raw.split_once("://").unwrap_or(("http", raw));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, hp)| hp)
        .unwrap_or(authority);
    let (host, port) = host_port.rsplit_once(':')?;
    if host.trim().is_empty() || port.trim().is_empty() {
        return None;
    }
    let endpoint = wry::ProxyEndpoint {
        host: host.trim().to_owned(),
        port: port.trim().to_owned(),
    };
    if scheme.eq_ignore_ascii_case("socks5") {
        Some(wry::ProxyConfig::Socks5(endpoint))
    } else {
        Some(wry::ProxyConfig::Http(endpoint))
    }
}
