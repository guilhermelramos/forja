//! Escolha, catálogo e projeção de arranjos — tudo sem egui e sem disco.
//!
//! O catálogo publicado pelo módulo `content` fornece os candidatos; aqui
//! resolvemos a precedência fábrica < usuário < projeto, projetamos o arranjo
//! no chrome legado e persistimos a escolha no store atômico do shell.
//! Campos ausentes (estado antigo) significam "sem escolha explícita".

use forja_kernel::layout_spec::{
    validate_segment, LayoutChrome, LayoutError, LayoutNodeSpec, LayoutSpecV1, LocalizedName,
    SCHEMA_VERSION, TabSpec,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

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

/// Projeção do arranjo sobre o chrome legado da Fatia 1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Projection {
    /// `None` herda a preferência do usuário.
    pub rail_visible: Option<bool>,
    pub status_bar_visible: Option<bool>,
    pub rail_hidden: Vec<String>,
    /// `None` mantém a ordem de registro.
    pub rail_order: Option<Vec<String>>,
    /// Ordem final de ids do trilho, já filtrada.
    pub effective_rail_order: Vec<String>,
    pub bench_tabs: Vec<String>,
    pub sidebar_tabs: Vec<String>,
    pub right_tabs: Vec<String>,
    /// Refs cujo slot não existe hoje (ou cuja view não está registrada). São
    /// sinalizadas ao usuário e preservadas na árvore ao reexportar.
    pub unavailable_refs: Vec<String>,
}

fn push_unique(list: &mut Vec<String>, seen: &mut BTreeSet<String>, id: String) {
    if seen.insert(id.clone()) {
        list.push(id);
    }
}

fn collect(
    node: &LayoutNodeSpec,
    known: &BTreeMap<String, String>,
    rails: &mut Vec<String>,
    bench: &mut Vec<String>,
    sidebar: &mut Vec<String>,
    right: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
    unavailable: &mut Vec<String>,
    unavailable_seen: &mut BTreeSet<String>,
) {
    match node {
        LayoutNodeSpec::Split { first, second, .. } => {
            collect(first, known, rails, bench, sidebar, right, seen, unavailable, unavailable_seen);
            collect(second, known, rails, bench, sidebar, right, seen, unavailable, unavailable_seen);
        }
        LayoutNodeSpec::Leaf { tabs, .. } => {
            for tab in tabs {
                let view = match tab {
                    TabSpec::Id(id) => id.clone(),
                    TabSpec::Table(table) => table.view.clone(),
                };
                // Uma referência desconhecida sobrevive como aba indisponível;
                // nunca é descartada — e agora também é sinalizada.
                match known.get(&view).map(String::as_str) {
                    Some("rail") => push_unique(rails, seen, view),
                    Some("sidebar") => push_unique(sidebar, seen, view),
                    Some("right_panel") => push_unique(right, seen, view),
                    Some("bench_tab") => push_unique(bench, seen, view),
                    None => {
                        push_unique(unavailable, unavailable_seen, view.clone());
                        push_unique(bench, seen, view);
                    }
                    Some(_) => push_unique(unavailable, unavailable_seen, view),
                }
            }
        }
    }
}

/// Projeta a árvore declarada no chrome legado: ordem de leitura da árvore,
/// agrupamento pelos slots registrados e escolhas do bloco `[chrome]`.
pub fn project(
    spec: &LayoutSpecV1,
    known: &BTreeMap<String, String>,
    registration_order: &[String],
) -> Projection {
    let mut rails = Vec::new();
    let mut bench = Vec::new();
    let mut sidebar = Vec::new();
    let mut right = Vec::new();
    let mut unavailable = Vec::new();
    let mut seen = BTreeSet::new();
    let mut unavailable_seen = BTreeSet::new();
    collect(
        &spec.root,
        known,
        &mut rails,
        &mut bench,
        &mut sidebar,
        &mut right,
        &mut seen,
        &mut unavailable,
        &mut unavailable_seen,
    );
    let hidden: BTreeSet<&str> = spec
        .chrome
        .rail_hidden
        .iter()
        .map(String::as_str)
        .collect();
    let declared = spec.chrome.rail_order.clone().unwrap_or_else(|| {
        // Sem ordem declarada, a leitura da árvore vem primeiro e o registro
        // completa a lista com as páginas que o arranjo não menciona.
        let mut order = rails.clone();
        order.extend(registration_order.iter().cloned());
        order
    });
    let mut rail_order = Vec::new();
    let mut rail_seen = BTreeSet::new();
    for id in declared {
        if rail_seen.insert(id.clone()) {
            rail_order.push(id);
        }
    }
    rail_order.retain(|id| registration_order.contains(id) && !hidden.contains(id.as_str()));
    Projection {
        rail_visible: spec.chrome.rail_visible,
        status_bar_visible: spec.chrome.status_bar_visible,
        rail_hidden: spec.chrome.rail_hidden.clone(),
        rail_order: spec.chrome.rail_order.clone(),
        effective_rail_order: rail_order,
        bench_tabs: bench,
        sidebar_tabs: sidebar,
        right_tabs: right,
        unavailable_refs: unavailable,
    }
}

