//! Instalar, copiar e remover skills.
//!
//! Toda escrita passa por uma pasta temporária ao lado do destino, e só então ela vira o nome
//! final. Se já existe uma skill com o mesmo nome, a operação devolve `Conflict` sem tocar em
//! nada. Só com `overwrite` a versão antiga vai para a lixeira, nunca para o apagamento direto.
//! O zip é conferido entrada por entrada antes de extrair: caminho que sai da pasta, caminho
//! absoluto ou link simbólico recusam a instalação inteira.

use crate::frontmatter;
use crate::model::SkillError;
use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Limites de uma instalação. Protegem contra zips que expandem demais.
pub const MAX_FILES: usize = 2000;
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 16;

/// Manda uma pasta para a lixeira. Trait para os testes trocarem por uma versão falsa.
pub trait Trasher {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// A lixeira do Windows, pelo crate `trash`.
pub struct SystemTrash;

impl Trasher for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        trash::delete(path).map_err(|error| error.to_string())
    }
}

/// O que a instalação fez.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placed {
    Installed(PathBuf),
    /// Já existe uma skill com esse nome e nada foi alterado. O caminho é o que existe.
    Conflict(PathBuf),
}

/// Instala uma pasta que tem `SKILL.md` como `<raiz>/<folder>`. Também serve para copiar uma
/// skill de um ajudante para outro, porque a pasta de origem é uma skill igual às outras.
pub fn install_folder(
    source: &Path,
    folder: &str,
    target_root: &Path,
    overwrite: bool,
    trasher: &dyn Trasher,
) -> Result<Placed, SkillError> {
    if !valid_folder(folder) {
        return Err(SkillError::new("skill-error-folder-name", folder));
    }
    check_source(source)?;
    let target = target_root.join(folder);
    if target.exists() && !overwrite {
        return Ok(Placed::Conflict(target));
    }
    fs::create_dir_all(target_root).map_err(|error| io_error("skill-error-write", target_root, &error))?;

    let staging = target_root.join(format!(".{folder}.incoming-{}", unique_suffix()));
    let _ = fs::remove_dir_all(&staging);
    let mut budget = Budget::default();
    if let Err(error) = copy_tree(source, &staging, &mut budget, 0) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if target.exists() {
        if let Err(detail) = trasher.trash(&target) {
            let _ = fs::remove_dir_all(&staging);
            return Err(SkillError::new("skill-error-trash", detail));
        }
    }
    if let Err(error) = fs::rename(&staging, &target) {
        let _ = fs::remove_dir_all(&staging);
        return Err(io_error("skill-error-write", &target, &error));
    }
    Ok(Placed::Installed(target))
}

/// Instala um zip que tem uma skill dentro: `SKILL.md` na raiz, ou uma única pasta de primeiro
/// nível com `SKILL.md`. O zip é extraído numa pasta temporária de `staging_root`, que é apagada
/// no fim, com sucesso ou não.
pub fn install_zip(
    archive: &Path,
    staging_root: &Path,
    target_root: &Path,
    overwrite: bool,
    trasher: &dyn Trasher,
) -> Result<Placed, SkillError> {
    let base = staging_root.join(format!("zip-{}", unique_suffix()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).map_err(|error| io_error("skill-error-write", &base, &error))?;
    let result = extract_and_install(archive, &base, target_root, overwrite, trasher);
    let _ = fs::remove_dir_all(&base);
    result
}

fn extract_and_install(
    archive: &Path,
    base: &Path,
    target_root: &Path,
    overwrite: bool,
    trasher: &dyn Trasher,
) -> Result<Placed, SkillError> {
    let file = fs::File::open(archive).map_err(|error| io_error("skill-error-read", archive, &error))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|error| SkillError::new("skill-error-zip", error.to_string()))?;
    if zip.len() > MAX_FILES {
        return Err(SkillError::new("skill-error-too-large", archive.display().to_string()));
    }

    let mut total = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| SkillError::new("skill-error-zip", error.to_string()))?;
        let name = entry.name().to_owned();
        let Some(relative) = safe_relative(&name) else {
            return Err(SkillError::new("skill-error-unsafe-entry", name));
        };
        if is_symlink(entry.unix_mode()) {
            return Err(SkillError::new("skill-error-symlink", name));
        }
        let destination = base.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| io_error("skill-error-write", &destination, &error))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| io_error("skill-error-write", parent, &error))?;
        }
        let remaining = MAX_BYTES.saturating_sub(total);
        let mut out = fs::File::create(&destination)
            .map_err(|error| io_error("skill-error-write", &destination, &error))?;
        let copied = io::copy(&mut (&mut entry).take(remaining + 1), &mut out)
            .map_err(|error| io_error("skill-error-read", &destination, &error))?;
        if copied > remaining {
            return Err(SkillError::new("skill-error-too-large", name));
        }
        total += copied;
    }

    let (source, folder) = locate_skill(base, archive)?;
    install_folder(&source, &folder, target_root, overwrite, trasher)
}

