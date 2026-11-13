//! De onde vêm as skills: as pastas de cada ajudante (bloco `[skills]` dos perfis em
//! `content/helpers`) e as da própria Forja. Funções puras: recebem o home e o projeto e não
//! tocam o disco, para que os testes usem pastas temporárias.

use crate::model::{Root, Scope, FORJA_OWNER};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Os mesmos perfis que o módulo `helper` embute. Só o bloco `[skills]` e o nome são lidos aqui.
const PROFILES: &[&str] = &[
    include_str!("../../../../content/helpers/aider.toml"),
    include_str!("../../../../content/helpers/amp.toml"),
    include_str!("../../../../content/helpers/antigravity.toml"),
    include_str!("../../../../content/helpers/claude-code.toml"),
    include_str!("../../../../content/helpers/cline.toml"),
    include_str!("../../../../content/helpers/codex.toml"),
    include_str!("../../../../content/helpers/continue.toml"),
    include_str!("../../../../content/helpers/copilot.toml"),
    include_str!("../../../../content/helpers/crush.toml"),
    include_str!("../../../../content/helpers/cursor-agent.toml"),
    include_str!("../../../../content/helpers/gemini.toml"),
    include_str!("../../../../content/helpers/goose.toml"),
    include_str!("../../../../content/helpers/kimi-code.toml"),
    include_str!("../../../../content/helpers/kiro.toml"),
    include_str!("../../../../content/helpers/opencode.toml"),
    include_str!("../../../../content/helpers/qwen-code.toml"),
];

/// As pastas de skills de um ajudante. `user` é relativa ao home; `project`, à raiz do projeto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperDirs {
    pub id: String,
    pub name: String,
    pub user: Vec<String>,
    pub project: Vec<String>,
}

#[derive(Deserialize)]
struct RawProfile {
    id: String,
    name: String,
    #[serde(default)]
    skills: RawSkills,
}

#[derive(Deserialize, Default)]
struct RawSkills {
    #[serde(default)]
    user: Vec<String>,
    #[serde(default)]
    project: Vec<String>,
}

/// Pastas de skills de todos os perfis embutidos, na ordem dos arquivos.
pub fn helper_dirs() -> Vec<HelperDirs> {
    PROFILES
        .iter()
        .filter_map(|text| toml::from_str::<RawProfile>(text).ok())
        .map(|raw| HelperDirs {
            id: raw.id,
            name: raw.name,
            user: raw.skills.user,
            project: raw.skills.project,
        })
        .collect()
}

/// Monta as raízes na ordem da tela: primeiro as da Forja, depois as de cada ajudante.
///
/// - `forja_user_dir`: a pasta de conteúdo do usuário (`~/.forja`), de onde vem `skills/`.
/// - `home`: a pasta pessoal; sem ela, as pastas de usuário dos ajudantes não entram.
/// - `project`: a pasta do projeto atual; sem ela, as raízes de projeto não entram.
pub fn build_roots(
    forja_user_dir: &Path,
    home: Option<&Path>,
    project: Option<&Path>,
    helpers: &[HelperDirs],
) -> Vec<Root> {
    let mut roots = vec![Root {
        owner: FORJA_OWNER.to_owned(),
        owner_name: "Forja".to_owned(),
        scope: Scope::User,
        path: forja_user_dir.join("skills"),
    }];
    if let Some(project) = project {
        roots.push(Root {
            owner: FORJA_OWNER.to_owned(),
            owner_name: "Forja".to_owned(),
            scope: Scope::Project,
            path: project.join(".forja").join("skills"),
        });
    }
    for helper in helpers {
        if let Some(home) = home {
            for dir in &helper.user {
                roots.push(Root {
                    owner: helper.id.clone(),
                    owner_name: helper.name.clone(),
                    scope: Scope::User,
                    path: expand_home(home, dir),
                });
            }
        }
        if let Some(project) = project {
            for dir in &helper.project {
                roots.push(Root {
                    owner: helper.id.clone(),
                    owner_name: helper.name.clone(),
                    scope: Scope::Project,
                    path: join_relative(project, dir),
                });
            }
        }
    }
    roots
}

/// `~/x/y` vira `<home>/x/y`. Qualquer outro caminho é unido ao home como está.
fn expand_home(home: &Path, dir: &str) -> PathBuf {
    match dir.strip_prefix("~/") {
        Some(rest) => join_relative(home, rest),
        None => join_relative(home, dir),
    }
}

/// Une um caminho relativo com `/` (como está nos perfis) a uma base, separando pelo sistema.
pub fn join_relative(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .filter(|part| !part.is_empty())
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

#[cfg(test)]
mod tests;
