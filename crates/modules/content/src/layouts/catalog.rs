//! Leitura das três camadas e snapshot do catálogo.

use crate::model::{self, Diagnostic, Layer};
use forja_kernel::layout_spec::{validate_segment, LayoutSpecV1};
use forja_kernel::settings::project_settings_path;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Um arranjo lido de um arquivo de uma camada.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutEntry {
    pub spec: LayoutSpecV1,
    pub text: String,
    pub layer: Layer,
    pub source: PathBuf,
    pub hash: String,
}

/// Resultado publicado de uma leitura em `Tasks`.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    pub generation: u64,
    pub project: Option<PathBuf>,
    pub entries: BTreeMap<String, LayoutEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub default_id: Option<String>,
    pub revision: u64,
}

/// Dados copiados para o worker: nada de `Ctx` cruza a chamada de `Tasks`.
#[derive(Clone, Debug)]
pub struct LayoutSources {
    pub builtin: Option<PathBuf>,
    pub user: PathBuf,
    pub project: Option<PathBuf>,
    pub generation: u64,
}

impl LayoutSources {
    pub fn from_paths(
        builtin: Option<PathBuf>,
        user: PathBuf,
        project: Option<PathBuf>,
        generation: u64,
    ) -> Self {
        Self {
            builtin,
            user,
            project,
            generation,
        }
    }

    /// Pais das pastas de arranjos, para observar criação e renomes.
    pub fn watch_roots(&self) -> Vec<PathBuf> {
        super::watch::watch_roots(
            &self.user,
            self.project.as_deref(),
            self.builtin.as_deref(),
        )
    }

    /// Pastas lidas, da menor para a maior precedência.
    pub fn roots(&self) -> Vec<(Layer, PathBuf)> {
        let mut roots = Vec::new();
        if let Some(builtin) = &self.builtin {
            roots.push((Layer::Builtin, builtin.clone()));
        }
        roots.push((Layer::User, self.user.clone()));
        if let Some(project) = &self.project {
            roots.push((Layer::Project, project.join(".forja")));
        }
        roots
    }
}

/// Lê no máximo `limit + 1` bytes: um arquivo maior é recusado sem ser
/// carregado inteiro na memória.
pub fn read_limited(path: &Path, limit: u64) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut text = String::new();
    let mut reader = file.take(limit + 1);
    reader
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() as u64 > limit {
        return Err(format!("layout file exceeds {limit} bytes"));
    }
    Ok(text)
}

/// Lê todas as camadas. Falha de um arquivo conserva a última versão válida
/// daquele mesmo caminho.
pub fn read_catalog(sources: &LayoutSources, previous: &BTreeMap<String, LayoutEntry>) -> Catalog {
    let mut entries = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for (layer, root) in sources.roots() {
        let mut seen_in_layer = BTreeSet::new();
        for path in model::files_in(&root, super::LAYOUT_DIR, "toml") {
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            let text = match read_limited(&path, forja_kernel::layout_spec::MAX_INPUT_BYTES as u64)
            {
                Ok(text) => text,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, layer, error));
                    keep_previous(&mut entries, previous, &path);
                    continue;
                }
            };
            let spec = match LayoutSpecV1::parse(&text) {
                Ok(spec) => spec,
                Err(error) => {
                    diagnostics.push(Diagnostic::new(&path, layer, error.to_string()));
                    keep_previous(&mut entries, previous, &path);
                    continue;
                }
            };
            if spec.id != stem {
                diagnostics.push(Diagnostic::new(
                    &path,
                    layer,
                    format!("file `{stem}.toml` declares id `{}`", spec.id),
                ));
                keep_previous(&mut entries, previous, &path);
                continue;
            }
            if let Err(error) = spec.validate() {
                diagnostics.push(Diagnostic::new(&path, layer, error.to_string()));
                keep_previous(&mut entries, previous, &path);
                continue;
            }
            if !seen_in_layer.insert(spec.id.clone()) {
                diagnostics.push(Diagnostic::new(
                    &path,
                    layer,
                    format!("layout `{}` repeats in this layer", spec.id),
                ));
                continue;
            }
            entries.insert(
                spec.id.clone(),
                LayoutEntry {
                    spec,
                    text: text.clone(),
                    layer,
                    source: path.clone(),
                    hash: crate::model::sha256_hex(&text),
                },
            );
        }
    }
    Catalog {
        generation: sources.generation,
        project: sources.project.clone(),
        entries,
        diagnostics,
        default_id: read_default_id(sources.project.as_deref()),
        revision: 0,
    }
}

fn read_default_id(project: Option<&Path>) -> Option<String> {
    let text = std::fs::read_to_string(project_settings_path(project?)).ok()?;
    let value: toml::Value = toml::from_str(&text).ok()?;
    let id = value
        .get("appearance")?
        .get("layout")?
        .get("default")?
        .as_str()?;
    validate_segment("appearance.layout.default", id).ok()?;
    Some(id.to_owned())
}

fn keep_previous(
    entries: &mut BTreeMap<String, LayoutEntry>,
    previous: &BTreeMap<String, LayoutEntry>,
    path: &Path,
) {
    if let Some(entry) = previous.values().find(|entry| entry.source == path) {
        entries.insert(entry.spec.id.clone(), entry.clone());
    }
}
