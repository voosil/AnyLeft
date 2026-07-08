//! Persisted application settings and the immutable helpers that update them.
//!
//! Settings live in a single JSON file under the app config directory. Every
//! mutation returns a *new* `AppSettings` value rather than editing in place —
//! callers persist the result and swap it into shared state.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AppResult;
use crate::models::AuthMethod;

/// Threshold (percent) at which a provider is considered "near its limit".
pub const NEAR_LIMIT_THRESHOLD: u8 = 85;

/// Default global shortcut used to summon the panel.
pub const DEFAULT_SHORTCUT: &str = "CommandOrControl+Shift+U";

/// Default accent color (Claude terracotta).
pub const DEFAULT_ACCENT: &str = "#C96442";

/// The provider ids connected on a fresh install. Limited to the providers with
/// a real integration so a new user never sees fabricated numbers — others can
/// be added from settings (and will show a "not yet integrated" state).
const DEFAULT_CONNECTED: &[&str] = &["claude", "gpt", "kimi", "minimax"];

/// A connected provider account. Secrets never live here — only whether one is
/// stored (the key itself sits in the OS keychain under `account_id`, see
/// `secrets.rs`).
///
/// A provider can now hold several accounts (e.g. two Kimi logins), so identity
/// is the unique `account_id`; `provider_id` is the catalog id it belongs to and
/// `label` is an optional user-chosen name. Legacy settings files stored a single
/// `id` (the provider id) per account — [`AppSettings::normalized`] backfills
/// `account_id` from it so existing keychain entries keep resolving.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default)]
    pub account_id: String,
    #[serde(alias = "id")]
    pub provider_id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub enabled: bool,
    pub auth_method: AuthMethod,
    pub has_secret: bool,
}

impl Account {
    fn connected_default(provider_id: &str) -> Self {
        Account {
            account_id: provider_id.to_string(),
            provider_id: provider_id.to_string(),
            label: None,
            enabled: true,
            auth_method: AuthMethod::Key,
            has_secret: false,
        }
    }
}

/// User preferences shown on the settings screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    /// Show the remaining quota percentage next to the menu-bar clock.
    pub menubar_percent: bool,
    /// Notify when any window crosses `NEAR_LIMIT_THRESHOLD`.
    pub near_limit_alert: bool,
    /// Launch AnyLeft at login.
    pub launch_at_login: bool,
    /// Sort dropdown rows by pressure (highest usage first).
    pub sort_by_pressure: bool,
    /// Global shortcut string that summons the panel.
    pub shortcut: String,
    /// Accent color as a CSS hex string.
    pub accent: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            menubar_percent: false,
            near_limit_alert: false,
            launch_at_login: true,
            sort_by_pressure: true,
            shortcut: DEFAULT_SHORTCUT.to_string(),
            accent: DEFAULT_ACCENT.to_string(),
        }
    }
}

/// The complete persisted state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub accounts: Vec<Account>,
    pub preferences: Preferences,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            accounts: DEFAULT_CONNECTED
                .iter()
                .map(|id| Account::connected_default(id))
                .collect(),
            preferences: Preferences::default(),
        }
    }
}

impl AppSettings {
    /// Load from disk, falling back to defaults when the file is missing or
    /// unreadable. A corrupt file is never fatal — we log and start fresh.
    pub fn load(path: &Path) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice::<AppSettings>(&bytes)
                .map(AppSettings::normalized)
                .unwrap_or_else(|err| {
                    eprintln!("[anyleft] settings parse failed, using defaults: {err}");
                    AppSettings::default()
                }),
            Err(_) => AppSettings::default(),
        }
    }

    /// Backfill fields that legacy settings files predate. Older accounts stored
    /// only the provider id (as `id`); here we mirror it into `account_id` so the
    /// single existing account keeps the same keychain key. Returns a new value.
    fn normalized(self) -> Self {
        let accounts = self
            .accounts
            .into_iter()
            .map(|account| {
                if account.account_id.trim().is_empty() {
                    Account {
                        account_id: account.provider_id.clone(),
                        ..account
                    }
                } else {
                    account
                }
            })
            .collect();
        Self {
            accounts,
            preferences: self.preferences,
        }
    }

    /// Persist to disk atomically (write temp, then rename).
    pub fn save(&self, path: &Path) -> AppResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self)?;
        let tmp = with_tmp_suffix(path);
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Look up an account by its unique `account_id`.
    pub fn account(&self, account_id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.account_id == account_id)
    }

    // ---- immutable updates: each returns a fresh AppSettings ----

    /// Add or replace an account (matched by `account_id`). Returns a new value.
    pub fn with_account(&self, account: Account) -> Self {
        let mut accounts: Vec<Account> = self
            .accounts
            .iter()
            .filter(|a| a.account_id != account.account_id)
            .cloned()
            .collect();
        accounts.push(account);
        Self {
            accounts,
            preferences: self.preferences.clone(),
        }
    }

    /// Remove a connected account by `account_id`. Returns a new value.
    pub fn without_account(&self, account_id: &str) -> Self {
        Self {
            accounts: self
                .accounts
                .iter()
                .filter(|a| a.account_id != account_id)
                .cloned()
                .collect(),
            preferences: self.preferences.clone(),
        }
    }

    /// Toggle a single account's enabled flag. Returns a new value.
    pub fn with_account_enabled(&self, account_id: &str, enabled: bool) -> Self {
        Self {
            accounts: self
                .accounts
                .iter()
                .map(|a| {
                    if a.account_id == account_id {
                        Account {
                            enabled,
                            ..a.clone()
                        }
                    } else {
                        a.clone()
                    }
                })
                .collect(),
            preferences: self.preferences.clone(),
        }
    }

    /// Replace the preferences block. Returns a new value.
    pub fn with_preferences(&self, preferences: Preferences) -> Self {
        Self {
            accounts: self.accounts.clone(),
            preferences,
        }
    }
}

