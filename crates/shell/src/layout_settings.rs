//! Escolha e catálogo de arranjos — tudo sem egui e sem disco.
//!
//! O catálogo publicado pelo módulo `content` fornece os candidatos; aqui
//! resolvemos a precedência fábrica < usuário < projeto e persistimos a escolha
//! no store atômico do shell. Campos ausentes (estado antigo) significam "sem
//! escolha explícita". A projeção da árvore vive em `layout_projection.rs`.

use forja_kernel::layout_spec::{validate_segment, LayoutSpecV1};
use serde_json::Value;
use std::collections::BTreeMap;

/// Camada de origem de um arranjo, na ordem de precedência decidida no design.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutLayer {
    #[default]
    Builtin,
    User,
    Project,
}

impl LayoutLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::User => "user",
            Self::Project => "project",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "builtin" => Some(Self::Builtin),
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            _ => None,
        }
    }

    /// Ordem de precedência: a camada maior vence na resolução efetiva.
    pub fn rank(self) -> u8 {
        match self {
            Self::Builtin => 0,
            Self::User => 1,
            Self::Project => 2,
        }
    }

    pub(crate) fn label_key(self) -> &'static str {
        match self {
            Self::Builtin => "shell.layout.origin-builtin",
            Self::User => "shell.layout.origin-user",
            Self::Project => "shell.layout.origin-project",
        }
    }
}

/// Um arranjo efetivo do catálogo, achatado para o seletor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutCandidate {
    pub id: String,
    pub name_pt: String,
    pub name_en: String,
    pub layer: LayoutLayer,
    pub source: String,
    pub hash: String,
}

impl LayoutCandidate {
    pub fn name(&self, portuguese: bool) -> &str {
        if portuguese {
            &self.name_pt
        } else {
            &self.name_en
        }
    }
}

/// Escolha persistida: id efetivo, camada de origem e revisão confirmada.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutSelection {
    pub id: String,
    pub layer: LayoutLayer,
    pub hash: String,
}

impl LayoutSelection {
    /// Um objeto ausente, inválido ou com id inválido vira "sem escolha", sem
    /// descartar o restante do estado.
    pub fn from_json(value: Option<&Value>) -> Option<Self> {
        let value = value?;
        let id = value.get("id")?.as_str()?.trim();
        if validate_segment("layout.id", id).is_err() {
            return None;
        }
        let layer = value
            .get("layer")
            .and_then(Value::as_str)
            .and_then(LayoutLayer::parse)
            .unwrap_or_default();
        let hash = value
            .get("hash")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Some(Self {
            id: id.to_owned(),
            layer,
            hash,
        })
    }

    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "layer": self.layer.as_str(),
            "hash": self.hash,
        })
    }
}

/// Resolve a escolha efetiva a partir da escolha salva, do padrão configurado e
/// dos candidatos publicados. A escolha salva é honrada na mesma camada; se o
/// arquivo desapareceu, cai para o candidato de maior precedência com o mesmo
/// id. Sem escolha válida, usa o padrão e, por fim, `oficina`.
pub fn resolve_selection(
    saved: Option<&LayoutSelection>,
    default_id: Option<&str>,
    candidates: &[LayoutCandidate],
) -> Option<LayoutSelection> {
    let best = |id: &str| -> Option<&LayoutCandidate> {
        candidates
            .iter()
            .filter(|candidate| candidate.id == id)
            .max_by_key(|candidate| candidate.layer.rank())
    };
    let selection = |candidate: &LayoutCandidate| LayoutSelection {
        id: candidate.id.clone(),
        layer: candidate.layer,
        hash: candidate.hash.clone(),
    };
    if let Some(saved) = saved {
        if !saved.id.is_empty() {
            if let Some(exact) = candidates
                .iter()
                .find(|candidate| candidate.id == saved.id && candidate.layer == saved.layer)
            {
                return Some(selection(exact));
            }
            if let Some(candidate) = best(&saved.id) {
                return Some(selection(candidate));
            }
        }
    }
    if let Some(id) = default_id.filter(|id| !id.is_empty()) {
        if let Some(candidate) = best(id) {
            return Some(selection(candidate));
        }
    }
    best("oficina").or_else(|| candidates.first()).map(selection)
}

/// Snapshot do catálogo publicado. `from_value` é puro e tolerante a campos
/// ausentes: nenhum dado vindo de fora vira `unwrap` na interface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogSnapshot {
    pub revision: u64,
    pub generation: u64,
    pub default_id: Option<String>,
    pub candidates: Vec<LayoutCandidate>,
    pub texts: BTreeMap<String, String>,
    pub known_views: BTreeMap<String, String>,
    pub diagnostics: Vec<String>,
}

