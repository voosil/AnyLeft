//! Keychain-backed storage for provider API keys.
//!
//! The design promises that "密钥仅保存在本机钥匙串" — keys are only stored in
//! the local keychain. We never persist a key to the settings JSON or return it
//! to the frontend; only a `has_secret` boolean is ever exposed.
//!
//! Reading a keychain item triggers a macOS password prompt on first access,
//! so every successful read (and the confirmed "no entry" result) is cached
//! in a process-local map. The cache keeps the panel from re-prompting on
//! every refresh while still picking up changes through `set_key`/`delete_key`.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use keyring::Entry;

use crate::error::AppResult;

/// Keychain service name. One entry per account id.
const SERVICE: &str = "com.voosil.anyleft";

/// Process-local cache. `Some(value)` when the keychain held a value, `None`
/// when we confirmed there is no entry. Either branch lets the next read skip
/// the keychain call (and the macOS prompt that goes with it).
static CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn entry(account_id: &str) -> AppResult<Entry> {
    Ok(Entry::new(SERVICE, account_id)?)
}

/// Store (or replace) the API key for an account. The cached entry is updated
/// so the next read in this process skips the keychain and uses the new value.
pub fn set_key(account_id: &str, key: &str) -> AppResult<()> {
    entry(account_id)?.set_password(key)?;
    CACHE
        .lock()
        .expect("secrets cache poisoned")
        .insert(account_id.to_string(), Some(key.to_string()));
    Ok(())
}

/// Delete a stored key. A missing entry is treated as success (idempotent).
/// On success the cached slot is set to `None` so a follow-up read returns
/// immediately instead of re-prompting the keychain.
pub fn delete_key(account_id: &str) -> AppResult<()> {
    match entry(account_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            CACHE
                .lock()
                .expect("secrets cache poisoned")
                .insert(account_id.to_string(), None);
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// Read a stored key, returning `None` when nothing is stored. Keyed by the
/// unique account id so several accounts of one provider each keep their own key.
///
/// Cached per-process so the macOS keychain prompt only fires on first access
/// for each account in this run. Other keyring errors are not cached — they
/// usually mean the user just denied a prompt, and re-trying immediately is
/// the right behaviour.
pub fn get_key(account_id: &str) -> AppResult<Option<String>> {
    if let Some(cached) = CACHE
        .lock()
        .expect("secrets cache poisoned")
        .get(account_id)
        .cloned()
    {
        return Ok(cached);
    }
    let result = match entry(account_id)?.get_password() {
        Ok(key) => Some(key),
        Err(keyring::Error::NoEntry) => None,
        Err(err) => return Err(err.into()),
    };
    CACHE
        .lock()
        .expect("secrets cache poisoned")
        .insert(account_id.to_string(), result.clone());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-seed the cache directly so we can exercise cache invariants without
    /// touching the real keychain (which would prompt the user on macOS).
    fn seed(account_id: &str, value: Option<&str>) {
        CACHE
            .lock()
            .unwrap()
            .insert(account_id.to_string(), value.map(str::to_string));
    }

    #[test]
    fn returns_cached_value_without_touching_keyring() {
        seed("test-cached", Some("secret"));
        assert_eq!(get_key("test-cached").unwrap(), Some("secret".to_string()));
    }

    #[test]
    fn returns_cached_absent() {
        seed("test-absent", None);
        assert_eq!(get_key("test-absent").unwrap(), None);
    }

    #[test]
    fn different_account_ids_dont_collide() {
        seed("a", Some("alpha"));
        seed("b", Some("beta"));
        assert_eq!(get_key("a").unwrap().as_deref(), Some("alpha"));
        assert_eq!(get_key("b").unwrap().as_deref(), Some("beta"));
    }

    // `set_key` and `delete_key` against the real keychain would prompt for
    // credentials, so end-to-end verification happens at runtime: open the
    // panel and confirm only one macOS keychain prompt appears per account.
}
