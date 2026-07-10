//! DeepSeek pay-as-you-go balance provider.
//!
//! Reads the user's API credit balance from `GET https://api.deepseek.com/user/balance`.
//! Only the充值余额 (`topped_up_balance`) is surfaced, formatted with the provider's
//! currency symbol (e.g. "¥100.00").

use async_trait::async_trait;
use serde::Deserialize;

use super::{ProviderContext, UsageProvider};
use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::secrets;
use crate::settings::Account;

const BALANCE_URL: &str = "https://api.deepseek.com/user/balance";

#[derive(Deserialize)]
struct BalanceResponse {
    #[serde(rename = "is_available")]
    _is_available: Option<bool>,
    balance_infos: Option<Vec<BalanceInfo>>,
}

#[derive(Deserialize)]
struct BalanceInfo {
    currency: Option<String>,
    #[serde(rename = "total_balance")]
    _total_balance: Option<String>,
    #[serde(rename = "granted_balance")]
    _granted_balance: Option<String>,
    topped_up_balance: Option<String>,
}

pub struct DeepseekProvider;

impl DeepseekProvider {
    pub fn new() -> Self {
        DeepseekProvider
    }
}

#[async_trait]
impl UsageProvider for DeepseekProvider {
    async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage> {
        let api_key = read_api_key(&account.account_id)?;
        let response = ctx
            .http
            .get(BALANCE_URL)
            .bearer_auth(api_key.trim())
            .header("Accept", "application/json")
            .header("User-Agent", "AnyLeft")
            .send()
            .await?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Usage(
                "DeepSeek API Key 无效或已过期，请重新配置".to_string(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::Usage(format!(
                "DeepSeek 余额接口返回 HTTP {}",
                status.as_u16()
            )));
        }

        let body: BalanceResponse = response
            .json()
            .await
            .map_err(|err| AppError::Usage(format!("DeepSeek 余额解析失败：{err}")))?;

        let info = body
            .balance_infos
            .and_then(|infos| infos.into_iter().next())
            .ok_or_else(|| AppError::Usage("DeepSeek 未返回余额信息".to_string()))?;

        let amount = info
            .topped_up_balance
            .as_deref()
            .and_then(|v| parse_amount(v))
            .ok_or_else(|| AppError::Usage("DeepSeek 未返回充值余额".to_string()))?;

        let currency = info.currency.as_deref().unwrap_or("CNY");
        let balance = format_balance(currency, amount);

        Ok(Usage {
            five_hour: 0,
            five_hour_reset: None,
            weekly: 0,
            weekly_reset: None,
            plan: None,
            balance: Some(balance),
        })
    }
}

fn read_api_key(account_id: &str) -> AppResult<String> {
    if let Some(key) = secrets::get_key(account_id)? {
        return Ok(key);
    }
    if let Some(key) = env_api_key() {
        return Ok(key);
    }
    Err(AppError::Usage(
        "未找到 DeepSeek API Key，请在设置中添加，或提供 DEEPSEEK_API_KEY".to_string(),
    ))
}

fn env_api_key() -> Option<String> {
    std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

fn parse_amount(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn format_balance(currency: &str, amount: f64) -> String {
    let symbol = match currency.trim().to_uppercase().as_str() {
        "USD" => "$",
        _ => "¥",
    };
    format!("{}{:.2}", symbol, amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_cny_balance() {
        assert_eq!(format_balance("CNY", 100.0), "¥100.00");
        assert_eq!(format_balance("cny", 0.5), "¥0.50");
    }

    #[test]
    fn formats_usd_balance() {
        assert_eq!(format_balance("USD", 12.345), "$12.35");
    }

    #[test]
    fn parses_amounts() {
        assert_eq!(parse_amount("100.00"), Some(100.0));
        assert_eq!(parse_amount("  50.5  "), Some(50.5));
        assert_eq!(parse_amount(""), None);
    }
}
