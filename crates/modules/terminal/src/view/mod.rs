//! Views do módulo: a aba de terminal e a lista de receitas da Bigorna.

mod chrome;
mod hub_ui;
mod input;
mod render;
mod terminal_ui;

use crate::core::{HubEvent, HubEventKind, StoredLayout};
use crate::Shared;
use crate::links::Link;
use crate::settings::TermSettings;
use forja_contracts::{
    FileOpenRequested, TerminalBell, TerminalCommandFinished, TerminalExited, TerminalTitleChanged,
};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{AnvilHit, Args, Ctx};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Textos usados pela view, resolvidos antes de bloquear o hub.
pub struct UiText {
    pub search_hint: String,
    pub previous: String,
    pub next: String,
    pub match_single: String,
    pub match_plural: String,
    pub open: String,
    pub copy: String,
    pub paste: String,
    pub dismiss: String,
    pub recipes: String,
    pub shell: String,
    pub new_tab: String,
    pub split_right: String,
    pub split_down: String,
    pub close_pane: String,
    pub click_to_type: String,
    pub empty_title: String,
    pub empty_hint: String,
    pub open_terminal: String,
    pub confirm_close_title: String,
    pub confirm_close_body: String,
    pub confirm_close_accept: String,
    pub cancel: String,
    pub recipe_confirm_title: String,
    pub recipe_confirm_body: String,
    pub recipe_confirm_accept: String,
    pub paste_title: String,
    pub paste_body: String,
    pub paste_accept: String,
    pub notice_dismiss: String,
    pub wsl: String,
    pub wsl_default: String,
    pub git_bash: String,
    pub powershell: String,
    pub pwsh: String,
    pub cmd: String,
    pub select_all: String,
    pub copy_output: String,
    pub clear: String,
    pub search: String,
    pub running: String,
    pub exited: String,
    pub recipe_empty: String,
    pub recipe_scope_global: String,
    pub recipe_scope_project: String,
    pub link_title: String,
    pub error_launch: String,
    pub close: String,
    pub failed: String,
    pub error_not_found_title: String,
    pub error_not_found_body: String,
    pub error_launch_body: String,
    pub error_retry: String,
    pub error_settings: String,
    pub error_details: String,
    pub locale: String,
}

impl UiText {
    /// Resolve todas as strings com o i18n do módulo.
    pub fn new(cx: &Ctx<'_>) -> Self {
        Self {
            search_hint: cx.t("terminal.view.search-hint"),
            previous: cx.t("terminal.view.previous"),
            next: cx.t("terminal.view.next"),
            match_single: cx.t("terminal.view.match"),
            match_plural: cx.t("terminal.view.matches"),
            open: cx.t("terminal.view.open"),
            copy: cx.t("terminal.view.copy"),
            paste: cx.t("terminal.view.paste"),
            dismiss: cx.t("terminal.view.dismiss"),
            recipes: cx.t("terminal.view.recipes"),
            shell: cx.t("terminal.view.shell"),
            new_tab: cx.t("terminal.view.new-tab"),
            split_right: cx.t("terminal.view.split-right"),
            split_down: cx.t("terminal.view.split-down"),
            close_pane: cx.t("terminal.view.close-pane"),
            click_to_type: cx.t("terminal.view.click-to-type"),
            empty_title: cx.t("terminal.view.empty-title"),
            empty_hint: cx.t("terminal.view.empty-hint"),
            open_terminal: cx.t("terminal.view.open-terminal"),
            confirm_close_title: cx.t("terminal.view.confirm-close-title"),
            confirm_close_body: cx.t("terminal.view.confirm-close-body"),
            confirm_close_accept: cx.t("terminal.view.confirm-close-accept"),
            cancel: cx.t("terminal.view.cancel"),
            recipe_confirm_title: cx.t("terminal.view.recipe-confirm-title"),
            recipe_confirm_body: cx.t("terminal.view.recipe-confirm-body"),
            recipe_confirm_accept: cx.t("terminal.view.recipe-confirm-accept"),
            paste_title: cx.t("terminal.view.paste-title"),
            paste_body: cx.t("terminal.view.paste-body"),
            paste_accept: cx.t("terminal.view.paste-accept"),
            notice_dismiss: cx.t("terminal.view.notice-dismiss"),
            wsl: cx.t("terminal.view.wsl"),
            wsl_default: cx.t("terminal.view.wsl-default"),
            git_bash: cx.t("terminal.view.git-bash"),
            powershell: cx.t("terminal.view.powershell"),
            pwsh: cx.t("terminal.view.pwsh"),
            cmd: cx.t("terminal.view.cmd"),
            select_all: cx.t("terminal.view.select-all"),
            copy_output: cx.t("terminal.view.copy-output"),
            clear: cx.t("terminal.view.clear"),
            search: cx.t("terminal.view.search"),
            running: cx.t("terminal.view.running"),
            exited: cx.t("terminal.view.exited"),
            recipe_empty: cx.t("terminal.view.recipe-empty"),
            recipe_scope_global: cx.t("terminal.view.recipe-scope-global"),
            recipe_scope_project: cx.t("terminal.view.recipe-scope-project"),
            link_title: cx.t("terminal.view.link-title"),
            error_launch: cx.t("terminal.view.error-launch"),
            close: cx.t("terminal.view.close"),
            failed: cx.t("terminal.view.failed"),
            error_not_found_title: cx.t("terminal.view.error-not-found-title"),
            error_not_found_body: cx.t_args(
                "terminal.view.error-not-found-body",
                &[("program", "{ $program }")],
            ),
            error_launch_body: cx.t("terminal.view.error-launch-body"),
            error_retry: cx.t("terminal.view.error-retry"),
            error_settings: cx.t("terminal.view.error-settings"),
            error_details: cx.t("terminal.view.error-details"),
            locale: cx.i18n().locale().id().to_owned(),
        }
    }
}

