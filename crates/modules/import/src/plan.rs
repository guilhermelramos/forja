//! Regras puras da importação: o que aparece no passo, o que cada item vira
//! e como os arquivos da Forja são escritos (receitas e notas em `~/.forja`).

use crate::harbor::{Scope, Session, Settings, SshTarget};
use forja_kernel::SettingValue;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Pasta das receitas do usuário, dentro da pasta de conteúdo.
pub const RECIPES_DIR: &str = "recipes";
/// Arquivo que recebe os quick commands importados.
pub const RECIPES_FILE: &str = "imported.toml";
/// Arquivo que recebe as notas importadas, na pasta de conteúdo.
pub const NOTES_FILE: &str = "imported-notes.md";

/// Tipo de um item mostrado no passo. A interface traduz pelo `import.kind.<tipo>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Project,
    Worktree,
    Recipe,
    Notes,
    Ssh,
    Theme,
    Font,
}

/// Um item que pode ser importado. `id` é estável e vale para o store.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub label: String,
    pub detail: String,
}

/// Lista o que existe nos arquivos, na ordem em que o passo mostra.
pub fn items(session: Option<&Session>, settings: Option<&Settings>) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(session) = session {
        for project in &session.projects {
            let path = display(&project.path);
            out.push(Item {
                id: format!("project:{path}"),
                kind: Kind::Project,
                label: first_non_empty(&[&project.display_name, &project.name, &path]),
                detail: path.clone(),
            });
            for workspace in &project.workspaces {
                let path = display(&workspace.path);
                out.push(Item {
                    id: format!("worktree:{path}"),
                    kind: Kind::Worktree,
                    label: first_non_empty(&[&workspace.name, &path]),
                    detail: path,
                });
            }
        }
        if !session.notes.trim().is_empty() {
            let lines = session.notes.lines().count();
            out.push(Item {
                id: "notes".to_owned(),
                kind: Kind::Notes,
                label: first_non_empty(&[session.notes.lines().next().unwrap_or("")]),
                detail: lines.to_string(),
            });
        }
    }
    if let Some(settings) = settings {
        for (index, command) in settings.quick_commands.entries.iter().enumerate() {
            out.push(Item {
                id: format!("recipe:{index}"),
                kind: Kind::Recipe,
                label: first_non_empty(&[&command.name, &command.command]),
                detail: command.command.clone(),
            });
        }
        for (index, target) in settings.remote.ssh_targets.iter().enumerate() {
            out.push(Item {
                id: format!("ssh:{index}"),
                kind: Kind::Ssh,
                label: first_non_empty(&[&target.label, &target.host]),
                detail: target.host.clone(),
            });
        }
        if let Some(dark) = settings.appearance.dark {
            let mode = if dark { "dark" } else { "light" };
            out.push(Item {
                id: "theme".to_owned(),
                kind: Kind::Theme,
                label: mode.to_owned(),
                detail: mode.to_owned(),
            });
        }
        if let Some(size) = settings.terminal.font_size {
            let size = code_size(size);
            out.push(Item {
                id: "font".to_owned(),
                kind: Kind::Font,
                label: size.to_string(),
                detail: size.to_string(),
            });
        }
    }
    out
}

/// Projeto a registrar com `project.add`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectAdd {
    pub path: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

/// Receita de terminal a gravar em `recipes/imported.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeEntry {
    pub id: String,
    pub name: String,
    pub command: String,
    pub project_scope: bool,
    pub enter: bool,
}

/// O que a importação faz para os itens escolhidos.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub projects: Vec<ProjectAdd>,
    /// Destinos SSH com o id do item (`ssh:<índice>`).
    pub ssh: Vec<(String, SshTarget)>,
    pub settings: Vec<(&'static str, SettingValue)>,
    pub recipes: Vec<RecipeEntry>,
    pub notes: Option<String>,
    pub worktrees: Vec<PathBuf>,
    pub imported: Vec<String>,
}

