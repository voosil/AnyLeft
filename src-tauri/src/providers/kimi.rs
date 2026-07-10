//! Kimi For Coding usage provider.
//!
//! Uses the Kimi Code API key endpoint when a key is available, then falls back
//! to the Kimi web billing endpoint with a `kimi-auth` token. The AnyLeft
//! keychain entry for `kimi` can hold either credential type; environment
//! variables are also supported for local/dev setups.

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::secrets;
use crate::settings::Account;

const DEFAULT_API_BASE_URL: &str = "https://api.kimi.com";
const WEB_USAGE_URL: &str =
    "https://www.kimi.com/apiv2/kimi.gateway.billing.v1.BillingService/GetUsages";

#[derive(Deserialize)]
struct ApiUsageResponse {
    usage: Option<QuotaDetail>,
    limits: Option<Vec<LimitWindow>>,
}

#[derive(Deserialize)]
struct WebUsageResponse {
    usages: Option<Vec<WebUsage>>,
}

#[derive(Deserialize)]
struct WebUsage {
    scope: Option<String>,
    detail: Option<QuotaDetail>,
    limits: Option<Vec<LimitWindow>>,
}

#[derive(Deserialize)]
struct LimitWindow {
    window: Option<WindowSpec>,
    detail: Option<QuotaDetail>,
}

#[derive(Deserialize)]
struct WindowSpec {
    duration: Option<Value>,
    #[serde(rename = "timeUnit")]
    time_unit: Option<String>,
}

#[derive(Deserialize)]
struct QuotaDetail {
    limit: Option<Value>,
    used: Option<Value>,
    remaining: Option<Value>,
    #[serde(rename = "resetTime")]
    reset_time: Option<String>,
}

pub struct KimiProvider;

impl KimiProvider {
    pub fn new() -> Self {
        KimiProvider
    }
}

#[async_trait]
impl UsageProvider for KimiProvider {
    async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage> {
        let stored_secret = stored_secret(&account.account_id)?;
        let mut api_auth_error = None;

        if let Some(api_key) = stored_secret.clone().or_else(env_api_key) {
            let (status, body) = request_api_usage(ctx, &api_key).await?;
            if status.is_success() {
                return parse_api_usage(&body);
            }
            if is_auth_status(status) {
                api_auth_error = Some(AppError::Usage(
                    "Kimi API Key 无效或已过期，请重新配置".to_string(),
                ));
            } else {
                return Err(AppError::Usage(format!(
                    "Kimi Code 用量接口返回 HTTP {}",
                    status.as_u16()
                )));
            }
        }

        if let Some(token) = stored_secret.or_else(env_auth_token) {
            return fetch_web_usage(ctx, &token).await;
        }

        Err(api_auth_error.unwrap_or_else(|| {
            AppError::Usage(
                "未找到 Kimi 凭据，请在设置中添加 Kimi Code API Key，或提供 KIMI_CODE_API_KEY / KIMI_AUTH_TOKEN"
                    .to_string(),
            )
        }))
    }
}

async fn fetch_web_usage(ctx: &ProviderContext, token: &str) -> AppResult<Usage> {
    let response = ctx
        .http
        .post(WEB_USAGE_URL)
        .bearer_auth(token.trim())
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("Origin", "https://www.kimi.com")
        .header("Referer", "https://www.kimi.com/code/console")
        .header("User-Agent", "AnyLeft")
        .body("{}")
        .send()
        .await?;

    let status = response.status();
    if is_auth_status(status) {
        return Err(AppError::Usage(
            "Kimi API Key/token 无效或已过期，请重新配置".to_string(),
        ));
    }
    if !status.is_success() {
        return Err(AppError::Usage(format!(
            "Kimi web 用量接口返回 HTTP {}",
            status.as_u16()
        )));
    }

    let body = response.text().await?;
    parse_web_usage(&body)
}

async fn request_api_usage(
    ctx: &ProviderContext,
    api_key: &str,
) -> AppResult<(StatusCode, String)> {
    let response = ctx
        .http
        .get(api_usage_url())
        .bearer_auth(api_key.trim())
        .header("Accept", "application/json")
        .header("User-Agent", "AnyLeft")
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    Ok((status, body))
}

fn parse_api_usage(body: &str) -> AppResult<Usage> {
    let parsed: ApiUsageResponse = serde_json::from_str(body)
        .map_err(|err| AppError::Usage(format!("Kimi 用量解析失败：{err}")))?;
    usage_from_parts(parsed.usage.as_ref(), five_hour_detail(&parsed.limits))
}

fn parse_web_usage(body: &str) -> AppResult<Usage> {
    let parsed: WebUsageResponse = serde_json::from_str(body)
        .map_err(|err| AppError::Usage(format!("Kimi 用量解析失败：{err}")))?;
    let usages = parsed
        .usages
        .as_ref()
        .filter(|usages| !usages.is_empty())
        .ok_or_else(|| AppError::Usage("Kimi 未返回 coding 用量".to_string()))?;

    let coding = usages
        .iter()
        .find(|usage| usage.scope.as_deref() == Some("FEATURE_CODING"))
        .unwrap_or(&usages[0]);
    usage_from_parts(coding.detail.as_ref(), five_hour_detail(&coding.limits))
}

