//! Tipo do projeto da bancada, para ordenar as receitas do terminal.
//!
//! A detecção olha os manifestos conhecidos na raiz e em `src/`, mais a
//! extensão dos arquivos dessas pastas. Ela nunca entra em subpastas fundas:
//! o seletor abre rápido e o resultado cobre os projetos que têm receita.

use crate::recipes::Recipe;
use std::collections::BTreeSet;
use std::path::Path;

/// Linguagem com receitas embutidas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProjectKind {
    /// Go.
    Go,
    /// Node.js, JavaScript e TypeScript.
    Node,
    /// Python.
    Python,
    /// Rust.
    Rust,
}

impl ProjectKind {
    /// Nome curto mostrado no seletor.
    pub fn label(self) -> &'static str {
        match self {
            Self::Go => "Go",
            Self::Node => "Node",
            Self::Python => "Python",
            Self::Rust => "Rust",
        }
    }

    /// Tipo indicado por um arquivo de manifesto.
    pub fn from_manifest(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "cargo.toml" | "cargo.lock" => Some(Self::Rust),
            "go.mod" | "go.sum" => Some(Self::Go),
            "package.json" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" | "bun.lock"
            | "bun.lockb" | "tsconfig.json" | "deno.json" | "deno.jsonc" => Some(Self::Node),
            "pyproject.toml"
            | "requirements.txt"
            | "setup.py"
            | "setup.cfg"
            | "pipfile"
            | "requirements-dev.txt"
            | "pipfile.lock"
            | "poetry.lock"
            | "uv.lock"
            | "pdm.lock"
            | "environment.yml"
            | "tox.ini" => Some(Self::Python),
            _ => None,
        }
    }

    /// Tipo indicado pela extensão de um arquivo fonte.
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "go" => Some(Self::Go),
            "py" => Some(Self::Python),
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "vue" | "svelte" => Some(Self::Node),
            _ => None,
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        let head = id
            .split(['-', '_', '.'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match head.as_str() {
            "go" => Some(Self::Go),
            "node" | "npm" | "npx" | "ts" | "js" | "typescript" | "javascript" => Some(Self::Node),
            "python" | "py" | "pytest" | "ruff" => Some(Self::Python),
            "rust" | "cargo" | "clippy" => Some(Self::Rust),
            _ => None,
        }
    }

    fn from_command(command: &str) -> Option<Self> {
        let program = command.split_whitespace().next()?.to_ascii_lowercase();
        let program = program
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(&program)
            .trim_end_matches(".exe");
        match program {
            "cargo" | "rustc" | "rustup" => Some(Self::Rust),
            "go" | "gofmt" | "golangci-lint" => Some(Self::Go),
            "npm" | "npx" | "node" | "pnpm" | "yarn" | "bun" | "tsc" | "prettier" | "eslint"
            | "vitest" | "jest" => Some(Self::Node),
            "python" | "python3" | "py" | "pytest" | "ruff" | "pip" | "uv" | "poetry"
            | "pipenv" | "tox" => Some(Self::Python),
            _ => None,
        }
    }

    /// Tipo de uma receita: manifesto, id ou programa do comando.
    pub fn of(recipe: &Recipe) -> Option<Self> {
        Self::from_manifest(&recipe.manifest)
            .or_else(|| Self::from_id(&recipe.id))
            .or_else(|| Self::from_command(&recipe.command))
    }
}

/// Linguagens encontradas na pasta da bancada.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectProfile {
    kinds: BTreeSet<ProjectKind>,
}

