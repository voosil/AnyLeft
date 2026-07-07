//! MiniMax Token Plan usage provider.
//!
//! Uses the same endpoint as the `minimax-status` CLI:
//! `GET https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains`.
//! MiniMax returns `*_remaining_percent` values, while AnyLeft stores used
//! percentages, so the provider converts each window with `100 - remaining`.

use std::path::Path;

use async_trait::async_trait;
use serde::Deserialize;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::secrets;
use crate::settings::Account;

const USAGE_URL: &str = "https://www.minimaxi.com/v1/api/openplatform/coding_plan/remains";
const CONFIG_FILE: &str = ".minimax-config.json";

#[derive(Deserialize)]
struct MinimaxConfig {
    token: Option<String>,
}

#[derive(Deserialize)]
struct RemainsResponse {
    model_remains: Option<Vec<ModelRemain>>,
}

#[derive(Deserialize)]
struct ModelRemain {
    current_interval_remaining_percent: Option<f64>,
    end_time: Option<i64>,
    current_weekly_remaining_percent: Option<f64>,
    weekly_end_time: Option<i64>,
}

pub struct MinimaxProvider;

impl MinimaxProvider {
    pub fn new() -> Self {
        MinimaxProvider
    }
}

#[async_trait]
impl UsageProvider for MinimaxProvider {
    async fn fetch(&self, ctx: &ProviderContext, _account: &Account) -> AppResult<Usage> {
        let token = read_token()?;
        let response = ctx
            .http
            .get(USAGE_URL)
            .bearer_auth(token.trim())
            .header("Referer", "https://platform.minimaxi.com/")
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Usage(
                "MiniMax token 无效或已过期，请重新配置".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::Usage(format!(
                "MiniMax 用量接口返回 HTTP {}",
                status.as_u16()
            )));
        }

        let body: RemainsResponse = response.json().await?;
        let model = body
            .model_remains
            .and_then(|mut models| {
                if models.is_empty() {
                    None
                } else {
                    Some(models.remove(0))
                }
            })
            .ok_or_else(|| AppError::Usage("MiniMax 未返回 Token Plan 用量".to_string()))?;

        Ok(Usage {
            five_hour: remaining_to_used(model.current_interval_remaining_percent),
            five_hour_reset: ts_to_iso(model.end_time),
            weekly: remaining_to_used(model.current_weekly_remaining_percent),
            weekly_reset: ts_to_iso(model.weekly_end_time),
        })
    }
}

fn ts_to_iso(ts: Option<i64>) -> Option<String> {
    let ms = ts?;
    if ms <= 0 {
        return None;
    }
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_millis(ms as u64);
    let dt = chrono::DateTime::<chrono::Utc>::from(d);
    Some(dt.to_rfc3339())
}

fn remaining_to_used(value: Option<f64>) -> u8 {
    let remaining = value.unwrap_or(100.0).round().clamp(0.0, 100.0);
    (100.0 - remaining).round() as u8
}

fn read_token() -> AppResult<String> {
    if let Some(token) = secrets::get_key("minimax")? {
        return Ok(token);
    }
    if let Some(token) = env_token() {
        return Ok(token);
    }
    if let Some(token) = config_token() {
        return Ok(token);
    }
    Err(AppError::Usage(
        "未找到 MiniMax token，请在设置中添加 MiniMax API Key，或提供 MINIMAX_TOKEN / ~/.minimax-config.json"
            .to_string(),
    ))
}

fn env_token() -> Option<String> {
    ["MINIMAX_TOKEN", "MINIMAX_API_KEY"]
        .iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn config_token() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let path = Path::new(&home).join(CONFIG_FILE);
    let text = std::fs::read_to_string(path).ok()?;
    let config: MinimaxConfig = serde_json::from_str(&text).ok()?;
    config
        .token
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}