/// Acha a pasta da skill dentro da extração: a própria raiz, ou a única pasta de primeiro nível.
fn locate_skill(base: &Path, archive: &Path) -> Result<(PathBuf, String), SkillError> {
    if base.join("SKILL.md").is_file() {
        let stem = archive
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok((base.to_path_buf(), folder_name_from(&stem)));
    }
    let candidates: Vec<PathBuf> = fs::read_dir(base)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_dir() && path.join("SKILL.md").is_file())
                .collect()
        })
        .unwrap_or_default();
    match candidates.as_slice() {
        [only] => {
            let folder = only
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            Ok((only.clone(), folder))
        }
        _ => Err(SkillError::new(
            "skill-error-no-skill-in-zip",
            archive.display().to_string(),
        )),
    }
}

/// Manda a pasta de uma skill para a lixeira. Recusa qualquer pasta que não tenha `SKILL.md`.
pub fn remove_folder(path: &Path, trasher: &dyn Trasher) -> Result<(), SkillError> {
    if !path.join("SKILL.md").is_file() {
        return Err(SkillError::new("skill-error-not-a-skill", path.display().to_string()));
    }
    trasher
        .trash(path)
        .map_err(|detail| SkillError::new("skill-error-trash", detail))
}

/// Confere a origem: é uma pasta, tem `SKILL.md` e o front matter serve.
fn check_source(source: &Path) -> Result<(), SkillError> {
    if !source.is_dir() {
        return Err(SkillError::new("skill-error-not-a-folder", source.display().to_string()));
    }
    let skill_file = source.join("SKILL.md");
    if !skill_file.is_file() {
        return Err(SkillError::new("skill-error-no-skill-file", source.display().to_string()));
    }
    let text = fs::read_to_string(&skill_file)
        .map_err(|error| io_error("skill-error-read", &skill_file, &error))?;
    frontmatter::parse(&text).map_err(|error| SkillError::new(error.key(), skill_file.display().to_string()))?;
    Ok(())
}

/// Copia uma árvore. Recusa links simbólicos e estoura o limite de arquivos, bytes ou
/// profundidade. Não segue links.
fn copy_tree(source: &Path, destination: &Path, budget: &mut Budget, depth: usize) -> Result<(), SkillError> {
    if depth > MAX_DEPTH {
        return Err(SkillError::new("skill-error-too-large", source.display().to_string()));
    }
    fs::create_dir_all(destination).map_err(|error| io_error("skill-error-write", destination, &error))?;
    let entries = fs::read_dir(source).map_err(|error| io_error("skill-error-read", source, &error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error("skill-error-read", source, &error))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| io_error("skill-error-read", &path, &error))?;
        if file_type.is_symlink() {
            return Err(SkillError::new("skill-error-symlink", path.display().to_string()));
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&path, &target, budget, depth + 1)?;
        } else if file_type.is_file() {
            let size = entry
                .metadata()
                .map_err(|error| io_error("skill-error-read", &path, &error))?
                .len();
            budget.files += 1;
            if budget.files > MAX_FILES || budget.bytes + size > MAX_BYTES {
                return Err(SkillError::new("skill-error-too-large", path.display().to_string()));
            }
            fs::copy(&path, &target).map_err(|error| io_error("skill-error-write", &target, &error))?;
            budget.bytes += size;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Budget {
    files: usize,
    bytes: u64,
}

/// Caminho relativo seguro de uma entrada do zip, ou `None` se ele sai da pasta de destino.
/// Recusa `..`, caminho absoluto, letra de unidade e nome com `:`. Aceita `\` como separador.
pub fn safe_relative(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\0') {
        return None;
    }
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') {
        return None;
    }
    let mut out = PathBuf::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            part if part.contains(':') => return None,
            part => out.push(part),
        }
    }
    if out.as_os_str().is_empty() {
        return None;
    }
    // Garante que o resultado não tem prefixo nem raiz, mesmo com nomes estranhos.
    if out.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(out)
}

/// Nome de pasta aceitável: não vazio, sem caracteres proibidos no Windows, não `.` nem `..`.
pub fn valid_folder(folder: &str) -> bool {
    !folder.is_empty()
        && folder.len() <= 255
        && folder != "."
        && folder != ".."
        && !folder.starts_with('.')
        && !folder
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
}

/// Transforma um texto em nome de pasta (troca os caracteres proibidos por `-`).
pub fn folder_name_from(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '-' } else { c })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '-' || c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        "skill".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn is_symlink(mode: Option<u32>) -> bool {
    const S_IFMT: u32 = 0o170000;
    const S_IFLNK: u32 = 0o120000;
    mode.is_some_and(|mode| mode & S_IFMT == S_IFLNK)
}

fn io_error(key: &'static str, path: &Path, error: &io::Error) -> SkillError {
    SkillError::new(key, format!("{}: {error}", path.display()))
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests;
