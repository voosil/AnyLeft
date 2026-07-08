//! Real Claude usage provider.
//!
//! Reads the local **Claude Code** OAuth login (macOS keychain item
//! `Claude Code-credentials`, or `~/.claude/.credentials.json`, or the
//! `CLAUDE_CODE_OAUTH_TOKEN` env var), refreshes the token if it is about to
//! expire, then calls Anthropic's OAuth usage endpoint and maps the two rolling
//! windows onto the panel's 5H / WEEK columns.
//!
//! Approach and endpoints follow the OpenUsage project's Claude provider.
//! No token or credential blob is ever logged or returned to the frontend.

use std::sync::Mutex;

use async_trait::async_trait;
use serde::Deserialize;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::settings::Account;

const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const CREDENTIALS_FILE: &str = ".claude/.credentials.json";
const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const REFRESH_URL: &str = "https://platform.claude.com/v1/oauth/token";
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const SCOPES: &str =
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";
const BETA_HEADER: &str = "oauth-2025-04-20";
const CLIENT_UA: &str = "claude-code/2.1.69";

/// Refresh the token when it expires within this window (milliseconds).
const REFRESH_SKEW_MS: f64 = 5.0 * 60.0 * 1000.0;

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuth>,
}

#[derive(Deserialize, Clone, Default)]
struct OAuth {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
    #[serde(rename = "refreshToken")]
    refresh_token: Option<String>,
    #[serde(rename = "expiresAt")]
    expires_at: Option<f64>,
    /// Subscription tier, e.g. "pro" or "max" — read live so the panel shows the
    /// real plan (and hides it when the credential doesn't carry one).
    #[serde(rename = "subscriptionType")]
    subscription_type: Option<String>,
}

impl OAuth {
    fn has_access(&self) -> bool {
        self.access().is_some()
    }

    fn access(&self) -> Option<&str> {
        self.access_token.as_deref().map(str::trim).filter(|t| !t.is_empty())
    }

