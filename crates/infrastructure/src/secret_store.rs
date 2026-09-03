//! Armazenamento de segredos no cofre nativo do SO.
//!
//! CLAUDE §11/13/18: "Never store plaintext passwords / Never hard-code tokens
//! / Use secure credential storage where appropriate". O banco SQLite do
//! MasterNote fica no diretório do usuário sem criptografia, então token de
//! sessão **não** pode morar lá.
//!
//! Backends por SO (via `keyring` 3.x):
//! - Windows: Credential Manager
//! - macOS: Keychain
//! - Linux: Secret Service (D-Bus) — exige um agente rodando (GNOME Keyring,
//!   KWallet). Em sessão headless/TTY não há cofre disponível; nesse caso
//!   `store`/`load` falham com `SecretStoreError::Unavailable` e o app trata
//!   como "sem sessão" em vez de degradar para texto plano.
//!
//! Nenhuma função aqui loga o valor do segredo (§13: "Never log secrets").

use thiserror::Error;

/// Serviço registrado no cofre. Aparece como nome do item para o usuário no
/// Credential Manager / Keychain, então é legível de propósito.
const SERVICE: &str = "MasterNote";

#[derive(Debug, Error)]
pub enum SecretStoreError {
    /// Não há cofre utilizável nesta sessão (típico em Linux headless).
    #[error("secret store unavailable on this system")]
    Unavailable,
    /// Cofre disponível mas a operação falhou.
    #[error("secret store operation failed")]
    Failed,
}

pub type SecretResult<T> = Result<T, SecretStoreError>;

/// Chaves usadas pelo MasterNote. Enum fechado para evitar chave digitada
/// errada em um lugar e certa em outro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKey {
    /// Refresh token do Mastersys. Guardamos só o refresh: o access token é
    /// de curta duração e fica em memória, então um vazamento do cofre não
    /// entrega uma sessão imediatamente utilizável sem o endpoint.
    MastersysRefreshToken,
}

impl SecretKey {
    fn account(&self) -> &'static str {
        match self {
            SecretKey::MastersysRefreshToken => "mastersys.refresh_token",
        }
    }
}

/// Fachada fina sobre `keyring`. Existe para (a) manter o resto do código sem
/// `use keyring::...` e (b) traduzir os erros do crate para dois casos que a
/// UI sabe explicar.
#[derive(Debug, Clone, Copy, Default)]
pub struct SecretStore;

impl SecretStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(&self, key: SecretKey) -> SecretResult<keyring::Entry> {
        keyring::Entry::new(SERVICE, key.account()).map_err(map_err)
    }

    pub fn store(&self, key: SecretKey, secret: &str) -> SecretResult<()> {
        self.entry(key)?.set_password(secret).map_err(map_err)
    }

    /// `Ok(None)` quando a chave simplesmente não existe — que é o estado
    /// normal antes do primeiro login, não um erro.
    pub fn load(&self, key: SecretKey) -> SecretResult<Option<String>> {
        match self.entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    /// Apaga a chave. Idempotente: apagar o que não existe é sucesso, para
    /// que `sign_out` nunca falhe por já estar deslogado.
    pub fn delete(&self, key: SecretKey) -> SecretResult<()> {
        match self.entry(key)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}

fn map_err(e: keyring::Error) -> SecretStoreError {
    match e {
        // Sem backend, sem D-Bus, ou plataforma sem cofre.
        keyring::Error::PlatformFailure(_) | keyring::Error::NoStorageAccess(_) => {
            SecretStoreError::Unavailable
        }
        _ => SecretStoreError::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_keys_have_distinct_accounts() {
        // Uma chave nova com o mesmo `account()` sobrescreveria a anterior em
        // silêncio; este teste falha se alguém duplicar o identificador.
        let all = [SecretKey::MastersysRefreshToken];
        let mut accounts: Vec<&str> = all.iter().map(|k| k.account()).collect();
        accounts.sort_unstable();
        let total = accounts.len();
        accounts.dedup();
        assert_eq!(accounts.len(), total);
    }

    /// Só roda quando há cofre real na máquina. No CI Linux sem Secret
    /// Service isso é `Unavailable`, e o teste passa reconhecendo esse caso
    /// em vez de falhar por causa do ambiente.
    #[test]
    fn roundtrip_when_a_vault_exists() {
        let store = SecretStore::new();
        let key = SecretKey::MastersysRefreshToken;

        let previous = match store.load(key) {
            Ok(v) => v,
            Err(SecretStoreError::Unavailable) => return,
            Err(e) => panic!("cofre disponível mas load falhou: {e}"),
        };

        store.store(key, "token-de-teste").unwrap();
        assert_eq!(store.load(key).unwrap().as_deref(), Some("token-de-teste"));

        store.delete(key).unwrap();
        assert_eq!(store.load(key).unwrap(), None);
        store.delete(key).unwrap(); // idempotente

        // Não deixa a máquina do dev deslogada por causa de um teste.
        if let Some(p) = previous {
            store.store(key, &p).unwrap();
        }
    }
}