fn usage_from_parts(
    weekly: Option<&QuotaDetail>,
    five_hour: Option<&QuotaDetail>,
) -> AppResult<Usage> {
    let five_pct = five_hour.and_then(detail_pct);
    let week_pct = weekly.and_then(detail_pct);

    if five_pct.is_none() && week_pct.is_none() {
        return Err(AppError::Usage("Kimi 未返回用量窗口".to_string()));
    }

    Ok(Usage {
        five_hour: five_pct.unwrap_or(0),
        five_hour_reset: five_hour.and_then(detail_reset),
        weekly: week_pct.unwrap_or(0),
        weekly_reset: weekly.and_then(detail_reset),
        plan: None,
        balance: None,
    })
}

fn five_hour_detail(limits: &Option<Vec<LimitWindow>>) -> Option<&QuotaDetail> {
    let limits = limits.as_ref()?;
    limits
        .iter()
        .find(|limit| is_five_hour_window(limit))
        .and_then(|limit| limit.detail.as_ref())
        .or_else(|| limits.iter().find_map(|limit| limit.detail.as_ref()))
}

fn is_five_hour_window(limit: &LimitWindow) -> bool {
    let Some(window) = limit.window.as_ref() else {
        return false;
    };
    let is_300_minutes = value_to_f64(&window.duration)
        .map(|duration| (duration - 300.0).abs() < f64::EPSILON)
        .unwrap_or(false);
    let is_minutes = window
        .time_unit
        .as_deref()
        .map(|unit| {
            unit.eq_ignore_ascii_case("TIME_UNIT_MINUTE") || unit.eq_ignore_ascii_case("MINUTE")
        })
        .unwrap_or(false);
    is_300_minutes && is_minutes
}

fn detail_pct(detail: &QuotaDetail) -> Option<u8> {
    let limit = value_to_f64(&detail.limit)?;
    if limit <= 0.0 {
        return None;
    }
    let used = value_to_f64(&detail.used).or_else(|| {
        let remaining = value_to_f64(&detail.remaining)?;
        Some((limit - remaining).max(0.0))
    })?;
    Some(((used / limit) * 100.0).round().clamp(0.0, 100.0) as u8)
}

fn detail_reset(detail: &QuotaDetail) -> Option<String> {
    detail
        .reset_time
        .as_ref()
        .map(|reset| reset.trim().to_string())
        .filter(|reset| !reset.is_empty())
}

fn value_to_f64(value: &Option<Value>) -> Option<f64> {
    match value.as_ref()? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn stored_secret(account_id: &str) -> AppResult<Option<String>> {
    Ok(secrets::get_key(account_id)?
        .map(|secret| secret.trim().to_string())
        .filter(|secret| !secret.is_empty()))
}

fn env_api_key() -> Option<String> {
    env_var("KIMI_CODE_API_KEY")
}

fn env_auth_token() -> Option<String> {
    env_var("KIMI_AUTH_TOKEN")
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn api_usage_url() -> String {
    let base = env_var("KIMI_CODE_BASE_URL").unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string());
    format!("{}/coding/v1/usages", base.trim_end_matches('/'))
}

fn is_auth_status(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_api_usage_response() {
        let body = r#"{
            "usage": {
                "limit": "2048",
                "used": "214",
                "remaining": "1834",
                "resetTime": "2026-01-09T15:23:13.716839300Z"
            },
            "limits": [{
                "window": {"duration": 300, "timeUnit": "TIME_UNIT_MINUTE"},
                "detail": {
                    "limit": "200",
                    "used": "139",
                    "remaining": "61",
                    "resetTime": "2026-01-06T13:33:02.717479433Z"
                }
            }]
        }"#;

        let usage = parse_api_usage(body).unwrap();
        assert_eq!(usage.five_hour, 70);
        assert_eq!(usage.weekly, 10);
        assert_eq!(
            usage.five_hour_reset.as_deref(),
            Some("2026-01-06T13:33:02.717479433Z")
        );
        assert_eq!(
            usage.weekly_reset.as_deref(),
            Some("2026-01-09T15:23:13.716839300Z")
        );
    }

    #[test]
    fn parses_web_usage_response() {
        let body = r#"{
            "usages": [{
                "scope": "FEATURE_CODING",
                "detail": {
                    "limit": 1024,
                    "remaining": 768,
                    "resetTime": "2026-01-09T15:23:13.716839300Z"
                },
                "limits": [{
                    "window": {"duration": "300", "timeUnit": "TIME_UNIT_MINUTE"},
                    "detail": {
                        "limit": 200,
                        "remaining": 125,
                        "resetTime": "2026-01-06T13:33:02.717479433Z"
                    }
                }]
            }]
        }"#;

        let usage = parse_web_usage(body).unwrap();
        assert_eq!(usage.five_hour, 38);
        assert_eq!(usage.weekly, 25);
    }
}
