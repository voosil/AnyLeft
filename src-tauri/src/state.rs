//! Shared application state and the read model that builds the dashboard.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::catalog;
use crate::error::AppResult;
use crate::models::{Dashboard, DashboardProvider, Usage};
use crate::providers::{ProviderContext, ProviderRegistry};
use crate::settings::{Account, AppSettings, NEAR_LIMIT_THRESHOLD};

/// How long a successful usage value is reused before refetching. Keeps the
/// panel snappy and avoids hammering rate-limited provider endpoints.
const OK_TTL: Duration = Duration::from_secs(60);

/// How long a failure is remembered before retrying. Shorter than `OK_TTL` so a
/// fresh login recovers quickly, but long enough that a persistent error (bad
/// token, not logged in) never spams the endpoint into a 429.
const ERR_TTL: Duration = Duration::from_secs(30);

/// HTTP timeout for provider usage calls.
const HTTP_TIMEOUT: Duration = Duration::from_secs(12);

/// A cached fetch outcome and when it was recorded.
struct CacheEntry {
    at: Instant,
    result: Result<Usage, String>,
}

/// Everything the commands need, managed by Tauri and shared across windows.
pub struct AppState {
    settings: Mutex<AppSettings>,
    config_path: PathBuf,
    registry: ProviderRegistry,
    ctx: ProviderContext,
    cache: Mutex<HashMap<String, CacheEntry>>,
}

impl AppState {
    /// Load settings from disk and wire up the provider registry + HTTP client.
    pub fn new(config_path: PathBuf) -> Self {
        let settings = AppSettings::load(&config_path);
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        AppState {
            settings: Mutex::new(settings),
            config_path,
            registry: ProviderRegistry::with_defaults(),
            ctx: ProviderContext { http },
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// A cheap clone of the current settings — never hand out the lock guard.
    pub fn settings_snapshot(&self) -> AppSettings {
        self.settings
            .lock()
            .expect("settings mutex poisoned")
            .clone()
    }

    /// Persist new settings, then swap them into memory. Immutable in spirit:
    /// the caller passes a freshly built value; on disk failure nothing changes.
    pub fn commit_settings(&self, next: AppSettings) -> AppResult<AppSettings> {
        next.save(&self.config_path)?;
        *self.settings.lock().expect("settings mutex poisoned") = next.clone();
        Ok(next)
    }

    /// Cached fetch outcome for one account. Both success and failure are cached
    /// (with different TTLs) so a persistent error never hammers the endpoint,
    /// while a fresh login still recovers within `ERR_TTL`. Errors carry a real
    /// message — never fabricated data.
    async fn usage_for(&self, account: &Account, force: bool) -> Result<Usage, String> {
        if !force {
            if let Some(result) = self.cached_result(&account.account_id) {
                return result;
            }
        }
        let result = match self.registry.fetch(&self.ctx, account).await {
            Ok(usage) => Ok(usage),
            Err(err) => {
                eprintln!(
                    "[anyleft] usage fetch failed for {}: {err}",
                    account.account_id
                );
                Err(err.to_string())
            }
        };
        self.cache.lock().expect("cache mutex poisoned").insert(
            account.account_id.clone(),
            CacheEntry {
                at: Instant::now(),
                result: result.clone(),
            },
        );
        result
    }

    fn cached_result(&self, account_id: &str) -> Option<Result<Usage, String>> {
        let cache = self.cache.lock().expect("cache mutex poisoned");
        let entry = cache.get(account_id)?;
        let ttl = if entry.result.is_ok() { OK_TTL } else { ERR_TTL };
        (entry.at.elapsed() < ttl).then(|| entry.result.clone())
    }

    /// Build the panel payload: enabled accounts joined with live usage, sorted
    /// by pressure when the preference is set. `force` bypasses the cache.
    pub async fn dashboard(&self, force: bool) -> AppResult<Dashboard> {
        let settings = self.settings_snapshot();

        let mut rows: Vec<DashboardProvider> = Vec::new();
        for account in settings.accounts.iter().filter(|a| a.enabled) {
            let meta = catalog::get(&account.provider_id)?;
            // Custom label wins over the catalog name so multiple same-provider
            // accounts stay distinguishable in the panel.
            let name = account
                .label
                .clone()
                .filter(|l| !l.trim().is_empty())
                .unwrap_or_else(|| meta.name.clone());
            let row = match self.usage_for(account, force).await {
                Ok(usage) => {
                    let plan = resolve_plan(&account.provider_id, &meta.plan, usage.plan.clone());
                    DashboardProvider::ok(
                        account.account_id.clone(),
                        account.provider_id.clone(),
                        name,
                        plan,
                        meta.accent,
                        usage,
                    )
                }
                Err(error) => {
                    let plan = resolve_plan(&account.provider_id, &meta.plan, None);
                    DashboardProvider::failed(
                        account.account_id.clone(),
                        account.provider_id.clone(),
                        name,
                        plan,
                        meta.accent,
                        error,
                    )
                }
            };
            rows.push(row);
        }

        if settings.preferences.sort_by_pressure {
            // Readable rows first (by pressure), failed rows sink to the bottom.
            rows.sort_by(|a, b| {
                b.has_usage()
                    .cmp(&a.has_usage())
                    .then(b.pressure().cmp(&a.pressure()))
            });
        }

        let highest = rows
            .iter()
            .filter(|r| r.has_usage())
            .map(DashboardProvider::pressure)
            .max();

        Ok(Dashboard {
            providers: rows,
            near_limit: highest.map(|h| h >= NEAR_LIMIT_THRESHOLD).unwrap_or(false),
            highest,
        })
    }
}

/// Decide the plan label to display for a row. A live value from the provider
/// always wins. Otherwise, providers whose plan is read live (Claude, ChatGPT)
/// show nothing when it's unknown — never a static guess — while every other
/// provider falls back to its catalog plan (a product name, e.g. "Token Plan").
fn resolve_plan(provider_id: &str, catalog_plan: &str, live_plan: Option<String>) -> Option<String> {
    if let Some(plan) = live_plan.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        return Some(plan);
    }
    if catalog::has_dynamic_plan(provider_id) {
        return None;
    }
    Some(catalog_plan.to_string())
}
