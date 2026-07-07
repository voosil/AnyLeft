//! A single error type shared across the native bridge.
//!
//! Every `#[tauri::command]` returns `Result<T, AppError>`, so the error must
//! serialize to something the frontend can display. We serialize to a plain
//! string message — detailed context is logged on the Rust side.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("配置读写失败 / config i/o failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("配置解析失败 / config parse failed: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("钥匙串访问失败 / keychain access failed: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("窗口操作失败 / window operation failed: {0}")]
    Tauri(#[from] tauri::Error),

    #[error("网络请求失败 / http request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("用量接口返回错误 / usage endpoint error: {0}")]
    Usage(String),

    #[error("未找到服务商 / unknown provider: {0}")]
    UnknownProvider(String),

    #[error("输入无效 / invalid input: {0}")]
    Invalid(String),
}

/// Serialize the error as its user-facing message string.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
