//! Catalog metadata for settings navigation; never duplicates the group list.
use forja_kernel::{Locale, SettingSpec};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub(crate) struct Group {
    pub id: String,
    pt: String,
    en: String,
}
impl Group {
    pub fn label(&self, locale: Locale) -> &str {
        if locale == Locale::PtBr {
            &self.pt
        } else {
            &self.en
        }
    }
}
pub(crate) struct Catalog {
    pub groups: Vec<Group>,
    settings: BTreeMap<String, String>,
    modules: BTreeMap<String, String>,
    restart: BTreeSet<String>,
    order: BTreeMap<String, usize>,
}
pub(crate) fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let table: toml::Value = toml::from_str(include_str!("../../../design/settings.toml"))
            .expect("settings catalog");
        let rows = |name: &str| table[name].as_array().expect("catalog array");
        let field = |row: &toml::Value, name: &str| {
            row.get(name)
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .to_owned()
        };
        Catalog {
            groups: rows("group")
                .iter()
                .map(|row| Group {
                    id: field(row, "id"),
                    pt: field(row, "label_pt"),
                    en: field(row, "label_en"),
                })
                .collect(),
            order: rows("setting")
                .iter()
                .enumerate()
                .map(|(index, row)| (field(row, "key"), index))
                .collect(),
            settings: rows("setting")
                .iter()
                .map(|row| (field(row, "key"), field(row, "group")))
                .collect(),
            modules: rows("module")
                .iter()
                .map(|row| (field(row, "id"), field(row, "group")))
                .collect(),
            restart: rows("setting")
                .iter()
                .filter(|row| row.get("restart").and_then(toml::Value::as_bool) == Some(true))
                .map(|row| field(row, "key"))
                .collect(),
        }
    })
}
impl Catalog {
    pub fn group_for(&self, spec: &SettingSpec) -> &str {
        self.settings
            .get(&spec.key)
            .filter(|id| self.groups.iter().any(|group| group.id == **id))
            .map(String::as_str)
            .unwrap_or_else(|| self.module_group(spec.module()))
    }
    pub fn module_group(&self, module: &str) -> &str {
        self.modules
            .get(module)
            .or_else(|| {
                module
                    .split_once('_')
                    .and_then(|(prefix, _)| self.modules.get(prefix))
            })
            .map(String::as_str)
            .unwrap_or("advanced")
    }
    pub fn order(&self, key: &str) -> usize {
        self.order.get(key).copied().unwrap_or(usize::MAX)
    }
    pub fn restart(&self, key: &str) -> bool {
        self.restart.contains(key)
    }
}
pub(crate) fn matches(query: &str, key: &str, label: &str, help: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || key.to_lowercase().contains(&query)
        || [label, help].iter().any(|text| {
            let text = text.to_lowercase();
            text.match_indices(&query).any(|(index, _)| {
                text[..index]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !c.is_alphanumeric())
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_restart_flags_come_from_the_catalog() {
        assert_eq!(catalog().groups.len(), 13);
        assert_eq!(catalog().groups[0].id, "appearance");
        assert!(catalog().restart("app.performance.worker_threads"));
        assert!(!catalog().restart("appearance.theme.mode"));
        assert_eq!(catalog().module_group("helper"), "helpers");
        assert_eq!(catalog().module_group("counter_jira"), "requests");
        let spec = SettingSpec::new(
            "terminal.recipes.confirm",
            forja_kernel::SettingKind::Bool,
            forja_kernel::SettingValue::Bool(false),
            "label",
            "help",
        );
        assert_eq!(catalog().group_for(&spec), "terminal");
    }
    #[test]
    fn search_matches_help_and_keys_without_matching_inside_unrelated_words() {
        assert!(matches("tema", "appearance.theme.mode", "Tema", ""));
        assert!(matches(
            "rolagem",
            "terminal.history.limit",
            "Histórico",
            "Limite de rolagem"
        ));
        assert!(!matches("tema", "browser.system", "Login no sistema", ""));
        assert!(matches("appearance.", "appearance.theme.mode", "", ""));
    }
}