impl CatalogSnapshot {
    pub fn from_value(value: &Value) -> Self {
        let mut candidates = Vec::new();
        let mut texts = BTreeMap::new();
        if let Some(layouts) = value.get("layouts").and_then(Value::as_array) {
            for item in layouts {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let name = item.get("name");
                let name_pt = name
                    .and_then(|name| name.get("pt-BR"))
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned();
                let name_en = name
                    .and_then(|name| name.get("en"))
                    .and_then(Value::as_str)
                    .unwrap_or(id)
                    .to_owned();
                let layer = item
                    .get("layer")
                    .and_then(Value::as_str)
                    .and_then(LayoutLayer::parse)
                    .unwrap_or_default();
                candidates.push(LayoutCandidate {
                    id: id.to_owned(),
                    name_pt,
                    name_en,
                    layer,
                    source: item
                        .get("source")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    hash: item
                        .get("hash")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                });
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    texts.insert(id.to_owned(), text.to_owned());
                }
            }
        }
        let known_views = value
            .get("known_views")
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .filter_map(|(id, slot)| Some((id.clone(), slot.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            revision: value
                .get("revision")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            generation: value
                .get("generation")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            default_id: value
                .get("default_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            candidates,
            texts,
            known_views,
            diagnostics: value
                .get("diagnostics")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect(),
        }
    }

    /// Entrada efetiva de um id: a de maior precedência.
    pub fn effective_for(&self, id: &str) -> Option<&LayoutCandidate> {
        self.candidates
            .iter()
            .filter(|candidate| candidate.id == id)
            .max_by_key(|candidate| candidate.layer.rank())
    }

    pub fn spec_for(&self, id: &str) -> Option<LayoutSpecV1> {
        LayoutSpecV1::parse(self.texts.get(id)?).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, layer: LayoutLayer, hash: &str) -> LayoutCandidate {
        LayoutCandidate {
            id: id.to_owned(),
            name_pt: id.to_owned(),
            name_en: id.to_owned(),
            layer,
            source: format!("{}/{id}.toml", layer.as_str()),
            hash: hash.to_owned(),
        }
    }

    #[test]
    fn saved_selection_wins_in_its_own_layer() {
        let candidates = vec![
            candidate("oficina", LayoutLayer::Builtin, "fab"),
            candidate("oficina", LayoutLayer::User, "usu"),
        ];
        let saved = LayoutSelection {
            id: "oficina".to_owned(),
            layer: LayoutLayer::User,
            hash: "usu".to_owned(),
        };
        assert_eq!(
            resolve_selection(Some(&saved), None, &candidates).unwrap(),
            saved
        );
    }

    #[test]
    fn precedence_is_factory_then_user_then_project() {
        let candidates = vec![
            candidate("revisao", LayoutLayer::Builtin, "fab"),
            candidate("revisao", LayoutLayer::Project, "proj"),
            candidate("revisao", LayoutLayer::User, "usu"),
        ];
        let resolved = resolve_selection(None, Some("revisao"), &candidates).unwrap();
        assert_eq!(resolved.layer, LayoutLayer::Project);
        assert_eq!(resolved.hash, "proj");
        let saved = LayoutSelection {
            id: "revisao".to_owned(),
            layer: LayoutLayer::Project,
            hash: "proj".to_owned(),
        };
        let without_project = vec![
            candidate("revisao", LayoutLayer::Builtin, "fab"),
            candidate("revisao", LayoutLayer::User, "usu"),
        ];
        let resolved = resolve_selection(Some(&saved), None, &without_project).unwrap();
        assert_eq!(resolved.layer, LayoutLayer::User);
        assert_eq!(resolved.hash, "usu");
    }

    #[test]
    fn falls_back_to_workshop_and_never_returns_a_missing_id() {
        let candidates = vec![candidate("oficina", LayoutLayer::Builtin, "fab")];
        let stale = LayoutSelection {
            id: "sumido".to_owned(),
            layer: LayoutLayer::Project,
            hash: "x".to_owned(),
        };
        let resolved = resolve_selection(Some(&stale), Some("sumido"), &candidates).unwrap();
        assert_eq!(resolved.id, "oficina");
        assert!(resolve_selection(None, None, &[]).is_none());
    }

    #[test]
    fn selection_survives_a_restart_and_old_state_has_no_selection() {
        let saved = LayoutSelection {
            id: "foco".to_owned(),
            layer: LayoutLayer::User,
            hash: "abc".to_owned(),
        };
        let raw = saved.to_json();
        assert_eq!(LayoutSelection::from_json(Some(&raw)), Some(saved));
        assert_eq!(LayoutSelection::from_json(None), None);
        assert_eq!(
            LayoutSelection::from_json(Some(&serde_json::json!({}))),
            None
        );
        let bad = serde_json::json!({ "id": "../fora", "layer": "user" });
        assert_eq!(LayoutSelection::from_json(Some(&bad)), None);
    }

    #[test]
    fn catalog_snapshot_reads_the_published_state_without_io() {
        let value = serde_json::json!({
            "revision": 7,
            "generation": 3,
            "default_id": "revisao",
            "layouts": [
                { "id": "oficina", "name": { "pt-BR": "Oficina", "en": "Workshop" },
                  "layer": "builtin", "source": "content/layouts/oficina.toml", "hash": "a",
                  "text": "" },
                { "id": "oficina", "name": { "pt-BR": "Oficina", "en": "Workshop" },
                  "layer": "user", "source": "user/layouts/oficina.toml", "hash": "b",
                  "text": "" }
            ],
            "known_views": { "editor.tab": "bench_tab" },
            "diagnostics": ["ruim"]
        });
        let snapshot = CatalogSnapshot::from_value(&value);
        assert_eq!(snapshot.revision, 7);
        assert_eq!(snapshot.candidates.len(), 2);
        assert_eq!(snapshot.default_id.as_deref(), Some("revisao"));
        assert_eq!(snapshot.effective_for("oficina").unwrap().layer, LayoutLayer::User);
        assert_eq!(snapshot.known_views.get("editor.tab").map(String::as_str), Some("bench_tab"));
        assert_eq!(snapshot.diagnostics, vec!["ruim".to_owned()]);
    }
}
