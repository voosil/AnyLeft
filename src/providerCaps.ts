/**
 * Provider capability flags shared across the UI. Mirrors the Rust
 * `catalog::is_single_instance` — providers authenticated through a local CLI
 * login (Claude Code / Codex) exist once on the machine, so the UI allows at
 * most one account for them. Everyone else can hold several accounts.
 */

const SINGLE_INSTANCE_PROVIDERS = new Set(["claude", "gpt"]);

/** Whether a provider may only have a single connected account. */
export function isSingleInstance(providerId: string): boolean {
  return SINGLE_INSTANCE_PROVIDERS.has(providerId);
}