impl ProjectProfile {
    /// Detecta o tipo olhando a raiz e `src/`.
    pub fn detect(root: Option<&Path>) -> Self {
        let Some(root) = root else {
            return Self::default();
        };
        let mut kinds = BTreeSet::new();
        for directory in [root.to_path_buf(), root.join("src")] {
            for (name, kind) in MANIFESTS {
                if directory.join(name).is_file() {
                    kinds.insert(*kind);
                }
            }
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let extension = path.extension().and_then(|value| value.to_str());
                if let Some(kind) = extension.and_then(ProjectKind::from_extension) {
                    kinds.insert(kind);
                }
            }
        }
        Self { kinds }
    }

    /// Nenhuma linguagem reconhecida.
    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Linguagens encontradas, na ordem estável de apresentação.
    pub fn kinds(&self) -> impl Iterator<Item = ProjectKind> + '_ {
        self.kinds.iter().copied()
    }

    /// A receita se aplica ao projeto.
    pub fn applies(&self, recipe: &Recipe) -> bool {
        ProjectKind::of(recipe).is_some_and(|kind| self.kinds.contains(&kind))
    }
}

const MANIFESTS: &[(&str, ProjectKind)] = &[
    ("Cargo.toml", ProjectKind::Rust),
    ("Cargo.lock", ProjectKind::Rust),
    ("go.mod", ProjectKind::Go),
    ("go.sum", ProjectKind::Go),
    ("package.json", ProjectKind::Node),
    ("package-lock.json", ProjectKind::Node),
    ("yarn.lock", ProjectKind::Node),
    ("pnpm-lock.yaml", ProjectKind::Node),
    ("bun.lock", ProjectKind::Node),
    ("bun.lockb", ProjectKind::Node),
    ("tsconfig.json", ProjectKind::Node),
    ("deno.json", ProjectKind::Node),
    ("deno.jsonc", ProjectKind::Node),
    ("pyproject.toml", ProjectKind::Python),
    ("requirements.txt", ProjectKind::Python),
    ("requirements-dev.txt", ProjectKind::Python),
    ("setup.py", ProjectKind::Python),
    ("setup.cfg", ProjectKind::Python),
    ("Pipfile", ProjectKind::Python),
    ("Pipfile.lock", ProjectKind::Python),
    ("poetry.lock", ProjectKind::Python),
    ("uv.lock", ProjectKind::Python),
    ("pdm.lock", ProjectKind::Python),
    ("environment.yml", ProjectKind::Python),
    ("tox.ini", ProjectKind::Python),
];

