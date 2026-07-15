//! Real Codex / ChatGPT usage provider.
//!
//! Reads the local **Codex CLI** login (`~/.codex/auth.json`,
//! `~/.config/codex/auth.json`, `$CODEX_HOME/auth.json`, or the macOS keychain
//! item `Codex Auth`), refreshes the token on a 401, then calls ChatGPT's usage
//! endpoint and maps the weekly rolling window onto the panel's WEEK column.
//!
//! (ChatGPT no longer exposes a separate 5-hour window; only the weekly window
//! is reported.)
//!
//! Approach and endpoints follow the OpenUsage project's Codex provider.
//! No token or credential blob is ever logged or returned to the frontend.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Clone, Default)]
struct CodexAuth {
    tokens: Option<Tokens>,
    #[serde(rename = "OPENAI_API_KEY")]
    openai_api_key: Option<String>,
}

#[derive(Deserialize)]
struct RefreshResponse {
    access_token: String,
}

#[derive(Deserialize, Serialize)]
struct Window {
    used_percent: Option<f64>,
    reset_at: Option<i64>,
}

#[derive(Deserialize, Serialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Deserialize, Serialize)]
struct UsageResponse {
    rate_limit: Option<RateLimit>,
}

pub struct CodexProvider {
    /// Process-lifetime credential cache. Reading the `Codex Auth` keychain
    /// item triggers a macOS password prompt, so we read it at most once per
    /// launch and reuse the result on subsequent fetches. A 401/403 (stale
    /// credentials, or the user re-logged in elsewhere) invalidates the cache
    /// so the next fetch re-reads the keychain once.
    cache: Mutex<Option<CodexAuth>>,
}

impl CodexProvider {
    pub fn new() -> Self {
        CodexProvider {
            cache: Mutex::new(None),
        }
    }

    fn cached(&self) -> Option<CodexAuth> {
        self.cache
            .lock()
            .expect("codex cache poisoned")
            .clone()
    }

    fn store(&self, auth: CodexAuth) {
        *self.cache.lock().expect("codex cache poisoned") = Some(auth);
    }

    fn invalidate(&self) {
        *self.cache.lock().expect("codex cache poisoned") = None;
    }

    /// Return cached auth or load it once and remember the result. Hitting the
    /// keychain happens at most once per launch — later fetches skip past it.
    fn load(&self) -> AppResult<CodexAuth> {
        if let Some(auth) = self.cached() {
            return Ok(auth);
        }
        let auth = read_local_auth()?;
        self.store(auth.clone());
        Ok(auth)
    }
}

/// Build the user-facing error for an auth blob that has no usable access
/// token. Kept at module scope so `fetch` can reuse it after a refresh fallback.
fn no_access_error(auth: &CodexAuth) -> AppError {
    if auth.openai_api_key.is_some() {
        AppError::Usage("API Key 无法读取订阅用量，请用 `codex` 登录".to_string())
    } else {
        AppError::Usage("未找到 Codex 登录凭据，请先运行 `codex` 登录".to_string())
    }
}

#[async_trait]
impl UsageProvider for CodexProvider {
    async fn fetch(&self, ctx: &ProviderContext, _account: &Account) -> AppResult<Usage> {
        let auth = self.load()?;
        let tokens = auth
            .tokens
            .as_ref()
            .filter(|t| t.access().is_some())
            .ok_or_else(|| no_access_error(&auth))?;
        let access = tokens.access().unwrap();
        let account_id = tokens.account_id.as_deref();
        let plan = plan_from_id_token(tokens.id_token.as_deref());

        let (status, body) = request_usage(ctx, access, account_id).await?;
        if status != 401 && status != 403 {
            return finish(status, &body, plan);
        }

        // 401/403 — try the refresh token in-memory first so a successful
        // refresh doesn't even reach the keychain. If that path is exhausted
        // (no refresh token or the server rejected it), drop the cached auth
        // and re-read the keychain so a fresh `codex` login is observed.
        if let Some(refresh) = tokens.refresh_token.as_deref() {
            if let Ok(fresh) = refresh_token(ctx, refresh).await {
                let (status, body) = request_usage(ctx, &fresh, account_id).await?;
                return finish(status, &body, plan);
            }
        }

        self.invalidate();
        let auth = read_local_auth()?;
        self.store(auth.clone());
        let tokens = auth
            .tokens
            .as_ref()
            .filter(|t| t.access().is_some())
            .ok_or_else(|| {
                AppError::Usage(
                    "Codex 会话已过期，请运行 `codex` 重新登录".to_string(),
                )
            })?;
        let access = tokens.access().unwrap();
        let account_id = tokens.account_id.as_deref();
        let plan = plan_from_id_token(tokens.id_token.as_deref());
        let (status, body) = request_usage(ctx, access, account_id).await?;
        finish(status, &body, plan)
    }
}

