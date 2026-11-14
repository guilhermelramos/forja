//! Receitas de terminal: `recipes/*.toml` em três camadas.
//!
//! Formato de `design/extensions.md`: cada arquivo tem `[[recipe]]` com `id`,
//! `title`, `command`, `scope`, `enter` e `directory`. A camada do projeto
//! substitui a do usuário, que substitui a embutida, pelo `id`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Uma receita pronta para o menu e a Bigorna.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// Identificador estável em inglês.
    pub id: String,
    /// Título em pt-BR.
    pub title_pt: String,
    /// Título em inglês.
    pub title_en: String,
    /// Comando enviado ao shell.
    pub command: String,
    /// `global` ou `project`.
    pub scope: String,
    /// Envia Enter depois do comando.
    pub enter: bool,
    /// Pasta relativa à bancada; vazia usa a pasta do terminal.
    pub directory: String,
    /// Arquivo de origem.
    pub source_path: String,
    /// Conteudo exato do arquivo de origem.
    pub content: String,
    /// SHA-256 do conteudo.
    pub content_hash: String,
}

impl Recipe {
    /// Lê a forma devolvida por `content.recipes.list`.
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        let id = value.get("id")?.as_str()?.to_owned();
        let command = value.get("command")?.as_str()?.to_owned();
        let title = value.get("title")?;
        let title_pt = title
            .get("pt-BR")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let title_en = title
            .get("en")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Some(Self {
            id,
            title_pt,
            title_en,
            command,
            scope: value
                .get("scope")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("global")
                .to_owned(),
            enter: value
                .get("enter")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            directory: value
                .get("directory")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            source_path: value
                .get("source_path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content: value
                .get("content")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            content_hash: value
                .get("content_hash")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
    }

    /// Título no idioma pedido.
    pub fn title(&self, locale: &str) -> &str {
        if locale.to_ascii_lowercase().starts_with("en") {
            &self.title_en
        } else {
            &self.title_pt
        }
    }
}

/// Carrega as três camadas, da menor para a maior precedência.
pub fn load(bench: Option<&Path>) -> Vec<Recipe> {
    let mut recipes: BTreeMap<String, Recipe> = BTreeMap::new();
    for directory in builtin_dirs() {
        read_layer(&directory, None, &mut recipes);
    }
    read_layer(&user_dir().join("recipes"), None, &mut recipes);
    if let Some(bench) = bench {
        read_layer(&bench.join(".forja").join("recipes"), Some(bench), &mut recipes);
    }
    recipes.into_values().collect()
}

fn read_layer(directory: &Path, project: Option<&Path>, recipes: &mut BTreeMap<String, Recipe>) {
    let project_root = match project {
        Some(root) => match root.canonicalize() {
            Ok(root) => Some(root),
            Err(_) => return,
        },
        None => None,
    };
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
        .collect();
    files.sort();
    for file in files {
        let (source, text) = if let Some(root) = &project_root {
            let Some(snapshot) = project_snapshot(&file, root) else { continue };
            snapshot
        } else {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            (file, text)
        };
        let hash = sha256_hex(&text);
        for mut recipe in parse(&text) {
            if project_root.is_some() { recipe.scope = "project".to_owned(); }
            recipe.source_path = source.display().to_string();
            recipe.content = text.clone();
            recipe.content_hash = hash.clone();
            recipes.insert(recipe.id.clone(), recipe);
        }
    }
}

// Resolve the path from the same open handle that supplies the approved snapshot.
#[cfg(windows)]
fn project_snapshot(file: &Path, root: &Path) -> Option<(PathBuf, String)> {
    use std::io::Read;
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED};

    let mut opened = std::fs::File::open(file).ok()?;
    let handle = opened.as_raw_handle() as _;
    // SAFETY: the handle remains open and the buffer is valid for its declared length.
    let length = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, FILE_NAME_NORMALIZED) };
    if length == 0 { return None; }
    let mut buffer = vec![0u16; length as usize + 1];
    // SAFETY: the buffer is writable and the handle remains open.
    let written = unsafe { GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, FILE_NAME_NORMALIZED) };
    if written == 0 || written as usize >= buffer.len() { return None; }
    buffer.truncate(written as usize);
    let source = PathBuf::from(std::ffi::OsString::from_wide(&buffer));
    if !source.starts_with(root) { return None; }
    let mut text = String::new();
    opened.read_to_string(&mut text).ok()?;
    Some((source, text))
}

#[cfg(not(windows))]
fn project_snapshot(file: &Path, root: &Path) -> Option<(PathBuf, String)> {
    let source = file.canonicalize().ok()?;
    if !source.starts_with(root) { return None; }
    let text = std::fs::read_to_string(&source).ok()?;
    Some((source, text))
}

/// SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(text.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Lê um documento TOML de receitas.
pub fn parse(text: &str) -> Vec<Recipe> {
    let Ok(value) = toml::from_str::<toml::Value>(text) else {
        return Vec::new();
    };
    let Some(list) = value.get("recipe").and_then(toml::Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| {
            let table = item.as_table()?;
            let id = table.get("id")?.as_str()?.trim();
            let command = table.get("command")?.as_str()?.trim();
            if id.is_empty() || command.is_empty() {
                return None;
            }
            let (title_pt, title_en) = read_title(table.get("title"));
            Some(Recipe {
                id: id.to_owned(),
                title_pt,
                title_en,
                command: command.to_owned(),
                scope: table
                    .get("scope")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("global")
                    .to_owned(),
                enter: table.get("enter").and_then(toml::Value::as_bool).unwrap_or(false),
                directory: table
                    .get("directory")
                    .and_then(toml::Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                source_path: String::new(),
                content: String::new(),
                content_hash: String::new(),
            })
        })
        .collect()
}

fn read_title(value: Option<&toml::Value>) -> (String, String) {
    match value {
        Some(toml::Value::String(text)) => (text.clone(), text.clone()),
        Some(toml::Value::Table(table)) => (
            table
                .get("pt-BR")
                .or_else(|| table.get("pt_BR"))
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            table
                .get("en")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        ),
        _ => (String::new(), String::new()),
    }
}

fn builtin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("content").join("recipes"));
            dirs.push(parent.join("..").join("content").join("recipes"));
        }
    }
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../content/recipes"));
    dirs
}

fn user_dir() -> PathBuf {
    if let Some(content) = std::env::var_os("FORJA_CONTENT_DIR") {
        return PathBuf::from(content);
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".forja")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_localized_titles_and_defaults() {
        let text = r#"
[[recipe]]
id = "lint"
title = { pt-BR = "Lint do workspace", en = "Workspace lint" }
command = "cargo clippy"
scope = "project"
enter = false

[[recipe]]
id = "test"
title = "Testes"
command = "cargo test"
"#;
        let recipes = parse(text);
        assert_eq!(recipes.len(), 2);
        assert_eq!(recipes[0].title_pt, "Lint do workspace");
        assert_eq!(recipes[0].title_en, "Workspace lint");
        assert_eq!(recipes[0].scope, "project");
        assert!(!recipes[0].enter);
        assert_eq!(recipes[1].title_en, "Testes");
        assert_eq!(recipes[1].scope, "global");
    }

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_recipes_carry_the_exact_file_content_and_hash() {
        let dir = TempGuard(std::env::temp_dir().join(format!(
            "forja-terminal-recipes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        let recipes_dir = dir.0.join(".forja").join("recipes");
        std::fs::create_dir_all(&recipes_dir).unwrap();
        let text = "[[recipe]]\nid = \"trusted\"\ntitle = \"Trusted\"\ncommand = \"echo ok\"\nscope = \"project\"\n";
        let file = recipes_dir.join("trusted.toml");
        std::fs::write(&file, text).unwrap();
        let recipes = load(Some(&dir.0));
        let recipe = recipes
            .iter()
            .find(|recipe| recipe.id == "trusted")
            .expect("project recipe");
        assert_eq!(recipe.scope, "project");
        assert_eq!(recipe.content, text);
        assert_eq!(recipe.content_hash, sha256_hex(text));
        assert!(recipe.source_path.ends_with("trusted.toml"));
    }

    #[cfg(windows)]
    #[test]
    fn project_junctions_show_the_canonical_origin_and_reject_external_targets() {
        let base = TempGuard(std::env::temp_dir().join(format!(
            "forja-recipe-junction-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let project = base.0.join("project");
        let inside = project.join("actual");
        let outside = base.0.join("outside");
        std::fs::create_dir_all(project.join(".forja")).unwrap();
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let text = "[[recipe]]\nid = \"linked\"\ncommand = \"echo ok\"\nscope = \"global\"\n";
        std::fs::write(inside.join("linked.toml"), text).unwrap();
        std::fs::write(outside.join("linked.toml"), text).unwrap();
        let link = project.join(".forja").join("recipes");
        let junction = |target: &Path| {
            let status = std::process::Command::new("cmd").args(["/C", "mklink", "/J"])
                .arg(&link).arg(target).status().unwrap();
            assert!(status.success(), "junction creation failed");
        };
        junction(&inside);
        let mut found = BTreeMap::new();
        read_layer(&link, Some(&project), &mut found);
        let recipe = found.get("linked").expect("internal junction is accepted");
        assert_eq!(PathBuf::from(&recipe.source_path), inside.join("linked.toml").canonicalize().unwrap());
        assert_eq!(recipe.scope, "project");
        assert_eq!(recipe.content_hash, sha256_hex(text));
        std::fs::remove_dir(&link).unwrap();
        junction(&outside);
        found.clear();
        read_layer(&link, Some(&project), &mut found);
        assert!(found.is_empty(), "external junction must be refused");
        let _ = std::fs::remove_dir(&link);
    }

    #[test]
    fn ignores_items_without_id_or_command() {
        let text = r#"
[[recipe]]
id = "broken"
title = "No command"
"#;
        assert!(parse(text).is_empty());
    }
}
