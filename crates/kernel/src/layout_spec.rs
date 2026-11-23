//! Arquivo de arranjo da oficina (`content/layouts/*.toml`).
//!
//! Este módulo não depende de egui, não lê arquivos e não conhece o registro de
//! views: apenas converte texto em uma árvore validada e de volta para TOML.
//! A validação estrutural (limites, ids, razões) é independente da janela.
//! A validação contra as contribuições registradas pertence ao carregador.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Versão do schema aceita por esta fatia.
pub const SCHEMA_VERSION: u32 = 1;
/// Maior arquivo aceito (256 KiB).
pub const MAX_INPUT_BYTES: usize = 256 * 1024;
/// Maior número de divisões num caminho da raiz até uma folha.
pub const MAX_SPLIT_DEPTH: usize = 7;
/// Maior número de folhas numa árvore.
pub const MAX_LEAVES: usize = 8;
/// Maior número de abas em toda a árvore.
pub const MAX_TABS: usize = 64;
/// Maior comprimento de um segmento de id.
pub const MAX_ID_LEN: usize = 64;

/// Erro de leitura ou validação de um arranjo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutError {
    /// Caminho lógico dentro do arquivo, por exemplo `root.second.ratio`.
    pub path: String,
    pub message: String,
}

impl LayoutError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for LayoutError {}

/// Nome exibido do arranjo, nos dois idiomas da interface.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedName {
    #[serde(rename = "pt-BR")]
    pub pt_br: String,
    pub en: String,
}

/// Escolhas de chrome declaradas pelo arranjo.
///
/// `None` significa "herdar a preferência do usuário"; o campo simplesmente
/// não aparece no arquivo.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutChrome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail_visible: Option<bool>,
    #[serde(default)]
    pub rail_hidden: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rail_order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_bar_visible: Option<bool>,
}

/// Direção de uma divisão. A primeira filha vem antes no eixo visual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDir {
    Columns,
    Rows,
}

/// Referência a uma view, com instância opaca estável.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewRef {
    pub view_id: String,
    pub instance: String,
}

impl ViewRef {
    pub fn new(view_id: impl Into<String>, instance: impl Into<String>) -> Self {
        Self {
            view_id: view_id.into(),
            instance: instance.into(),
        }
    }
}

/// Uma aba no arquivo: uma string com o id da view ou uma tabela opcional.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TabSpec {
    Id(String),
    Table {
        view: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance: Option<String>,
    },
}

impl TabSpec {
    /// Instância ausente usa a chave canônica `"1"`.
    pub fn view_ref(&self) -> ViewRef {
        match self {
            Self::Id(view) => ViewRef::new(view.clone(), "1"),
            Self::Table { view, instance } => {
                ViewRef::new(view.clone(), instance.clone().unwrap_or_else(|| "1".to_owned()))
            }
        }
    }
}

/// Nó da árvore declarada no arquivo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum LayoutNodeSpec {
    Split {
        dir: SplitDir,
        ratio: f64,
        first: Box<LayoutNodeSpec>,
        second: Box<LayoutNodeSpec>,
    },
    Leaf {
        id: String,
        #[serde(default)]
        collapsible: bool,
        #[serde(default)]
        tabs: Vec<TabSpec>,
        #[serde(default)]
        active: usize,
    },
}

impl LayoutNodeSpec {
    /// Todas as referências de abas, em ordem de visita (primeira filha primeiro).
    pub fn view_refs(&self) -> Vec<ViewRef> {
        let mut refs = Vec::new();
        self.collect_refs(&mut refs);
        refs
    }

    /// Ids de todas as folhas, em ordem de visita.
    pub fn leaf_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.collect_leaf_ids(&mut ids);
        ids
    }

    fn collect_refs(&self, out: &mut Vec<ViewRef>) {
        match self {
            Self::Split { first, second, .. } => {
                first.collect_refs(out);
                second.collect_refs(out);
            }
            Self::Leaf { tabs, .. } => out.extend(tabs.iter().map(TabSpec::view_ref)),
        }
    }

    fn collect_leaf_ids(&self, out: &mut Vec<String>) {
        match self {
            Self::Split { first, second, .. } => {
                first.collect_leaf_ids(out);
                second.collect_leaf_ids(out);
            }
            Self::Leaf { id, .. } => out.push(id.clone()),
        }
    }
}

