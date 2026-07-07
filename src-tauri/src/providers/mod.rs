//! Pluggable usage providers.
//!
//! Each LLM vendor exposes usage differently (or not at all). The `UsageProvider`
//! trait abstracts "given a connected account, what fraction of each quota
//! window is used?" so real integrations drop in per provider without touching
//! the commands or UI.
//!
//! Real integrations today: `claude`, `gpt` (ChatGPT/Codex), and `minimax`.
//! They read local credentials and call each provider's usage endpoint — there
//! is no mock data. Any other id reports a clear "not yet integrated" error
//! that the panel surfaces per row.

pub mod claude;
pub mod codex;
pub mod minimax;

use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::{AppError, AppResult};
use crate::models::Usage;
use crate::settings::Account;

/// Cheaply-cloneable context handed to every provider fetch (shared HTTP pool).
#[derive(Clone)]
pub struct ProviderContext {
    pub http: reqwest::Client,
}

/// A source of live usage numbers for one provider.
#[async_trait]
pub trait UsageProvider: Send + Sync {
    /// Fetch the current usage for the given connected account.
    async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage>;
}

/// Maps provider ids to their concrete `UsageProvider` implementation.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn UsageProvider>>,
}

impl ProviderRegistry {
    /// Register every real integration. Unregistered ids error at fetch time.
    pub fn with_defaults() -> Self {
        let mut providers: HashMap<String, Box<dyn UsageProvider>> = HashMap::new();
        providers.insert("claude".to_string(), Box::new(claude::ClaudeProvider::new()));
        providers.insert("gpt".to_string(), Box::new(codex::CodexProvider::new()));
        providers.insert("minimax".to_string(), Box::new(minimax::MinimaxProvider::new()));
        ProviderRegistry { providers }
    }

    /// Fetch usage for one account. An id without a real integration returns a
    /// friendly "not yet supported" error rather than any fabricated data.
    pub async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage> {
        match self.providers.get(&account.id) {
            Some(provider) => provider.fetch(ctx, account).await,
            None => Err(AppError::Usage(
                "暂未接入自动读取用量，敬请期待".to_string(),
            )),
        }
    }
}
