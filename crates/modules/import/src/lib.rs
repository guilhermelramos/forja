//! Módulo `import`: importador opcional de dados do Harbor 0.1.
//!
//! Lê `session.json` e `settings.json` da pasta do Harbor (somente leitura) e
//! mostra o que pode vir: projetos, bancadas (worktrees), quick commands como
//! receitas, notas, destinos SSH, tema e fonte. O usuário escolhe os itens.
//!
//! Regras:
//! - nada de segredos: só os campos listados em `harbor.rs`;
//! - o Harbor nunca é escrito;
//! - IO roda em `cx.spawn`; projetos, SSH e ajustes são feitos pelos comandos
//!   dos módulos donos (`project.*`, `remote.ssh.*`, `set_setting`);
//! - a decisão (importado ou pulado) fica no store e o passo não volta;
//! - importar de novo não duplica: ids já importados e projetos já registrados
//!   são pulados.
//!
//! Comandos: `import.harbor.status`, `import.harbor.preview`,
//! `import.harbor.run` (`items?`) e `import.harbor.skip`. Veja `MODULE.md`.

mod harbor;
mod plan;
mod view;

use forja_kernel::ui_contract::View;
use forja_kernel::{Args, Ctx, Module, ModuleManifest, Registrar, Slot};
use harbor::SshTarget;
use plan::{Item, Plan, ProjectAdd};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use view::SettingsView;

#[forja_kernel::module]
static IMPORT: fn() -> Box<dyn Module> = || {
    Box::new(ImportModule::new(
        harbor::default_source_dir(),
        forja_kernel::paths::default_user_dir(),
    ))
};

/// Pastas usadas pelo importador. Testes passam pastas temporárias.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Pasta do Harbor; `None` quando não há como localizá-la.
    pub source: Option<PathBuf>,
    /// Pasta de conteúdo da Forja (`~/.forja` por padrão).
    pub user_dir: PathBuf,
}

/// Decisão registrada no store.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Decision {
    /// `imported` ou `skipped`.
    pub outcome: String,
    pub at: String,
}

/// Estado persistido: a decisão e os ids já importados.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Record {
    pub decision: Option<Decision>,
    pub imported: Vec<String>,
}

/// Contagens da última importação, para a tela.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub projects: usize,
    pub benches: usize,
    pub recipes: usize,
    pub notes: usize,
    pub ssh: usize,
    pub settings: usize,
    /// Itens que já estavam importados e foram pulados.
    pub skipped: usize,
    /// Mensagens de erro de cada passo (texto de comando, não de interface).
    pub errors: Vec<String>,
}

#[derive(Default)]
struct Snapshot {
    loaded: bool,
    busy: bool,
    available: bool,
    record: Record,
    items: Vec<Item>,
    /// Chave i18n do erro de leitura, quando existe.
    error: Option<String>,
    summary: Option<Summary>,
}

#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Snapshot>>);

impl Shared {
    fn with<T>(&self, f: impl FnOnce(&mut Snapshot) -> T) -> Option<T> {
        self.0.lock().ok().map(|mut snapshot| f(&mut snapshot))
    }

    /// Marca como ocupado; `false` quando já há uma operação em curso.
    fn start(&self) -> bool {
        self.with(|s| {
            if s.busy {
                false
            } else {
                s.busy = true;
                true
            }
        })
        .unwrap_or(false)
    }

    fn status(&self) -> Value {
        self.with(|s| {
            let decided = s.record.decision.clone();
            let items: Vec<Value> = s
                .items
                .iter()
                .map(|item| {
                    let mut value = serde_json::to_value(item).unwrap_or(Value::Null);
                    if let Value::Object(map) = &mut value {
                        map.insert(
                            "imported".to_owned(),
                            json!(s.record.imported.contains(&item.id)),
                        );
                    }
                    value
                })
                .collect();
            json!({
                "loaded": s.loaded,
                "busy": s.busy,
                "available": s.available,
                "pending": s.available && decided.is_none(),
                "decided": decided.map(|d| d.outcome),
                "error": s.error,
                "items": items,
                "summary": s.summary,
            })
        })
        .unwrap_or_else(|| json!({ "loaded": false, "busy": false, "available": false, "pending": false }))
    }
}

