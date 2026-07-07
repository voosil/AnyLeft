/**
 * Browser-preview fixture — used ONLY when the app runs in a plain browser
 * (Vite dev without Tauri), so the UI can be reviewed without the Rust backend.
 * It is never bundled into the packaged app; the real app has no fabricated
 * usage data. To preview both UI states it shows Claude as a successful read and
 * ChatGPT as a "needs login" failure (a browser can't read either for real).
 */

import type {
  AppSettings,
  AuthMethod,
  CatalogProvider,
  Dashboard,
  DashboardProvider,
  Preferences,
} from "../types";

const CATALOG: CatalogProvider[] = [
  {
    id: "claude",
    name: "Claude",
    company: "Anthropic",
    mono: "C",
    plan: "Max 5×",
    accent: "#C96442",
    tint: "rgba(201,100,66,.13)",
  },
  {
    id: "gpt",
    name: "ChatGPT",
    company: "OpenAI",
    mono: "GPT",
    plan: "Pro",
    accent: "#5F7F58",
    tint: "rgba(95,127,88,.16)",
  },
  {
    id: "glm",
    name: "GLM",
    company: "Zhipu",
    mono: "GLM",
    plan: "Coding Pro",
    accent: "#2C5288",
    tint: "rgba(44,82,136,.13)",
  },
  {
    id: "kimi",
    name: "Kimi",
    company: "Moonshot",
    mono: "K",
    plan: "Kimi+",
    accent: "#B4831F",
    tint: "rgba(224,178,74,.22)",
  },
  {
    id: "minimax",
    name: "MiniMax",
    company: "MiniMax",
    mono: "M",
    plan: "Token Plan",
    accent: "#9A5A34",
    tint: "rgba(154,90,52,.15)",
  },
  {
    id: "gemini",
    name: "Gemini",
    company: "Google",
    mono: "G",
    plan: "Advanced",
    accent: "#3B6CB3",
    tint: "rgba(59,108,179,.14)",
  },
  {
    id: "grok",
    name: "Grok",
    company: "xAI",
    mono: "X",
    plan: "SuperGrok",
    accent: "#4A4A4A",
    tint: "rgba(74,74,74,.12)",
  },
  {
    id: "cursor",
    name: "Cursor",
    company: "Anysphere",
    mono: "Cu",
    plan: "Pro",
    accent: "#6E8A4E",
    tint: "rgba(110,138,78,.15)",
  },
  {
    id: "deepseek",
    name: "DeepSeek",
    company: "DeepSeek",
    mono: "DS",
    plan: "Pay-as-go",
    accent: "#4457A6",
    tint: "rgba(68,87,166,.13)",
  },
];

/** Preview-only: a successful read (Claude) to visualise the populated row. */
const PREVIEW_USAGE: Record<string, { fiveHour: number; weekly: number }> = {
  claude: { fiveHour: 42, weekly: 68 },
  minimax: { fiveHour: 27, weekly: 14 },
};

/** Preview-only: a failure state (ChatGPT) to visualise the error row. */
const PREVIEW_ERROR: Record<string, string> = {
  gpt: "未找到 Codex 登录凭据，请先运行 `codex` 登录",
};

const NOT_INTEGRATED = "暂未接入自动读取用量，敬请期待";
const NEAR_LIMIT = 85;

let settings: AppSettings = {
  accounts: ["claude", "gpt", "minimax"].map((id) => ({
    id,
    enabled: true,
    authMethod: "key" as AuthMethod,
    hasSecret: false,
  })),
  preferences: {
    menubarPercent: false,
    nearLimitAlert: false,
    launchAtLogin: true,
    sortByPressure: true,
    shortcut: "CommandOrControl+Shift+U",
    accent: "#C96442",
  },
};

const meta = (id: string) => CATALOG.find((p) => p.id === id);

function buildDashboard(): Dashboard {
  const rows: DashboardProvider[] = settings.accounts
    .filter((a) => a.enabled)
    .flatMap((a): DashboardProvider[] => {
      const m = meta(a.id);
      if (!m) return [];
      const usage = PREVIEW_USAGE[a.id];
      const base = { id: m.id, name: m.name, plan: m.plan, accent: m.accent, enabled: true };
      if (usage) {
        return [
          {
            ...base,
            fiveHour: usage.fiveHour,
            fiveHourReset: null,
            weekly: usage.weekly,
            weeklyReset: null,
            error: null,
          },
        ];
      }
      return [
        {
          ...base,
          fiveHour: null,
          fiveHourReset: null,
          weekly: null,
          weeklyReset: null,
          error: PREVIEW_ERROR[a.id] ?? NOT_INTEGRATED,
        },
      ];
    });

  if (settings.preferences.sortByPressure) {
    const has = (r: DashboardProvider) => (r.fiveHour != null || r.weekly != null ? 1 : 0);
    const press = (r: DashboardProvider) => Math.max(r.fiveHour ?? 0, r.weekly ?? 0);
    rows.sort((a, b) => has(b) - has(a) || press(b) - press(a));
  }

  const readable = rows.filter((r) => r.fiveHour != null || r.weekly != null);
  const highest = readable.length
    ? Math.max(...readable.map((r) => Math.max(r.fiveHour ?? 0, r.weekly ?? 0)))
    : null;

  return { providers: rows, highest, nearLimit: highest != null && highest >= NEAR_LIMIT };
}

export const previewBackend = {
  get_catalog: async (): Promise<CatalogProvider[]> => CATALOG.map((p) => ({ ...p })),
  get_settings: async (): Promise<AppSettings> => structuredClone(settings),
  get_dashboard: async (): Promise<Dashboard> => buildDashboard(),
  refresh: async (): Promise<Dashboard> => buildDashboard(),

  connect_account: async (args: { id: string; authMethod: AuthMethod; apiKey?: string }) => {
    const accounts = settings.accounts.filter((a) => a.id !== args.id);
    accounts.push({
      id: args.id,
      enabled: true,
      authMethod: args.authMethod,
      hasSecret: args.authMethod === "key" && !!args.apiKey?.trim(),
    });
    settings = { ...settings, accounts };
    return structuredClone(settings);
  },

  disconnect_account: async (args: { id: string }) => {
    settings = { ...settings, accounts: settings.accounts.filter((a) => a.id !== args.id) };
    return structuredClone(settings);
  },

  set_account_enabled: async (args: { id: string; enabled: boolean }) => {
    settings = {
      ...settings,
      accounts: settings.accounts.map((a) =>
        a.id === args.id ? { ...a, enabled: args.enabled } : a,
      ),
    };
    return structuredClone(settings);
  },

  set_preferences: async (args: { preferences: Preferences }) => {
    settings = { ...settings, preferences: { ...args.preferences } };
    return structuredClone(settings);
  },

  open_settings: async () => {},
  close_settings: async () => {},
  hide_panel: async () => {},
  quit: async () => {},
};

export type PreviewCommand = keyof typeof previewBackend;