/// Filtra pelo texto e separa o que se aplica ao projeto do resto.
///
/// As duas listas saem ordenadas por linguagem e título, com as receitas sem
/// tipo reconhecido no fim da seção "Outras".
pub fn arrange<'a>(
    recipes: &'a [Recipe],
    profile: &ProjectProfile,
    query: &str,
) -> (Vec<&'a Recipe>, Vec<&'a Recipe>) {
    let query = query.trim().to_lowercase();
    let matches = |recipe: &Recipe| {
        if query.is_empty() {
            return true;
        }
        let mut haystack = format!(
            "{} {} {} {}",
            recipe.id, recipe.title_pt, recipe.title_en, recipe.command
        );
        if let Some(kind) = ProjectKind::of(recipe) {
            haystack.push(' ');
            haystack.push_str(kind.label());
        }
        haystack.to_lowercase().contains(&query)
    };
    let mut applicable = Vec::new();
    let mut others = Vec::new();
    for recipe in recipes.iter().filter(|recipe| matches(recipe)) {
        if profile.applies(recipe) {
            applicable.push(recipe);
        } else {
            others.push(recipe);
        }
    }
    let key = |recipe: &Recipe| {
        (
            ProjectKind::of(recipe)
                .map(|kind| kind as usize)
                .unwrap_or(usize::MAX),
            recipe.title_pt.to_lowercase(),
            recipe.id.clone(),
        )
    };
    applicable.sort_by_key(|recipe| key(recipe));
    others.sort_by_key(|recipe| key(recipe));
    (applicable, others)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_guard(label: &str) -> TempGuard {
        let dir = std::env::temp_dir().join(format!(
            "forja-terminal-project-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        TempGuard(dir)
    }

    fn recipe(id: &str, title: &str, command: &str, manifest: &str) -> Recipe {
        Recipe {
            id: id.to_owned(),
            title_pt: title.to_owned(),
            title_en: title.to_owned(),
            command: command.to_owned(),
            scope: "project".to_owned(),
            enter: true,
            directory: String::new(),
            manifest: manifest.to_owned(),
            source_path: String::new(),
            content: String::new(),
            content_hash: String::new(),
        }
    }

    #[test]
    fn manifests_and_source_files_reveal_the_project_kind() {
        let guard = temp_guard("detect");
        std::fs::create_dir_all(guard.0.join("src")).unwrap();
        std::fs::write(guard.0.join("package.json"), "{}").unwrap();
        std::fs::write(guard.0.join("src").join("app.py"), "print('ola')").unwrap();
        std::fs::write(guard.0.join("src").join("login.ts"), "export {}").unwrap();
        let profile = ProjectProfile::detect(Some(&guard.0));
        assert!(profile.applies(&recipe("node-test", "Testes", "npm test", "package.json")));
        assert!(profile.applies(&recipe("python-test", "Testes", "pytest", "")));
        assert!(!profile.applies(&recipe("go-test", "Testes", "go test ./...", "go.mod")));
        assert!(!profile.applies(&recipe("rust-test", "Testes", "cargo test", "Cargo.toml")));
        assert_eq!(
            profile.kinds().collect::<Vec<_>>(),
            vec![ProjectKind::Node, ProjectKind::Python]
        );
    }

    #[test]
    fn supported_manifests_reveal_their_project_kind() {
        for (name, expected) in [
            ("Cargo.toml", ProjectKind::Rust),
            ("go.mod", ProjectKind::Go),
            ("package.json", ProjectKind::Node),
            ("pyproject.toml", ProjectKind::Python),
            ("requirements.txt", ProjectKind::Python),
        ] {
            let guard = temp_guard("manifest");
            std::fs::write(guard.0.join(name), "").unwrap();
            let profile = ProjectProfile::detect(Some(&guard.0));
            assert!(
                profile.kinds().any(|kind| kind == expected),
                "{name} should detect {expected:?}"
            );
        }
    }

    #[test]
    fn unknown_recipes_fall_back_to_the_command_program() {
        let profile = ProjectProfile::detect(None);
        assert!(profile.is_empty());
        assert_eq!(
            ProjectKind::of(&recipe("custom", "Lint", "cargo clippy", "")),
            Some(ProjectKind::Rust)
        );
        assert_eq!(
            ProjectKind::of(&recipe("custom", "Testes", "npx vitest", "")),
            Some(ProjectKind::Node)
        );
        assert_eq!(
            ProjectKind::of(&recipe("custom", "Testes", "echo oi", "")),
            None
        );
    }

    #[test]
    fn arrange_puts_applicable_recipes_first_and_filters_by_text() {
        let guard = temp_guard("arrange");
        std::fs::create_dir_all(guard.0.join("src")).unwrap();
        std::fs::write(guard.0.join("src").join("app.py"), "").unwrap();
        std::fs::write(guard.0.join("src").join("login.ts"), "").unwrap();
        let profile = ProjectProfile::detect(Some(&guard.0));
        let recipes = vec![
            recipe("rust-test", "Testes Rust", "cargo test", "Cargo.toml"),
            recipe("go-test", "Testes Go", "go test ./...", "go.mod"),
            recipe("node-test", "Testes Node", "npm test", "package.json"),
            recipe("python-test", "Testes Python", "pytest", "pyproject.toml"),
        ];
        let (applicable, others) = arrange(&recipes, &profile, "");
        let applicable: Vec<&str> = applicable.iter().map(|recipe| recipe.id.as_str()).collect();
        let others: Vec<&str> = others.iter().map(|recipe| recipe.id.as_str()).collect();
        assert_eq!(applicable, vec!["node-test", "python-test"]);
        assert_eq!(others, vec!["go-test", "rust-test"]);

        let (applicable, others) = arrange(&recipes, &profile, "cargo");
        assert!(applicable.is_empty());
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].id, "rust-test");

        let (applicable, others) = arrange(&recipes, &profile, "python");
        assert_eq!(applicable.len(), 1);
        assert_eq!(applicable[0].id, "python-test");
        assert!(others.is_empty());
    }
}
