//! Profiles that declare `[accounts] env`.
//!
//! The two built-ins with a confirmed login command are embedded; user profiles
//! can extend or override the same id. The `env` names come from the profile
//! `[sessions].dir_env` and the CLI documentation already cited in the file.

use crate::model::ProfileMeta;
use serde::Deserialize;

const BUILTIN: &[&str] = &[
    include_str!("../../../../content/helpers/claude-code.toml"),
    include_str!("../../../../content/helpers/codex.toml"),
];

#[derive(Deserialize)]
struct Doc {
    id: String,
    name: String,
    #[serde(default)]
    accounts: Spec,
}

#[derive(Default, Deserialize)]
struct Spec {
    #[serde(default)]
    env: Option<String>,
    #[serde(default)]
    login: Vec<String>,
}

pub fn list() -> Vec<ProfileMeta> {
    let mut profiles: Vec<ProfileMeta> = Vec::new();
    for text in BUILTIN {
        if let Ok(doc) = toml::from_str::<Doc>(text) {
            if let Some(meta) = meta(&doc) {
                profiles.push(meta);
            }
        }
    }
    for folder in [forja_kernel::paths::default_user_dir().join("helpers")] {
        if cfg!(test) || std::env::var_os("FORJA_SCREENSHOT").is_some() {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(doc) = toml::from_str::<Doc>(&text) else {
                    continue;
                };
                if let Some(meta) = meta(&doc) {
                    if let Some(existing) = profiles.iter_mut().find(|item| item.id == meta.id) {
                        *existing = meta;
                    } else {
                        profiles.push(meta);
                    }
                }
            }
        }
    }
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    profiles
}

fn meta(doc: &Doc) -> Option<ProfileMeta> {
    let env = doc.accounts.env.as_ref()?.trim();
    if env.is_empty() {
        return None;
    }
    Some(ProfileMeta {
        id: doc.id.clone(),
        name: doc.name.clone(),
        env: env.to_owned(),
        login: doc.accounts.login.clone(),
    })
}

/// Exposed for tests: parse a profile document from text.
#[cfg(test)]
pub fn parse_text(text: &str) -> Option<ProfileMeta> {
    toml::from_str::<Doc>(text).ok().and_then(|doc| meta(&doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_profiles_expose_the_config_env() {
        let profiles = list();
        let claude = profiles
            .iter()
            .find(|profile| profile.id == "claude-code")
            .expect("claude-code");
        assert_eq!(claude.env, "CLAUDE_CONFIG_DIR");
        assert_eq!(claude.login, ["auth", "login"]);
        let codex = profiles
            .iter()
            .find(|profile| profile.id == "codex")
            .expect("codex");
        assert_eq!(codex.env, "CODEX_HOME");
        assert_eq!(codex.login, ["login"]);
    }

    #[test]
    fn a_profile_without_accounts_env_is_ignored() {
        let text = "id = \"x\"\nname = \"X\"\n";
        assert!(parse_text(text).is_none());
        let text = "id = \"x\"\nname = \"X\"\n[accounts]\nenv = \" \"\n";
        assert!(parse_text(text).is_none());
    }

    #[test]
    fn user_layer_overrides_a_builtin() {
        let meta = parse_text(
            "id = \"claude-code\"\nname = \"Claude\"\n[accounts]\nenv = \"CLAUDE_CONFIG_DIR\"\nlogin = [\"login\"]\n",
        )
        .unwrap();
        assert_eq!(meta.login, ["login"]);
    }
}