pub struct ImportModule {
    paths: Paths,
    state: Shared,
}

impl ImportModule {
    /// Cria o módulo com as pastas dadas.
    pub fn new(source: Option<PathBuf>, user_dir: PathBuf) -> Self {
        Self {
            paths: Paths { source, user_dir },
            state: Shared::default(),
        }
    }
}

impl Module for ImportModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("import", "import.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(include_str!("../i18n/en.ftl"), include_str!("../i18n/pt-BR.ftl"));
        r.keymap(include_str!("../keymap.toml"));

        let status_state = self.state.clone();
        r.command("import.harbor.status", "import.command.status", move |_cx, _args| {
            Ok(status_state.status())
        });

        let preview_state = self.state.clone();
        let preview_paths = self.paths.clone();
        r.command("import.harbor.preview", "import.command.preview", move |cx, _args| {
            preview(cx, &preview_state, &preview_paths);
            Ok(json!({ "started": true }))
        });

        let run_state = self.state.clone();
        let run_paths = self.paths.clone();
        r.command("import.harbor.run", "import.command.run", move |cx, args| {
            let selected = selected_ids(&args);
            if !run_state.start() {
                return Ok(json!({ "started": false, "reason": "busy" }));
            }
            run(cx, &run_state, &run_paths, selected);
            Ok(json!({ "started": true }))
        });

        let skip_state = self.state.clone();
        r.command("import.harbor.skip", "import.command.skip", move |cx, _args| {
            if !skip_state.start() {
                return Ok(json!({ "started": false, "reason": "busy" }));
            }
            skip(cx, &skip_state);
            Ok(json!({ "started": true }))
        });

        let settings_state = self.state.clone();
        r.view(
            Slot::SettingsSection,
            "import.settings",
            "import.settings.title",
            40,
            move || Box::new(SettingsView::new(settings_state.clone())) as Box<dyn View>,
        );

        for key in [
            "import.kind.project",
            "import.kind.worktree",
            "import.kind.recipe",
            "import.kind.notes",
            "import.kind.ssh",
            "import.kind.theme",
            "import.kind.font",
            "import.error.corrupt",
            "import.error.unreadable",
            "import.result.line",
            "import.settings.hint",
            "import.settings.run",
            "import.settings.none",
            "import.settings.loading",
            "import.settings.imported",
            "import.settings.skipped",
        ] {
            r.uses_i18n(key);
        }
    }
}

fn selected_ids(args: &Args) -> Option<Vec<String>> {
    args.get("items").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|value| value.as_str().map(str::to_owned))
            .collect()
    })
}

/// Carrega o Harbor e o store em segundo plano.
fn preview(cx: &mut Ctx<'_>, shared: &Shared, paths: &Paths) {
    if !shared.start() {
        return;
    }
    let store = cx.store().path();
    let work_paths = paths.clone();
    let done_shared = shared.clone();
    let _ = cx.spawn(
        move || load(&work_paths, &store),
        move |_cx, loaded| {
            done_shared.with(|s| {
                s.loaded = true;
                s.busy = false;
                s.available = loaded.available;
                s.record = loaded.record;
                s.items = loaded.items;
                s.error = loaded.error;
            });
        },
    );
}

struct Loaded {
    available: bool,
    record: Record,
    items: Vec<Item>,
    error: Option<String>,
}

