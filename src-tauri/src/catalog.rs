//! The static catalog of supported LLM providers.
//!
//! This is the single source of truth for provider metadata (name, company,
//! badge, plan, colors). It mirrors the catalog in the Design project so the
//! UI renders identically whether data comes from Rust or a design preview.

use crate::error::{AppError, AppResult};
use crate::models::CatalogProvider;

/// Raw catalog rows. Kept as tuples to stay compact and easy to audit:
/// (id, name, company, mono, plan, accent, tint)
const CATALOG: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    ("claude", "Claude", "Anthropic", "C", "Max 5×", "#C96442", "rgba(201,100,66,.13)"),
    ("gpt", "ChatGPT", "OpenAI", "GPT", "Pro", "#5F7F58", "rgba(95,127,88,.16)"),
    ("glm", "GLM", "Zhipu", "GLM", "Coding Pro", "#2C5288", "rgba(44,82,136,.13)"),
    ("kimi", "Kimi", "Moonshot", "K", "Kimi+", "#B4831F", "rgba(224,178,74,.22)"),
    ("minimax", "MiniMax", "MiniMax", "M", "Token Plan", "#9A5A34", "rgba(154,90,52,.15)"),
    ("gemini", "Gemini", "Google", "G", "Advanced", "#3B6CB3", "rgba(59,108,179,.14)"),
    ("grok", "Grok", "xAI", "X", "SuperGrok", "#4A4A4A", "rgba(74,74,74,.12)"),
    ("cursor", "Cursor", "Anysphere", "Cu", "Pro", "#6E8A4E", "rgba(110,138,78,.15)"),
    ("deepseek", "DeepSeek", "DeepSeek", "DS", "Pay-as-go", "#4457A6", "rgba(68,87,166,.13)"),
];

fn row_to_provider(row: &(&str, &str, &str, &str, &str, &str, &str)) -> CatalogProvider {
    CatalogProvider {
        id: row.0.to_string(),
        name: row.1.to_string(),
        company: row.2.to_string(),
        mono: row.3.to_string(),
        plan: row.4.to_string(),
        accent: row.5.to_string(),
        tint: row.6.to_string(),
    }
}

/// The full ordered catalog.
pub fn all() -> Vec<CatalogProvider> {
    CATALOG.iter().map(row_to_provider).collect()
}

/// Look up one catalog entry, erroring on an unknown id (input validation for
/// anything that arrives from the frontend).
pub fn get(id: &str) -> AppResult<CatalogProvider> {
    CATALOG
        .iter()
        .find(|row| row.0 == id)
        .map(row_to_provider)
        .ok_or_else(|| AppError::UnknownProvider(id.to_string()))
}

/// Whether an id exists in the catalog.
pub fn exists(id: &str) -> bool {
    CATALOG.iter().any(|row| row.0 == id)
}
