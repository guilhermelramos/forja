//! Descoberta: lê cada raiz e devolve as skills. Uma pasta sem `SKILL.md` não é skill e é
//! ignorada; uma com `SKILL.md` ruim aparece na lista com o motivo. Pastas que começam com `.`
//! são ignoradas: é ali que a instalação deixa a cópia temporária.

use crate::frontmatter::{self, Front};
use crate::model::{Listing, Root, RootInfo, Skill};
use std::path::{Path, PathBuf};

/// Limite por raiz, para uma pasta com milhares de entradas não travar a tela.
pub const MAX_SKILLS_PER_ROOT: usize = 500;
/// Um `SKILL.md` maior que isto não é lido: vira erro.
const MAX_SKILL_FILE: u64 = 256 * 1024;

/// Skills de uma raiz, em ordem de nome de pasta.
pub fn scan_root(root: &Root) -> Vec<Skill> {
    let Ok(entries) = std::fs::read_dir(&root.path) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir() && !is_hidden(path))
        .collect();
    dirs.sort();
    dirs.into_iter()
        .filter(|dir| dir.join("SKILL.md").is_file())
        .take(MAX_SKILLS_PER_ROOT)
        .map(|dir| read_skill(root, &dir))
        .collect()
}

/// Lê todas as raízes. `project` é a pasta do projeto atual, só para mostrar na tela.
pub fn scan_all(roots: &[Root], project: Option<String>) -> Listing {
    let infos = roots
        .iter()
        .map(|root| RootInfo {
            owner: root.owner.clone(),
            owner_name: root.owner_name.clone(),
            scope: root.scope,
            path: root.path.display().to_string(),
            exists: root.path.is_dir(),
        })
        .collect();
    let skills = roots.iter().flat_map(scan_root).collect();
    Listing {
        roots: infos,
        skills,
        project,
    }
}

/// Lê o `SKILL.md` de uma pasta. Erro de front matter vira a chave em `error`.
pub fn read_skill(root: &Root, dir: &Path) -> Skill {
    let folder = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut skill = Skill {
        owner: root.owner.clone(),
        owner_name: root.owner_name.clone(),
        scope: root.scope,
        folder,
        path: dir.display().to_string(),
        name: None,
        description: None,
        error: None,
    };
    match read_front(&dir.join("SKILL.md")) {
        Ok(front) => {
            skill.name = Some(front.name);
            skill.description = Some(front.description);
        }
        Err(key) => skill.error = Some(key.to_owned()),
    }
    skill
}

/// Lê só o front matter de um arquivo. Erro é a chave de tradução.
pub fn read_front(path: &Path) -> Result<Front, &'static str> {
    let metadata = std::fs::metadata(path).map_err(|_| "skill-error-read")?;
    if metadata.len() > MAX_SKILL_FILE {
        return Err("skill-error-too-large");
    }
    let text = std::fs::read_to_string(path).map_err(|_| "skill-error-read")?;
    frontmatter::parse(&text).map_err(|error| error.key())
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.to_string_lossy().starts_with('.'))
}

#[cfg(test)]
mod tests;