/// Arranjo serializado: `LayoutSpecV1 { schema_version, id, name, chrome, root }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutSpecV1 {
    pub schema_version: u32,
    pub id: String,
    pub name: LocalizedName,
    #[serde(default)]
    pub chrome: LayoutChrome,
    pub root: LayoutNodeSpec,
}

impl LayoutSpecV1 {
    /// Lê e valida um arquivo de arranjo.
    pub fn parse(text: &str) -> Result<Self, LayoutError> {
        if text.len() > MAX_INPUT_BYTES {
            return Err(LayoutError::new(
                "file",
                format!("layout file exceeds {MAX_INPUT_BYTES} bytes"),
            ));
        }
        let spec: Self = toml::from_str(text)
            .map_err(|error| LayoutError::new("file", error.to_string()))?;
        spec.validate()?;
        Ok(spec)
    }

    /// Serializa de volta para TOML, preservando o formato das abas.
    pub fn to_toml(&self) -> Result<String, LayoutError> {
        toml::to_string(self)
            .map_err(|error| LayoutError::new("file", error.to_string()))
    }

    /// Validação estrutural completa, independente do registro de views.
    pub fn validate(&self) -> Result<(), LayoutError> {
        self.validate_views(None)
    }

    /// Validação que também confere os ids de view conhecidos, quando fornecidos.
    ///
    /// Referências desconhecidas e bem formadas são preservadas: elas viram
    /// cartão de painel indisponível na interface, não erro de arquivo.
    pub fn validate_views(&self, known_views: Option<&BTreeSet<String>>) -> Result<(), LayoutError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(LayoutError::new(
                "schema_version",
                format!(
                    "unsupported layout version {} (expected {SCHEMA_VERSION})",
                    self.schema_version
                ),
            ));
        }
        validate_segment("id", &self.id)?;
        if self.name.pt_br.trim().is_empty() || self.name.en.trim().is_empty() {
            return Err(LayoutError::new("name", "both locales are required"));
        }
        if self.name.pt_br.len() > 128 || self.name.en.len() > 128 {
            return Err(LayoutError::new("name", "localized name is too long"));
        }
        for (index, id) in self.chrome.rail_hidden.iter().enumerate() {
            validate_view_id(&format!("chrome.rail_hidden[{index}]"), id)?;
        }
        if let Some(order) = &self.chrome.rail_order {
            for (index, id) in order.iter().enumerate() {
                validate_view_id(&format!("chrome.rail_order[{index}]"), id)?;
            }
        }

        // O conjunto de views registradas ainda não decide multiplicidade nesta
        // fatia: uma referência desconhecida e bem formada é preservada como
        // painel indisponível. Aqui só importa a forma e a unicidade do par.
        let _ = known_views;
        let refs = self.root.view_refs();
        let mut seen_refs = BTreeSet::new();
        for view in &refs {
            validate_view_id("root", &view.view_id)?;
            validate_segment("root", &view.instance)?;
            if !seen_refs.insert(view.clone()) {
                return Err(LayoutError::new(
                    "root",
                    format!(
                        "repeated view reference `{}.{}`",
                        view.view_id, view.instance
                    ),
                ));
            }
        }
        if refs.len() > MAX_TABS {
            return Err(LayoutError::new(
                "root",
                format!("layout has more than {MAX_TABS} tabs"),
            ));
        }

        let leaf_ids = self.root.leaf_ids();
        let mut seen_leaves = BTreeSet::new();
        for id in &leaf_ids {
            if !seen_leaves.insert(id) {
                return Err(LayoutError::new(
                    "root",
                    format!("repeated leaf id `{id}`"),
                ));
            }
        }
        if leaf_ids.len() > MAX_LEAVES {
            return Err(LayoutError::new(
                "root",
                format!("layout has more than {MAX_LEAVES} leaves"),
            ));
        }
        validate_node(&self.root, "root", 0)?;
        Ok(())
    }
}

