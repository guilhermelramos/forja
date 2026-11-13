//! Cofre de produção: Gerenciador de Credenciais do Windows, pelo crate `keyring`.
//!
//! Só guarda segredo no cofre do sistema. Não há caminho para gravar em arquivo.
//! A mensagem de erro do cofre nunca inclui o valor guardado.

use crate::secrets::SecretStore;
use forja_contracts::CounterError;

/// Cofre do sistema. Stateless: cada operação abre a entrada pelo serviço e pela conta.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemStore;

impl SecretStore for SystemStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, CounterError> {
        let entry = keyring::Entry::new(service, account).map_err(vault_error)?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(vault_error(error)),
        }
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), CounterError> {
        let entry = keyring::Entry::new(service, account).map_err(vault_error)?;
        entry.set_password(secret).map_err(vault_error)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), CounterError> {
        let entry = keyring::Entry::new(service, account).map_err(vault_error)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(vault_error(error)),
        }
    }
}

fn vault_error(error: keyring::Error) -> CounterError {
    CounterError::Vault {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lê uma entrada que não existe. Não grava nada no cofre do usuário. Roda com
    /// `cargo test -p forja-counter-kit -- --ignored`, porque toca o Gerenciador de Credenciais.
    #[test]
    #[ignore = "touches the Windows Credential Manager (read-only, unique name)"]
    fn reading_a_missing_entry_is_none_not_an_error() {
        let account = format!("never-written-{}", std::process::id());
        assert_eq!(SystemStore.get("forja/counter_test_probe", &account), Ok(None));
    }
}
