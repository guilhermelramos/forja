//! Accounts with separate CLI config homes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub profile_id: String,
    pub id: String,
    pub label: String,
    /// Config home for this account.
    pub dir: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    pub accounts: Vec<Account>,
    /// Active account per profile.
    #[serde(default)]
    pub active: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMeta {
    pub id: String,
    pub name: String,
    pub env: String,
    pub login: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountView {
    pub profile_id: String,
    pub id: String,
    pub label: String,
    pub dir: String,
    pub active: bool,
    pub exists: bool,
}

impl Data {
    pub fn active_id(&self, profile_id: &str) -> Option<&str> {
        self.active.get(profile_id).map(String::as_str)
    }

    pub fn active_account(&self, profile_id: &str) -> Option<&Account> {
        let id = self.active_id(profile_id)?;
        self.accounts
            .iter()
            .find(|account| account.profile_id == profile_id && account.id == id)
    }

    pub fn find(&self, profile_id: &str, id: &str) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.profile_id == profile_id && account.id == id)
    }

    pub fn view(&self, profile_id: &str) -> Vec<AccountView> {
        self.accounts
            .iter()
            .filter(|account| account.profile_id == profile_id)
            .map(|account| AccountView {
                profile_id: account.profile_id.clone(),
                id: account.id.clone(),
                label: account.label.clone(),
                dir: account.dir.clone(),
                active: self.active_id(profile_id) == Some(account.id.as_str()),
                exists: std::path::Path::new(&account.dir).is_dir(),
            })
            .collect()
    }
}
