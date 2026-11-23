//! Store do módulo: preferências por perfil e consentimento de hooks.

use crate::launch::ProfileOverride;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Última sessão associada a uma bancada.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchSession {
    /// Perfil usado na execução.
    pub profile_id: String,
    /// Id da sessão conhecido (chat/archive), quando houver.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Caminho da bancada.
    pub bench_path: String,
    /// Momento do registro em RFC 3339.
    #[serde(default)]
    pub updated_at: String,
}

/// Arquivo guardado em `%APPDATA%\Forja\modules\helper.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HelperFile {
    /// Ajustes por id de perfil.
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileOverride>,
    /// Consentimento de hooks por id de perfil.
    #[serde(default)]
    pub hook_consent: BTreeMap<String, bool>,
    /// Última sessão por bancada (chave normalizada do caminho).
    #[serde(default)]
    pub bench_sessions: BTreeMap<String, BenchSession>,
}
