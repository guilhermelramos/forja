//! The Forja shell window.
//!
//! Layout regions are fixed by the product (`design/product.md`); the content
//! of every region comes from registered slots. The shell knows nothing about
//! specific product modules.

use crate::actions::{self, AnvilMode};
use crate::hosts;
use crate::rpc::RpcHost;
use crate::screenshot::{self, ScreenshotJob};
use crate::shell_module::ShellModule;
use crate::tabs::TabSet;
use crate::tray::{self, TrayAction};
use eframe::egui::{self, Key, Modifiers, ViewportCommand};
use forja_kernel::keymap::KeyChord;
use forja_kernel::settings::SettingValue;
use forja_kernel::store::LoadResult;
use forja_kernel::ui::Slot;
use forja_kernel::ui_contract::{self, View};
use forja_kernel::{Args, Module, Registry, Runtime};
use forja_ui_kit::Tokens;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(crate) struct ViewInstance {
    pub(crate) module: String,
    pub(crate) id: String,
    pub(crate) title_key: String,
    pub(crate) icon_glyph: Option<String>,
    pub(crate) icon_png: Option<Arc<[u8]>>,
    pub(crate) view: Box<dyn View>,
}

impl ViewInstance {
    fn collect(registry: &Registry, slot: Slot) -> Vec<Self> {
        registry
            .views_in(slot)
            .into_iter()
            .filter_map(|contribution| {
                let view = ui_contract::downcast(contribution.instantiate());
                if view.is_none() {
                    eprintln!(
                        "forja: view `{}` did not return a ui_contract::View",
                        contribution.id
                    );
                }
                view.map(|view| Self {
                    module: contribution.module.clone(),
                    id: contribution.id.clone(),
                    title_key: contribution.title_key.clone(),
                    icon_glyph: contribution.icon_glyph.clone(),
                    icon_png: contribution.icon_png.clone(),
                    view,
                })
            })
            .collect()
    }
}

pub struct Session {
    pub theme: Option<String>,
    pub density: Option<String>,
    pub open: Option<String>,
    pub anvil_query: Option<String>,
    /// `--rail <id>`: página do trilho que abre (captura de tela).
    pub rail: Option<String>,
    /// `--settings <módulo>`: só a seção desse módulo nos Ajustes (captura de tela).
    pub settings: Option<String>,
    /// `--modal <id>`: mostra essa camada modal mesmo fechada (captura de tela).
    pub modal: Option<String>,
    /// `--bench-tab <id>`: aba da bancada que abre (captura de tela).
    pub bench_tab: Option<String>,
    /// `--right-tab <id>`: aba do painel direito que abre (captura de tela).
    pub right_tab: Option<String>,
    /// `--test-driver`: answers `ui.*` RPC so a play-test agent can use the app.
    pub test_driver: bool,
    pub locale_locked: bool,
}