fn validate_node(node: &LayoutNodeSpec, path: &str, depth: usize) -> Result<(), LayoutError> {
    match node {
        LayoutNodeSpec::Split {
            ratio, first, second, ..
        } => {
            if depth >= MAX_SPLIT_DEPTH {
                return Err(LayoutError::new(
                    path,
                    format!("more than {MAX_SPLIT_DEPTH} splits in a path"),
                ));
            }
            if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                return Err(LayoutError::new(
                    format!("{path}.ratio"),
                    "ratio must be finite and strictly between 0 and 1",
                ));
            }
            validate_node(first, &format!("{path}.first"), depth + 1)?;
            validate_node(second, &format!("{path}.second"), depth + 1)
        }
        LayoutNodeSpec::Leaf { id, tabs, active, .. } => {
            validate_segment(&format!("{path}.id"), id)?;
            let out_of_range = if tabs.is_empty() {
                *active != 0
            } else {
                *active >= tabs.len()
            };
            if out_of_range {
                return Err(LayoutError::new(
                    format!("{path}.active"),
                    "active tab index is out of range",
                ));
            }
            Ok(())
        }
    }
}

/// Ids de arquivo, folha e instância: segmentos ASCII minúsculos.
pub fn validate_segment(path: &str, id: &str) -> Result<(), LayoutError> {
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .enumerate()
            .all(|(index, byte)| match index {
                0 => byte.is_ascii_lowercase() || byte.is_ascii_digit(),
                _ => byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_',
            });
    if valid {
        Ok(())
    } else {
        Err(LayoutError::new(
            path,
            format!("invalid id `{id}` (expected lowercase ASCII segments up to {MAX_ID_LEN} chars)"),
        ))
    }
}

/// Ids de view são segmentos pontuados, exatamente como registrados.
pub fn validate_view_id(path: &str, id: &str) -> Result<(), LayoutError> {
    if id.is_empty() || id.len() > MAX_ID_LEN * 2 {
        return Err(LayoutError::new(path, format!("invalid view id `{id}`")));
    }
    for segment in id.split('.') {
        validate_segment(path, segment)?;
    }
    Ok(())
}

