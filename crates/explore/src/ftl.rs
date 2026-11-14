//! Loads all `i18n/en.ftl` and `i18n/pt-BR.ftl` files in the repository
//! to detect untranslated visible UI strings (invariant 2).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Dictionary of locale-specific static UI strings parsed from `.ftl` files.
#[derive(Debug, Clone, Default)]
pub struct TranslationDict {
    /// Static `en.ftl` values that differ from their `pt-BR` counterpart
    /// and never appear as a valid `pt-BR` value.
    en_only: HashSet<String>,
    /// Static `pt-BR.ftl` values that differ from their `en` counterpart
    /// and never appear as a valid `en` value.
    pt_only: HashSet<String>,
    /// All static `en.ftl` values (lowercased) for distinguishing legitimate
    /// English labels in `en` locale from raw technical errors.
    en_static_lower: HashSet<String>,
}

impl TranslationDict {
    /// Auto-discovers the repository root (from `hint`, cwd, or `CARGO_MANIFEST_DIR`)
    /// and loads every `i18n/{en,pt-BR}.ftl` pair found under it.
    pub fn discover_and_load(hint: Option<&Path>) -> Self {
        if let Some(root) = find_repo_root(hint) {
            return Self::load_from_repo(&root);
        }
        Self::default()
    }

    /// Loads all `i18n/en.ftl` and `i18n/pt-BR.ftl` pairs under `repo_root`.
    pub fn load_from_repo(repo_root: &Path) -> Self {
        let mut pairs = Vec::new();
        collect_i18n_dirs(repo_root, &mut pairs);
        let mut loaded_pairs = Vec::new();
        for dir in pairs {
            let en_path = dir.join("en.ftl");
            let pt_path = dir.join("pt-BR.ftl");
            let en_text = std::fs::read_to_string(&en_path).unwrap_or_default();
            let pt_text = std::fs::read_to_string(&pt_path).unwrap_or_default();
            loaded_pairs.push((en_text, pt_text));
        }
        Self::from_ftl_pairs(&loaded_pairs)
    }

    /// Builds a dictionary from `(en_ftl_content, pt_br_ftl_content)` pairs.
    pub fn from_ftl_pairs(pairs: &[(String, String)]) -> Self {
        let mut all_en = HashSet::new();
        let mut all_pt = HashSet::new();
        let mut en_static_lower = HashSet::new();
        let mut parsed_pairs = Vec::with_capacity(pairs.len());

        for (en_raw, pt_raw) in pairs {
            let en_map = parse_ftl(en_raw);
            let pt_map = parse_ftl(pt_raw);
            for val in en_map.values() {
                if is_static_checkable(val) {
                    all_en.insert(val.clone());
                }
                if !val.contains('{') && !val.contains('}') {
                    en_static_lower.insert(val.to_ascii_lowercase());
                }
            }
            for val in pt_map.values() {
                if is_static_checkable(val) {
                    all_pt.insert(val.clone());
                }
            }
            parsed_pairs.push((en_map, pt_map));
        }

        let mut en_only = HashSet::new();
        let mut pt_only = HashSet::new();

        for (en_map, pt_map) in &parsed_pairs {
            for (key, en_val) in en_map {
                if !is_static_checkable(en_val) {
                    continue;
                }
                let pt_val = pt_map.get(key);
                if pt_val != Some(en_val) && !all_pt.contains(en_val) {
                    en_only.insert(en_val.clone());
                }
            }
            for (key, pt_val) in pt_map {
                if !is_static_checkable(pt_val) {
                    continue;
                }
                let en_val = en_map.get(key);
                if en_val != Some(pt_val) && !all_en.contains(pt_val) {
                    pt_only.insert(pt_val.clone());
                }
            }
        }

        Self {
            en_only,
            pt_only,
            en_static_lower,
        }
    }