/// Ações que precisam de `Ctx` depois que o hub é liberado.
pub enum ViewAction {
    /// Abrir um link confirmado.
    OpenLink(Link),
    /// Abrir os ajustes do aplicativo.
    OpenSettings,
}

/// Aba da bancada com todos os painéis de terminal.
pub struct TerminalView {
    core: Shared,
    booted: bool,
    textures: HashMap<u32, egui::TextureHandle>,
    content_revision: Option<u64>,
    content_checked: Option<Instant>,
}

impl TerminalView {
    /// Cria a view ligada ao hub do módulo.
    pub fn new(core: Shared) -> Self {
        Self {
            core,
            booted: false,
            textures: HashMap::new(),
            content_revision: None,
            content_checked: None,
        }
    }

    /// Consulta temas e receitas no contrato `content.*`; sem o módulo, o hub
    /// continua lendo os arquivos diretamente.
    fn refresh_contract_content(&mut self, cx: &mut Ctx<'_>) {
        let due = self
            .content_checked
            .is_none_or(|at| at.elapsed() >= Duration::from_millis(500));
        if !due {
            return;
        }
        self.content_checked = Some(Instant::now());
        let bench = cx
            .run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| {
                value
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            });
        let mut args = Args::new();
        if let Some(bench) = &bench {
            args.insert("project_path", json!(bench.display().to_string()));
        }
        let themes = cx.run_command("content.themes.list", args.clone()).ok();
        let recipes = cx.run_command("content.recipes.list", args).ok();
        if themes.is_none() && recipes.is_none() {
            return;
        }
        let revision = themes
            .as_ref()
            .and_then(|value| value.get("revision").and_then(serde_json::Value::as_u64))
            .or_else(|| {
                recipes
                    .as_ref()
                    .and_then(|value| value.get("revision").and_then(serde_json::Value::as_u64))
            });
        if revision.is_some() && revision == self.content_revision {
            return;
        }
        self.content_revision = revision;
        let theme_texts: Vec<String> = themes
            .as_ref()
            .and_then(|value| value.get("themes"))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| {
                        item.get("kind").and_then(serde_json::Value::as_str) == Some("terminal")
                    })
                    .filter_map(|item| {
                        item.get("toml")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let recipes: Vec<crate::recipes::Recipe> = recipes
            .as_ref()
            .and_then(|value| value.get("recipes"))
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(crate::recipes::Recipe::from_value)
                    .collect()
            })
            .unwrap_or_default();
        let _ = self.core.with_hub(|hub| hub.set_contract_content(theme_texts, recipes));
    }

    fn boot(&mut self, cx: &mut Ctx<'_>, settings: &TermSettings, store: &Path) {
        if self.booted {
            return;
        }
        self.booted = true;
        let bench = cx
            .run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| {
                value
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            });
        if let Some(bench) = bench {
            let _ = self.core.with_hub(|hub| hub.set_bench(Some(bench)));
        }
        let _ = self.core.with_hub(|hub| {
            hub.restore_pending = true;
            hub.ensure_content();
        });
        let path = store.to_path_buf();
        let core = Arc::clone(&self.core);
        let _ = settings;
        cx.spawn(
            move || {
                std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<StoredLayout>(&bytes).ok())
            },
            move |cx, snapshot| {
                let settings = TermSettings::from_ctx(cx);
                let _ = core.with_hub(|hub| {
                    hub.restore_pending = false;
                    if let Some(snapshot) = snapshot {
                        hub.restore(snapshot, &settings);
                    }
                    hub.ensure_content();
                });
            },
        );
    }

    fn apply(&self, action: ViewAction, cx: &mut Ctx<'_>, settings: &TermSettings) {
        match action {
            ViewAction::OpenLink(link) => open_link(cx, link, settings),
            ViewAction::OpenSettings => {
                let _ = cx.run_command("settings.open", Args::new());
            }
        }
    }
}