fn with_tmp_suffix(path: &Path) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(".tmp");
    PathBuf::from(os)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> AppSettings {
        serde_json::from_str::<AppSettings>(json)
            .expect("valid settings json")
            .normalized()
    }

    #[test]
    fn migrates_legacy_account_format() {
        // Pre-multi-account files keyed each account by its provider id under `id`
        // and carried neither `accountId` nor `label`.
        let legacy = r##"{
            "accounts": [
                {"id": "kimi", "enabled": true, "authMethod": "key", "hasSecret": true},
                {"id": "claude", "enabled": false, "authMethod": "login", "hasSecret": false}
            ],
            "preferences": {
                "menubarPercent": false,
                "nearLimitAlert": false,
                "launchAtLogin": true,
                "sortByPressure": true,
                "shortcut": "CommandOrControl+Shift+U",
                "accent": "#C96442"
            }
        }"##;

        let settings = parse(legacy);
        let kimi = &settings.accounts[0];
        // `id` maps onto provider_id, and account_id is backfilled from it so the
        // existing keychain entry (keyed by "kimi") keeps resolving.
        assert_eq!(kimi.provider_id, "kimi");
        assert_eq!(kimi.account_id, "kimi");
        assert!(kimi.label.is_none());
        assert!(kimi.has_secret);

        let claude = &settings.accounts[1];
        assert_eq!(claude.account_id, "claude");
        assert!(!claude.enabled);
        assert_eq!(claude.auth_method, AuthMethod::Login);
    }

    #[test]
    fn parses_new_multi_account_format() {
        let json = r##"{
            "accounts": [
                {"accountId": "kimi-1", "providerId": "kimi", "label": "工作", "enabled": true, "authMethod": "key", "hasSecret": true},
                {"accountId": "kimi-2", "providerId": "kimi", "label": null, "enabled": true, "authMethod": "key", "hasSecret": true}
            ],
            "preferences": {
                "menubarPercent": false,
                "nearLimitAlert": false,
                "launchAtLogin": true,
                "sortByPressure": true,
                "shortcut": "CommandOrControl+Shift+U",
                "accent": "#C96442"
            }
        }"##;

        let settings = parse(json);
        assert_eq!(settings.accounts.len(), 2);
        assert_eq!(settings.accounts[0].account_id, "kimi-1");
        assert_eq!(settings.accounts[0].label.as_deref(), Some("工作"));
        assert_eq!(settings.accounts[1].account_id, "kimi-2");
        // Two accounts, same provider — the whole point of the feature.
        assert_eq!(settings.accounts[0].provider_id, settings.accounts[1].provider_id);
    }

    #[test]
    fn with_account_upserts_by_account_id() {
        let base = AppSettings::default();
        let before = base.accounts.len();
        let account = Account {
            account_id: "kimi-99".to_string(),
            provider_id: "kimi".to_string(),
            label: Some("副号".to_string()),
            enabled: true,
            auth_method: AuthMethod::Key,
            has_secret: true,
        };
        // Adding a fresh account id grows the list…
        let added = base.with_account(account.clone());
        assert_eq!(added.accounts.len(), before + 1);
        // …while reusing the same account id replaces in place.
        let replaced = added.with_account(Account {
            label: Some("改名".to_string()),
            ..account
        });
        assert_eq!(replaced.accounts.len(), before + 1);
        assert_eq!(
            replaced.account("kimi-99").and_then(|a| a.label.as_deref()),
            Some("改名")
        );
    }
}
