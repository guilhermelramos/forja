//! Estado em memória, descoberta das camadas e recarga com `notify`.

use crate::model::{
    self, ContentCommand, ContentRecipe, ContentTemplate, ContentTheme, Diagnostic, Layer,
};
use forja_kernel::{Args, Ctx};
use notify::Watcher as _;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Janela sem mudanças antes de reler os arquivos.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// Um observador de arquivos vivo enquanto o módulo existir.
pub struct Watch {
    _watcher: notify::RecommendedWatcher,
    rx: Receiver<()>,
}

/// Registro de tudo o que foi lido.
#[derive(Default)]
pub struct ContentState {
    pub loaded: bool,
    pub project: Option<PathBuf>,
    pub commands: BTreeMap<String, ContentCommand>,
    pub themes: BTreeMap<String, ContentTheme>,
    pub templates: BTreeMap<String, ContentTemplate>,
    pub recipes: BTreeMap<String, ContentRecipe>,
    pub diagnostics: Vec<Diagnostic>,
    pub revision: u64,
    pub watch: Option<Watch>,
    pub watched: Vec<PathBuf>,
    pub dirty_at: Option<Instant>,
    pub last_bench_check: Option<Instant>,
}

/// Estado compartilhado do módulo.
pub struct ContentCore {
    pub state: Mutex<ContentState>,
}

impl ContentCore {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ContentState::default()),
        }
    }

    pub fn with<T>(&self, run: impl FnOnce(&mut ContentState) -> T) -> Option<T> {
        self.state.lock().ok().map(|mut state| run(&mut state))
    }
}

impl Default for ContentCore {
    fn default() -> Self {
        Self::new()
    }
}

/// Bancada atual, se houver.
pub fn current_project(cx: &mut Ctx<'_>) -> Option<PathBuf> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("path")
                .and_then(serde_json::Value::as_str)
                .map(PathBuf::from)
        })
}

/// Garante que o estado corresponde à bancada atual.
pub fn ensure(core: &ContentCore, cx: &mut Ctx<'_>, force: bool) {
    let project = current_project(cx);
    let reload = {
        let Ok(mut state) = core.state.lock() else {
            return;
        };
        let changed = state.project != project;
        state.last_bench_check = Some(Instant::now());
        force || !state.loaded || changed
    };
    if reload {
        load(core, cx, project);
    }
    ensure_watcher(core, cx);
    tick(core, cx);
}