fn load(paths: &Paths, store: &Path) -> Loaded {
    let record = read_record(store);
    let Some(dir) = paths.source.as_deref() else {
        return Loaded { available: false, record, items: Vec::new(), error: None };
    };
    let (session, session_error) = split(harbor::read_session(dir));
    let (settings, settings_error) = split(harbor::read_settings(dir));
    let available = session.is_some() || session_error.is_some();
    let items = if available {
        plan::items(session.as_ref(), settings.as_ref())
    } else {
        Vec::new()
    };
    let error = session_error.or(settings_error).map(|message| error_key(&message));
    Loaded { available, record, items, error }
}

fn split<T>(result: Result<Option<T>, String>) -> (Option<T>, Option<String>) {
    match result {
        Ok(value) => (value, None),
        Err(message) => (None, Some(message)),
    }
}

fn error_key(message: &str) -> String {
    if message.starts_with("corrupt") {
        "import.error.corrupt".to_owned()
    } else {
        "import.error.unreadable".to_owned()
    }
}

/// O que a parte em segundo plano devolve para a interface.
struct Applied {
    store: PathBuf,
    plan: Plan,
    record: Record,
    recipes: usize,
    notes: bool,
    benches: usize,
    skipped: usize,
    errors: Vec<String>,
}

fn run(cx: &mut Ctx<'_>, shared: &Shared, paths: &Paths, selected: Option<Vec<String>>) {
    let store = cx.store().path();
    let work_paths = paths.clone();
    let done_shared = shared.clone();
    let _ = cx.spawn(
        move || apply(&work_paths, store, selected),
        move |cx, applied| finish(cx, &done_shared, applied),
    );
}

/// Parte em segundo plano: lê de novo o Harbor e grava só em `user_dir`.
fn apply(paths: &Paths, store: PathBuf, selected: Option<Vec<String>>) -> Applied {
    let record = read_record(&store);
    let mut errors = Vec::new();
    let (session, settings) = match paths.source.as_deref() {
        Some(dir) => {
            let (session, error) = split(harbor::read_session(dir));
            errors.extend(error);
            let (settings, error) = split(harbor::read_settings(dir));
            errors.extend(error);
            (session, settings)
        }
        None => (None, None),
    };
    let all = plan::items(session.as_ref(), settings.as_ref());
    let already = record.imported.clone();
    let skipped = all.iter().filter(|item| already.contains(&item.id)).count();
    let chosen: Vec<String> = match selected {
        Some(ids) => ids
            .into_iter()
            .filter(|id| all.iter().any(|item| &item.id == id) && !already.contains(id))
            .collect(),
        None => all
            .iter()
            .filter(|item| !already.contains(&item.id))
            .map(|item| item.id.clone())
            .collect(),
    };
    let mut plan = plan::plan(session.as_ref(), settings.as_ref(), &chosen);

    let recipes = match plan::write_recipes(&paths.user_dir, &plan.recipes) {
        Ok(count) => count,
        Err(error) => {
            errors.push(error);
            plan.imported.retain(|id| !id.starts_with("recipe:"));
            0
        }
    };
    let notes = match plan.notes.as_deref() {
        Some(text) => match plan::write_notes(&paths.user_dir, text) {
            Ok(written) => written,
            Err(error) => {
                errors.push(error);
                plan.imported.retain(|id| id != "notes");
                false
            }
        },
        None => false,
    };
    let benches = plan.worktrees.iter().filter(|path| path.is_dir()).count();
    Applied { store, plan, record, recipes, notes, benches, skipped, errors }
}