pub struct ForjaApp {
    pub(crate) runtime: Runtime,
    pub(crate) tokens: Tokens,
    pub(crate) rail_views: Vec<ViewInstance>,
    pub(crate) sidebar_views: Vec<ViewInstance>,
    pub(crate) bench_views: Vec<ViewInstance>,
    pub(crate) right_views: Vec<ViewInstance>,
    pub(crate) status_views: Vec<ViewInstance>,
    pub(crate) settings_views: Vec<ViewInstance>,
    pub(crate) modal_views: Vec<ViewInstance>,
    pub(crate) anvil_views: Vec<ViewInstance>,
    pub(crate) top_views: Vec<ViewInstance>,
    /// Rail view shown in the center. `None` keeps the bench tabs.
    pub(crate) rail_page: Option<String>,
    pub(crate) rail_icons: BTreeMap<String, egui::TextureHandle>,
    pub(crate) active_tab: usize,
    pub(crate) tabs: TabSet,
    /// Remembered sidebar contribution when more than one module fills the slot.
    pub(crate) sidebar_tab: usize,
    /// Remembered right-panel contribution when more than one module fills the slot.
    pub(crate) right_tab: usize,
    pub(crate) settings_open: bool,
    pub(crate) keymap_open: bool,
    /// Módulo cuja seção é a única mostrada nos Ajustes (`--settings`).
    pub(crate) settings_section: Option<String>,
    /// Camada modal forçada a aparecer para a captura (`--modal`).
    pub(crate) modal_capture: Option<String>,
    pub(crate) settings_query: String,
    pub(crate) setting_drafts: BTreeMap<String, String>,
    pub(crate) anvil_open: bool,
    pub(crate) anvil_query: String,
    pub(crate) anvil_query_seen: String,
    pub(crate) anvil_selected: usize,
    pub(crate) anvil_group: usize,
    pub(crate) anvil_history: Vec<String>,
    pub(crate) focus_anvil: bool,
    pub(crate) bench_cache: Vec<(String, String)>,
    pub(crate) bench_cached_at: Option<Instant>,
    pub(crate) file_cache: Vec<(String, String)>,
    pub(crate) file_cache_query: String,
    pub(crate) show_right_panel: bool,
    pub(crate) fullscreen: bool,
    pub(crate) status: Option<String>,
    pub(crate) slot_rects: BTreeMap<String, egui::Rect>,
    pub(crate) theme_override: Option<String>,
    pub(crate) interface_theme: Option<String>,
    pub(crate) theme_checked: Instant,
    pub(crate) density_override: Option<String>,
    pub(crate) locale_locked: bool,
    pub(crate) system_dark: bool,
    pub(crate) system_probed: Instant,
    pub(crate) fonts_ready: bool,
    pub(crate) logo: Option<egui::TextureHandle>,
    pub(crate) quitting: bool,
    pub(crate) pending_toast: bool,
    pub(crate) pending_onboarding: bool,
    screenshot: Option<ScreenshotJob>,
    screenshot_error: Arc<Mutex<Option<String>>>,
    rpc: Option<RpcHost>,
    driver: Option<crate::test_driver::Driver>,
    pub(crate) module_count: usize,
    geometry_writer: crate::window_geometry::GeometryWriter,
    geometry_checked: Instant,
    geometry_close_pending: bool,
    geometry_close_allowed: bool,
}

