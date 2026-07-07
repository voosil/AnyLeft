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
  id: string;
  name: string;
  plan: string;
  accent: string;
  enabled: boolean;
  /** Used quota percentage; display code converts this to remaining quota. */
  fiveHour: number | null;
  fiveHourReset: string | null;
  weekly: number | null;
  weeklyReset: string | null;
  /** User-facing failure message; null on success. */
  error: string | null;
}

export interface Dashboard {
  providers: DashboardProvider[];
  /** Highest used quota across readable providers; null when nothing was read. */
  highest: number | null;
  nearLimit: boolean;
}

export interface Account {
  id: string;
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
