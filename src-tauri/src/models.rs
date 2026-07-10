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

/// The live used-quota numbers for one provider, as two rolling windows plus an
/// optional API-credit balance for pay-as-you-go providers (e.g. DeepSeek).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Percentage of the 5-hour window consumed.
    pub five_hour: Percent,
    pub five_hour_reset: Option<String>,
    /// Percentage of the weekly window consumed.
    pub weekly: Percent,
    pub weekly_reset: Option<String>,
    /// Subscription/plan type read live from the provider (e.g. Claude's
    /// `subscriptionType`, ChatGPT's `chatgpt_plan_type`). `None` when the
    /// provider doesn't expose it — the UI then hides the plan label rather than
    /// showing a static guess.
    #[serde(default)]
    pub plan: Option<String>,
    /// Formatted API credit balance (e.g. "¥100.00"), `None` for quota-only providers.
    #[serde(default)]
    pub balance: Option<String>,
}

/// One row in the menu-bar dropdown: catalog metadata joined with live usage.
///
/// `account_id` identifies the specific connected account (a provider can now
/// have several); `provider_id` is the catalog id it belongs to. `five_hour`/
/// `weekly` are present only on a successful read; `error` carries a user-facing
/// message when the provider couldn't be read (not logged in, network failure,
/// not yet integrated). Exactly one side is populated. `plan` is `None` when the
/// subscription type is unknown — the UI then omits the label. `balance` is
/// present for pay-as-you-go providers (e.g. DeepSeek) and displayed instead of
/// the 5h/weekly percentages.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardProvider {
    pub account_id: String,
    pub provider_id: String,
    pub name: String,
    pub plan: Option<String>,
    pub accent: String,
    pub enabled: bool,
    pub five_hour: Option<Percent>,
    pub five_hour_reset: Option<String>,
    pub weekly: Option<Percent>,
    pub weekly_reset: Option<String>,
    pub balance: Option<String>,
    pub error: Option<String>,
}

impl DashboardProvider {
    /// A successfully-read row with live usage.
    pub fn ok(
        account_id: String,
        provider_id: String,
        name: String,
        plan: Option<String>,
        accent: String,
        usage: Usage,
    ) -> Self {
        DashboardProvider {
            account_id,
            provider_id,
            name,
            plan,
            accent,
            enabled: true,
            five_hour: Some(usage.five_hour),
            five_hour_reset: usage.five_hour_reset,
            weekly: Some(usage.weekly),
            weekly_reset: usage.weekly_reset,
            balance: usage.balance,
            error: None,
        }
    }

    /// A row that failed to read, carrying a user-facing message.
    pub fn failed(
        account_id: String,
        provider_id: String,
        name: String,
        plan: Option<String>,
        accent: String,
        error: String,
    ) -> Self {
        DashboardProvider {
            account_id,
            provider_id,
            name,
            plan,
            accent,
            enabled: true,
            five_hour: None,
            five_hour_reset: None,
            weekly: None,
            weekly_reset: None,
            balance: None,
            error: Some(error),
        }
    }

    /// Whether this row carries live data (quota usage or credit balance).
    pub fn has_usage(&self) -> bool {
        self.five_hour.is_some() || self.weekly.is_some() || self.balance.is_some()
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
