//! Keychain-backed storage for provider API keys.
//!
//! The design promises that "密钥仅保存在本机钥匙串" — keys are only stored in
//! the local keychain. We never persist a key to the settings JSON or return it
//! to the frontend; only a `has_secret` boolean is ever exposed.

use keyring::Entry;

use crate::error::AppResult;

/// Keychain service name. One entry per provider id.
const SERVICE: &str = "com.voosil.anyleft";

fn entry(provider_id: &str) -> AppResult<Entry> {
    Ok(Entry::new(SERVICE, provider_id)?)
}

/// Store (or replace) the API key for a provider.
pub fn set_key(provider_id: &str, key: &str) -> AppResult<()> {
    entry(provider_id)?.set_password(key)?;
    Ok(())
}

/// Delete a stored key. A missing entry is treated as success (idempotent).
pub fn delete_key(provider_id: &str) -> AppResult<()> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Read a stored key, returning `None` when nothing is stored. Used by real
/// usage providers when fetching live data (the mock provider ignores it).
#[allow(dead_code)]
pub fn get_key(provider_id: &str) -> AppResult<Option<String>> {
    match entry(provider_id)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}