impl View for TerminalView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let settings = TermSettings::from_ctx(cx);
        let texts = UiText::new(cx);
        let store = cx.store().path();
        self.boot(cx, &settings, &store);
        self.refresh_contract_content(cx);
        let wsl = self.core.wsl_distros();
        let now = Instant::now();
        let mut actions = Vec::new();
        let (events, persist, alive) = {
            let Ok(mut hub) = self.core.hub.lock() else {
                ui.label(texts.error_launch.clone());
                return;
            };
            hub.ui_context = Some(ui.ctx().clone());
            hub.ensure_content();
            if !hub.restore_pending {
                hub.ensure_open(&settings);
            }
            hub.seed_preview(&settings);
            let events = hub.poll(now, &settings, Some(ui.ctx()));
            let alive = hub.terminals.values().any(|terminal| terminal.alive());
            let mut approve_recipe =
                |recipe: &crate::recipes::Recipe| crate::recipe_trust::approve(cx, recipe);
            hub_ui::draw(
                &mut hub,
                ui,
                &settings,
                &texts,
                &wsl,
                &mut self.textures,
                &mut approve_recipe,
                &mut actions,
            );
            let persist = hub.persist_if_due(now, false);
            (events, persist, alive)
        };
        for action in actions {
            self.apply(action, cx, &settings);
        }
        for event in events {
            emit(cx, event);
        }
        if let Some(snapshot) = persist {
            let path = store.clone();
            cx.spawn(
                move || {
                    if let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) {
                        let _ = forja_kernel::store::write_atomic_bytes(&path, &bytes);
                    }
                },
                |_, _| {},
            );
        }
        ui.ctx().request_repaint_after(Duration::from_millis(if alive { 16 } else { 350 }));
    }
}

fn emit(cx: &mut Ctx<'_>, event: HubEvent) {
    match event.kind {
        HubEventKind::Bell => {
            let _ = cx.emit(&TerminalBell {
                terminal: event.terminal,
            });
        }
        HubEventKind::Exited => {
            let _ = cx.emit(&TerminalExited {
                terminal: event.terminal,
                exit_code: event.exit_code,
            });
        }
        HubEventKind::CommandFinished => {
            let _ = cx.emit(&TerminalCommandFinished {
                terminal: event.terminal,
                exit_code: event.exit_code,
            });
        }
        HubEventKind::TitleChanged => {
            let _ = cx.emit(&TerminalTitleChanged {
                terminal: event.terminal,
            });
        }
    }
}

fn open_link(cx: &mut Ctx<'_>, link: Link, settings: &TermSettings) {
    match link {
        Link::Url(url) => {
            if settings.links_open_with == "app" {
                let mut args = Args::new();
                args.insert("url", json!(url));
                if cx.run_command("browser.open", args).is_ok() {
                    return;
                }
            }
            open_external(cx, url);
        }
        Link::File {
            path,
            line,
            column,
        } => {
            let mut args = Args::new();
            args.insert("path", json!(path.display().to_string()));
            if let Some(line) = line {
                args.insert("line", json!(line));
            }
            if let Some(column) = column {
                args.insert("column", json!(column));
            }
            let editor_present = cx.registry().command("file.open").is_some();
            if editor_present {
                let event = FileOpenRequested {
                    path: path.display().to_string(),
                    line,
                    column,
                };
                if cx.run_command("file.open", args.clone()).is_ok() {
                    return;
                }
                let _ = cx.emit(&event);
                return;
            }
            let event = FileOpenRequested {
                path: path.display().to_string(),
                line,
                column,
            };
            let _ = cx.emit(&event);
            open_external(cx, path.display().to_string());
        }
    }
}

fn open_external(cx: &mut Ctx<'_>, target: String) {
    cx.spawn(
        move || crate::shells::open_external(&target),
        |_, _| {},
    );
}

struct RecipeSnapshot {
    bench: Option<PathBuf>,
    recipes: Vec<crate::recipes::Recipe>,
}

/// Lista de receitas contribuída à Bigorna.
pub struct RecipeView {
    core: Shared,
    error: Option<String>,
    requested_bench: Option<Option<PathBuf>>,
    cached: Arc<Mutex<Option<RecipeSnapshot>>>,
}

impl RecipeView {
    /// Cria a lista ligada ao mesmo hub.
    pub fn new(core: Shared) -> Self {
        Self { core, error: None, requested_bench: None, cached: Arc::new(Mutex::new(None)) }
    }
}