    /// Whether the stored access token is at/near expiry and should be refreshed.
    fn expiring(&self) -> bool {
        self.expires_at
            .map(|expires_at| expires_at - now_ms() <= REFRESH_SKEW_MS)
            .unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
    /// Lifetime in seconds; lets us cache the refreshed token until it too nears
    /// expiry instead of refreshing on every fetch.
    #[serde(default)]
    expires_in: Option<f64>,
    #[serde(default)]
    refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct UsageWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct UsageResponse {
    five_hour: Option<UsageWindow>,
    seven_day: Option<UsageWindow>,
}

pub struct ClaudeProvider {
    /// Process-lifetime credential cache. Reading the `Claude Code-credentials`
    /// keychain item triggers a macOS password prompt, so we read it at most once
    /// per launch and refresh the access token in memory thereafter — this is
    /// what stops the panel from re-prompting on every 60-second refresh.
    cache: Mutex<Option<OAuth>>,
}

impl ClaudeProvider {
    pub fn new() -> Self {
        ClaudeProvider {
            cache: Mutex::new(None),
        }
    }

    fn cached(&self) -> Option<OAuth> {
        self.cache.lock().expect("claude cache poisoned").clone()
    }

    fn store(&self, oauth: OAuth) {
        *self.cache.lock().expect("claude cache poisoned") = Some(oauth);
    }

    fn invalidate(&self) {
        *self.cache.lock().expect("claude cache poisoned") = None;
    }

    /// Return usable credentials. Reads the keychain only when the cache is empty
    /// (or `force`, after a 401), refreshing the access token in memory when it
    /// nears expiry and caching the result so the next fetch skips both the
    /// keychain prompt and the refresh endpoint.
    async fn credentials(&self, ctx: &ProviderContext, force: bool) -> AppResult<OAuth> {
        let existing = if force { None } else { self.cached() };
        let oauth = match existing {
            Some(oauth) => oauth,
            None => read_local_oauth()?, // may prompt the keychain — at most once
        };
        let fresh = ensure_fresh_token(ctx, &oauth).await?;
        self.store(fresh.clone());
        Ok(fresh)
    }
}

#[async_trait]
impl UsageProvider for ClaudeProvider {
    async fn fetch(&self, ctx: &ProviderContext, _account: &Account) -> AppResult<Usage> {
        let oauth = self.credentials(ctx, false).await?;
        let (status, body) = request_usage(ctx, oauth.access().unwrap_or_default()).await?;

        // A 401/403 means the cached token is stale (e.g. the user re-logged in).
        // Drop the cache, re-read once, and retry.
        let (status, body, oauth) = if status == 401 || status == 403 {
            self.invalidate();
            let oauth = self.credentials(ctx, true).await?;
            let (status, body) = request_usage(ctx, oauth.access().unwrap_or_default()).await?;
            (status, body, oauth)
        } else {
            (status, body, oauth)
        };

        if !(200..300).contains(&status) {
            return Err(AppError::Usage(format!("Claude 用量接口返回 HTTP {status}")));
        }

        let parsed: UsageResponse = serde_json::from_str(&body)
            .map_err(|err| AppError::Usage(format!("Claude 用量解析失败：{err}")))?;
        Ok(Usage {
            five_hour: window_pct(&parsed.five_hour),
            five_hour_reset: window_reset(&parsed.five_hour),
            weekly: window_pct(&parsed.seven_day),
            weekly_reset: window_reset(&parsed.seven_day),
            plan: super::pretty_plan(oauth.subscription_type.as_deref()),
        })
    }
}

/// Call the OAuth usage endpoint, returning the raw (status, body) so the caller
/// can distinguish an auth failure (retry after re-reading) from other errors.
async fn request_usage(ctx: &ProviderContext, token: &str) -> AppResult<(u16, String)> {
    let response = ctx
        .http
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("anthropic-beta", BETA_HEADER)
        .header("User-Agent", CLIENT_UA)
        .send()
        .await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    Ok((status, body))
}

fn window_pct(window: &Option<UsageWindow>) -> u8 {
    let raw = window.as_ref().and_then(|w| w.utilization).unwrap_or(0.0);
    raw.round().clamp(0.0, 100.0) as u8
}

fn window_reset(window: &Option<UsageWindow>) -> Option<String> {
    window.as_ref().and_then(|w| w.resets_at.clone())
}

/// Load the Claude Code OAuth credentials from env → keychain → file.
fn read_local_oauth() -> AppResult<OAuth> {
    if let Some(token) = env_token() {
        return Ok(OAuth {
            access_token: Some(token),
            ..OAuth::default()
        });
    }
    if let Some(json) = read_keychain_json() {
        if let Some(oauth) = parse_oauth(&json) {
            return Ok(oauth);
        }
    }
    if let Some(oauth) = read_file_oauth() {
        return Ok(oauth);
    }
    Err(AppError::Usage(
        "未找到 Claude Code 登录凭据，请先在终端运行 `claude` 登录".to_string(),
    ))
}

fn env_token() -> Option<String> {
    std::env::var("CLAUDE_CODE_OAUTH_TOKEN")
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn parse_oauth(text: &str) -> Option<OAuth> {
    serde_json::from_str::<CredentialsFile>(text)
        .ok()
        .and_then(|file| file.claude_ai_oauth)
        .filter(OAuth::has_access)
}

fn read_file_oauth() -> Option<OAuth> {
    let home = std::env::var_os("HOME")?;
    let path = std::path::Path::new(&home).join(CREDENTIALS_FILE);
    let text = std::fs::read_to_string(path).ok()?;
    parse_oauth(&text)
}

/// Read the credential blob from the macOS keychain via `security`. The user is
/// prompted by macOS on first access; nothing is cached or logged here.
#[cfg(target_os = "macos")]
fn read_keychain_json() -> Option<String> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(not(target_os = "macos"))]
fn read_keychain_json() -> Option<String> {
    None
}

/// Return usable credentials, refreshing the access token in-memory when the
/// stored one is expiring and a refresh token is available. The returned value
/// carries the (possibly refreshed) token so callers can cache it.
async fn ensure_fresh_token(ctx: &ProviderContext, oauth: &OAuth) -> AppResult<OAuth> {
    if oauth.expiring() {
        if let Some(refresh) = oauth.refresh_token.as_deref() {
            if let Ok(refreshed) = refresh_token(ctx, oauth, refresh).await {
                return Ok(refreshed);
            }
        }
    }

    if oauth.has_access() {
        Ok(oauth.clone())
    } else {
        Err(AppError::Usage("Claude 凭据缺少 access token".to_string()))
    }
}

async fn refresh_token(ctx: &ProviderContext, base: &OAuth, refresh: &str) -> AppResult<OAuth> {
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh,
        "client_id": CLIENT_ID,
        "scope": SCOPES,
    });
    let response = ctx.http.post(REFRESH_URL).json(&body).send().await?;
    if !response.status().is_success() {
        return Err(AppError::Usage(format!(
            "Claude token 刷新失败 HTTP {}",
            response.status().as_u16()
        )));
    }
    let parsed: RefreshResponse = response.json().await?;
    let expires_at = parsed
        .expires_in
        .map(|secs| now_ms() + secs * 1000.0)
        .or(base.expires_at);
    Ok(OAuth {
        access_token: Some(parsed.access_token),
        refresh_token: parsed.refresh_token.or_else(|| base.refresh_token.clone()),
        expires_at,
        subscription_type: base.subscription_type.clone(),
    })
}

fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