/// Monta o plano para os ids escolhidos. Ids desconhecidos são ignorados.
/// Um worktree escolhido também registra o projeto dele, para o bench
/// reconhecê-lo.
pub fn plan(session: Option<&Session>, settings: Option<&Settings>, selected: &[String]) -> Plan {
    let mut plan = Plan::default();
    let chosen = |id: &str| selected.iter().any(|s| s == id);
    if let Some(session) = session {
        for project in &session.projects {
            let path = display(&project.path);
            let project_id = format!("project:{path}");
            let worktree_chosen = project
                .workspaces
                .iter()
                .any(|w| chosen(&format!("worktree:{}", display(&w.path))));
            if chosen(&project_id) || worktree_chosen {
                push_project(&mut plan, project_add(project));
                if chosen(&project_id) {
                    plan.imported.push(project_id);
                }
            }
            for workspace in &project.workspaces {
                let path = display(&workspace.path);
                let id = format!("worktree:{path}");
                if chosen(&id) {
                    plan.worktrees.push(PathBuf::from(&path));
                    plan.imported.push(id);
                }
            }
        }
        if chosen("notes") && !session.notes.trim().is_empty() {
            plan.notes = Some(session.notes.clone());
            plan.imported.push("notes".to_owned());
        }
    }
    if let Some(settings) = settings {
        for (index, command) in settings.quick_commands.entries.iter().enumerate() {
            let id = format!("recipe:{index}");
            if chosen(&id) {
                plan.recipes.push(RecipeEntry {
                    id: format!("imported-{}-{}", index, slug(&command.name)),
                    name: first_non_empty(&[&command.name, &command.command]),
                    command: command.command.clone(),
                    project_scope: matches!(command.scope, Scope::Project(_)),
                    enter: command.append_enter,
                });
                plan.imported.push(id);
            }
        }
        for (index, target) in settings.remote.ssh_targets.iter().enumerate() {
            let id = format!("ssh:{index}");
            if chosen(&id) {
                plan.ssh.push((id.clone(), target.clone()));
                plan.imported.push(id);
            }
        }
        if chosen("theme") {
            if let Some(dark) = settings.appearance.dark {
                let mode = if dark { "dark" } else { "light" };
                plan.settings
                    .push(("appearance.theme.mode", SettingValue::Text(mode.to_owned())));
                plan.imported.push("theme".to_owned());
            }
        }
        if chosen("font") {
            if let Some(size) = settings.terminal.font_size {
                let size = i64::from(code_size(size));
                plan.settings
                    .push(("appearance.ui.code_size", SettingValue::Integer(size)));
                plan.imported.push("font".to_owned());
            }
        }
    }
    plan
}

fn push_project(plan: &mut Plan, project: ProjectAdd) {
    if !plan.projects.iter().any(|p| p.path == project.path) {
        plan.projects.push(project);
    }
}

fn project_add(project: &crate::harbor::Project) -> ProjectAdd {
    let path = display(&project.path);
    let name = first_non_empty(&[&project.display_name, &project.name, &path]);
    ProjectAdd {
        path,
        name,
        icon: non_empty(&project.icon),
        color: valid_color(&project.badge_color),
    }
}

/// Tamanho do código na Forja: 10 a 24, inteiro.
pub fn code_size(terminal_size: f64) -> u32 {
    terminal_size.round().clamp(10.0, 24.0) as u32
}

/// Cor `#RRGGBB`; qualquer outra forma é descartada.
pub fn valid_color(value: &str) -> Option<String> {
    let hex = value.strip_prefix('#')?;
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", hex.to_ascii_uppercase()))
}

/// Acrescenta as receitas importadas ao arquivo, sem repetir ids. Um arquivo
/// existente que não lê como TOML não é tocado.
pub fn write_recipes(user_dir: &Path, recipes: &[RecipeEntry]) -> Result<usize, String> {
    if recipes.is_empty() {
        return Ok(0);
    }
    let dir = user_dir.join(RECIPES_DIR);
    let path = dir.join(RECIPES_FILE);
    let mut root = match std::fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<toml::Table>(&text)
            .map_err(|error| format!("recipes file is not valid TOML: {error}"))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => toml::Table::new(),
        Err(error) => return Err(format!("recipes file unreadable: {error}")),
    };
    let mut list = match root.remove("recipe") {
        Some(toml::Value::Array(list)) => list,
        Some(_) => return Err("recipes file has a non-array `recipe` key".to_owned()),
        None => Vec::new(),
    };
    let known: Vec<String> = list
        .iter()
        .filter_map(|item| item.get("id").and_then(toml::Value::as_str).map(str::to_owned))
        .collect();
    let mut added = 0;
    for recipe in recipes {
        if known.contains(&recipe.id) {
            continue;
        }
        list.push(toml::Value::Table(recipe_table(recipe)));
        added += 1;
    }
    if added == 0 {
        return Ok(0);
    }
    root.insert("recipe".to_owned(), toml::Value::Array(list));
    let text = toml::to_string(&root).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(&path, text.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(added)
}

