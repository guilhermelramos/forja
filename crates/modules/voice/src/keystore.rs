//! Cofre da chave da OpenAI: o Gerenciador de Credenciais do Windows, pelo crate `keyring`. Nunca
//! grava em arquivo. A mensagem de erro não inclui o valor guardado. Os testes usam uma versão
//! em memória, para não tocar no cofre do usuário.

pub const SERVICE: &str = "forja.voice";
pub const ACCOUNT: &str = "openai";

/// Onde a chave fica guardada. Trait para os testes trocarem por uma versão em memória.
pub trait KeyStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, &'static str>;
    fn set(&self, key: &str) -> Result<(), &'static str>;
    fn delete(&self) -> Result<(), &'static str>;
}

/// O cofre do sistema.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemKeyring;

impl KeyStore for SystemKeyring {
    fn get(&self) -> Result<Option<String>, &'static str> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| "voice-error-keyring")?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("voice-error-keyring"),
        }
    }

    fn set(&self, key: &str) -> Result<(), &'static str> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| "voice-error-keyring")?;
        entry.set_password(key).map_err(|_| "voice-error-keyring")
    }

    fn delete(&self) -> Result<(), &'static str> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| "voice-error-keyring")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("voice-error-keyring"),
        }
    }
}

#[cfg(test)]
pub mod memory {
    use super::KeyStore;
    use std::sync::Mutex;

    /// Cofre de teste: guarda a chave na memória do processo.
    #[derive(Default)]
    pub struct MemoryKeyStore(pub Mutex<Option<String>>);

    impl KeyStore for MemoryKeyStore {
        fn get(&self) -> Result<Option<String>, &'static str> {
            Ok(self.0.lock().map_err(|_| "voice-error-keyring")?.clone())
        }

        fn set(&self, key: &str) -> Result<(), &'static str> {
            *self.0.lock().map_err(|_| "voice-error-keyring")? = Some(key.to_owned());
            Ok(())
        }

        fn delete(&self) -> Result<(), &'static str> {
            *self.0.lock().map_err(|_| "voice-error-keyring")? = None;
            Ok(())
        }
    }
}
