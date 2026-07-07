//! Data structures shared between the Rust bridge and the React frontend.
//!
//! All types serialize with `camelCase` field names so they map cleanly onto
//! the TypeScript interfaces in `src/types.ts`. Nothing here mutates in place —
//! callers build new values (see `settings.rs` for the immutable update helpers).

use serde::{Deserialize, Serialize};

/// A percentage in the inclusive range `0..=100`.
pub type Percent = u8;

/// Authorization strategy for connecting a provider account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// Paste an API key; stored in the OS keychain.
    Key,
    /// Authorize by logging in through the browser.
    Login,
}

impl Default for AuthMethod {
    fn default() -> Self {
        AuthMethod::Key
    }
}

/// A provider as it appears in the "add account" catalog. Static reference
/// data — see `catalog.rs`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvider {
    pub id: String,
    /// Display name, e.g. "Claude".
    pub name: String,
    /// Company behind the product, e.g. "Anthropic".
    pub company: String,
    /// Short monogram shown in the badge, e.g. "GPT".
    pub mono: String,
    /// Default plan label, e.g. "Max 5×".
    pub plan: String,
    /// Accent color as a CSS hex string.
    pub accent: String,
    /// Faint tint background as a CSS rgba string.
    pub tint: String,
}

/// The live used-quota numbers for one provider, as two rolling windows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Percentage of the 5-hour window consumed.
    pub five_hour: Percent,
    /// Percentage of the weekly window consumed.
    pub weekly: Percent,
}

/// One row in the menu-bar dropdown: catalog metadata joined with live usage.
///
/// `five_hour`/`weekly` are present only on a successful read; `error` carries a
/// user-facing message when the provider couldn't be read (not logged in,
/// network failure, not yet integrated). Exactly one side is populated.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardProvider {
    pub id: String,
    pub name: String,
    pub plan: String,
    pub accent: String,
    pub enabled: bool,
    pub five_hour: Option<Percent>,
    pub weekly: Option<Percent>,
    pub error: Option<String>,
}

impl DashboardProvider {
    /// A successfully-read row with live usage.
    pub fn ok(
        id: String,
        name: String,
        plan: String,
        accent: String,
        usage: Usage,
    ) -> Self {
        DashboardProvider {
            id,
            name,
            plan,
            accent,
            enabled: true,
            five_hour: Some(usage.five_hour),
            weekly: Some(usage.weekly),
            error: None,
        }
    }

    /// A row that failed to read, carrying a user-facing message.
    pub fn failed(id: String, name: String, plan: String, accent: String, error: String) -> Self {
        DashboardProvider {
            id,
            name,
            plan,
            accent,
            enabled: true,
            five_hour: None,
            weekly: None,
            error: Some(error),
        }
    }

    /// Whether this row carries live usage.
    pub fn has_usage(&self) -> bool {
        self.five_hour.is_some() || self.weekly.is_some()
    }

    /// The window under the most pressure — drives sorting and the alert. Rows
    /// without usage sort last (pressure 0).
    pub fn pressure(&self) -> Percent {
        self.five_hour.unwrap_or(0).max(self.weekly.unwrap_or(0))
    }
}

/// The full payload rendered by the panel screen.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub providers: Vec<DashboardProvider>,
    /// Highest used quota across providers with usage — `None` when nothing
    /// could be read. UI surfaces convert it to "remaining" for display.
    pub highest: Option<Percent>,
    /// Whether any readable provider crossed the near-limit threshold.
    pub near_limit: bool,
}