impl ForjaApp {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        runtime: Runtime,
        screenshot: Option<ScreenshotJob>,
        screenshot_error: Arc<Mutex<Option<String>>>,
        session: Session,
    ) -> Self {
        let _ = forja_ui_kit::install_fonts(&creation.egui_ctx);
        if let Some(job) = screenshot.as_ref().filter(|job| job.ui_tree.is_some()) {
            creation.egui_ctx.enable_accesskit();
            creation.egui_ctx.add_plugin(crate::ui_tree::UiTreeCapture {
                latest: Arc::clone(&job.tree),
            });
        }
        let rail_views = ViewInstance::collect(runtime.registry(), Slot::Rail);
        let sidebar_views = ViewInstance::collect(runtime.registry(), Slot::Sidebar);
        let bench_views = ViewInstance::collect(runtime.registry(), Slot::BenchTab);
        let right_views = ViewInstance::collect(runtime.registry(), Slot::RightPanel);
        let status_views = ViewInstance::collect(runtime.registry(), Slot::StatusBar);
        let settings_views = ViewInstance::collect(runtime.registry(), Slot::SettingsSection);
        let modal_views = ViewInstance::collect(runtime.registry(), Slot::Modal);
        let anvil_views = ViewInstance::collect(runtime.registry(), Slot::AnvilSource);
        let top_views = ViewInstance::collect(runtime.registry(), Slot::TopBar);
        let module_count = runtime.registry().modules().len();
        for warning in runtime.warnings() {
            eprintln!("forja: {warning}");
        }
        let anvil_history = match runtime.store().load::<Value>("shell") {
            LoadResult::Loaded(value) => crate::anvil::load_history(&value),
            _ => Vec::new(),
        };
        let right_tab = session
            .right_tab
            .as_deref()
            .and_then(|id| right_views.iter().position(|view| view.id == id))
            .or_else(|| {
                std::env::var("FORJA_RIGHT_TAB")
                    .ok()
                    .and_then(|name| right_views.iter().position(|view| view.id.contains(&name)))
            })
            .unwrap_or(0);
        let active_tab = session
            .bench_tab
            .as_deref()
            .and_then(|id| bench_views.iter().position(|view| view.id == id))
            .unwrap_or(0);
        let mut app = Self {
            runtime,
            tokens: Tokens::brasa_dark(),
            rail_views,
            sidebar_views,
            bench_views,
            right_views,
            status_views,
            settings_views,
            modal_views,
            anvil_views,
            top_views,
            rail_page: None,
            rail_icons: BTreeMap::new(),
            active_tab,
            tabs: TabSet::default(),
            sidebar_tab: 0,
            right_tab,
            settings_open: false,
            keymap_open: false,
            settings_section: None,
            modal_capture: session.modal.clone(),
            settings_query: String::new(),
            setting_drafts: BTreeMap::new(),
            anvil_open: false,
            anvil_query: String::new(),
            anvil_query_seen: String::new(),
            anvil_selected: 0,
            anvil_group: 0,
            anvil_history,
            focus_anvil: false,
            bench_cache: Vec::new(),
            bench_cached_at: None,
            file_cache: Vec::new(),
            file_cache_query: String::new(),
            show_right_panel: true,
            fullscreen: false,
            status: None,
            slot_rects: BTreeMap::new(),
            theme_override: session.theme,
            interface_theme: None,
            theme_checked: Instant::now() - std::time::Duration::from_secs(2),
            density_override: session.density,
            locale_locked: session.locale_locked,
            system_dark: true,
            system_probed: Instant::now() - std::time::Duration::from_secs(2),
            fonts_ready: true,
            logo: None,
            quitting: false,
            pending_toast: session.open.as_deref() == Some("toast"),
            pending_onboarding: session.open.as_deref() == Some("onboarding"),
            screenshot,
            screenshot_error,
            rpc: None,
            driver: None,
            module_count,
            geometry_writer: crate::window_geometry::GeometryWriter::default(),
            geometry_checked: Instant::now(),
            geometry_close_pending: false,
            geometry_close_allowed: false,
        };
        if session.test_driver {
            let driver = crate::test_driver::Driver::new();
            creation.egui_ctx.enable_accesskit();
            creation.egui_ctx.add_plugin(crate::ui_tree::UiTreeCapture {
                latest: Arc::clone(&driver.tree),
            });
            app.driver = Some(driver);
        }
        if app.screenshot.is_none() {
            let context = creation.egui_ctx.clone();
            let wake: forja_kernel::tasks::Wake = Arc::new(move || context.request_repaint());
            app.runtime.set_wake(Arc::clone(&wake));
            match RpcHost::start(wake) {
                Ok(host) => app.rpc = Some(host),
                Err(error) => app.status = Some(format!("RPC: {error}")),
            }
        }
        if let Some(slot) = app.screenshot.as_ref().and_then(|job| job.slot.as_deref()) {
            match slot {
                "settings" => app.settings_open = true,
                "anvil" => app.open_anvil(AnvilMode::All),
                _ => {}
            }
        }
        if session.open.as_deref() == Some("anvil") {
            app.open_anvil(AnvilMode::All);
        }
        if session.open.as_deref() == Some("rail") {
            app.rail_page = app.rail_views.first().map(|view| view.id.clone());
        }
        if let Some(view_id) = session.rail.clone() {
            if app.rail_views.iter().any(|view| view.id == view_id) {
                app.rail_page = Some(view_id);
            } else {
                eprintln!("forja-app: --rail {view_id}: no rail page has that id");
            }
        }
        if let Some(view_id) = session.modal.clone() {
            if !app.modal_views.iter().any(|view| view.id == view_id) {
                eprintln!("forja-app: --modal {view_id}: no modal view has that id");
            }
        }
        if let Some(module) = session.settings.clone() {
            app.settings_open = true;
            app.settings_section = Some(module);
        }
        if let Some(query) = session.anvil_query {
            app.anvil_open = true;
            app.anvil_query = query;
            app.focus_anvil = true;
        }
        app.refresh_look(&creation.egui_ctx);
        app
    }

    pub(crate) fn text(&self, key: &str) -> String {
        self.runtime.i18n().t(key)
    }

    pub(crate) fn text_args(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.runtime.i18n().t_args(key, args)
    }

    pub(crate) fn open_anvil(&mut self, mode: AnvilMode) {
        let already = self.anvil_open;
        self.anvil_open = true;
        self.focus_anvil = true;
        self.anvil_group = match mode {
            AnvilMode::All => 0,
            AnvilMode::Commands => 1,
            AnvilMode::Files => 3,
        };
        if !already {
            self.anvil_query.clear();
            self.anvil_query_seen.clear();
            self.anvil_selected = 0;
        }
    }

    fn drain_rpc(&mut self, ctx: &egui::Context) {
        loop {
            let call = self.rpc.as_ref().and_then(|rpc| rpc.try_recv());
            let Some(call) = call else { break };
            let reply_to = match self.driver.as_mut() {
                Some(driver) => match driver.accept(&call.line, call.reply, ctx) {
                    Some(reply_to) => reply_to,
                    None => continue,
                },
                None => call.reply,
            };
            let reply = self.runtime.handle_rpc_line(&call.line);
            let _ = reply_to.send(reply);
        }
    }

    pub(crate) fn run_command(&mut self, id: &str) {
        match self.runtime.run_command(id, Args::new()) {
            Ok(_) => self.status = None,
            Err(error) => {
                let message = error.to_string();
                let text =
                    self.text_args("shell.command.failed", &[("id", id), ("message", &message)]);
                self.status = Some(text);
            }
        }
    }

    fn drain_actions(&mut self, ctx: &egui::Context) {
        for action in actions::drain() {
            match action {
                actions::Action::OpenAnvil(mode) => self.open_anvil(mode),
                actions::Action::OpenSettings => {
                    self.settings_open = true;
                    self.settings_query.clear();
                    self.settings_section = None;
                }
                actions::Action::OpenSettingsSection(section) => {
                    self.settings_open = true;
                    self.settings_query.clear();
                    self.settings_section = Some(section);
                }
                actions::Action::ShowKeymap => self.keymap_open = true,
                actions::Action::OpenRail(view_id) => {
                    if self.rail_views.iter().any(|view| view.id == view_id) {
                        self.rail_page = Some(view_id);
                    } else {
                        self.status = Some(format!("rail page `{view_id}` not found"));
                    }
                }
                actions::Action::ToggleSidebar => {
                    let visible = self.setting_bool("appearance.sidebar.visible", true);
                    let _ = self
                        .runtime
                        .set_setting("appearance.sidebar.visible", SettingValue::Bool(!visible));
                }
                actions::Action::TogglePanel => self.show_right_panel = !self.show_right_panel,
                actions::Action::ZoomIn => {
                    let scale = self.setting_number("appearance.ui.scale", 1.0);
                    let next = actions::adjust_zoom_scale(scale, 0.1);
                    let _ = self
                        .runtime
                        .set_setting("appearance.ui.scale", SettingValue::Number(next));
                }
                actions::Action::ZoomOut => {
                    let scale = self.setting_number("appearance.ui.scale", 1.0);
                    let next = actions::adjust_zoom_scale(scale, -0.1);
                    let _ = self
                        .runtime
                        .set_setting("appearance.ui.scale", SettingValue::Number(next));
                }
                actions::Action::ZoomReset => {
                    let _ = self
                        .runtime
                        .set_setting("appearance.ui.scale", SettingValue::Number(1.0));
                }
                actions::Action::ToggleFullscreen => {
                    self.fullscreen = !self.fullscreen;
                    ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
                }
                actions::Action::NextTab => {
                    if let Some(index) = self.tabs.next(self.active_tab, self.bench_views.len()) {
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::PreviousTab => {
                    if let Some(index) = self.tabs.previous(self.active_tab, self.bench_views.len()) {
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::CloseTab => {
                    if let Some(index) = self.tabs.close(self.active_tab, self.bench_views.len()) {
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::ReopenTab => {
                    if let Some(index) = self.tabs.reopen(self.bench_views.len()) {
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::SwitchTab(number) => {
                    if let Some(index) = self.tabs.switch(number, self.bench_views.len()) {
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
            }
        }
    }

    fn show_demos(&mut self) {
        if self.pending_onboarding {
            self.pending_onboarding = false;
            let _ = self.runtime.run_command("onboarding.open", Args::new());
        }
        if self.pending_toast {
            self.pending_toast = false;
            let title = self.text("shell.toast.title");
            let body = self.text("shell.toast.body");
            let mut args = Args::new();
            args.insert("title", Value::String(title));
            args.insert("body", Value::String(body));
            args.insert("level", Value::String("success".into()));
            let _ = self.runtime.run_command("notify.show", args);
        }
    }

    fn sync_tray(&mut self, ctx: &egui::Context) {
        let enabled = self.setting_bool("app.tray.enabled", false) && self.screenshot.is_none();
        if !enabled {
            tray::remove();
            return;
        }
        match tray::poll() {
            Some(TrayAction::Show) => {
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            Some(TrayAction::Quit) => {
                self.quitting = true;
                tray::remove();
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
            None => {}
        }
        let closing = ctx.input(|input| input.viewport().close_requested());
        if closing && !self.quitting {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            let open = self.text("shell.tray.show");
            let quit = self.text("shell.tray.quit");
            let tip = self.text("shell.tray.tip");
            tray::ensure(&open, &quit, &tip);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let text_busy = ctx.wants_keyboard_input();
        let bindings: Vec<(String, KeyChord)> = self
            .runtime
            .keymap()
            .bindings()
            .iter()
            .map(|resolved| {
                (
                    resolved.binding.command.as_str().to_owned(),
                    resolved.binding.chord.clone(),
                )
            })
            .collect();
        let mut fired = Vec::new();
        let browser_surface = self
            .bench_views
            .get(self.active_tab)
            .is_some_and(|view| view.module == "browser")
            && self.rail_page.is_none();
        for (id, chord) in bindings {
            let always = matches!(
                id.as_str(),
                "anvil.open"
                    | "anvil.command_mode"
                    | "find.file"
                    | "settings.open"
                    | "settings.keymap"
                    | "help.keymap"
                    | "panel.toggle"
                    | "view.zoom.in"
                    | "view.zoom.out"
                    | "view.zoom.reset"
                    | "window.fullscreen"
                    | "tab.next"
                    | "tab.previous"
                    | "tab.close_force"
                    | "tab.reopen"
                    | "tab.switch_1"
                    | "tab.switch_2"
                    | "tab.switch_3"
                    | "tab.switch_4"
                    | "tab.switch_5"
                    | "tab.switch_6"
                    | "tab.switch_7"
                    | "tab.switch_8"
                    | "tab.switch_9"
                    | "tab.switch_10"
            );
            if browser_surface
                && matches!(
                    id.as_str(),
                    "tab.close" | "tab.reopen" | "tab.switch_10"
                )
            {
                continue;
            }
            if id == "sidebar.toggle" && (text_busy || self.anvil_open) {
                continue;
            }
            if !always && (text_busy || self.anvil_open || self.settings_open) {
                continue;
            }
            if chord_pressed(ctx, &chord) {
                fired.push(id);
            }
        }
        for id in fired {
            self.run_command(&id);
        }
    }

    fn capture(&mut self, ctx: &egui::Context) {
        let Some(mut job) = self.screenshot.take() else {
            return;
        };
        if job.done {
            return;
        }
        job.frames += 1;
        if !job.requested {
            if job.frames >= 4 {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
                job.requested = true;
            }
            self.screenshot = Some(job);
            ctx.request_repaint();
            return;
        }
        let image = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            let result = screenshot::save(
                &image,
                &job.path,
                job.slot.as_deref(),
                &self.slot_rects,
                ctx.pixels_per_point(),
            );
            if let Err(error) = result {
                if let Ok(mut slot) = self.screenshot_error.lock() {
                    *slot = Some(error);
                }
            }
            if let Some(path) = job.ui_tree.clone() {
                match crate::ui_tree::write(&path, &job.tree, ctx.pixels_per_point()) {
                    Ok(()) => eprintln!("forja: ui tree saved to {}", path.display()),
                    Err(error) => {
                        if let Ok(mut slot) = self.screenshot_error.lock() {
                            *slot = Some(format!("ui tree: {error}"));
                        }
                    }
                }
            }
            job.done = true;
            eprintln!("forja: screenshot saved to {}", job.path.display());
            self.quitting = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            self.screenshot = Some(job);
            ctx.request_repaint();
        }
    }
}

impl eframe::App for ForjaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let closing = ctx.input(|input| input.viewport().close_requested());
        if self.screenshot.is_none() && self.setting_bool("appearance.window.remember_geometry", true)
            && (closing || self.geometry_checked.elapsed() >= std::time::Duration::from_secs(1))
        {
            self.geometry_checked = Instant::now();
            if let Some(geometry) = ctx.input(|input| crate::window_geometry::Geometry::from_viewport(input.viewport())) {
                self.geometry_writer.record(&mut self.runtime, geometry);
            }
        }
        let hiding_in_tray = self.setting_bool("app.tray.enabled", false) && !self.quitting;
        if closing && !self.geometry_close_allowed && !hiding_in_tray
            && self.screenshot.is_none() && self.setting_bool("appearance.window.remember_geometry", true)
        {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.geometry_close_pending = true;
        }
        if self.geometry_close_pending {
            if self.geometry_writer.busy() {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            } else {
                self.geometry_close_pending = false;
                self.geometry_close_allowed = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
        self.sync_tray(ctx);
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        self.runtime.set_viewport_focused(focused);
        self.drain_rpc(ctx);
        self.runtime.poll_tasks();
        self.handle_shortcuts(ctx);
        self.drain_actions(ctx);
        self.show_demos();
        self.refresh_look(ctx);
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.left_panels(ctx);
        self.right_panel(ctx);
        self.central_area(ctx);
        self.settings_window(ctx);
        self.keymap_window(ctx);
        self.anvil_window(ctx);
        self.modal_windows(ctx);
        self.capture(ctx);
        if let Some(driver) = self.driver.as_mut() {
            driver.tick(ctx);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(driver) = self.driver.as_mut() {
            driver.feed(raw_input);
        }
    }
}

fn chord_pressed(ctx: &egui::Context, chord: &KeyChord) -> bool {
    let Some(key) = egui_key(&chord.key) else {
        return false;
    };
    let modifiers = Modifiers {
        alt: chord.alt,
        ctrl: chord.ctrl,
        shift: chord.shift,
        mac_cmd: chord.meta,
        command: chord.ctrl || chord.meta,
    };
    ctx.input_mut(|input| input.consume_key(modifiers, key))
}

fn egui_key(name: &str) -> Option<Key> {
    match name {
        "BracketLeft" => Some(Key::OpenBracket),
        "BracketRight" => Some(Key::CloseBracket),
        "Plus" => Some(Key::Plus),
        "Equal" => Some(Key::Equals),
        "0" => Some(Key::Num0),
        other => Key::from_name(other),
    }
}

/// Builds the runtime used by both the window and the headless acceptance run.
pub fn build_runtime(
    locale: Option<String>,
    data_dir: Option<PathBuf>,
    user_dir: Option<PathBuf>,
    include_modules: bool,
) -> forja_kernel::Result<Runtime> {
    let mut modules: Vec<Box<dyn Module>> = vec![Box::new(ShellModule)];
    let mut seen = std::collections::BTreeSet::new();
    if include_modules {
        let linked = forja_kernel::discovered_modules()
            .into_iter()
            .chain(crate::linked_modules::linked());
        for module in linked {
            let id = module.manifest().id;
            if seen.insert(id) {
                modules.push(module);
            }
        }
    }
    // Shell-owned placeholder hosts (e.g. a bare `find` that only binds Ctrl+P) step aside
    // when a real module with the same id is linked.
    for host in hosts::hosts() {
        if seen.insert(host.manifest().id) {
            modules.push(host);
        }
    }
    Runtime::new(
        modules,
        forja_kernel::RuntimeOptions {
            data_dir,
            user_dir,
            locale,
            wake: None,
        },
    )
}
