//! Real Codex / ChatGPT usage provider.
//!
//! Reads the local **Codex CLI** login (`~/.codex/auth.json`,
//! `~/.config/codex/auth.json`, `$CODEX_HOME/auth.json`, or the macOS keychain
//! item `Codex Auth`), refreshes the token on a 401, then calls ChatGPT's usage
//! endpoint and maps the two rolling windows onto the panel's 5H / WEEK columns.
//!
//! Approach and endpoints follow the OpenUsage project's Codex provider.
//! No token or credential blob is ever logged or returned to the frontend.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::settings::Account;

const KEYCHAIN_SERVICE: &str = "Codex Auth";
const USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REFRESH_URL: &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

#[derive(Deserialize, Clone, Default)]
struct Tokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    account_id: Option<String>,
    /// OpenID id token — a JWT whose `https://api.openai.com/auth` claim carries
    /// `chatgpt_plan_type` ("plus", "pro", …). Read locally for the live plan.
    id_token: Option<String>,
}

impl Tokens {
    fn access(&self) -> Option<&str> {
        self.access_token.as_deref().filter(|t| !t.trim().is_empty())
    }
}

#[derive(Deserialize, Default)]
struct CodexAuth {
    tokens: Option<Tokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct Window {
    used_percent: Option<f64>,
    reset_at: Option<i64>,
}

#[derive(Deserialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Deserialize)]
struct UsageResponse {
    rate_limit: Option<RateLimit>,
}

pub struct CodexProvider;

impl CodexProvider {
    pub fn new() -> Self {
        CodexProvider
    }
}

#[async_trait]
impl UsageProvider for CodexProvider {
    async fn fetch(&self, ctx: &ProviderContext, _account: &Account) -> AppResult<Usage> {
        let auth = read_local_auth()?;
        let tokens = auth.tokens.filter(|t| t.access().is_some()).ok_or_else(|| {
            if auth.openai_api_key.is_some() {
                AppError::Usage("API Key 无法读取订阅用量，请用 `codex` 登录".to_string())
            } else {
                AppError::Usage("未找到 Codex 登录凭据，请先运行 `codex` 登录".to_string())
            }
        })?;

        let access = tokens.access().unwrap().to_string();
        let account_id = tokens.account_id.as_deref();
        let plan = plan_from_id_token(tokens.id_token.as_deref());

        let (status, body) = request_usage(ctx, &access, account_id).await?;
        if status == 401 || status == 403 {
            let refresh = tokens.refresh_token.as_deref().ok_or_else(|| {
                AppError::Usage("Codex 会话已过期，请运行 `codex` 重新登录".to_string())
            })?;
            let fresh = refresh_token(ctx, refresh).await?;
            let (retry_status, retry_body) = request_usage(ctx, &fresh, account_id).await?;
            return finish(retry_status, &retry_body, plan);
        }
        finish(status, &body, plan)
    }
}

fn finish(status: u16, body: &str, plan: Option<String>) -> AppResult<Usage> {
    if !(200..300).contains(&status) {
        return Err(AppError::Usage(format!("Codex 用量接口返回 HTTP {status}")));
    }
    Ok(Usage {
        plan,
        ..parse_usage(body)?
    })
}

/// Extract `chatgpt_plan_type` from the id token's `https://api.openai.com/auth`
/// claim. The JWT is trusted local data, so we decode (not verify) its payload.
/// Returns `None` when absent/unparseable, hiding the plan rather than guessing.
fn plan_from_id_token(id_token: Option<&str>) -> Option<String> {
    let payload = id_token?.trim().split('.').nth(1)?;
    let bytes = base64url_decode(payload)?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let plan = claims
        .get("https://api.openai.com/auth")?
        .get("chatgpt_plan_type")?
        .as_str()?;
    super::pretty_plan(Some(plan))
}

/// Minimal base64url (no padding) decoder — avoids pulling in a base64 crate for
/// the one JWT payload we read.
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    fn sextet(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }
    let input = input.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for &c in input {
        buffer = (buffer << 6) | sextet(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn parse_usage(body: &str) -> AppResult<Usage> {
    let parsed: UsageResponse = serde_json::from_str(body)
        .map_err(|err| AppError::Usage(format!("Codex 用量解析失败：{err}")))?;
    let rate_limit = parsed.rate_limit;
    let five = rate_limit
        .as_ref()
        .and_then(|r| r.primary_window.as_ref())
        .and_then(|w| w.used_percent);
    let week = rate_limit
        .as_ref()
        .and_then(|r| r.secondary_window.as_ref())
        .and_then(|w| w.used_percent);

    let five_reset = rate_limit
        .as_ref()
        .and_then(|r| r.primary_window.as_ref())
        .and_then(|w| w.reset_at);
    let week_reset = rate_limit
        .as_ref()
        .and_then(|r| r.secondary_window.as_ref())
        .and_then(|w| w.reset_at);

    if five.is_none() && week.is_none() {
        return Err(AppError::Usage("Codex 未返回用量窗口".to_string()));
    }
    Ok(Usage {
        five_hour: to_pct(five.unwrap_or(0.0)),
        five_hour_reset: ts_to_iso(five_reset),
        weekly: to_pct(week.unwrap_or(0.0)),
        weekly_reset: ts_to_iso(week_reset),
        plan: None,
    })
}

fn ts_to_iso(ts: Option<i64>) -> Option<String> {
    let secs = ts?;
    if secs <= 0 {
        return None;
    }
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_secs(secs as u64);
    let dt = chrono::DateTime::<chrono::Utc>::from(d);
    Some(dt.to_rfc3339())
}

fn to_pct(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

async fn request_usage(
    ctx: &ProviderContext,
    access: &str,
    account_id: Option<&str>,
) -> AppResult<(u16, String)> {
    let mut request = ctx
        .http
        .get(USAGE_URL)
        .header("Authorization", format!("Bearer {access}"))
        .header("Accept", "application/json")
        .header("User-Agent", "AnyLeft");
    if let Some(id) = account_id.filter(|id| !id.is_empty()) {
        request = request.header("ChatGPT-Account-Id", id);
    }
    let response = request.send().await?;
    let status = response.status().as_u16();
    let body = response.text().await?;
    Ok((status, body))
}

async fn refresh_token(ctx: &ProviderContext, refresh: &str) -> AppResult<String> {
    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", CLIENT_ID),
        ("refresh_token", refresh),
    ];
    let response = ctx.http.post(REFRESH_URL).form(&params).send().await?;
    if !response.status().is_success() {
        return Err(AppError::Usage(format!(
            "Codex token 刷新失败 HTTP {}",
            response.status().as_u16()
        )));
    }
    let parsed: RefreshResponse = response.json().await?;
    Ok(parsed.access_token)
}

/// Load Codex auth from the CLI's file locations, then the keychain.
fn read_local_auth() -> AppResult<CodexAuth> {
    for path in auth_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(auth) = parse_auth(&text) {
                return Ok(auth);
            }
        }
    }
    if let Some(json) = read_keychain_json() {
        if let Some(auth) = parse_auth(&json) {
            return Ok(auth);
        }
    }
    Err(AppError::Usage(
        "未找到 Codex 登录凭据，请先运行 `codex` 登录".to_string(),
    ))
}

fn auth_paths() -> Vec<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        return vec![Path::new(&home).join("auth.json")];
    }
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let home = Path::new(&home);
        paths.push(home.join(".config/codex/auth.json"));
        paths.push(home.join(".codex/auth.json"));
    }
    paths
}

fn parse_auth(text: &str) -> Option<CodexAuth> {
    let auth: CodexAuth = serde_json::from_str(text).ok()?;
    let has_token = auth
        .tokens
        .as_ref()
        .map(|t| t.access().is_some())
        .unwrap_or(false);
    if has_token || auth.openai_api_key.is_some() {
        Some(auth)
    } else {
        None
    }
}

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
