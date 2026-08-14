//! OpenCode Go subscription usage provider.
//!
//! Reads the OpenCode CLI's local auth file (`auth.json`, where the CLI stores
//! the `opencode-go` API key), then calls the Go plan's usage endpoint and maps
//! its rolling / weekly windows onto the panel's 5H / WEEK columns. The monthly
//! window is not surfaced (the panel has no month column). A key explicitly
//! stored in the AnyLeft keychain (or `OPENCODE_GO_API_KEY`) takes precedence
//! over the CLI file. No key is ever logged or returned to the frontend.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Deserialize;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::secrets;
use crate::settings::Account;

const USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
/// The key under which the OpenCode CLI stores the Go plan's API key.
const CLI_AUTH_KEY: &str = "opencode-go";

#[derive(Deserialize)]
struct UsageBody {
    usage: Option<UsageWindows>,
}

#[derive(Deserialize)]
struct UsageWindows {
    rolling: Option<Window>,
    weekly: Option<Window>,
}

#[derive(Deserialize)]
struct Window {
    percent: Option<f64>,
    #[serde(rename = "resetsAt")]
    resets_at: Option<String>,
}

pub struct OpenCodeGoProvider;

impl OpenCodeGoProvider {
    pub fn new() -> Self {
        OpenCodeGoProvider
    }
}