    /// Returns `Some(matched_text)` if `text` is an untranslated string from the opposite locale.
    pub fn check_untranslated(&self, locale: &str, text: &str) -> Option<String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let is_en = locale.trim().to_ascii_lowercase().starts_with("en");
        let foreign = if is_en { &self.pt_only } else { &self.en_only };
        if foreign.contains(trimmed) {
            return Some(trimmed.to_owned());
        }
        None
    }

    /// Returns `true` if `text` is a known static string in `en.ftl`.
    pub fn is_known_en_label(&self, text: &str) -> bool {
        self.en_static_lower.contains(&text.trim().to_ascii_lowercase())
    }

    pub fn en_only_count(&self) -> usize {
        self.en_only.len()
    }

    pub fn pt_only_count(&self) -> usize {
        self.pt_only.len()
    }
}

fn is_static_checkable(value: &str) -> bool {
    if value.contains('{') || value.contains('}') {
        return false;
    }
    value.chars().filter(|c| c.is_alphabetic()).count() >= 2
}

/// Parses `key = value` entries from a `.ftl` file, joining indented continuation lines.
pub fn parse_ftl(content: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();

    for line in content.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            if let Some(key) = current_key.take() {
                map.insert(key, current_val.trim().to_owned());
                current_val.clear();
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                if !key.is_empty() && !key.starts_with('.') {
                    current_key = Some(key.to_owned());
                    current_val.push_str(v.trim_start());
                }
            }
        } else if current_key.is_some() && !trimmed_start.is_empty() {
            if !current_val.is_empty() {
                current_val.push(' ');
            }
            current_val.push_str(trimmed_start);
        }
    }
    if let Some(key) = current_key.take() {
        map.insert(key, current_val.trim().to_owned());
    }
    map
}

fn find_repo_root(hint: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = hint {
        candidates.push(path.to_path_buf());
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        candidates.push(PathBuf::from(manifest_dir));
    }
    for start in candidates {
        let mut cur = Some(start.as_path());
        while let Some(dir) = cur {
            if dir.join("Cargo.toml").is_file() && dir.join("crates/shell/i18n/en.ftl").is_file() {
                return Some(dir.to_path_buf());
            }
            cur = dir.parent();
        }
    }
    None
}

fn collect_i18n_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut has_en = false;
    let mut has_pt = false;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name_str.as_ref(), "target" | ".git" | "node_modules") {
                continue;
            }
            subdirs.push(path);
        } else if name_str == "en.ftl" {
            has_en = true;
        } else if name_str == "pt-BR.ftl" {
            has_pt = true;
        }
    }
    if has_en && has_pt && dir.file_name().is_some_and(|n| n == "i18n") {
        out.push(dir.to_path_buf());
    }
    for sub in subdirs {
        collect_i18n_dirs(&sub, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_untranslated_strings_in_both_directions() {
        let en = "shell-settings = Settings\nshell-name = Forja\nshell-greet = Hi { $name }\n".to_owned();
        let pt = "shell-settings = Ajustes\nshell-name = Forja\nshell-greet = Oi { $name }\n".to_owned();
        let dict = TranslationDict::from_ftl_pairs(&[(en, pt)]);

        assert_eq!(dict.check_untranslated("pt-BR", "Settings"), Some("Settings".to_owned()));
        assert_eq!(dict.check_untranslated("pt-BR", "Ajustes"), None);
        assert_eq!(dict.check_untranslated("pt-BR", "Forja"), None);

        assert_eq!(dict.check_untranslated("en", "Ajustes"), Some("Ajustes".to_owned()));
        assert_eq!(dict.check_untranslated("en", "Settings"), None);
        assert_eq!(dict.check_untranslated("en", "Forja"), None);
    }

    #[test]
    fn excludes_strings_valid_in_target_locale_via_another_key() {
        let en = "k1 = Open\nk2 = Open\n".to_owned();
        let pt = "k1 = Abrir\nk2 = Open\n".to_owned();
        let dict = TranslationDict::from_ftl_pairs(&[(en, pt)]);
        assert_eq!(dict.check_untranslated("pt-BR", "Open"), None);
    }

    #[test]
    fn loads_repo_ftl_files() {
        let dict = TranslationDict::discover_and_load(None);
        assert!(dict.en_only_count() > 50);
        assert!(dict.pt_only_count() > 50);
        assert_eq!(dict.check_untranslated("pt-BR", "Settings"), Some("Settings".into()));
        assert_eq!(dict.check_untranslated("en", "Ajustes"), Some("Ajustes".into()));
    }
}
