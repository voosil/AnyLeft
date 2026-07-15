/**
 * The typed native bridge — thin wrappers around Tauri `invoke`.
 *
 * When running inside Tauri, calls go to the Rust commands. In a plain browser
 * they fall back to `mockBackend` so the UI previews and design-reviews cleanly.
 * Every function returns a Promise and throws a string on error.
 */

import type {
  AppSettings,
  AuthMethod,
  CatalogProvider,
  Dashboard,
  DashboardProvider,
  Preferences,
} from "../types";
import { previewBackend, type PreviewCommand } from "./preview";

/** True when the Tauri runtime is present. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Invoke a command. In Tauri this hits the Rust bridge; in a plain browser it
 *  falls back to the preview fixture (never used in the packaged app). */
async function call<T>(command: PreviewCommand, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(command, args);
  }
  const handler = previewBackend[command] as (a?: Record<string, unknown>) => Promise<T>;
  return handler(args);
}

/**
 * Stream the dashboard one provider at a time as soon as each fetch completes.
 * `onProvider` is called for every row; `onComplete` is called once with the
 * final summary (highest/near-limit) after every provider has finished.
 */
async function watchDashboard(
  force: boolean,
  onProvider: (provider: DashboardProvider) => void,
  onComplete?: (dashboard: Dashboard) => void,
): Promise<void> {
  if (isTauri()) {
    const { Channel } = await import("@tauri-apps/api/core");
    const channel = new Channel(onProvider);
    const dashboard = await call<Dashboard>(force ? "refresh" : "get_dashboard", { channel });
    onComplete?.(dashboard);
    return;
  }

  const dashboard = await call<Dashboard>(force ? "refresh" : "get_dashboard");
  // Preview mode: simulate streaming so the UI looks the same as the real app.
  dashboard.providers.forEach((provider, index) => {
    setTimeout(() => onProvider(provider), index * 120);
  });
  setTimeout(() => onComplete?.(dashboard), dashboard.providers.length * 120 + 50);
}

export const bridge = {
  getCatalog: () => call<CatalogProvider[]>("get_catalog"),
  getSettings: () => call<AppSettings>("get_settings"),
  /**
   * Stream the dashboard one provider at a time as soon as each fetch completes.
   * `force=false` serves the cache; `force=true` bypasses it.
   */
  watchDashboard: (
    force: boolean,
    onProvider: (provider: DashboardProvider) => void,
    onComplete?: (dashboard: Dashboard) => void,
  ) => watchDashboard(force, onProvider, onComplete),
  /**
   * Load the dashboard (cached). Rows are delivered one by one via `onProvider`;
   * the final summary is delivered via `onComplete`.
   */
  getDashboard: (
    onProvider: (provider: DashboardProvider) => void,
    onComplete?: (dashboard: Dashboard) => void,
  ) => watchDashboard(false, onProvider, onComplete),
  /**
   * Force a fresh dashboard fetch. Rows are delivered one by one via `onProvider`;
   * the final summary is delivered via `onComplete`.
   */
  refresh: (
    onProvider: (provider: DashboardProvider) => void,
    onComplete?: (dashboard: Dashboard) => void,
  ) => watchDashboard(true, onProvider, onComplete),

  /**
   * Connect, reconfigure, or rename an account. `accountId` reconfigures an
   * existing one (edit key/label); omit it to add a new account.
   */
  connectAccount: (
    providerId: string,
    authMethod: AuthMethod,
    apiKey?: string,
    label?: string,
    accountId?: string,
  ) => call<AppSettings>("connect_account", { providerId, authMethod, apiKey, label, accountId }),
  disconnectAccount: (accountId: string) =>
    call<AppSettings>("disconnect_account", { accountId }),
  setAccountEnabled: (accountId: string, enabled: boolean) =>
    call<AppSettings>("set_account_enabled", { accountId, enabled }),
  setPreferences: (preferences: Preferences) =>
    call<AppSettings>("set_preferences", { preferences }),

  openSettings: () => call<void>("open_settings"),
  closeSettings: () => call<void>("close_settings"),
  hidePanel: () => call<void>("hide_panel"),
  quit: () => call<void>("quit"),
};

/** Toggle launch-at-login via the autostart plugin (no-op outside Tauri). */
export async function setAutostart(enabled: boolean): Promise<void> {
  if (!isTauri()) return;
  const { enable, disable } = await import("@tauri-apps/plugin-autostart");
  if (enabled) await enable();
  else await disable();
}

/** Open an external URL in the default browser (no-op outside Tauri). */
export async function openExternal(url: string): Promise<void> {
  if (!isTauri()) {
    window.open(url, "_blank");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}
