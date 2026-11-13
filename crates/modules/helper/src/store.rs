//! Store do módulo: preferências por perfil e consentimento de hooks.

use crate::launch::ProfileOverride;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Arquivo guardado em `%APPDATA%\Forja\modules\helper.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HelperFile {
    /// Ajustes por id de perfil.
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileOverride>,
    /// Consentimento de hooks por id de perfil.
    #[serde(default)]
    pub hook_consent: BTreeMap<String, bool>,
}
