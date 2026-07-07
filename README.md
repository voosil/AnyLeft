# AnyLeft 剩了么

A macOS **menu-bar app** that tracks how much of your subscription quota is left
across LLM providers — Claude, ChatGPT, GLM, Kimi, MiniMax, Gemini, Grok, Cursor,
DeepSeek. Click the menu-bar icon to see every provider's **5-hour** and **weekly**
usage at a glance.

Built with **Tauri v2 + React/Vite**, with a **Rust native bridge** for state,
persistence, and OS integration. UI implemented from the `AnyLeft.dc.html` and
`AnyLeft Settings.dc.html` designs.

## Screens

- **Panel** — the menu-bar dropdown. Provider rows sorted by pressure (highest
  usage first), with the peak percentage shown next to the clock.
- **Settings** — connected accounts (enable/disable per provider), an add-account
  flow (pick provider → API Key or browser login), and preferences (menu-bar
  percentage, near-limit alert, launch at login, summon shortcut ⌘⇧U).

## Tech stack

| Layer | Choice |
| --- | --- |
| Desktop shell | Tauri v2 (tray icon, transparent windows, global shortcut, autostart) |
| Frontend | React 18 + Vite 5 + TypeScript |
| Native bridge | Rust — commands, settings persistence, keychain, pluggable providers |
| Secrets | OS keychain via the `keyring` crate (keys never touch disk or the UI) |

## Project layout

```
any-left/
├── index.html                 # single entry; ?window=settings selects the screen
├── src/                       # React frontend
│   ├── api/bridge.ts          # typed invoke() wrappers (+ browser mock fallback)
│   ├── api/mock.ts            # in-memory backend for browser preview
│   ├── types.ts               # TS mirrors of the Rust models
│   ├── theme.ts               # design tokens (colors, fonts, gradient)
│   ├── components/            # Toggle, ProviderBadge, Kbd, ProviderRow, AddAccountModal
│   ├── screens/Panel.tsx      # menu-bar dropdown
│   └── screens/Settings.tsx   # settings window
└── src-tauri/                 # Rust native bridge
    ├── tauri.conf.json        # windows (panel + settings), tray, bundle
    ├── capabilities/          # per-window permission grants
    └── src/
        ├── lib.rs             # app setup: plugins, state, tray, shortcut
        ├── commands.rs        # the invoke surface
        ├── state.rs           # shared state + dashboard read model
        ├── settings.rs        # persisted settings + immutable update helpers
        ├── catalog.rs         # static provider catalog
        ├── secrets.rs         # keychain-backed API keys
        ├── providers/         # UsageProvider trait + mock implementation
        ├── tray.rs            # menu-bar icon, live %, panel positioning
        └── windows.rs         # show/hide helpers
```

## Running

Prerequisites: Node 18+, pnpm, and the Rust toolchain (`rustup`).

```bash
pnpm install
pnpm app:dev      # tauri dev — launches the menu-bar app
```

Build a distributable:

```bash
pnpm app:build    # tauri build — produces a .app / .dmg
```

Preview just the UI in a browser (uses the in-memory mock backend, no Rust):

```bash
pnpm dev
# open http://localhost:1420/            → panel
# open http://localhost:1420/?window=settings → settings
```

## How the data flows

The frontend only ever talks to the Rust bridge through the typed helpers in
`src/api/bridge.ts`. Each mutating command validates its input, builds a **new**
settings value (nothing is mutated in place), persists it to
`~/Library/Application Support/com.voosil.anyleft/settings.json`, refreshes the
menu-bar number, and returns the fresh settings so the UI renders from a single
source of truth.

Every provider id resolves through an async `UsageProvider` trait, so real vendor
integrations drop in per provider without touching the commands or UI:

```rust
// src-tauri/src/providers/mod.rs
#[async_trait]
pub trait UsageProvider: Send + Sync {
    async fn fetch(&self, ctx: &ProviderContext, account: &Account) -> AppResult<Usage>;
}
```

**There is no mock data.** Two providers are real integrations:

- **Claude** (`providers/claude.rs`) — reads the local **Claude Code** OAuth login
  (macOS keychain `Claude Code-credentials`, `~/.claude/.credentials.json`, or
  `CLAUDE_CODE_OAUTH_TOKEN`), refreshes the token if needed, calls
  `GET https://api.anthropic.com/api/oauth/usage`, and maps
  `five_hour.utilization` → **5H**, `seven_day.utilization` → **WEEK**.
- **ChatGPT / Codex** (`providers/codex.rs`) — reads the local **Codex CLI** login
  (`~/.codex/auth.json`, `~/.config/codex/auth.json`, `$CODEX_HOME/auth.json`, or
  keychain `Codex Auth`), refreshes on a 401, calls
  `GET https://chatgpt.com/backend-api/wham/usage`, and maps
  `rate_limit.primary_window.used_percent` → **5H**,
  `secondary_window.used_percent` → **WEEK**.

Endpoints follow the [OpenUsage](https://github.com/robinebers/openusage) project.
macOS prompts for keychain access on first read.

When a provider can't be read (not logged in, network error, or **not yet
integrated** for the other catalog entries), that row shows a real **failure
state** (⚠ + short reason, full message on hover) — never a fabricated number.
A fresh install connects only the integrated providers (`claude`, `gpt`); others
can be added from settings and will show the "not yet integrated" state.

To add a real integration, implement `UsageProvider` (reading an API key with
`secrets::get_key` where relevant) and register it in
`ProviderRegistry::with_defaults`.

Successful reads are cached for 60s (and shared with the menu-bar number) to keep
the panel snappy and avoid hammering rate-limited endpoints — "refresh" bypasses
the cache; failures are not cached, so they retry on the next open.

## Notes

- The app runs as a macOS *accessory* — menu bar only, no Dock icon.
- API keys are stored in the login keychain under the service `com.voosil.anyleft`.
- Closing the settings window hides it; the app keeps running in the menu bar.
