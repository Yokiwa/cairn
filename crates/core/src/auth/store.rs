//! Where refresh tokens live: the OS keychain (never a plain file).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::{AppError, Result};

pub const KEYRING_SERVICE: &str = "cairn";

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

fn keyring_err(e: keyring::Error) -> AppError {
    AppError::new("error.auth.keyring").with_detail(e.to_string())
}

/// Windows Credential Manager, macOS Keychain, Linux Secret Service.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(keyring_err)?
            .get_password()
        {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_err(e)),
        }
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(keyring_err)?
            .set_password(value)
            .map_err(keyring_err)
    }
    fn delete(&self, key: &str) -> Result<()> {
        match keyring::Entry::new(KEYRING_SERVICE, key)
            .map_err(keyring_err)?
            .delete_credential()
        {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_err(e)),
        }
    }
}

/// In-memory store for tests.
#[derive(Debug, Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(key.into(), value.into());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}
