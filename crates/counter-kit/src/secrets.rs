//! Cofre de segredos dos balcões. Tokens vivem só no cofre do sistema, sob o nome
//! `forja/<módulo>` + conta (o host do balcão). Nunca em arquivo, log ou resposta RPC.

use forja_contracts::CounterError;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// Nome de serviço no cofre para um módulo: `forja/counter_github`.
pub fn service_for(module: &str) -> String {
    format!("forja/{module}")
}

pub trait SecretStore: Send + Sync {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CounterError>;
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CounterError>;
    /// Apagar o que não existe não é erro.
    fn delete(&self, service: &str, account: &str) -> Result<(), CounterError>;
}

/// Cofre em memória, para testes. Não toca o cofre do sistema.
#[derive(Default)]
pub struct MemorySecretStore {
    entries: Mutex<BTreeMap<(String, String), String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CounterError> {
        let entries = self.entries.lock().map_err(|_| vault_poisoned())?;
        Ok(entries
            .get(&(service.to_owned(), account.to_owned()))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CounterError> {
        let mut entries = self.entries.lock().map_err(|_| vault_poisoned())?;
        entries.insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CounterError> {
        let mut entries = self.entries.lock().map_err(|_| vault_poisoned())?;
        entries.remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

/// Cofre usado enquanto o acesso ao Gerenciador de Credenciais do Windows não está
/// instalado. Toda operação falha com `Vault`; o segredo nunca cai em arquivo.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnavailableStore;

impl SecretStore for UnavailableStore {
    fn get(&self, _service: &str, _account: &str) -> Result<Option<String>, CounterError> {
        Err(unavailable())
    }

    fn set(&self, _service: &str, _account: &str, _secret: &str) -> Result<(), CounterError> {
        Err(unavailable())
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), CounterError> {
        Err(unavailable())
    }
}

fn unavailable() -> CounterError {
    CounterError::Vault {
        message: "Windows Credential Manager is not available in this build".to_owned(),
    }
}

fn vault_poisoned() -> CounterError {
    CounterError::Vault {
        message: "secret store lock was poisoned".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_and_deletes_quietly() {
        let store = MemorySecretStore::new();
        let service = service_for("counter_github");
        assert_eq!(store.get(&service, "github.com"), Ok(None));
        store.set(&service, "github.com", "ghp_test").unwrap();
        assert_eq!(store.get(&service, "github.com"), Ok(Some("ghp_test".into())));
        assert_eq!(store.get(&service, "other.host"), Ok(None));
        store.delete(&service, "github.com").unwrap();
        store.delete(&service, "github.com").unwrap();
        assert_eq!(store.get(&service, "github.com"), Ok(None));
    }

    #[test]
    fn service_names_follow_the_forja_module_pattern() {
        assert_eq!(service_for("counter_jira"), "forja/counter_jira");
    }

    #[test]
    fn unavailable_store_never_pretends_to_save() {
        let store = UnavailableStore;
        assert!(matches!(store.set("s", "a", "x"), Err(CounterError::Vault { .. })));
        assert!(matches!(store.get("s", "a"), Err(CounterError::Vault { .. })));
    }
}