impl View for RecipeView {
    fn provides_anvil_hits(&self) -> bool {
        true
    }

    fn anvil_hits(&mut self, _query: &str, cx: &mut Ctx<'_>) -> Vec<AnvilHit> {
        let settings = TermSettings::from_ctx(cx);
        let locale = cx.i18n().locale().id().to_owned();
        let bench = crate::commands::recipe_bench(cx);
        let (loaded_for, mut recipes) = self.core.with_hub(|hub| {
            (hub.bench_path.clone(), hub.recipes.clone())
        }).unwrap_or_default();
        let stale_fallback = self.cached.lock().ok().and_then(|cache| cache.as_ref()
            .map(|snapshot| snapshot.bench != bench && snapshot.recipes == recipes))
            .unwrap_or(false);
        if recipes.is_empty() || loaded_for != bench || stale_fallback {
            recipes = self.cached.lock().ok().and_then(|cache| cache.as_ref()
                .filter(|snapshot| snapshot.bench == bench).map(|snapshot| snapshot.recipes.clone()))
                .unwrap_or_default();
            if recipes.is_empty() && self.requested_bench.as_ref() != Some(&bench) {
                self.requested_bench = Some(bench.clone());
                let requested = bench.clone();
                let worker_bench = bench.clone();
                let cache = Arc::clone(&self.cached);
                let core = Arc::clone(&self.core);
                cx.spawn(move || crate::recipes::load(worker_bench.as_deref()), move |cx, loaded| {
                    if crate::commands::recipe_bench(cx) != requested { return; }
                    let recipes = core.with_hub(|hub| {
                        if hub.bench_path == requested && !hub.recipes.is_empty() {
                            hub.recipes.clone()
                        } else {
                            hub.recipes = loaded.clone();
                            loaded
                        }
                    }).unwrap_or_default();
                    if let Ok(mut cache) = cache.lock() {
                        *cache = Some(RecipeSnapshot { bench: requested, recipes });
                    }
                    cx.request_repaint();
                });
            }
        }
        recipes.into_iter().filter(|recipe| settings.recipe_listed(&recipe.scope)).map(|recipe| {
            let scope = cx.t(if recipe.scope == "project" {
                "terminal.view.recipe-scope-project"
            } else {
                "terminal.view.recipe-scope-global"
            });
            let mut args = serde_json::Map::new();
            args.insert("recipe_id".into(), json!(&recipe.id));
            args.insert("content_hash".into(), json!(&recipe.content_hash));
            args.insert("bench_path".into(), json!(bench.as_ref().map(|path| path.to_string_lossy().into_owned())));
            AnvilHit {
                title: recipe.title(&locale).to_owned(),
                detail: scope,
                icon_glyph: None,
                score: 0,
                command: "terminal.recipe.run".into(),
                args,
            }
        }).collect()
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let settings = TermSettings::from_ctx(cx);
        let recipes = self
            .core
            .with_hub(|hub| {
                hub.ensure_content();
                hub.recipes.clone()
            })
            .unwrap_or_default();
        ui.label(egui::RichText::new(cx.t("terminal.recipes.title")).strong());
        if recipes.is_empty() {
            let message = self
                .error
                .clone()
                .unwrap_or_else(|| cx.t("terminal.recipes.empty"));
            ui.label(egui::RichText::new(message).weak());
            return;
        }
        let locale = cx.i18n().locale().id().to_owned();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .max_height(220.0)
            .show(ui, |ui| {
                for recipe in &recipes {
                    let scope = if recipe.scope == "project" {
                        cx.t("terminal.view.recipe-scope-project")
                    } else {
                        cx.t("terminal.view.recipe-scope-global")
                    };
                    let label = format!("{}  ·  {scope}", recipe.title(&locale));
                    if ui
                        .button(label)
                        .on_hover_text(recipe.command.clone())
                        .clicked()
                    {
                        chosen = Some(recipe.clone());
                    }
                }
            });
        if let Some(recipe) = chosen {
            match crate::recipe_trust::approve(cx, &recipe) {
                Ok(()) => match self
                    .core
                    .with_hub(|hub| hub.run_recipe(&recipe, &settings))
                    .unwrap_or_else(Err)
                {
                    Ok(()) => self.error = None,
                    Err(error) => self.error = Some(error),
                },
                Err(error) => {
                    if recipe.scope == "project" {
                        let _ = self
                            .core
                            .with_hub(|hub| hub.pending_trust_recipe = Some(recipe));
                    }
                    self.error = Some(error);
                }
            }
        }
        if let Some(error) = &self.error {
            ui.label(egui::RichText::new(error).color(egui::Color32::LIGHT_RED));
        }
    }
}