#[async_trait]
impl UsageProvider for OpenCodeGoProvider {
    async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage> {
        let api_key = read_api_key(&account.account_id)?;
        let response = ctx
            .http
            .get(USAGE_URL)
            .bearer_auth(api_key.trim())
            .header("Accept", "application/json")
            .header("User-Agent", "AnyLeft")
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(AppError::Usage(
                "OpenCode Go API Key 无效或已过期，请在 opencode 中重新登录".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::Usage(format!(
                "OpenCode Go 用量接口返回 HTTP {}",
                status.as_u16()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|err| AppError::Usage(format!("OpenCode Go 用量响应读取失败：{err}")))?;
        parse_usage(&body)
    }
}

fn parse_usage(body: &str) -> AppResult<Usage> {
    let parsed: UsageBody = serde_json::from_str(body)
        .map_err(|err| AppError::Usage(format!("OpenCode Go 用量解析失败：{err}")))?;
    let windows = parsed.usage.ok_or_else(|| {
        AppError::Usage("未找到 OpenCode Go 用量，请确认已订阅 Go 套餐".to_string())
    })?;
    let rolling = windows.rolling.as_ref();
    let weekly = windows.weekly.as_ref();
    if rolling.is_none() && weekly.is_none() {
        return Err(AppError::Usage("OpenCode Go 未返回用量窗口".to_string()));
    }

    Ok(Usage {
        five_hour: rolling.and_then(|w| w.percent).map(to_pct),
        five_hour_reset: rolling.and_then(|w| w.resets_at.clone()),
        weekly: to_pct(weekly.and_then(|w| w.percent).unwrap_or(0.0)),
        weekly_reset: weekly.and_then(|w| w.resets_at.clone()),
        plan: None,
        balance: None,
    })
}

fn to_pct(value: f64) -> u8 {
    value.round().clamp(0.0, 100.0) as u8
}

/// Resolve the Go API key: an explicit keychain entry wins, then the OpenCode
/// CLI's auth file, then the environment.
fn read_api_key(account_id: &str) -> AppResult<String> {
    if let Some(key) = secrets::get_key(account_id)? {
        return Ok(key);
    }
    if let Some(key) = read_cli_key() {
        return Ok(key);
    }
    if let Some(key) = env_api_key() {
        return Ok(key);
    }
    Err(AppError::Usage(
        "未找到 OpenCode Go API Key，请在 opencode 中登录并订阅 Go 套餐，或粘贴 API Key".to_string(),
    ))
}

/// Load the `opencode-go` API key from the CLI's `auth.json`. Returns `None`
/// when the file, entry, or key is missing — the caller then reports a clear
/// error instead of guessing.
fn read_cli_key() -> Option<String> {
    let text = cli_auth_paths()
        .into_iter()
        .find_map(|path| std::fs::read_to_string(path).ok())?;
    parse_cli_key(&text)
}

/// Parse the Go API key out of an `auth.json` payload. Only `type: "api"`
/// entries carry a usable key (`oauth`/`wellknown` entries don't).
fn parse_cli_key(text: &str) -> Option<String> {
    let entries: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text).ok()?;
    let entry = entries.get(CLI_AUTH_KEY)?;
    if entry.get("type").and_then(|v| v.as_str()) != Some("api") {
        return None;
    }
    entry
        .get("key")?
        .as_str()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

/// Where the OpenCode CLI keeps its auth file. `xdg-basedir` (which opencode
/// uses) resolves `XDG_DATA_HOME`, defaulting to `~/.local/share`; the macOS
/// Application Support path is included as a fallback for future changes.
fn cli_auth_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        paths.push(PathBuf::from(xdg).join("opencode").join("auth.json"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(
            PathBuf::from(&home)
                .join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json"),
        );
        paths.push(
            PathBuf::from(&home)
                .join("Library")
                .join("Application Support")
                .join("opencode")
                .join("auth.json"),
        );
    }
    paths
}

fn env_api_key() -> Option<String> {
    std::env::var("OPENCODE_GO_API_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_usage_into_panel_windows() {
        let usage = parse_usage(
            r#"{"usage":{
                "rolling":{"status":"ok","percent":9.4,"resetsAt":"2026-08-14T06:40:41.905Z"},
                "weekly":{"status":"ok","percent":3,"resetsAt":"2026-08-17T00:00:00.905Z"},
                "monthly":{"status":"ok","percent":1,"resetsAt":"2026-09-13T03:38:30.905Z"}
            }}"#,
        )
        .unwrap();
        assert_eq!(usage.five_hour, Some(9));
        assert_eq!(usage.five_hour_reset.as_deref(), Some("2026-08-14T06:40:41.905Z"));
        assert_eq!(usage.weekly, 3);
        assert_eq!(usage.weekly_reset.as_deref(), Some("2026-08-17T00:00:00.905Z"));
    }

    #[test]
    fn treats_missing_rolling_window_as_no_five_hour_column() {
        let usage = parse_usage(
            r#"{"usage":{
                "weekly":{"status":"ok","percent":55,"resetsAt":"2026-08-17T00:00:00.000Z"}
            }}"#,
        )
        .unwrap();
        assert_eq!(usage.five_hour, None);
        assert_eq!(usage.five_hour_reset, None);
        assert_eq!(usage.weekly, 55);
    }

    #[test]
    fn errors_when_no_usage_block_is_present() {
        let err = parse_usage(r#"{"usage":null}"#).unwrap_err();
        assert!(err.to_string().contains("订阅"), "unexpected message: {err}");
    }

    #[test]
    fn errors_when_all_windows_are_missing() {
        let err = parse_usage(r#"{"usage":{}}"#).unwrap_err();
        assert!(err.to_string().contains("用量窗口"), "unexpected message: {err}");
    }

    #[test]
    fn errors_on_malformed_json() {
        let err = parse_usage("not json").unwrap_err();
        assert!(err.to_string().contains("解析失败"), "unexpected message: {err}");
    }

    #[test]
    fn clamps_percent_to_valid_range() {
        let usage = parse_usage(
            r#"{"usage":{
                "rolling":{"status":"ok","percent":120,"resetsAt":null},
                "weekly":{"status":"ok","percent":-5,"resetsAt":null}
            }}"#,
        )
        .unwrap();
        assert_eq!(usage.five_hour, Some(100));
        assert_eq!(usage.weekly, 0);
    }

    #[test]
    fn extracts_api_key_from_cli_auth_json() {
        let text = r#"{
            "opencode-go": {"type": "api", "key": "sk-go-abc123"},
            "opencode": {"type": "api", "key": "sk-zen-xyz"}
        }"#;
        assert_eq!(parse_cli_key(text).as_deref(), Some("sk-go-abc123"));
    }

    #[test]
    fn rejects_non_api_and_missing_entries() {
        assert_eq!(parse_cli_key(r#"{}"#), None);
        assert_eq!(
            parse_cli_key(r#"{"opencode-go":{"type":"oauth","refresh":"r","access":"a"}}"#),
            None
        );
        assert_eq!(parse_cli_key(r#"{"opencode-go":{"type":"api"}}"#), None);
        assert_eq!(parse_cli_key("not json"), None);
    }
}