/// Exporta a edição de volta para TOML, preservando a árvore e todas as refs —
/// inclusive as ainda indisponíveis no registro.
pub fn export(spec: &LayoutSpecV1, edited: &Projection) -> Result<String, LayoutError> {
    let mut next = spec.clone();
    next.chrome = LayoutChrome {
        rail_visible: edited.rail_visible,
        rail_hidden: edited.rail_hidden.clone(),
        rail_order: edited.rail_order.clone(),
        status_bar_visible: edited.status_bar_visible,
    };
    next.to_toml()
}

/// Arranjo novo quando não há árvore de origem: uma folha com até três abas.
pub fn new_spec(id: &str, name_pt: &str, name_en: &str, tabs: &[String]) -> LayoutSpecV1 {
    LayoutSpecV1 {
        schema_version: SCHEMA_VERSION,
        id: id.to_owned(),
        name: LocalizedName {
            pt_br: name_pt.to_owned(),
            en: name_en.to_owned(),
        },
        chrome: LayoutChrome::default(),
        root: LayoutNodeSpec::Leaf {
            id: "principal".to_owned(),
            collapsible: false,
            tabs: tabs
                .iter()
                .take(3)
                .map(|view| TabSpec::Id(view.clone()))
                .collect(),
            active: 0,
        },
    }
}

/// Ordena as abas laterais pela ordem declarada no arranjo: as refs declaradas
/// vêm primeiro (na ordem de leitura da árvore), as demais completam na ordem de
/// registro. Devolve também a primeira ref declarada que existe no registro,
/// para virar a aba ativa numa troca explícita de arranjo.
pub(crate) fn order_side_tabs(
    current: &[String],
    declared: &[String],
) -> (Vec<String>, Option<String>) {
    let mut order: Vec<String> = declared
        .iter()
        .filter(|id| current.contains(id))
        .cloned()
        .collect();
    let mut seen: BTreeSet<String> = order.iter().cloned().collect();
    for id in current {
        if seen.insert(id.clone()) {
            order.push(id.clone());
        }
    }
    let active = order.first().filter(|id| declared.contains(id)).cloned();
    (order, active)
}

pub(crate) enum RailEdit {
    Move(String, isize),
    Hide(String),
    Show(String),
}

