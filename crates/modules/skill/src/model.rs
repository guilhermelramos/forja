//! Tipos do módulo: escopo, dono, raiz de busca, skill listada e a foto que a tela e o RPC leem.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Id do dono das skills da própria Forja (`~/.forja/skills` e `<projeto>/.forja/skills`).
pub const FORJA_OWNER: &str = "forja";

/// Usuário (pasta pessoal, vale para todos os projetos) ou projeto (dentro da raiz do projeto).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    Project,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            _ => None,
        }
    }
}

/// Uma pasta onde a Forja procura skills: de um dono, num escopo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub owner: String,
    pub owner_name: String,
    pub scope: Scope,
    pub path: PathBuf,
}

impl Root {
    /// Pastas que são da Forja. Escrever fora delas exige consentimento registrado.
    pub fn is_forja(&self) -> bool {
        self.owner == FORJA_OWNER
    }
}

/// Uma skill encontrada numa raiz. Sem `name` e `description` quando o front matter não serve;
/// nesse caso `error` diz o motivo, pela chave de tradução.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skill {
    pub owner: String,
    pub owner_name: String,
    pub scope: Scope,
    /// Nome da pasta. É o que identifica a skill dentro da raiz.
    pub folder: String,
    pub path: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub error: Option<String>,
}

/// Raiz que a busca olhou, existindo ou não. A tela mostra os caminhos para o usuário saber onde
/// a Forja procurou.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RootInfo {
    pub owner: String,
    pub owner_name: String,
    pub scope: Scope,
    pub path: String,
    pub exists: bool,
}

/// A última leitura das raízes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Listing {
    pub roots: Vec<RootInfo>,
    pub skills: Vec<Skill>,
    /// Pasta do projeto atual, quando há uma bancada ativa.
    pub project: Option<String>,
}

/// Falha de uma operação. `key` é a chave de tradução; `detail` é o texto técnico (caminho, nome
/// do arquivo no zip) que aparece junto dela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillError {
    pub key: &'static str,
    pub detail: String,
}

impl SkillError {
    pub fn new(key: &'static str, detail: impl Into<String>) -> Self {
        Self {
            key,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for SkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.key, self.detail)
    }
}
