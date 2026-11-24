//! The Forja shell window.
//!
//! Layout regions are fixed by the product (`design/product.md`); the content
//! of every region comes from registered slots. The shell knows nothing about
//! specific product modules.

#[path = "close_confirm.rs"]
pub(crate) mod close_confirm;
#[path = "shell_store.rs"]
pub(crate) mod shell_store;
#[path = "tab_bar.rs"]
pub(crate) mod tab_bar;

use crate::actions::{self, AnvilMode};
use crate::hosts;
use crate::rpc::RpcHost;
use crate::screenshot::{self, ScreenshotJob};
use crate::shell_module::ShellModule;
use crate::tabs::TabSet;
use crate::tray;
use eframe::egui::{self, Key, Modifiers, ViewportCommand};
use forja_kernel::keymap::{KeyChord, Scope};
use forja_kernel::settings::SettingValue;
use forja_kernel::store::LoadResult;
use forja_kernel::ui::Slot;
use forja_kernel::ui_contract::{self, View};
use forja_kernel::{Args, Module, Registry, Runtime};
use forja_ui_kit::Tokens;
use serde_json::{json, Value};
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
    pub(crate) failure: Option<String>,
}

impl ViewInstance {
    fn collect(registry: &Registry, slot: Slot) -> Vec<Self> {
        registry
            .views_in(slot)
            .into_iter()
            .filter_map(|contribution| {
                let view = ui_contract::downcast(contribution.instantiate());
                if view.is_none() {
                    forja_kernel::logging::log(
                        forja_kernel::logging::LogLevel::Warn,
                        format!(
                            "forja: view `{}` did not return a ui_contract::View",
                            contribution.id
                        ),
                    );
                }
                view.map(|view| Self {
                    module: contribution.module.clone(),
                    id: contribution.id.clone(),
                    title_key: contribution.title_key.clone(),
                    icon_glyph: contribution.icon_glyph.clone(),
                    icon_png: contribution.icon_png.clone(),
                    view,
                    failure: None,
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
    /// Background windows re-apply this logical size after creation.
    pub test_window_size: Option<[f32; 2]>,
    pub locale_locked: bool,
}

pub struct ForjaApp {
    pub(crate) runtime: Runtime,
    pub(crate) tokens: Tokens,
    pub(crate) rail_views: Vec<ViewInstance>,
    pub(crate) sidebar_views: Vec<ViewInstance>,
    pub(crate) bench_views: Vec<ViewInstance>,
    pub(crate) home_view: Option<ViewInstance>,
    pub(crate) project_was_open: bool,
    pub(crate) right_views: Vec<ViewInstance>,
    pub(crate) status_views: Vec<ViewInstance>,
    pub(crate) settings_views: Vec<ViewInstance>,
    pub(crate) modal_views: Vec<ViewInstance>,
    pub(crate) anvil_views: Vec<ViewInstance>,
    pub(crate) top_views: Vec<ViewInstance>,
    /// Rail view shown in the center. `None` keeps the bench tabs.
    pub(crate) rail_page: Option<String>,
    /// Project and bench names of the last activation, shown in the top bar.
    pub(crate) active_bench: Option<(String, String)>,
    pub(crate) rail_icons: BTreeMap<String, egui::TextureHandle>,
    pub(crate) active_tab: usize,
    pub(crate) tabs: TabSet,
    pub(crate) known_tab_content: BTreeMap<String, bool>,
    /// Remembered sidebar contribution when more than one module fills the slot.
    pub(crate) sidebar_tab: usize,
    /// Remembered right-panel contribution when more than one module fills the slot.
    pub(crate) right_tab: usize,
    pub(crate) settings_open: bool,
    pub(crate) settings_page: crate::settings_page::State,
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
    pub(crate) anvil_selection_explicit: bool,
    pub(crate) anvil_selection_id: Option<String>,
    pub(crate) anvil_selection_missing: bool,
    pub(crate) anvil_rail_select: bool,
    pub(crate) anvil_muster_title: bool,
    /// Missing command values being asked in the Anvil, one field at a time.
    pub(crate) anvil_ask: Option<crate::anvil_ask::Ask>,
    pub(crate) anvil_helper_notice: Option<crate::anvil::AnvilHelperNotice>,
    pub(crate) anvil_group: usize,
    pub(crate) shell_data: shell_store::ShellData,
    pub(crate) shell_writer: shell_store::ShellWriter,
    /// Seletor, projeção e gravação de arranjos (estado em memória).
    pub(crate) layout_state: crate::layout_presets::LayoutState,
    pub(crate) current_bench_id: Option<String>,
    pub(crate) show_sidebar: bool,
    pub(crate) last_sidebar_setting: bool,
    pub(crate) focus_anvil: bool,
    pub(crate) focus_content_tab: bool,
    pub(crate) bench_cache: Vec<(String, String)>,
    pub(crate) bench_cached_at: Option<Instant>,
    pub(crate) active_crumb: Option<String>,
    pub(crate) active_crumb_checked: Option<Instant>,
    pub(crate) file_cache: Vec<(String, String)>,
    pub(crate) file_cache_query: String,
    pub(crate) show_right_panel: bool,
    pub(crate) fullscreen: bool,
    pub(crate) status: Option<forja_kernel::HumanError>,
    pub(crate) status_details_open: bool,
    pub(crate) slot_rects: BTreeMap<String, egui::Rect>,
    pub(crate) theme_override: Option<String>,
    pub(crate) interface_theme: Option<String>,
    pub(crate) theme_checked: Instant,
    /// Bumped when the interface theme file hands out a different theme.
    pub(crate) theme_revision: u64,
    /// Avoids rebuilding the theme and the style on every frame.
    pub(crate) look_gate: crate::appearance::LookGate,
    pub(crate) density_override: Option<String>,
    pub(crate) locale_locked: bool,
    pub(crate) system_dark: bool,
    pub(crate) system_probed: Instant,
    pub(crate) fonts_ready: bool,
    pub(crate) logo: Option<egui::TextureHandle>,
    pub(crate) quitting: bool,
    pub(crate) pending_toast: bool,
    pub(crate) pending_onboarding: bool,
    pub(crate) screenshot: Option<ScreenshotJob>,
    screenshot_error: Arc<Mutex<Option<String>>>,
    rpc: Option<RpcHost>,
    rpc_wake: Option<forja_kernel::tasks::Wake>,
    rpc_want_seen: Option<bool>,
    pub(crate) pending_storage_change: Option<crate::settings_storage::PendingStorageChange>,
    pub(crate) storage_notice: Option<String>,
    pub(crate) driver: Option<crate::test_driver::Driver>,
    pub(crate) geometry_writer: crate::window_geometry::GeometryWriter,
    pub(crate) geometry_checked: Instant,
    /// Frame counter and the `app.performance.*` budget.
    pub(crate) pacer: crate::pacer::FramePacer,
    /// Numbers behind the diagnostics overlay.
    pub(crate) diagnostics: crate::diagnostics::Diagnostics,
    pub(crate) close_pending: bool,
    pub(crate) close_allowed: bool,
    pub(crate) close_confirm: Option<close_confirm::ActiveWork>,
    pub(crate) close_confirmed: bool,
    pub(crate) driver_close_flushed: bool,
}

impl ForjaApp {
    pub fn new(
        creation: &eframe::CreationContext<'_>,
        runtime: Runtime,
        screenshot: Option<ScreenshotJob>,
        screenshot_error: Arc<Mutex<Option<String>>>,
        session: Session,
    ) -> Self {
        if let Err(error) = forja_ui_kit::install_fonts(&creation.egui_ctx) {
            forja_kernel::logging::log(forja_kernel::logging::LogLevel::Warn, error.to_string());
        }
        if let Some(size) = session.test_window_size {
            // Windows clamps a window larger than the monitor at creation time.
            // Re-applying the size here keeps test layouts at the requested
            // logical size even when the screen is smaller.
            creation
                .egui_ctx
                .send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
        }
        if let Some(job) = screenshot.as_ref().filter(|job| job.ui_tree.is_some()) {
            creation.egui_ctx.enable_accesskit();
            creation.egui_ctx.add_plugin(crate::ui_tree::UiTreeCapture {
                latest: Arc::clone(&job.tree),
            });
        }
        let rail_views = ViewInstance::collect(runtime.registry(), Slot::Rail);
        let sidebar_views = ViewInstance::collect(runtime.registry(), Slot::Sidebar);
        let mut bench_views = ViewInstance::collect(runtime.registry(), Slot::BenchTab);
        let home_view = bench_views
            .iter()
            .position(|view| view.id == "project.home")
            .map(|index| bench_views.remove(index));
        let right_views = ViewInstance::collect(runtime.registry(), Slot::RightPanel);
        let status_views = ViewInstance::collect(runtime.registry(), Slot::StatusBar);
        let settings_views = ViewInstance::collect(runtime.registry(), Slot::SettingsSection);
        let modal_views = ViewInstance::collect(runtime.registry(), Slot::Modal);
        let anvil_views = ViewInstance::collect(runtime.registry(), Slot::AnvilSource);
        let top_views = ViewInstance::collect(runtime.registry(), Slot::TopBar);
        for warning in runtime.warnings() {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Warn,
                format!("forja: {warning}"),
            );
        }
        let mut runtime = runtime;
        let default_sidebar_open =
            Self::runtime_setting_bool(&runtime, "appearance.sidebar.visible", true);
        let shell_data = match runtime.store().load::<Value>("shell") {
            LoadResult::Loaded(ref value) => {
                shell_store::ShellData::from_json(value, default_sidebar_open)
            }
            _ => shell_store::ShellData::default(),
        };
        let mut shell_writer = shell_store::ShellWriter::default();
        shell_writer.seed(shell_data.clone());
        let current_bench_id = Self::query_current_bench_id(&mut runtime);
        let saved_selection = current_bench_id
            .as_deref()
            .and_then(|id| shell_data.layout.get(id))
            .and_then(|layout| layout.layout.clone());
        let rail_registration: Vec<String> =
            rail_views.iter().map(|view| view.id.clone()).collect();
        let resolved = {
            let base_layout = current_bench_id
                .as_deref()
                .and_then(|id| shell_data.layout.get(id))
                .cloned()
                .unwrap_or_else(|| shell_store::BenchLayout {
                    sidebar_open: default_sidebar_open,
                    ..Default::default()
                });
            let bench_ids: Vec<&str> = bench_views.iter().map(|view| view.id.as_str()).collect();
            let rail_ids: Vec<&str> = rail_views.iter().map(|view| view.id.as_str()).collect();
            let sidebar_ids: Vec<&str> =
                sidebar_views.iter().map(|view| view.id.as_str()).collect();
            let right_ids: Vec<&str> = right_views.iter().map(|view| view.id.as_str()).collect();
            let initial_open = if base_layout.open_tabs.is_none() {
                Some(Self::initial_open_views(
                    &mut runtime,
                    &bench_views,
                    current_bench_id.as_deref(),
                ))
            } else {
                None
            };
            let mut resolved = base_layout.resolve_with_defaults(
                &bench_ids,
                &rail_ids,
                &sidebar_ids,
                &right_ids,
                initial_open.as_deref(),
            );
            let env_right_tab = std::env::var("FORJA_RIGHT_TAB").ok();
            let cli = shell_store::CliOverrides {
                open: session.open.as_deref(),
                rail: session.rail.as_deref(),
                bench_tab: session.bench_tab.as_deref(),
                right_tab: session.right_tab.as_deref(),
                env_right_tab: env_right_tab.as_deref(),
            };
            resolved.apply_cli_overrides(&cli, &bench_ids, &rail_ids, &right_ids);
            resolved
        };
        let known_tab_content = bench_views
            .iter()
            .filter(|view| view.view.empty_by_nature())
            .map(|view| (view.id.clone(), view.view.has_content()))
            .collect();
        let mut app = Self {
            runtime,
            tokens: Tokens::brasa_dark(),
            rail_views,
            sidebar_views,
            bench_views,
            home_view,
            project_was_open: false,
            right_views,
            status_views,
            settings_views,
            modal_views,
            anvil_views,
            top_views,
            rail_page: resolved.rail_page,
            active_bench: None,
            rail_icons: BTreeMap::new(),
            active_tab: resolved.active_tab,
            tabs: resolved.tabs,
            known_tab_content,
            sidebar_tab: resolved.sidebar_tab,
            right_tab: resolved.right_tab,
            settings_open: false,
            settings_page: crate::settings_page::State::default(),
            keymap_open: false,
            settings_section: None,
            modal_capture: session.modal.clone(),
            settings_query: String::new(),
            setting_drafts: BTreeMap::new(),
            anvil_open: false,
            anvil_query: String::new(),
            anvil_query_seen: String::new(),
            anvil_selected: 0,
            anvil_selection_explicit: false,
            anvil_selection_id: None,
            anvil_selection_missing: false,
            anvil_rail_select: false,
            anvil_muster_title: false,
            anvil_ask: None,
            anvil_helper_notice: None,
            anvil_group: 0,
            shell_data,
            shell_writer,
            layout_state: crate::layout_presets::LayoutState {
                selection: saved_selection,
                rail_registration,
                ..Default::default()
            },
            current_bench_id,
            show_sidebar: resolved.sidebar_open,
            last_sidebar_setting: default_sidebar_open,
            focus_anvil: false,
            focus_content_tab: false,
            bench_cache: Vec::new(),
            bench_cached_at: None,
            active_crumb: None,
            active_crumb_checked: None,
            file_cache: Vec::new(),
            file_cache_query: String::new(),
            show_right_panel: resolved.right_panel_open,
            fullscreen: false,
            status: None,
            status_details_open: false,
            slot_rects: BTreeMap::new(),
            theme_override: session.theme,
            interface_theme: None,
            theme_checked: Instant::now() - std::time::Duration::from_secs(2),
            theme_revision: 0,
            look_gate: crate::appearance::LookGate::default(),
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
            rpc_wake: None,
            rpc_want_seen: None,
            pending_storage_change: None,
            storage_notice: None,
            driver: None,
            geometry_writer: crate::window_geometry::GeometryWriter::default(),
            geometry_checked: Instant::now(),
            pacer: crate::pacer::FramePacer::new(),
            diagnostics: crate::diagnostics::Diagnostics::default(),
            close_pending: false,
            close_allowed: false,
            close_confirm: None,
            close_confirmed: false,
            driver_close_flushed: false,
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
            // Background work wakes the UI, but never sooner than the frame
            // budget: coalescing these wakes is what `max_fps` caps. The RPC
            // pipe stays immediate, because a request is a user action, and so
            // does every wake while a driver or a capture waits for a frame:
            // deferring it there means losing exactly the frame being awaited.
            let paced = app.driver.is_none() && app.screenshot.is_none();
            let gate = app.pacer.gate();
            let task_context = context.clone();
            let task_wake: forja_kernel::tasks::Wake = Arc::new(move || {
                if paced {
                    gate.request_repaint(&task_context);
                } else {
                    task_context.request_repaint();
                }
            });
            app.runtime.set_wake(Arc::clone(&task_wake));
            let delayed_context = context.clone();
            app.runtime
                .set_wake_after(Arc::new(move |delay| delayed_context.request_repaint_after(delay)));
            let rpc_wake: forja_kernel::tasks::Wake =
                Arc::new(move || context.request_repaint());
            app.rpc_wake = Some(rpc_wake);
            app.sync_rpc_host();
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
        if let Some(view_id) = session.rail.clone() {
            if !app.rail_views.iter().any(|view| view.id == view_id) {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Warn,
                    format!("forja-app: --rail {view_id}: no rail page has that id"),
                );
            }
        }
        if let Some(view_id) = session.modal.clone() {
            if !app.modal_views.iter().any(|view| view.id == view_id) {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Warn,
                    format!("forja-app: --modal {view_id}: no modal view has that id"),
                );
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
            self.anvil_rail_select = false;
            self.anvil_muster_title = false;
            self.anvil_ask = None;
            self.anvil_helper_notice = None;
            self.anvil_query.clear();
            self.anvil_query_seen.clear();
            self.anvil_selected = 0;
            self.anvil_selection_explicit = false;
            self.anvil_selection_id = None;
            self.anvil_selection_missing = false;
        }
    }

    fn sync_rpc_host(&mut self) {
        let want = crate::rpc::should_run_rpc_server(
            self.setting_bool("app.cli.enabled", true),
            self.driver.is_some(),
            self.screenshot.is_some(),
        );
        if want {
            if self.rpc.is_none() && self.rpc_want_seen != Some(true) {
                self.rpc_want_seen = Some(true);
                if let Some(wake) = self.rpc_wake.as_ref() {
                    match RpcHost::start(Arc::clone(wake)) {
                        Ok(host) => self.rpc = Some(host),
                        Err(error) => self.set_status_error(&error.to_string()),
                    }
                }
            }
        } else {
            self.rpc_want_seen = Some(false);
            if self.rpc.take().is_some() {
                forja_kernel::logging::log(
                    forja_kernel::logging::LogLevel::Info,
                    "forja: local CLI/RPC server stopped (app.cli.enabled = false)",
                );
            }
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
        self.run_command_args(id, Args::new());
    }

    /// Runs a command with values collected by the Anvil or supplied by a module.
    pub(crate) fn run_command_args(&mut self, id: &str, args: Args) {
        self.active_crumb_checked = None;
        if id == "terminal.toggle"
            && self.rail_page.is_none()
            && self
                .bench_views
                .get(self.active_tab)
                .is_some_and(|view| view.id == "terminal.tab")
            && self
                .tabs
                .is_visible(self.active_tab, self.bench_views.len())
        {
            actions::push(actions::Action::CloseTab);
            return;
        }
        match self.runtime.run_command(id, args) {
            Ok(_) => {
                self.clear_status();
                if matches!(
                    id,
                    "terminal.open" | "terminal.search" | "terminal.toggle" | "terminal.focus"
                ) {
                    actions::push(actions::Action::ShowBenchTab("terminal.tab".to_owned()));
                }
                if matches!(id, "helper.list" | "helper.status") {
                    actions::push(actions::Action::OpenRail("helper.tab".to_owned()));
                }
                if id == "diff.open" {
                    actions::push(actions::Action::ShowBenchTab("diff.tab".to_owned()));
                }
            }
            Err(error) => {
                self.set_command_error(id, &error);
            }
        }
    }

    pub(crate) fn clear_status(&mut self) {
        self.status = None;
        self.status_details_open = false;
    }

    pub(crate) fn set_status_error(&mut self, raw: &str) {
        self.status = Some(forja_kernel::human_error::humanize_error(
            self.runtime.i18n(),
            raw,
        ));
        self.status_details_open = false;
    }

    pub(crate) fn set_command_error(&mut self, id: &str, error: &forja_kernel::KernelError) {
        let title = self
            .runtime
            .registry()
            .command(id)
            .map(|spec| self.text(&spec.title_key))
            .unwrap_or_else(|| self.text("shell.anvil.group.commands"));
        let human = forja_kernel::human_error::humanize_kernel_error(self.runtime.i18n(), error);
        let summary = self.text_args(
            "shell.command.failed",
            &[("id", &title), ("message", &human.summary)],
        );
        let detail = human.detail.map(|detail| format!("{id}: {detail}"));
        self.status = Some(forja_kernel::HumanError { summary, detail });
        self.status_details_open = false;
    }

    pub(crate) fn no_project(&mut self) -> bool {
        self.home_view.is_some()
            && self
                .runtime
                .run_command("project.current", Args::new())
                .is_ok_and(|value| value.is_null())
    }

    pub(crate) fn active_project_crumb(&mut self) -> String {
        let stale = self
            .active_crumb_checked
            .is_none_or(|at| at.elapsed() >= std::time::Duration::from_millis(500));
        if stale {
            self.active_crumb_checked = Some(Instant::now());
            self.active_crumb = self.resolve_active_crumb();
        }
        self.active_crumb
            .clone()
            .unwrap_or_else(|| self.text("shell.top.no-project"))
    }

    fn resolve_active_crumb(&mut self) -> Option<String> {
        if self.no_project() {
            return None;
        }
        let projects = if self.runtime.registry().command("project.list").is_some() {
            self.runtime
                .run_command("project.list", Args::new())
                .ok()
                .and_then(|v| v.get("projects").and_then(Value::as_array).cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let active_project = if self.runtime.registry().command("project.current").is_some() {
            self.runtime
                .run_command("project.current", Args::new())
                .ok()
                .and_then(|value| {
                    let id = value.get("id").and_then(Value::as_str)?.trim().to_owned();
                    let name = value.get("name").and_then(Value::as_str)?.trim().to_owned();
                    (!id.is_empty() && !name.is_empty()).then_some((id, name))
                })
        } else {
            None
        };
        if self.runtime.registry().command("bench.current").is_some() {
            if let Ok(value) = self.runtime.run_command("bench.current", Args::new()) {
                let bench_id = value
                    .get("bench_id")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned);
                if bench_id.is_some() && self.current_bench_id != bench_id {
                    let keep_page = self.rail_page.is_some();
                    self.switch_bench_layout(bench_id.clone(), keep_page);
                }
                // A política de retomada também vale na bancada restaurada no boot.
                if self.runtime.registry().command("helper.resume.evaluate").is_some() {
                    let mut args = Args::new();
                    if let Some(id) = &bench_id {
                        args.insert("bench_id", json!(id));
                    }
                    if let Some(path) = value.get("path").and_then(Value::as_str) {
                        args.insert("bench_path", json!(path));
                    }
                    if let Some(name) = value.get("name").and_then(Value::as_str) {
                        args.insert("name", json!(name));
                    }
                    let _ = self.runtime.run_command("helper.resume.evaluate", args);
                }
                let bench_name = value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty());
                let project_id = value.get("project_id").and_then(Value::as_str);
                if let Some((active_id, active_name)) = &active_project {
                    if project_id == Some(active_id.as_str()) {
                        return Some(match bench_name {
                            Some(bench) if bench != active_name => {
                                format!("{active_name} · {bench}")
                            }
                            _ => active_name.clone(),
                        });
                    }
                } else {
                    let project_name = project_id.and_then(|id| {
                        projects.iter().find_map(|p| {
                            (p.get("id").and_then(Value::as_str) == Some(id))
                                .then(|| p.get("name").and_then(Value::as_str))
                                .flatten()
                        })
                    });
                    match (project_name, bench_name) {
                        (Some(project), Some(bench)) if project != bench => {
                            return Some(format!("{project} · {bench}"));
                        }
                        (Some(project), _) => return Some(project.to_owned()),
                        (None, Some(bench)) => return Some(bench.to_owned()),
                        (None, None) => {}
                    }
                }
            }
        }
        if let Some((_, active_name)) = active_project {
            return Some(active_name);
        }
        if !self.setting_bool("app.startup.open_last", true) {
            return None;
        }
        projects
            .first()
            .and_then(|p| p.get("name").and_then(Value::as_str))
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
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
                        self.status = Some(forja_kernel::HumanError::with_detail(
                            self.text("shell.rail.not-found"),
                            view_id,
                        ));
                        self.status_details_open = false;
                    }
                }
                actions::Action::OpenBench {
                    project,
                    bench,
                    bench_id,
                    keep_page,
                } => {
                    let next_id =
                        bench_id.or_else(|| Self::query_current_bench_id(&mut self.runtime));
                    self.switch_bench_layout(next_id, keep_page);
                    self.active_bench = Some((project, bench));
                    self.active_crumb_checked = None;
                }
                actions::Action::ShowBenchTab(view_id) => {
                    if let Some(index) = self.bench_views.iter().position(|view| view.id == view_id)
                    {
                        self.tabs.show(index, self.bench_views.len());
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::ToggleSidebar => {
                    self.show_sidebar = !self.show_sidebar;
                    self.last_sidebar_setting = self.show_sidebar;
                    let _ = self.runtime.set_setting(
                        "appearance.sidebar.visible",
                        SettingValue::Bool(self.show_sidebar),
                    );
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
                        if ctx.wants_keyboard_input() {
                            if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                                forja_ui_kit::release_keyboard_focus(ctx, focused);
                            }
                            self.focus_content_tab = true;
                        }
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::PreviousTab => {
                    if let Some(index) = self.tabs.previous(self.active_tab, self.bench_views.len())
                    {
                        if ctx.wants_keyboard_input() {
                            if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                                forja_ui_kit::release_keyboard_focus(ctx, focused);
                            }
                            self.focus_content_tab = true;
                        }
                        self.active_tab = index;
                        self.rail_page = None;
                    }
                }
                actions::Action::FocusTabs => {
                    self.rail_page = None;
                    self.tabs.show(self.active_tab, self.bench_views.len());
                    if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                        forja_ui_kit::release_keyboard_focus(ctx, focused);
                    }
                    self.focus_content_tab = true;
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
                action @ (actions::Action::OpenLayoutPicker
                | actions::Action::SwitchLayout { .. }
                | actions::Action::SaveLayout
                | actions::Action::ResetLayout
                | actions::Action::ManageLayouts) => self.handle_layout_action(action),
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

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Only the Anvil shortcuts may open above these shell windows. Other
        // modal layers, including dialogs and popups, keep exclusive control.
        let shell_window_active = (self.settings_open
            && ctx.memory(|memory| memory.top_modal_layer().is_none()))
            || (self.keymap_open
                && forja_ui_kit::keyboard_layer_active(
                    ctx,
                    egui::Id::new("forja.keymap.keyboard"),
                ));
        if self.anvil_open
            || self.close_confirm.is_some()
            || egui::Popup::is_any_open(ctx)
            || (ctx.memory(|memory| memory.top_modal_layer().is_some()) && !shell_window_active)
        {
            return;
        }
        let text_busy = ctx.wants_keyboard_input();
        let active_module = self
            .bench_views
            .get(self.active_tab)
            .filter(|_| self.rail_page.is_none())
            .map(|view| view.module.as_str());
        let bindings: Vec<(String, KeyChord)> = self
            .runtime
            .keymap()
            .bindings()
            .iter()
            .filter(|resolved| shortcut_scope_applies(&resolved.binding.scope, active_module))
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
            if (self.settings_open || self.keymap_open)
                && (!shell_window_active || !is_anvil_shortcut(&id))
            {
                continue;
            }
            let always = always_shortcut(&id);
            if browser_surface
                && matches!(id.as_str(), "tab.close" | "tab.reopen" | "tab.switch_10")
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
        let has_create_dialog = self
            .runtime
            .registry()
            .command("bench.create_dialog")
            .is_some();
        for id in fired {
            let target = shortcut_target(&id, has_create_dialog);
            // A command that needs context asks for it in the Anvil instead of
            // failing with a missing-argument error.
            if !self.start_ask(target) {
                self.run_command(target);
            }
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
                    Ok(()) => forja_kernel::logging::log(
                        forja_kernel::logging::LogLevel::Info,
                        format!("forja: ui tree saved to {}", path.display()),
                    ),
                    Err(error) => {
                        if let Ok(mut slot) = self.screenshot_error.lock() {
                            *slot = Some(format!("ui tree: {error}"));
                        }
                    }
                }
            }
            job.done = true;
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Info,
                format!("forja: screenshot saved to {}", job.path.display()),
            );
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
        self.pacer.begin();
        self.handle_close(ctx);
        let focused = ctx.input(|input| input.viewport().focused.unwrap_or(true));
        self.runtime.set_viewport_focused(focused);
        let budget = self.frame_budget(focused);
        self.drain_rpc(ctx);
        if self
            .driver
            .as_mut()
            .is_some_and(crate::test_driver::Driver::take_main_panic)
        {
            panic!("test-driver requested a panic in the main loop");
        }
        let tasks_applied = self.runtime.poll_tasks();
        self.runtime.poll_ticks();
        self.poll_layout_catalog();
        self.handle_shortcuts(ctx);
        self.drain_actions(ctx);
        self.show_demos();
        self.refresh_look(ctx);
        self.sync_sidebar_setting();
        self.sync_dynamic_tabs();
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.left_panels(ctx);
        self.right_panel(ctx);
        self.central_area(ctx);
        self.settings_window(ctx);
        self.sync_rpc_host();
        self.keymap_window(ctx);
        self.anvil_window(ctx);
        self.modal_windows(ctx);
        self.layout_picker_window(ctx);
        self.layout_save_window(ctx);
        self.sync_shell_store();
        self.diagnostics_overlay(ctx);
        self.capture(ctx);
        if let Some(driver) = self.driver.as_mut() {
            driver.tick(ctx, self.runtime.tasks(), tasks_applied);
        }
        self.pacer.end(budget);
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(driver) = self.driver.as_mut() {
            driver.feed(ctx, raw_input);
        }
    }
}

impl Drop for ForjaApp {
    fn drop(&mut self) {
        self.rpc.take();
        tray::remove();
    }
}

fn is_anvil_shortcut(id: &str) -> bool {
    matches!(id, "anvil.open" | "anvil.command_mode" | "find.file")
}

fn shortcut_target(id: &str, has_create_dialog: bool) -> &str {
    if id == "bench.create" && has_create_dialog {
        "bench.create_dialog"
    } else {
        id
    }
}

/// Shortcuts that keep working while a text surface or a shell window owns the
/// keyboard. `terminal.toggle` belongs here because `design/keymap.toml` marks
/// it `even_in_terminal = true`.
fn always_shortcut(id: &str) -> bool {
    matches!(
        id,
        "anvil.open"
            | "anvil.command_mode"
            | "bench.create"
            | "find.file"
            | "settings.open"
            | "settings.keymap"
            | "help.keymap"
            | "panel.toggle"
            | "terminal.toggle"
            | "view.zoom.in"
            | "view.zoom.out"
            | "view.zoom.reset"
            | "window.fullscreen"
            | "tab.focus"
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
    )
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
    ctx.input_mut(|input| {
        let matching_shift = input.events.iter().any(|event| {
            matches!(event, egui::Event::Key { key: event_key, pressed: true, modifiers: event_modifiers, .. }
                if *event_key == key && event_modifiers.shift == chord.shift)
        });
        matching_shift && input.consume_key(modifiers, key)
    })
}

fn shortcut_scope_applies(scope: &Scope, active_module: Option<&str>) -> bool {
    match scope {
        Scope::Global => true,
        Scope::Bench => active_module.is_some(),
        Scope::Editor => active_module == Some("editor"),
        Scope::Terminal => active_module == Some("terminal"),
        Scope::Custom(name) => active_module == Some(name.as_str()),
    }
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

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn shell_windows_allow_only_anvil_shortcuts() {
        assert!(is_anvil_shortcut("anvil.open"));
        assert!(is_anvil_shortcut("find.file"));
        assert!(!is_anvil_shortcut("editor.delete_line"));
        assert!(!is_anvil_shortcut("tab.close"));
    }

    #[test]
    fn terminal_toggle_works_while_the_terminal_owns_the_keyboard() {
        assert!(always_shortcut("terminal.toggle"));
        assert!(always_shortcut("bench.create"));
        assert!(always_shortcut("tab.focus"));
        assert!(!always_shortcut("terminal.copy"));
        assert!(!always_shortcut("editor.save"));
    }

    #[test]
    fn bench_create_shortcut_targets_create_dialog() {
        assert_eq!(shortcut_target("bench.create", true), "bench.create_dialog");
        assert_eq!(shortcut_target("bench.create", false), "bench.create");
        assert_eq!(shortcut_target("tab.focus", true), "tab.focus");
    }

    #[test]
    fn shifted_chord_does_not_trigger_unshifted_shortcut() {
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::K,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                shift: true,
                command: true,
                ..Default::default()
            },
        });
        let _ = ctx.run(raw, |ctx| {
            assert!(!chord_pressed(ctx, &KeyChord::parse("Ctrl+K").unwrap()));
            assert!(chord_pressed(
                ctx,
                &KeyChord::parse("Ctrl+Shift+K").unwrap()
            ));
        });
    }

    #[test]
    fn shortcuts_only_apply_to_the_active_surface() {
        assert!(shortcut_scope_applies(&Scope::Global, None));
        assert!(shortcut_scope_applies(&Scope::Editor, Some("editor")));
        assert!(!shortcut_scope_applies(&Scope::Editor, Some("terminal")));
        assert!(shortcut_scope_applies(&Scope::Terminal, Some("terminal")));
        assert!(!shortcut_scope_applies(&Scope::Terminal, None));
        assert!(!shortcut_scope_applies(
            &Scope::Custom("anvil".into()),
            Some("editor")
        ));
        assert!(shortcut_scope_applies(
            &Scope::Custom("browser".into()),
            Some("browser")
        ));
    }
}
