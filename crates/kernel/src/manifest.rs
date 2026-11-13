//! Module metadata shown by the shell and checked by contract tests.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// Dotted id in English, for example `hello`.
    pub id: String,
    /// i18n key of the module name, resolved by the shell.
    pub name_key: String,
    pub version: String,
    /// Ids of modules that must register before this one.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

impl ModuleManifest {
    pub fn new(id: impl Into<String>, name_key: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name_key: name_key.into(),
            version: version.into(),
            depends_on: Vec::new(),
        }
    }

    pub fn depends_on(mut self, ids: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.depends_on = ids.into_iter().map(Into::into).collect();
        self
    }
}
