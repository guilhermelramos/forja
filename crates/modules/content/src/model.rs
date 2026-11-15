//! Formatos de conteúdo e leitura de arquivos.
//!
//! Nada aqui executa nada: só transforma texto em valores validados e guarda o
//! arquivo de origem, o conteúdo exato e o hash usado pelo `trust`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Camada de origem de um item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Builtin,
    User,
    Project,
}

impl Layer {
    pub fn label(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::User => "user",
            Self::Project => "project",
        }
    }
}

/// Texto localizado no formato `{ ["pt-BR"] = "...", en = "..." }`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Localized {
    #[serde(rename = "pt-BR")]
    pub pt_br: String,
    pub en: String,
}

impl Localized {
    pub fn resolve(&self, locale: &str) -> &str {
        if locale.to_ascii_lowercase().starts_with("pt") && !self.pt_br.is_empty() {
            &self.pt_br
        } else if !self.en.is_empty() {
            &self.en
        } else if !self.pt_br.is_empty() {
            &self.pt_br
        } else {
            ""
        }
    }

    fn from_toml(value: Option<&toml::Value>) -> Self {
        match value {
            Some(toml::Value::String(text)) => Self {
                pt_br: text.clone(),
                en: text.clone(),
            },
            Some(toml::Value::Table(table)) => Self {
                pt_br: table
                    .get("pt-BR")
                    .or_else(|| table.get("pt_BR"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                en: table
                    .get("en")
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            },
            _ => Self::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.pt_br.is_empty() && self.en.is_empty()
    }
}

/// Um argumento declarado por um comando.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandArgument {
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: String,
}

/// Comando de `commands/*.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentCommand {
    pub id: String,
    pub name: String,
    pub title: Localized,
    pub description: Localized,
    pub scope: Vec<String>,
    pub run: String,
    pub helper: Option<String>,
    pub recipe: Option<String>,
    pub permissions: String,
    pub context: Vec<String>,
    pub shortcut: Option<String>,
    pub arguments: Vec<CommandArgument>,
    pub body: String,
    pub layer: Layer,
    pub source_path: String,
    pub content: String,
    pub content_hash: String,
    pub blocked: bool,
    pub block_reason: Option<String>,
}

impl ContentCommand {
    /// Texto do prompt com os argumentos e o contexto preenchidos.
    pub fn render(&self, arguments: &BTreeMap<String, String>, context: &BTreeMap<String, String>) -> (String, Vec<String>) {
        let mut text = self.body.clone();
        let mut warnings = Vec::new();
        for argument in &self.arguments {
            let value = arguments
                .get(&argument.name)
                .cloned()
                .unwrap_or_else(|| argument.default.clone());
            text = text.replace(&format!("{{{{{}}}}}", argument.name), &value);
        }
        for (key, value) in context {
            text = text.replace(&format!("{{{{{key}}}}}"), value);
        }
        if let Some(start) = text.find("{{") {
            if let Some(end) = text[start + 2..].find("}}") {
                warnings.push(text[start + 2..start + 2 + end].to_owned());
            }
        }
        (text, warnings)
    }

    /// O comando pede uma ação que só roda com a pasta confiável.
    pub fn is_risky(&self) -> bool {
        self.layer == Layer::Project
            && (self.run == "terminal"
                || self.permissions == "automatic"
                || self.permissions == "full"
                || self.run == "recipe")
    }
}

/// Tema de terminal ou de interface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentTheme {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub toml: String,
    pub layer: Layer,
    pub source_path: String,
    pub content_hash: String,
}

/// Receita de terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRecipe {
    pub id: String,
    pub title: Localized,
    pub command: String,
    pub scope: String,
    pub enter: bool,
    pub directory: String,
    /// Arquivo que marca o tipo de projeto (por exemplo `Cargo.toml`).
    pub manifest: String,
    pub layer: Layer,
    pub source_path: String,
    pub content: String,
    pub content_hash: String,
}

/// Modelo de arquivo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentTemplate {
    pub id: String,
    pub name: String,
    pub use_kind: String,
    pub body: String,
    pub layer: Layer,
    pub source_path: String,
    pub content: String,
    pub content_hash: String,
}

/// Problema encontrado ao ler um arquivo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: String,
    pub layer: String,
    pub message: String,
}

impl Diagnostic {
    pub fn new(path: &Path, layer: Layer, message: impl Into<String>) -> Self {
        Self {
            path: path.display().to_string(),
            layer: layer.label().to_owned(),
            message: message.into(),
        }
    }
}

/// Hash SHA-256 em hexadecimal minúsculo.
pub fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn read_files(directory: &Path, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        })
        .collect();
    files.sort();
    files
}