/// Lê todas as camadas para `project`.
pub fn load(core: &ContentCore, cx: &mut Ctx<'_>, project: Option<PathBuf>) {
    let project_for_keymap = project.clone();
    // A failed read or parse keeps the previous version of that one file alive.
    let (previous_commands, previous_themes, previous_templates, previous_recipes) = core
        .with(|state| {
            (
                state.commands.clone(),
                state.themes.clone(),
                state.templates.clone(),
                state.recipes.clone(),
            )
        })
        .unwrap_or_default();
    let mut commands: BTreeMap<String, ContentCommand> = BTreeMap::new();
    let mut themes: BTreeMap<String, ContentTheme> = BTreeMap::new();
    let mut templates: BTreeMap<String, ContentTemplate> = BTreeMap::new();
    let mut recipes: BTreeMap<String, ContentRecipe> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let user = cx.user_dir().to_path_buf();
    let mut roots: Vec<(Layer, PathBuf)> = Vec::new();
    if let Some(root) = model::builtin_root() {
        roots.push((Layer::Builtin, root));
    }
    roots.push((Layer::User, user));
    if let Some(project) = &project {
        roots.push((Layer::Project, project.join(".forja")));
    }

    for (layer, root) in &roots {
        for path in model::files_in(root, "commands", "md") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match model::parse_command(&path, *layer, &text) {
                    Ok(command) => {
                        if let Some(previous) = commands.get(&command.id) {
                            if previous.layer == command.layer {
                                diagnostics.push(Diagnostic::new(
                                    &path,
                                    *layer,
                                    format!("command `{}` repeats in this layer", command.id),
                                ));
                            }
                        }
                        commands.insert(command.id.clone(), command);
                    }
                    Err(message) => {
                        diagnostics.push(Diagnostic::new(&path, *layer, message));
                        if let Some(previous) = previous_commands
                            .values()
                            .find(|item| item.source_path == path.display().to_string())
                        {
                            commands.insert(previous.id.clone(), previous.clone());
                        }
                    }
                },
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, *layer, error.to_string()));
                    if let Some(previous) = previous_commands
                        .values()
                        .find(|item| item.source_path == path.display().to_string())
                    {
                        commands.insert(previous.id.clone(), previous.clone());
                    }
                }
            }
        }
        for path in model::files_in(root, "themes", "toml") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match model::parse_theme(&path, *layer, &text) {
                    Ok(theme) => {
                        if theme.kind == "unknown" {
                            diagnostics.push(Diagnostic::new(
                                &path,
                                *layer,
                                "theme has neither a terminal palette nor interface colors",
                            ));
                            if let Some(previous) = previous_themes
                                .values()
                                .find(|item| item.source_path == path.display().to_string())
                            {
                                themes.insert(previous.id.clone(), previous.clone());
                            }
                        } else {
                            themes.insert(theme.id.clone(), theme);
                        }
                    }
                    Err(message) => {
                        diagnostics.push(Diagnostic::new(&path, *layer, message));
                        if let Some(previous) = previous_themes
                            .values()
                            .find(|item| item.source_path == path.display().to_string())
                        {
                            themes.insert(previous.id.clone(), previous.clone());
                        }
                    }
                },
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, *layer, error.to_string()));
                    if let Some(previous) = previous_themes
                        .values()
                        .find(|item| item.source_path == path.display().to_string())
                    {
                        themes.insert(previous.id.clone(), previous.clone());
                    }
                }
            }
        }
        for path in model::files_in(root, "templates", "md") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match model::parse_template(&path, *layer, &text) {
                    Ok(template) => {
                        templates.insert(template.id.clone(), template);
                    }
                    Err(message) => {
                        diagnostics.push(Diagnostic::new(&path, *layer, message));
                        if let Some(previous) = previous_templates
                            .values()
                            .find(|item| item.source_path == path.display().to_string())
                        {
                            templates.insert(previous.id.clone(), previous.clone());
                        }
                    }
                },
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, *layer, error.to_string()));
                    if let Some(previous) = previous_templates
                        .values()
                        .find(|item| item.source_path == path.display().to_string())
                    {
                        templates.insert(previous.id.clone(), previous.clone());
                    }
                }
            }
        }
        for path in model::files_in(root, "recipes", "toml") {
            match std::fs::read_to_string(&path) {
                Ok(text) => match model::parse_recipes(&path, *layer, &text) {
                    Ok(items) => {
                        for recipe in items {
                            recipes.insert(recipe.id.clone(), recipe);
                        }
                    }
                    Err(message) => {
                        diagnostics.push(Diagnostic::new(&path, *layer, message));
                        for previous in previous_recipes
                            .values()
                            .filter(|item| item.source_path == path.display().to_string())
                        {
                            recipes.insert(previous.id.clone(), previous.clone());
                        }
                    }
                },
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, *layer, error.to_string()));
                    for previous in previous_recipes
                        .values()
                        .filter(|item| item.source_path == path.display().to_string())
                    {
                        recipes.insert(previous.id.clone(), previous.clone());
                    }
                }
            }
        }
    }

    if let Ok(mut state) = core.state.lock() {
        state.loaded = true;
        state.project = project;
        state.commands = commands;
        state.themes = themes;
        state.templates = templates;
        state.recipes = recipes;
        state.diagnostics = diagnostics;
        state.revision = state.revision.wrapping_add(1);
    }
    cx.reload_keymap(project_for_keymap.as_deref());
}

fn watch_roots(cx: &Ctx<'_>, project: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = vec![
        cx.user_dir().join("commands"),
        cx.user_dir().join("themes"),
        cx.user_dir().join("templates"),
        cx.user_dir().join("recipes"),
    ];
    if let Some(project) = project {
        let root = project.join(".forja");
        roots.push(root.join("commands"));
        roots.push(root.join("themes"));
        roots.push(root.join("templates"));
        roots.push(root.join("recipes"));
    }
    roots.retain(|root| root.is_dir());
    roots
}

fn ensure_watcher(core: &ContentCore, cx: &mut Ctx<'_>) {
    let enabled = cx
        .setting("content.reload.watch")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    if !enabled {
        return;
    }
    let project = core.with(|state| state.project.clone()).flatten();
    let roots = watch_roots(cx, project.as_deref());
    let current = core
        .with(|state| state.watched.clone())
        .unwrap_or_default();
    if roots.is_empty() || roots == current {
        return;
    }
    let (tx, rx) = channel();
    let mut watcher = match notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        if result.is_ok() {
            let _ = tx.send(());
        }
    }) {
        Ok(watcher) => watcher,
        Err(_) => return,
    };
    for root in &roots {
        let _ = watcher.watch(root, notify::RecursiveMode::Recursive);
    }
    if let Ok(mut state) = core.state.lock() {
        state.watch = Some(Watch {
            _watcher: watcher,
            rx,
        });
        state.watched = roots;
    }
}

/// Drena o observador e recarrega depois de 300 ms sem mudanças.
pub fn tick(core: &ContentCore, cx: &mut Ctx<'_>) {
    let mut should_reload = false;
    if let Ok(mut state) = core.state.lock() {
        let mut touched = false;
        if let Some(watch) = &state.watch {
            while watch.rx.try_recv().is_ok() {
                touched = true;
            }
        }
        if touched {
            state.dirty_at = Some(Instant::now());
        }
        if state
            .dirty_at
            .is_some_and(|at| at.elapsed() >= DEBOUNCE)
        {
            state.dirty_at = None;
            should_reload = true;
        }
    }
    if should_reload {
        let project = core.with(|state| state.project.clone()).flatten();
        load(core, cx, project);
        cx.request_repaint();
    }
}