pub(crate) fn apply_rail_edit(projection: &mut Projection, edit: RailEdit) {
    match edit {
        RailEdit::Move(id, delta) => {
            if let Some(from) = projection.effective_rail_order.iter().position(|item| item == &id)
            {
                let to = (from as isize + delta)
                    .clamp(0, projection.effective_rail_order.len() as isize - 1)
                    as usize;
                let item = projection.effective_rail_order.remove(from);
                projection.effective_rail_order.insert(to, item);
                projection.rail_order = Some(projection.effective_rail_order.clone());
            }
        }
        RailEdit::Hide(id) => {
            projection.effective_rail_order.retain(|item| item != &id);
            if !projection.rail_hidden.contains(&id) {
                projection.rail_hidden.push(id);
            }
            projection.rail_order = Some(projection.effective_rail_order.clone());
        }
        RailEdit::Show(id) => {
            projection.rail_hidden.retain(|item| item != &id);
            if !projection.effective_rail_order.contains(&id) {
                projection.effective_rail_order.push(id.clone());
            }
            projection.rail_order = Some(projection.effective_rail_order.clone());
        }
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

    fn known() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("muster.page".to_owned(), "rail".to_owned()),
            ("board.page".to_owned(), "rail".to_owned()),
            ("project.sidebar".to_owned(), "sidebar".to_owned()),
            ("board.sidebar".to_owned(), "sidebar".to_owned()),
            ("explorer.panel".to_owned(), "sidebar".to_owned()),
            ("git.changes".to_owned(), "right_panel".to_owned()),
            ("editor.tab".to_owned(), "bench_tab".to_owned()),
            ("terminal.tab".to_owned(), "bench_tab".to_owned()),
            ("chat.tab".to_owned(), "bench_tab".to_owned()),
            ("diff.tab".to_owned(), "bench_tab".to_owned()),
        ])
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

    #[test]
    fn projection_groups_tabs_by_slot_and_keeps_registration_order() {
        let spec = LayoutSpecV1::parse(include_str!("../../../content/layouts/oficina.toml"))
            .expect("factory layout parses");
        let registration = vec![
            "board.page".to_owned(),
            "routine.page".to_owned(),
            "muster.page".to_owned(),
        ];
        let projection = project(&spec, &known(), &registration);
        assert_eq!(projection.bench_tabs, vec![
            "editor.tab".to_owned(), "diff.tab".to_owned(), "terminal.tab".to_owned(),
            "chat.tab".to_owned(),
        ]);
        assert_eq!(projection.sidebar_tabs, vec![
            "project.sidebar".to_owned(), "board.sidebar".to_owned(), "explorer.panel".to_owned(),
        ]);
        assert_eq!(projection.right_tabs, vec!["git.changes".to_owned()]);
        assert_eq!(projection.effective_rail_order, registration);
        assert_eq!(projection.rail_visible, Some(true));
        assert_eq!(projection.status_bar_visible, Some(true));
    }

    #[test]
    fn focus_layout_hides_rail_and_status_bar() {
        let spec = LayoutSpecV1::parse(include_str!("../../../content/layouts/foco.toml"))
            .expect("focus layout parses");
        let projection = project(&spec, &known(), &["muster.page".to_owned()]);
        assert_eq!(projection.rail_visible, Some(false));
        assert_eq!(projection.status_bar_visible, Some(false));
        assert_eq!(projection.bench_tabs, vec![
            "editor.tab".to_owned(), "terminal.tab".to_owned(), "chat.tab".to_owned(),
        ]);
    }

    #[test]
    fn rail_order_and_hidden_are_projected_in_that_order() {
        let spec = LayoutSpecV1::parse(include_str!("../../../content/layouts/mutirao.toml"))
            .expect("muster layout parses");
        let registration = vec![
            "board.page".to_owned(),
            "routine.page".to_owned(),
            "muster.page".to_owned(),
        ];
        let projection = project(&spec, &known(), &registration);
        assert_eq!(projection.effective_rail_order[0], "muster.page");
        let mut edited = projection.clone();
        apply_rail_edit(&mut edited, RailEdit::Hide("muster.page".to_owned()));
        apply_rail_edit(&mut edited, RailEdit::Move("board.page".to_owned(), 1));
        assert!(!edited.effective_rail_order.contains(&"muster.page".to_owned()));
        assert!(edited.rail_hidden.contains(&"muster.page".to_owned()));
        assert_eq!(edited.effective_rail_order[0], "routine.page");
        apply_rail_edit(&mut edited, RailEdit::Show("muster.page".to_owned()));
        assert!(edited.effective_rail_order.contains(&"muster.page".to_owned()));
        assert!(!edited.rail_hidden.contains(&"muster.page".to_owned()));
    }

    #[test]
    fn side_tabs_follow_the_declared_order() {
        let current: Vec<String> = ["project.sidebar", "board.sidebar", "explorer.panel"]
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
        let declared: Vec<String> = ["board.sidebar", "explorer.panel"]
            .iter()
            .map(|id| (*id).to_owned())
            .collect();
        let (order, first) = order_side_tabs(&current, &declared);
        assert_eq!(order, vec![
            "board.sidebar".to_owned(),
            "explorer.panel".to_owned(),
            "project.sidebar".to_owned(),
        ]);
        assert_eq!(first.as_deref(), Some("board.sidebar"));
        let (order, first) = order_side_tabs(&current, &[]);
        assert_eq!(order, current);
        assert_eq!(first, None);
    }

    #[test]
    fn export_preserves_the_tree_and_unavailable_refs() {
        let text = "schema_version = 1\nid = \"obscuro\"\nname = { pt-BR = \"Obscuro\", en = \"Obscure\" }\n[root]\nkind = \"leaf\"\nid = \"a\"\ntabs = [\"editor.tab\", \"future.tab\"]\nactive = 1\n";
        let spec = LayoutSpecV1::parse(text).expect("spec with an unknown ref parses");
        let mut projection = project(&spec, &known(), &[]);
        assert_eq!(
            projection.unavailable_refs,
            vec!["future.tab".to_owned()],
            "an unknown ref must be diagnosed, not silently dropped"
        );
        projection.rail_visible = Some(false);
        projection.rail_hidden = vec!["board.page".to_owned()];
        let out = export(&spec, &projection).expect("export");
        let again = LayoutSpecV1::parse(&out).expect("exported TOML parses");
        assert_eq!(again.root, spec.root, "tree and refs must survive an edit");
        assert_eq!(again.chrome.rail_visible, Some(false));
        assert_eq!(again.chrome.rail_hidden, vec!["board.page".to_owned()]);
        assert_eq!(again.root.view_refs()[1].view_id, "future.tab");
    }

    #[test]
    fn unavailable_ref_messages_exist_in_both_locales() {
        let en = include_str!("../i18n/en.ftl");
        let pt_br = include_str!("../i18n/pt-BR.ftl");
        for key in [
            "shell-layout-unavailable-ref-title",
            "shell-layout-unavailable-ref",
        ] {
            assert!(en.contains(&format!("{key} =")), "missing {key} in en");
            assert!(pt_br.contains(&format!("{key} =")), "missing {key} in pt-BR");
        }
    }

    #[test]
    fn new_layout_exports_at_most_three_tabs() {
        let tabs: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| (*s).to_owned()).collect();
        let spec = new_spec("novo", "Novo", "New", &tabs);
        assert_eq!(spec.validate(), Ok(()));
        match &spec.root {
            LayoutNodeSpec::Leaf { tabs, .. } => assert_eq!(tabs.len(), 3),
            _ => panic!("new layout must be a single leaf"),
        }
    }
}
