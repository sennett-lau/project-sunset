//! Where API keys live. On macOS: the Keychain, under service = the app's bundle id and
//! account = the provider id. Never SQLite, config files, logs or the webview.
//! Calls block; run them off the async runtime (`spawn_blocking`).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::Result;

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, secret: &str) -> Result<()>;
    /// Removing a missing secret is not an error.
    fn delete(&self, account: &str) -> Result<()>;
}

/// For tests and non-macOS builds. Forgets everything on exit.
#[derive(Default)]
pub struct MemorySecretStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.0.lock().expect("lock").get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        self.0
            .lock()
            .expect("lock")
            .insert(account.to_owned(), secret.to_owned());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        self.0.lock().expect("lock").remove(account);
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use keychain::KeychainSecretStore;

#[cfg(target_os = "macos")]
mod keychain {
    use std::sync::Arc;

    use apple_native_keyring_store::keychain::Store;
    use keyring_core::api::CredentialStoreApi;
    use keyring_core::{Entry, Error};

    use super::SecretStore;
    use crate::{ProviderError, Result};

    /// The user's login keychain.
    pub struct KeychainSecretStore {
        service: String,
        store: Arc<Store>,
    }

    impl KeychainSecretStore {
        pub fn new(service: impl Into<String>) -> Result<Self> {
            Ok(Self {
                service: service.into(),
                store: Store::new().map_err(err)?,
            })
        }

        fn entry(&self, account: &str) -> Result<Entry> {
            self.store.build(&self.service, account, None).map_err(err)
        }
    }

    impl SecretStore for KeychainSecretStore {
        fn get(&self, account: &str) -> Result<Option<String>> {
            match self.entry(account)?.get_password() {
                Ok(secret) => Ok(Some(secret)),
                Err(Error::NoEntry) => Ok(None),
                Err(e) => Err(err(e)),
            }
        }

        fn set(&self, account: &str, secret: &str) -> Result<()> {
            self.entry(account)?.set_password(secret).map_err(err)
        }

        fn delete(&self, account: &str) -> Result<()> {
            match self.entry(account)?.delete_credential() {
                Ok(()) | Err(Error::NoEntry) => Ok(()),
                Err(e) => Err(err(e)),
            }
        }
    }

    fn err(e: Error) -> ProviderError {
        ProviderError::Secret(e.to_string())
    }
}
