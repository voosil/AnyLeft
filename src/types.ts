/**
 * TypeScript mirrors of the Rust bridge models (`src-tauri/src/models.rs` and
 * `settings.rs`). Field names match the serde `camelCase` output exactly.
 */

export type AuthMethod = "key" | "login";

export interface CatalogProvider {
  id: string;
  name: string;
  company: string;
  mono: string;
  plan: string;
  accent: string;
  tint: string;
}

export interface DashboardProvider {
  /** Unique per connected account (a provider can have several). */
  accountId: string;
  /** Catalog id this account belongs to, e.g. "kimi". */
  providerId: string;
  /** Display name — the account's custom label, or the catalog name. */
  name: string;
  /** Subscription/plan label; null when unknown (then hidden). */
  plan: string | null;
  accent: string;
  enabled: boolean;
  /** Used quota percentage; display code converts this to remaining quota. */
  fiveHour: number | null;
  fiveHourReset: string | null;
  weekly: number | null;
  weeklyReset: string | null;
  /** Formatted API credit balance (e.g. "¥100.00"); null for quota-only providers. */
  balance: string | null;
  /** User-facing failure message; null on success. */
  error: string | null;
  /** True when this row is a placeholder still waiting for its fetch to finish. */
  loading?: boolean;
}

export interface Dashboard {
  providers: DashboardProvider[];
  /** Highest used quota across readable providers; null when nothing was read. */
  highest: number | null;
  nearLimit: boolean;
}

export interface Account {
  /** Unique per account; the keychain key. */
  accountId: string;
  /** Catalog id this account belongs to. */
  providerId: string;
  /** Optional user-chosen name; null falls back to the catalog name. */
  label: string | null;
  enabled: boolean;
  authMethod: AuthMethod;
  hasSecret: boolean;
}

export interface Preferences {
  menubarPercent: boolean;
  nearLimitAlert: boolean;
  launchAtLogin: boolean;
  sortByPressure: boolean;
  shortcut: string;
  accent: string;
}

export interface AppSettings {
  accounts: Account[];
  preferences: Preferences;
}