/// Divide o frontmatter TOML (`+++`) do corpo.
fn split_frontmatter(text: &str) -> Result<(String, String), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().ok_or("empty file")?;
    if first.trim() != "+++" {
        return Err("missing `+++` frontmatter".to_owned());
    }
    let mut front = String::new();
    let mut offset = first.len();
    let mut closed = false;
    for line in lines {
        offset += line.len();
        if line.trim() == "+++" {
            closed = true;
            break;
        }
        front.push_str(line);
    }
    if !closed {
        return Err("frontmatter has no closing `+++`".to_owned());
    }
    Ok((front, text[offset..].to_owned()))
}

/// Lê um comando. Um erro devolve diagnóstico e nenhum item.
pub fn parse_command(
    path: &Path,
    layer: Layer,
    text: &str,
) -> Result<ContentCommand, String> {
    let (front, body) = split_frontmatter(text)?;
    let value: toml::Value = toml::from_str(&front).map_err(|error| error.to_string())?;
    let table = value
        .as_table()
        .ok_or_else(|| "frontmatter is not a table".to_owned())?;
    let title = Localized::from_toml(table.get("title"));
    if title.is_empty() {
        return Err("missing `title`".to_owned());
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "file has no name".to_owned())?;
    let scope = string_list(table.get("scope"));
    let run = table
        .get("run")
        .and_then(toml::Value::as_str)
        .unwrap_or("helper")
        .to_owned();
    if !matches!(run.as_str(), "helper" | "recipe" | "terminal") {
        return Err(format!("unknown `run` value `{run}`"));
    }
    let permissions = table
        .get("permissions")
        .and_then(toml::Value::as_str)
        .unwrap_or("ask")
        .to_owned();
    if !matches!(
        permissions.as_str(),
        "ask" | "accept-edits" | "automatic" | "full"
    ) {
        return Err(format!("unknown `permissions` value `{permissions}`"));
    }
    let arguments = table
        .get("arguments")
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let item = item.as_table()?;
                    let name = item.get("name")?.as_str()?.trim();
                    if name.is_empty() {
                        return None;
                    }
                    Some(CommandArgument {
                        name: name.to_owned(),
                        required: item
                            .get("required")
                            .and_then(toml::Value::as_bool)
                            .unwrap_or(false),
                        default: item
                            .get("default")
                            .and_then(toml::Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let command = ContentCommand {
        id: format!("cmd.{stem}"),
        name: stem.to_owned(),
        title,
        description: Localized::from_toml(table.get("description")),
        scope: if scope.is_empty() {
            vec!["anvil".to_owned(), "chat".to_owned()]
        } else {
            scope
        },
        run,
        helper: table
            .get("helper")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        recipe: table
            .get("recipe")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        permissions,
        context: string_list(table.get("context")),
        shortcut: table
            .get("shortcut")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        arguments,
        body: body.trim_start_matches(['\r', '\n']).to_owned(),
        layer,
        source_path: path.display().to_string(),
        content: text.to_owned(),
        content_hash: sha256_hex(text),
        blocked: false,
        block_reason: None,
    };
    if command.body.trim().is_empty() && command.run != "recipe" {
        return Err("body is empty".to_owned());
    }
    Ok(command)
}

fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Lê um tema. `kind` é `terminal` quando há paleta ANSI, `interface` quando há
/// tokens de interface, e `unknown` quando o TOML não reconhece nenhum dos dois.
pub fn parse_theme(path: &Path, layer: Layer, text: &str) -> Result<ContentTheme, String> {
    let value: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
    let table = value
        .as_table()
        .ok_or_else(|| "theme is not a table".to_owned())?;
    let id = table
        .get("id")
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
        })
        .ok_or_else(|| "theme has no id".to_owned())?;
    let name = table
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or(&id)
        .to_owned();
    let kind = if table.contains_key("ansi") || table.contains_key("background") {
        "terminal"
    } else if table.contains_key("mode") || table.contains_key("color") {
        "interface"
    } else {
        "unknown"
    };
    Ok(ContentTheme {
        id,
        name,
        kind: kind.to_owned(),
        toml: text.to_owned(),
        layer,
        source_path: path.display().to_string(),
        content_hash: sha256_hex(text),
    })
}

/// Lê um arquivo de receitas. Cada `[[recipe]]` vira um item.
pub fn parse_recipes(path: &Path, layer: Layer, text: &str) -> Result<Vec<ContentRecipe>, String> {
    let value: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
    let list = value
        .get("recipe")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "missing `[[recipe]]`".to_owned())?;
    let hash = sha256_hex(text);
    let mut recipes = Vec::new();
    for item in list {
        let table = item
            .as_table()
            .ok_or_else(|| "recipe is not a table".to_owned())?;
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "recipe has no id".to_owned())?;
        let command = table
            .get("command")
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("recipe `{id}` has no command"))?;
        recipes.push(ContentRecipe {
            id: id.to_owned(),
            title: Localized::from_toml(table.get("title")),
            command: command.to_owned(),
            scope: table
                .get("scope")
                .and_then(toml::Value::as_str)
                .unwrap_or("global")
                .to_owned(),
            enter: table
                .get("enter")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false),
            directory: table
                .get("directory")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            manifest: table
                .get("manifest")
                .and_then(toml::Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            layer,
            source_path: path.display().to_string(),
            content: text.to_owned(),
            content_hash: hash.clone(),
        });
    }
    Ok(recipes)
}

