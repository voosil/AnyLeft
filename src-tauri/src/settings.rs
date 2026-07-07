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
const DEFAULT_CONNECTED: &[&str] = &["claude", "gpt"];

/// A connected provider account. Secrets never live here — only whether one is
/// stored (the key itself sits in the OS keychain, see `secrets.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub enabled: bool,
    pub auth_method: AuthMethod,
    pub has_secret: bool,
}

impl Account {
    fn connected_default(id: &str) -> Self {
        Account {
            id: id.to_string(),
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
            menubar_percent: true,
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
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|err| {
                eprintln!("[anyleft] settings parse failed, using defaults: {err}");
                AppSettings::default()
            }),
            Err(_) => AppSettings::default(),
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

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    // ---- immutable updates: each returns a fresh AppSettings ----

    /// Add (or update) a connected account. Returns a new value.
    pub fn with_account(&self, id: &str, auth: AuthMethod, has_secret: bool) -> Self {
        let mut accounts: Vec<Account> = self
            .accounts
            .iter()
            .filter(|a| a.id != id)
            .cloned()
            .collect();
        accounts.push(Account {
            id: id.to_string(),
            enabled: true,
            auth_method: auth,
            has_secret,
        });
        Self {
            accounts,
            preferences: self.preferences.clone(),
        }
    }

    /// Remove a connected account. Returns a new value.
    pub fn without_account(&self, id: &str) -> Self {
        Self {
            accounts: self
                .accounts
                .iter()
                .filter(|a| a.id != id)
                .cloned()
                .collect(),
            preferences: self.preferences.clone(),
        }
    }

    /// Toggle a single account's enabled flag. Returns a new value.
    pub fn with_account_enabled(&self, id: &str, enabled: bool) -> Self {
        Self {
            accounts: self
                .accounts
                .iter()
                .map(|a| {
                    if a.id == id {
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