/// Conta folhas e abas sem alocar as listas completas.
pub fn count_tree(node: &LayoutNodeSpec) -> (usize, usize) {
    match node {
        LayoutNodeSpec::Split { first, second, .. } => {
            let (a_leaves, a_tabs) = count_tree(first);
            let (b_leaves, b_tabs) = count_tree(second);
            (a_leaves + b_leaves, a_tabs + b_tabs)
        }
        LayoutNodeSpec::Leaf { tabs, .. } => (1, tabs.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str, tabs: &[&str]) -> LayoutNodeSpec {
        LayoutNodeSpec::Leaf {
            id: id.to_owned(),
            collapsible: false,
            tabs: tabs
                .iter()
                .map(|view| TabSpec::Id((*view).to_owned()))
                .collect(),
            active: 0,
        }
    }

    fn spec(root: LayoutNodeSpec) -> LayoutSpecV1 {
        LayoutSpecV1 {
            schema_version: SCHEMA_VERSION,
            id: "teste".to_owned(),
            name: LocalizedName {
                pt_br: "Teste".to_owned(),
                en: "Test".to_owned(),
            },
            chrome: LayoutChrome::default(),
            root,
        }
    }

    fn split(first: LayoutNodeSpec, second: LayoutNodeSpec) -> LayoutNodeSpec {
        LayoutNodeSpec::Split {
            dir: SplitDir::Columns,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    #[test]
    fn parses_and_round_trips_a_split_with_mixed_tab_shapes() {
        let mut first = leaf("code", &["editor.tab"]);
        if let LayoutNodeSpec::Leaf { tabs, .. } = &mut first {
            *tabs = vec![
                TabSpec::Id("editor.tab".to_owned()),
                TabSpec::Table {
                    view: "chat.tab".to_owned(),
                    instance: None,
                },
            ];
        }
        let root = split(first, leaf("tools", &[]));
        let original = spec(root);
        let text = original.to_toml().unwrap();
        let parsed = LayoutSpecV1::parse(&text).unwrap();
        assert_eq!(parsed, original);
        assert_eq!(
            parsed.root.view_refs(),
            vec![
                ViewRef::new("editor.tab", "1"),
                ViewRef::new("chat.tab", "1"),
            ]
        );
    }

    #[test]
    fn tabs_table_keeps_the_instance_id() {
        let text = "schema_version = 1\nid = \"x\"\nname = { pt-BR = \"X\", en = \"X\" }\n[root]\nkind = \"leaf\"\nid = \"a\"\ntabs = [{ view = \"terminal.tab\", instance = \"2\" }]\nactive = 0\n";
        let parsed = LayoutSpecV1::parse(text).unwrap();
        assert_eq!(
            parsed.root.view_refs(),
            vec![ViewRef::new("terminal.tab", "2")]
        );
        assert!(parsed.to_toml().unwrap().contains("instance = \"2\""));
    }

    #[test]
    fn rejects_a_wrong_version_and_unknown_fields() {
        let mut wrong = spec(leaf("a", &["editor.tab"]));
        wrong.schema_version = 2;
        assert!(wrong.validate().unwrap_err().path == "schema_version");
        let text = "schema_version = 1\nid = \"x\"\nname = { pt-BR = \"X\", en = \"X\" }\nextra = 1\n[root]\nkind = \"leaf\"\nid = \"a\"\n";
        assert!(LayoutSpecV1::parse(text).is_err());
    }

    #[test]
    fn rejects_oversized_input() {
        assert!(LayoutSpecV1::parse(&"x".repeat(MAX_INPUT_BYTES + 1)).is_err());
    }

    #[test]
    fn rejects_bad_ids_ratios_and_active_index() {
        let mut bad = spec(leaf("A", &["editor.tab"]));
        assert!(bad.validate().is_err());
        bad.root = leaf("a", &["editor.tab"]);
        if let LayoutNodeSpec::Leaf { active, .. } = &mut bad.root {
            *active = 1;
        }
        assert!(bad.validate().is_err());
        let mut ratio = spec(split(leaf("a", &[]), leaf("b", &[])));
        if let LayoutNodeSpec::Split { ratio: value, .. } = &mut ratio.root {
            *value = f64::NAN;
        }
        assert!(ratio.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_leaves_and_references() {
        let duplicate_leaf = spec(split(leaf("a", &[]), leaf("a", &[])));
        assert!(duplicate_leaf.validate().is_err());
        let duplicate_ref = spec(split(
            leaf("a", &["editor.tab"]),
            leaf("b", &["editor.tab"]),
        ));
        assert!(duplicate_ref.validate().is_err());
    }

    #[test]
    fn enforces_depth_leaf_and_tab_limits() {
        let mut deep = leaf("leaf", &[]);
        for index in 0..=MAX_SPLIT_DEPTH {
            deep = split(deep, leaf(&format!("extra{index}"), &[]));
        }
        assert!(spec(deep).validate().is_err());

        // Use distinct instance ids to exercise the tab count, not uniqueness.
        let mut wide = leaf("only", &[]);
        if let LayoutNodeSpec::Leaf { tabs, .. } = &mut wide {
            *tabs = (0..=MAX_TABS)
                .map(|index| TabSpec::Table {
                    view: "x".to_owned(),
                    instance: Some(index.to_string()),
                })
                .collect();
        }
        assert!(spec(wide).validate().is_err());
    }

    #[test]
    fn known_views_still_accept_unknown_but_well_formed_ids() {
        let known = BTreeSet::from(["editor.tab".to_owned()]);
        let layout = spec(leaf("a", &["editor.tab", "future.tab"]));
        assert!(layout.validate_views(Some(&known)).is_ok());
    }

    #[test]
    fn factory_tables_are_readable_and_round_trip() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/layouts");
        for id in ["oficina", "foco", "revisao", "mutirao"] {
            let text = std::fs::read_to_string(root.join(format!("{id}.toml")))
                .unwrap_or_else(|error| panic!("{id}: {error}"));
            let parsed = LayoutSpecV1::parse(&text).unwrap_or_else(|error| panic!("{id}: {error}"));
            assert_eq!(parsed.id, id);
            assert_eq!(parsed.schema_version, SCHEMA_VERSION);
            let (leaves, tabs) = count_tree(&parsed.root);
            assert!(leaves > 0 && leaves <= MAX_LEAVES);
            assert!(tabs > 0 && tabs <= MAX_TABS);
            let again = LayoutSpecV1::parse(&parsed.to_toml().unwrap()).unwrap();
            assert_eq!(again, parsed, "{id} did not survive a round trip");
        }
        let focus = LayoutSpecV1::parse(
            &std::fs::read_to_string(root.join("foco.toml")).unwrap(),
        )
        .unwrap();
        assert_eq!(focus.chrome.rail_visible, Some(false));
        assert_eq!(focus.chrome.status_bar_visible, Some(false));
    }
}
