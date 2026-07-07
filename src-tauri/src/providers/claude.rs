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
}

impl OAuth {
    fn has_access(&self) -> bool {
        self.access_token
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
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

pub struct ClaudeProvider;

impl ClaudeProvider {
    pub fn new() -> Self {
        ClaudeProvider
    }
}

#[async_trait]
impl UsageProvider for ClaudeProvider {
    async fn fetch(&self, ctx: &ProviderContext, _account: &Account) -> AppResult<Usage> {
        let oauth = read_local_oauth()?;
        let token = ensure_fresh_token(ctx, &oauth).await?;

        let response = ctx
            .http
            .get(USAGE_URL)
            .header("Authorization", format!("Bearer {}", token.trim()))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("anthropic-beta", BETA_HEADER)
            .header("User-Agent", CLIENT_UA)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Usage(format!(
                "Claude 用量接口返回 HTTP {}",
                status.as_u16()
            )));
        }

        let body: UsageResponse = response.json().await?;
        Ok(Usage {
            five_hour: window_pct(&body.five_hour),
            five_hour_reset: window_reset(&body.five_hour),
            weekly: window_pct(&body.seven_day),
            weekly_reset: window_reset(&body.seven_day),
        })
    }
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

/// Return a usable access token, refreshing in-memory when the stored one is
/// expiring and a refresh token is available.
async fn ensure_fresh_token(ctx: &ProviderContext, oauth: &OAuth) -> AppResult<String> {
    let expiring = oauth
        .expires_at
        .map(|expires_at| expires_at - now_ms() <= REFRESH_SKEW_MS)
        .unwrap_or(false);

    if expiring {
        if let Some(refresh) = oauth.refresh_token.as_deref() {
            if let Ok(access) = refresh_token(ctx, refresh).await {
                return Ok(access);
            }
        }
    }

    oauth
        .access_token
        .clone()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| AppError::Usage("Claude 凭据缺少 access token".to_string()))
}

async fn refresh_token(ctx: &ProviderContext, refresh: &str) -> AppResult<String> {
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
    Ok(parsed.access_token)
}

fn now_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}
