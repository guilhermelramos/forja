//! Creating, selecting and removing account homes.

use crate::model::{Account, Data, ProfileMeta};
use forja_kernel::Ctx;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

/// Accounts live under `<data>/accounts/<profile>/<id>`.
pub fn root() -> PathBuf {
    forja_kernel::paths::default_data_dir().join("accounts")
}

/// The same root, derived from the runtime store so isolated screenshots and
/// tests never touch the real `%APPDATA%\Forja`.
pub fn root_from(cx: &Ctx<'_>) -> PathBuf {
    cx.store()
        .path()
        .parent()
        .and_then(std::path::Path::parent)
        .map(|data| data.join("accounts"))
        .unwrap_or_else(root)
}

pub fn create_at(
    data: &mut Data,
    meta: &ProfileMeta,
    label: &str,
    root: &Path,
) -> Result<Account, String> {
    if meta.env.trim().is_empty() {
        return Err(format!("profile `{}` has no account env", meta.id));
    }
    let label = label.trim();
    let label = if label.is_empty() { "Account" } else { label };
    let id = new_id(data, &meta.id, label);
    let dir = root.join(&meta.id).join(&id);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("could not create account home: {error}"))?;
    let account = Account {
        profile_id: meta.id.clone(),
        id: id.clone(),
        label: label.to_owned(),
        dir: dir.display().to_string(),
    };
    data.accounts.push(account.clone());
    if !data.active.contains_key(&meta.id) {
        data.active.insert(meta.id.clone(), id);
    }
    Ok(account)
}

pub fn select(data: &mut Data, profile_id: &str, id: &str) -> Result<(), String> {
    if data.find(profile_id, id).is_none() {
        return Err("unknown account".to_owned());
    }
    data.active.insert(profile_id.to_owned(), id.to_owned());
    Ok(())
}

pub fn remove_at(
    data: &mut Data,
    profile_id: &str,
    id: &str,
    delete_files: bool,
    root: &Path,
) -> Result<(), String> {
    let index = data
        .accounts
        .iter()
        .position(|account| account.profile_id == profile_id && account.id == id)
        .ok_or_else(|| "unknown account".to_owned())?;
    let account = data.accounts.remove(index);
    if data.active_id(profile_id) == Some(id) {
        match data
            .accounts
            .iter()
            .find(|item| item.profile_id == profile_id)
        {
            Some(next) => {
                data.active.insert(profile_id.to_owned(), next.id.clone());
            }
            None => {
                data.active.remove(profile_id);
            }
        }
    }
    if delete_files {
        remove_folder(Path::new(&account.dir), root, profile_id)?;
    }
    Ok(())
}

/// Deletes a config home. Only `<root>/<profile>/<id>` is accepted; `..` and
/// paths outside the accounts root are refused.
pub fn remove_folder(path: &Path, root: &Path, profile_id: &str) -> Result<(), String> {
    let climbs = path
        .components()
        .any(|part| matches!(part, Component::ParentDir));
    let parent = root.join(profile_id);
    if climbs || !path.starts_with(root) || path.parent() != Some(parent.as_path()) {
        return Err("refusing to delete a folder outside the accounts root".to_owned());
    }
    if !path.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(path).map_err(|error| format!("could not remove account home: {error}"))
}

pub fn env_for(
    data: &Data,
    profiles: &[ProfileMeta],
    profile_id: &str,
) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    let Some(meta) = profiles.iter().find(|meta| meta.id == profile_id) else {
        return env;
    };
    if let Some(account) = data.active_account(profile_id) {
        env.insert(meta.env.clone(), account.dir.clone());
    }
    env
}

pub fn account_env(meta: &ProfileMeta, account: &Account) -> BTreeMap<String, String> {
    BTreeMap::from([(meta.env.clone(), account.dir.clone())])
}

pub fn roots(data: &Data) -> Vec<Account> {
    data.accounts.clone()
}

fn new_id(data: &Data, profile_id: &str, label: &str) -> String {
    let slug: String = label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(20)
        .collect::<String>()
        .to_ascii_lowercase();
    let slug = if slug.is_empty() {
        "account".to_owned()
    } else {
        slug
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut candidate = format!("{slug}-{stamp:x}");
    let mut suffix = 1;
    while data
        .accounts
        .iter()
        .any(|account| account.profile_id == profile_id && account.id == candidate)
    {
        candidate = format!("{slug}-{stamp:x}-{suffix}");
        suffix += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str) -> ProfileMeta {
        ProfileMeta {
            id: id.into(),
            name: id.into(),
            env: "CLAUDE_CONFIG_DIR".into(),
            login: vec!["auth".into(), "login".into()],
        }
    }

    fn temp_root() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-accounts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_select_and_remove_keep_the_active_account_valid() {
        let root = temp_root();
        let meta = meta("claude-code");
        let mut data = Data::default();
        let first = create_at(&mut data, &meta, "Ana", &root).unwrap();
        let second = create_at(&mut data, &meta, "Bo", &root).unwrap();
        assert_eq!(data.active_id(&meta.id), Some(first.id.as_str()));
        select(&mut data, &meta.id, &second.id).unwrap();
        assert_eq!(
            env_for(&data, std::slice::from_ref(&meta), &meta.id)["CLAUDE_CONFIG_DIR"],
            second.dir
        );
        remove_at(&mut data, &meta.id, &second.id, true, &root).unwrap();
        assert_eq!(data.active_id(&meta.id), Some(first.id.as_str()));
        assert!(!Path::new(&second.dir).exists());
        remove_at(&mut data, &meta.id, &first.id, true, &root).unwrap();
        assert!(data.active_id(&meta.id).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_folder_outside_the_accounts_root_is_never_deleted() {
        let root = temp_root();
        let meta = meta("claude-code");
        let mut data = Data::default();
        create_at(&mut data, &meta, "Ana", &root).unwrap();
        let victim = root.join("..").join("victim");
        std::fs::create_dir_all(&victim).unwrap();
        data.accounts[0].dir = victim.display().to_string();
        let id = data.accounts[0].id.clone();
        assert!(remove_at(&mut data, &meta.id, &id, true, &root).is_err());
        assert!(victim.exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(victim);
    }

    #[test]
    fn duplicate_labels_get_distinct_ids() {
        let root = temp_root();
        let meta = meta("codex");
        let mut data = Data::default();
        let first = create_at(&mut data, &meta, "Work", &root).unwrap();
        let second = create_at(&mut data, &meta, "Work", &root).unwrap();
        assert_ne!(first.id, second.id);
        let _ = std::fs::remove_dir_all(root);
    }
}