/// Parte na interface: chama os módulos donos e grava a decisão. Só entram no
/// store os itens cujo comando deu certo; os que falharam podem ser tentados de novo.
fn finish(cx: &mut Ctx<'_>, shared: &Shared, applied: Applied) {
    let Applied { store, plan, mut record, recipes, notes, benches, skipped, mut errors } = applied;
    let mut summary = Summary {
        recipes,
        notes: usize::from(notes),
        benches,
        skipped,
        ..Summary::default()
    };
    let mut failed: Vec<String> = Vec::new();
    let (projects, failed_projects) = add_projects(cx, &plan.projects, &mut errors);
    summary.projects = projects;
    failed.extend(failed_projects.iter().map(|path| format!("project:{path}")));
    let (ssh, failed_ssh) = add_ssh(cx, &plan.ssh, &mut errors);
    summary.ssh = ssh;
    failed.extend(failed_ssh);
    for (key, value) in &plan.settings {
        match cx.set_setting(key, value.clone()) {
            Ok(()) => summary.settings += 1,
            Err(error) => {
                errors.push(error.to_string());
                failed.push(setting_item(key).to_owned());
            }
        }
    }
    summary.errors = errors;

    for id in &plan.imported {
        if !failed.contains(id) && !record.imported.contains(id) {
            record.imported.push(id.clone());
        }
    }
    // Desmarcar tudo equivale a pular: a decisão não diz "importado" sem nada importado.
    let outcome = if plan.imported.is_empty() { "skipped" } else { "imported" };
    record.decision = Some(Decision {
        outcome: outcome.to_owned(),
        at: now(),
    });
    let to_store = record.clone();
    shared.with(|s| {
        s.record = record;
        s.busy = false;
        s.summary = Some(summary);
    });
    let done_shared = shared.clone();
    let _ = cx.spawn(
        move || write_record(&store, &to_store),
        move |_cx, result| {
            if let Err(error) = result {
                done_shared.with(|s| {
                    if let Some(summary) = s.summary.as_mut() {
                        summary.errors.push(error);
                    }
                });
            }
        },
    );
}

/// Item do plano que corresponde a uma chave de configuração.
fn setting_item(key: &str) -> &'static str {
    match key {
        "appearance.theme.mode" => "theme",
        _ => "font",
    }
}

/// Registra os projetos que ainda não existem. Devolve quantos foram criados e
/// os caminhos que falharam.
fn add_projects(
    cx: &mut Ctx<'_>,
    projects: &[ProjectAdd],
    errors: &mut Vec<String>,
) -> (usize, Vec<String>) {
    if projects.is_empty() {
        return (0, Vec::new());
    }
    let existing = match cx.run_command("project.list", Args::new()) {
        Ok(value) => value
            .get("projects")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|p| p.get("path").and_then(Value::as_str).map(same_path_key))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default(),
        Err(error) => {
            errors.push(error.to_string());
            return (0, projects.iter().map(|p| p.path.clone()).collect());
        }
    };
    let mut added = 0;
    let mut failed = Vec::new();
    for project in projects {
        if existing.contains(&same_path_key(&project.path)) {
            continue;
        }
        let mut args = serde_json::Map::new();
        args.insert("path".to_owned(), json!(project.path));
        args.insert("name".to_owned(), json!(project.name));
        if let Some(icon) = &project.icon {
            args.insert("icon".to_owned(), json!(icon));
        }
        if let Some(color) = &project.color {
            args.insert("color".to_owned(), json!(color));
        }
        match cx.run_command("project.add", Args::from_map(args)) {
            Ok(_) => added += 1,
            Err(error) => {
                errors.push(format!("{}: {error}", project.path));
                failed.push(project.path.clone());
            }
        }
    }
    (added, failed)
}