fn recipe_table(recipe: &RecipeEntry) -> toml::Table {
    let mut title = toml::Table::new();
    title.insert("pt-BR".to_owned(), toml::Value::String(recipe.name.clone()));
    title.insert("en".to_owned(), toml::Value::String(recipe.name.clone()));
    let mut table = toml::Table::new();
    table.insert("id".to_owned(), toml::Value::String(recipe.id.clone()));
    table.insert("title".to_owned(), toml::Value::Table(title));
    table.insert("command".to_owned(), toml::Value::String(recipe.command.clone()));
    let scope = if recipe.project_scope { "project" } else { "global" };
    table.insert("scope".to_owned(), toml::Value::String(scope.to_owned()));
    table.insert("enter".to_owned(), toml::Value::Boolean(recipe.enter));
    table.insert("directory".to_owned(), toml::Value::String(".".to_owned()));
    table
}

/// Grava as notas em `imported-notes.md`. Nunca sobrescreve um arquivo que já
/// existe. Devolve `true` quando gravou.
pub fn write_notes(user_dir: &Path, text: &str) -> Result<bool, String> {
    let path = user_dir.join(NOTES_FILE);
    if path.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(user_dir).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(&path, text.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(true)
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_owned())
        .unwrap_or_default()
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "command".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harbor::{Appearance, QuickCommand, QuickCommands, Remote, Terminal};
    use serde_json::json;

    fn session() -> Session {
        serde_json::from_value(json!({
            "projects": [{
                "name": "exemplo-api",
                "display_name": "API de exemplo",
                "path": "C:/exemplo/api",
                "icon": "A",
                "badge_color": "#3366cc",
                "workspaces": [{ "name": "feature", "path": "C:/exemplo/api-feature" }]
            }, {
                "name": "web",
                "path": "C:/exemplo/web",
                "badge_color": "azul"
            }],
            "notes": "uma\nduas"
        }))
        .unwrap()
    }

    fn settings() -> Settings {
        Settings {
            quick_commands: QuickCommands {
                entries: vec![
                    QuickCommand {
                        name: "Testes Rust!".to_owned(),
                        command: "cargo test".to_owned(),
                        scope: Scope::Global,
                        append_enter: true,
                    },
                    QuickCommand {
                        name: "Build".to_owned(),
                        command: "cargo build".to_owned(),
                        scope: Scope::Project(PathBuf::from("C:/exemplo/api")),
                        append_enter: false,
                    },
                ],
            },
            appearance: Appearance { dark: Some(false) },
            terminal: Terminal { font_size: Some(40.0) },
            remote: Remote {
                ssh_targets: vec![SshTarget {
                    id: "srv".to_owned(),
                    label: "Servidor".to_owned(),
                    host: "exemplo.invalid".to_owned(),
                    user: "ana".to_owned(),
                    port: 2222,
                    identity_file: String::new(),
                    proxy_jump: String::new(),
                    forwards: Vec::new(),
                }],
            },
        }
    }

    #[test]
    fn lists_every_kind_in_order() {
        let items = items(Some(&session()), Some(&settings()));
        let kinds: Vec<Kind> = items.iter().map(|item| item.kind).collect();
        assert_eq!(
            kinds,
            vec![
                Kind::Project,
                Kind::Worktree,
                Kind::Project,
                Kind::Notes,
                Kind::Recipe,
                Kind::Recipe,
                Kind::Ssh,
                Kind::Theme,
                Kind::Font,
            ]
        );
        assert_eq!(items[0].label, "API de exemplo");
        assert_eq!(items[2].label, "web", "falls back to the name");
        assert_eq!(items[3].label, "uma", "notes are labelled by their first line");
        assert_eq!(items[3].detail, "2", "notes count lines");
    }

    #[test]
    fn maps_each_item_to_the_forja_setting_or_command() {
        let plan = plan(
            Some(&session()),
            Some(&settings()),
            &[
                "project:C:/exemplo/api".to_owned(),
                "recipe:1".to_owned(),
                "ssh:0".to_owned(),
                "theme".to_owned(),
                "font".to_owned(),
                "notes".to_owned(),
            ],
        );
        assert_eq!(plan.projects.len(), 1);
        assert_eq!(plan.projects[0].name, "API de exemplo");
        assert_eq!(plan.projects[0].icon.as_deref(), Some("A"));
        assert_eq!(plan.projects[0].color.as_deref(), Some("#3366CC"));
        assert_eq!(plan.recipes.len(), 1);
        assert!(plan.recipes[0].project_scope);
        assert!(!plan.recipes[0].enter);
        assert_eq!(plan.ssh[0].1.port, 2222);
        assert!(plan
            .settings
            .contains(&("appearance.theme.mode", SettingValue::Text("light".to_owned()))));
        assert!(plan
            .settings
            .contains(&("appearance.ui.code_size", SettingValue::Integer(24))));
        assert_eq!(plan.notes.as_deref(), Some("uma\nduas"));
    }

    #[test]
    fn worktree_selection_registers_its_project() {
        let plan = plan(
            Some(&session()),
            None,
            &["worktree:C:/exemplo/api-feature".to_owned()],
        );
        assert_eq!(plan.projects.len(), 1);
        assert_eq!(plan.projects[0].path, "C:/exemplo/api");
        assert_eq!(plan.worktrees, vec![PathBuf::from("C:/exemplo/api-feature")]);
    }

    #[test]
    fn unknown_ids_and_bad_colors_are_ignored() {
        let plan = plan(Some(&session()), Some(&settings()), &["nope".to_owned()]);
        assert_eq!(plan, Plan::default());
        assert_eq!(valid_color("azul"), None);
        assert_eq!(valid_color("#12345G"), None);
    }

    #[test]
    fn recipes_are_written_once_even_when_run_twice() {
        let dir = std::env::temp_dir().join(format!("forja-import-plan-recipes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let recipes = vec![RecipeEntry {
            id: "imported-0-testes-rust".to_owned(),
            name: "Testes Rust!".to_owned(),
            command: "cargo test".to_owned(),
            project_scope: false,
            enter: true,
        }];
        assert_eq!(write_recipes(&dir, &recipes).unwrap(), 1);
        assert_eq!(write_recipes(&dir, &recipes).unwrap(), 0);
        let text = std::fs::read_to_string(dir.join(RECIPES_DIR).join(RECIPES_FILE)).unwrap();
        assert_eq!(text.matches("imported-0-testes-rust").count(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn invalid_recipes_file_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("forja-import-plan-bad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(RECIPES_DIR)).unwrap();
        let path = dir.join(RECIPES_DIR).join(RECIPES_FILE);
        std::fs::write(&path, "isto não é [toml").unwrap();
        let recipes = vec![RecipeEntry {
            id: "x".to_owned(),
            name: "x".to_owned(),
            command: "x".to_owned(),
            project_scope: false,
            enter: true,
        }];
        assert!(write_recipes(&dir, &recipes).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "isto não é [toml");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn notes_are_never_overwritten() {
        let dir = std::env::temp_dir().join(format!("forja-import-plan-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(write_notes(&dir, "primeira").unwrap());
        assert!(!write_notes(&dir, "segunda").unwrap());
        assert_eq!(std::fs::read_to_string(dir.join(NOTES_FILE)).unwrap(), "primeira");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn code_size_is_clamped() {
        assert_eq!(code_size(5.0), 10);
        assert_eq!(code_size(15.4), 15);
        assert_eq!(code_size(99.0), 24);
    }
}