/// Lê um modelo de arquivo.
pub fn parse_template(path: &Path, layer: Layer, text: &str) -> Result<ContentTemplate, String> {
    let (front, body) = split_frontmatter(text)?;
    let value: toml::Value = toml::from_str(&front).map_err(|error| error.to_string())?;
    let table = value
        .as_table()
        .ok_or_else(|| "frontmatter is not a table".to_owned())?;
    let name = table
        .get("name")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "missing `name`".to_owned())?;
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "file has no name".to_owned())?;
    Ok(ContentTemplate {
        id: stem.to_owned(),
        name: name.to_owned(),
        use_kind: table
            .get("use")
            .and_then(toml::Value::as_str)
            .unwrap_or("file")
            .to_owned(),
        body: body.trim_start_matches(['\r', '\n']).to_owned(),
        layer,
        source_path: path.display().to_string(),
        content: text.to_owned(),
        content_hash: sha256_hex(text),
    })
}

/// Lê todos os arquivos de uma pasta com uma extensão.
pub fn files_in(root: &Path, kind: &str, extension: &str) -> Vec<PathBuf> {
    read_files(&root.join(kind), extension)
}

/// Caminho do conteúdo embutido: pasta do repositório ou pasta ao lado do exe.
pub fn builtin_root() -> Option<PathBuf> {
    if let Some(value) = std::env::var_os("FORJA_BUILTIN_CONTENT_DIR") {
        return Some(PathBuf::from(value));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../content");
    if manifest.is_dir() {
        return Some(manifest);
    }
    let exe = std::env::current_exe().ok()?;
    let parent = exe.parent()?;
    [parent.join("content"), parent.join("..").join("content")]
        .into_iter()
        .find(|candidate| candidate.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const COMMAND: &str = "+++\ntitle = { pt-BR = \"Revisar\", en = \"Review\" }\nrun = \"helper\"\narguments = [ { name = \"focus\", default = \"all\" } ]\n+++\nRevise {{focus}}.\n";

    #[test]
    fn command_frontmatter_and_placeholders() {
        let path = Path::new("C:/p/.forja/commands/review.md");
        let command = parse_command(path, Layer::Project, COMMAND).unwrap();
        assert_eq!(command.id, "cmd.review");
        assert_eq!(command.title.resolve("pt-BR"), "Revisar");
        assert_eq!(command.title.resolve("en"), "Review");
        assert!(!command.is_risky());
        let mut args = BTreeMap::new();
        args.insert("focus".to_owned(), "bugs".to_owned());
        let (text, warnings) = command.render(&args, &BTreeMap::new());
        assert_eq!(text.trim(), "Revise bugs.");
        assert!(warnings.is_empty());
    }

    #[test]
    fn terminal_command_in_a_project_is_risky() {
        let text = "+++\ntitle = \"Deploy\"\nrun = \"terminal\"\n+++\necho deploy\n";
        let command = parse_command(Path::new("deploy.md"), Layer::Project, text).unwrap();
        assert!(command.is_risky());
    }

    #[test]
    fn malformed_frontmatter_is_an_error() {
        assert!(parse_command(Path::new("x.md"), Layer::User, "no frontmatter").is_err());
        assert!(parse_command(
            Path::new("x.md"),
            Layer::User,
            "+++\ntitle = \"x\"\n+++\n\n"
        )
        .is_err());
    }

    #[test]
    fn theme_kind_is_detected() {
        let terminal = "id = \"t\"\nname = \"T\"\nbackground = \"#000000\"\n[ansi]\nblack = \"#000000\"\n";
        assert_eq!(
            parse_theme(Path::new("t.toml"), Layer::Builtin, terminal)
                .unwrap()
                .kind,
            "terminal"
        );
        let interface = "mode = \"dark\"\n[color]\naccent = \"#E8743B\"\n";
        assert_eq!(
            parse_theme(Path::new("i.toml"), Layer::Builtin, interface)
                .unwrap()
                .kind,
            "interface"
        );
    }

    #[test]
    fn recipe_keeps_the_project_manifest() {
        let text = "[[recipe]]\nid = \"node-test\"\ntitle = \"Testes\"\ncommand = \"npm test\"\nmanifest = \"package.json\"\n";
        let recipe = parse_recipes(Path::new("node.toml"), Layer::Builtin, text)
            .unwrap()
            .remove(0);
        assert_eq!(recipe.manifest, "package.json");
        assert_eq!(recipe.scope, "global");
    }

    #[test]
    fn template_keeps_placeholders() {
        let text = "+++\nname = \"PR\"\nuse = \"pr\"\n+++\n## Summary\n{{summary}}\n";
        let template = parse_template(Path::new("pr.md"), Layer::User, text).unwrap();
        assert_eq!(template.name, "PR");
        assert!(template.body.contains("{{summary}}"));
    }
}