/// Grava os destinos SSH que ainda não existem (mesmo host e rótulo). Devolve
/// quantos foram salvos e os ids dos que falharam.
fn add_ssh(
    cx: &mut Ctx<'_>,
    targets: &[(String, SshTarget)],
    errors: &mut Vec<String>,
) -> (usize, Vec<String>) {
    if targets.is_empty() {
        return (0, Vec::new());
    }
    let existing: Vec<(String, String)> = match cx.run_command("remote.ssh.list", Args::new()) {
        Ok(value) => value
            .get("targets")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .map(|t| {
                        (
                            t.get("host").and_then(Value::as_str).unwrap_or("").to_owned(),
                            t.get("label").and_then(Value::as_str).unwrap_or("").to_owned(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Err(error) => {
            errors.push(error.to_string());
            return (0, targets.iter().map(|(id, _)| id.clone()).collect());
        }
    };
    let mut saved = 0;
    let mut failed = Vec::new();
    for (id, target) in targets {
        if existing.iter().any(|(host, label)| host == &target.host && label == &target.label) {
            continue;
        }
        let forwards: Vec<Value> = target
            .forwards
            .iter()
            .map(|f| {
                json!({
                    "local_port": f.local_port,
                    "remote_host": f.remote_host,
                    "remote_port": f.remote_port,
                    "active": false,
                })
            })
            .collect();
        let value = json!({
            "id": "",
            "label": target.label,
            "host": target.host,
            "user": target.user,
            "port": target.port,
            "identity_file": target.identity_file,
            "proxy_jump": target.proxy_jump,
            "forwards": forwards,
        });
        let mut args = serde_json::Map::new();
        args.insert("target".to_owned(), value);
        match cx.run_command("remote.ssh.save", Args::from_map(args)) {
            Ok(result) if result.get("saved") == Some(&Value::Bool(true)) => saved += 1,
            Ok(result) => {
                errors.push(
                    result
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("remote-error-invalid")
                        .to_owned(),
                );
                failed.push(id.clone());
            }
            Err(error) => {
                errors.push(error.to_string());
                failed.push(id.clone());
            }
        }
    }
    (saved, failed)
}

/// Pular: registra a decisão sem importar nada.
fn skip(cx: &mut Ctx<'_>, shared: &Shared) {
    let store = cx.store().path();
    let done_shared = shared.clone();
    let _ = cx.spawn(
        move || {
            let mut record = read_record(&store);
            if record.decision.is_none() {
                record.decision = Some(Decision {
                    outcome: "skipped".to_owned(),
                    at: now(),
                });
            }
            let result = write_record(&store, &record);
            (record, result)
        },
        move |_cx, (record, result)| {
            done_shared.with(|s| {
                s.record = record;
                s.busy = false;
                if let Err(error) = result {
                    s.summary = Some(Summary {
                        errors: vec![error],
                        ..Summary::default()
                    });
                }
            });
        },
    );
}

fn read_record(store: &Path) -> Record {
    std::fs::read(store)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write_record(store: &Path, record: &Record) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(store, &bytes).map_err(|error| error.to_string())
}

fn same_path_key(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_ascii_lowercase()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-import-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("harbor")
    }

    fn runtime(data: &Path, user: &Path, source: Option<PathBuf>) -> Runtime {
        Runtime::new(
            vec![Box::new(ImportModule::new(source, user.to_path_buf()))],
            RuntimeOptions {
                data_dir: Some(data.to_path_buf()),
                user_dir: Some(user.to_path_buf()),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("module registers cleanly")
    }

    #[test]
    fn contract_holds() {
        let data = temp_dir("contract-data");
        let user = temp_dir("contract-user");
        let runtime = runtime(&data, &user, None);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn load_maps_fixture_items_without_writing_anything() {
        let data = temp_dir("load-data");
        let user = temp_dir("load-user");
        let store = data.join("import.json");
        let before = std::fs::read(fixture_dir().join("session.json")).unwrap();
        let paths = Paths { source: Some(fixture_dir()), user_dir: user.clone() };
        let loaded = load(&paths, &store);
        assert!(loaded.available);
        assert!(loaded.error.is_none());
        assert_eq!(loaded.record, Record::default());
        assert!(loaded.items.iter().any(|item| item.id == "recipe:1"));
        assert!(loaded.items.iter().any(|item| item.id == "theme"));
        assert_eq!(std::fs::read(fixture_dir().join("session.json")).unwrap(), before);
        assert!(!user.join("recipes").exists());
        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn corrupt_session_is_reported_and_nothing_is_written() {
        let data = temp_dir("corrupt-data");
        let user = temp_dir("corrupt-user");
        let source = temp_dir("corrupt-source");
        std::fs::write(source.join(harbor::SESSION_FILE), b"{ quebrado").unwrap();
        let paths = Paths { source: Some(source.clone()), user_dir: user.clone() };
        let loaded = load(&paths, &data.join("import.json"));
        assert!(loaded.available, "the file exists, so the step still asks");
        assert_eq!(loaded.error.as_deref(), Some("import.error.corrupt"));
        assert!(loaded.items.is_empty());

        let applied = apply(&paths, data.join("import.json"), None);
        assert_eq!(applied.recipes, 0);
        assert!(!applied.notes);
        assert!(!user.join("recipes").exists());
        assert!(applied.errors.iter().any(|e| e.starts_with("corrupt")));
        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn running_twice_does_not_duplicate_anything() {
        let data = temp_dir("twice-data");
        let user = temp_dir("twice-user");
        let store = data.join("import.json");
        let paths = Paths { source: Some(fixture_dir()), user_dir: user.clone() };

        let first = apply(&paths, store.clone(), None);
        assert_eq!(first.recipes, 2);
        assert!(first.notes);
        assert!(first.errors.is_empty(), "errors: {:?}", first.errors);
        // O que `finish` gravaria quando nenhum comando falha.
        let mut record = first.record.clone();
        record.imported = first.plan.imported.clone();
        write_record(&store, &record).unwrap();

        let second = apply(&paths, store.clone(), None);
        assert_eq!(second.recipes, 0, "nothing new to write");
        assert!(second.plan.projects.is_empty());
        assert!(second.plan.recipes.is_empty());
        assert_eq!(second.skipped, first.plan.imported.len());

        let text = std::fs::read_to_string(user.join(plan::RECIPES_DIR).join(plan::RECIPES_FILE)).unwrap();
        assert_eq!(text.matches("[[recipe]]").count(), 2);
        let notes = std::fs::read_to_string(user.join(plan::NOTES_FILE)).unwrap();
        assert!(notes.contains("Notas de exemplo"));

        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn harbor_source_is_never_written() {
        let data = temp_dir("readonly-data");
        let user = temp_dir("readonly-user");
        let source = temp_dir("readonly-source");
        for name in [harbor::SESSION_FILE, harbor::SETTINGS_FILE] {
            std::fs::copy(fixture_dir().join(name), source.join(name)).unwrap();
        }
        let before: Vec<Vec<u8>> = [harbor::SESSION_FILE, harbor::SETTINGS_FILE]
            .iter()
            .map(|name| std::fs::read(source.join(name)).unwrap())
            .collect();
        let paths = Paths { source: Some(source.clone()), user_dir: user.clone() };
        let _ = apply(&paths, data.join("import.json"), None);
        let after: Vec<Vec<u8>> = [harbor::SESSION_FILE, harbor::SETTINGS_FILE]
            .iter()
            .map(|name| std::fs::read(source.join(name)).unwrap())
            .collect();
        assert_eq!(before, after);
        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
        let _ = std::fs::remove_dir_all(source);
    }

    #[test]
    fn status_without_a_source_is_quiet() {
        let data = temp_dir("status-data");
        let user = temp_dir("status-user");
        let mut runtime = runtime(&data, &user, None);
        let value = runtime.run_command("import.harbor.status", Args::new()).unwrap();
        assert_eq!(value["pending"], false);
        assert_eq!(value["available"], false);
        let _ = std::fs::remove_dir_all(data);
        let _ = std::fs::remove_dir_all(user);
    }

    #[test]
    fn same_path_key_ignores_separator_and_case() {
        assert_eq!(same_path_key("C:\\Exemplo\\Api\\"), same_path_key("c:/exemplo/api"));
    }
}