fn finish(status: u16, body: &str, plan: Option<String>) -> AppResult<Usage> {
    if !(200..300).contains(&status) {
        return Err(AppError::Usage(format!("Codex 用量接口返回 HTTP {status}")));
    }
    Ok(Usage {
        plan,
        balance: None,
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
    let (five, week) = classify_windows(parsed.rate_limit.as_ref());

    if five.is_none() && week.is_none() {
        return Err(AppError::Usage("Codex 未返回用量窗口".to_string()));
    }
    Ok(Usage {
        five_hour: five.and_then(|w| w.used_percent).map(to_pct),
        five_hour_reset: five.and_then(|w| ts_to_iso(w.reset_at)),
        weekly: to_pct(week.and_then(|w| w.used_percent).unwrap_or(0.0)),
        weekly_reset: week.and_then(|w| ts_to_iso(w.reset_at)),
        plan: None,
        balance: None,
    })
}

/// Classify the two windows by their reset time, not by their field name.
/// ChatGPT no longer has a 5-hour window; a single returned window is treated
/// as weekly, and if one window resets in >24h while the other resets sooner,
/// the longer one is the weekly window.
fn classify_windows(rate_limit: Option<&RateLimit>) -> (Option<&Window>, Option<&Window>) {
    let rate_limit = match rate_limit {
        Some(r) => r,
        None => return (None, None),
    };

    let primary = rate_limit.primary_window.as_ref();
    let secondary = rate_limit.secondary_window.as_ref();

    let is_weekly = |w: &Window| -> bool {
        w.reset_at
            .map(|ts| {
                let now = chrono::Utc::now().timestamp();
                ts - now > 24 * 3600
            })
            .unwrap_or(false)
    };

    match (primary, secondary) {
        (Some(p), None) => (None, Some(p)),
        (None, Some(s)) => (None, Some(s)),
        (Some(p), Some(s)) => {
            if is_weekly(p) && !is_weekly(s) {
                (Some(s), Some(p))
            } else if is_weekly(s) && !is_weekly(p) {
                (Some(p), Some(s))
            } else {
                (Some(p), Some(s))
            }
        }
        (None, None) => (None, None),
    }
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


#[cfg(test)]
mod tests {
    use super::*;

    fn auth_with_access(access: &str) -> CodexAuth {
        CodexAuth {
            tokens: Some(Tokens {
                access_token: Some(access.to_string()),
                refresh_token: None,
                account_id: None,
                id_token: None,
            }),
            openai_api_key: None,
        }
    }

    #[test]
    fn cache_is_empty_at_start() {
        let p = CodexProvider::new();
        assert!(p.cached().is_none());
    }

    #[test]
    fn store_then_cached_round_trips() {
        let p = CodexProvider::new();
        p.store(auth_with_access("access-1"));
        let stored = p.cached().unwrap();
        assert_eq!(
            stored.tokens.unwrap().access_token.as_deref(),
            Some("access-1")
        );
    }

    #[test]
    fn invalidate_clears_cache() {
        let p = CodexProvider::new();
        p.store(auth_with_access("access-1"));
        assert!(p.cached().is_some());
        p.invalidate();
        assert!(p.cached().is_none());
    }

    // End-to-end verification of the keychain-side behaviour — i.e. the panel
    // only prompts for credentials once after launch — happens at runtime.

    #[test]
    fn treats_single_window_as_weekly() {
        let now = chrono::Utc::now().timestamp();
        let rate_limit = RateLimit {
            primary_window: Some(Window {
                used_percent: Some(42.0),
                reset_at: Some(now + 7 * 24 * 3600),
            }),
            secondary_window: None,
        };
        let usage = parse_usage(
            &serde_json::to_string(&UsageResponse {
                rate_limit: Some(rate_limit),
            })
            .unwrap(),
        )
        .unwrap();
        assert_eq!(usage.five_hour, None);
        assert_eq!(usage.weekly, 42);
    }

    #[test]
    fn classifies_windows_by_reset_time() {
        let now = chrono::Utc::now().timestamp();
        let rate_limit = RateLimit {
            primary_window: Some(Window {
                used_percent: Some(10.0),
                reset_at: Some(now + 7 * 24 * 3600),
            }),
            secondary_window: Some(Window {
                used_percent: Some(80.0),
                reset_at: Some(now + 4 * 3600),
            }),
        };
        let usage = parse_usage(
            &serde_json::to_string(&UsageResponse {
                rate_limit: Some(rate_limit),
            })
            .unwrap(),
        )
        .unwrap();
        // primary window resets in a week → weekly
        // secondary window resets in 4h → 5h
        assert_eq!(usage.five_hour, Some(80));
        assert_eq!(usage.weekly, 10);
    }
}