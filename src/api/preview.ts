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
    plan: "Kimi Code",
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
  kimi: { fiveHour: 70, weekly: 10 },
  minimax: { fiveHour: 27, weekly: 14 },
};

/** Preview-only: API credit balance for pay-as-you-go providers. */
const PREVIEW_BALANCE: Record<string, string> = {
  deepseek: "¥100.00",
};

/** Preview-only: live plan for providers that expose one (Claude). */
const PREVIEW_PLAN: Record<string, string> = {
  claude: "Max",
};

/** Preview-only: a failure state (ChatGPT) to visualise the error row. */
const PREVIEW_ERROR: Record<string, string> = {
  gpt: "未找到 Codex 登录凭据，请先运行 `codex` 登录",
};

const NOT_INTEGRATED = "暂未接入自动读取用量，敬请期待";
const NEAR_LIMIT = 85;

/** Providers whose plan is read live and hidden when unknown (mirrors Rust). */
const DYNAMIC_PLAN_PROVIDERS = new Set(["claude", "gpt"]);

let settings: AppSettings = {
  accounts: ["claude", "gpt", "kimi", "minimax", "deepseek"].map((id) => ({
    accountId: id,
    providerId: id,
    label: null,
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

/** Mirror of the backend `resolve_plan`: live value wins; dynamic-plan providers
 *  hide an unknown plan; others fall back to the catalog plan. */
function resolvePlan(providerId: string, catalogPlan: string, livePlan?: string): string | null {
  if (livePlan) return livePlan;
  return DYNAMIC_PLAN_PROVIDERS.has(providerId) ? null : catalogPlan;
}

function buildDashboard(): Dashboard {
  const rows: DashboardProvider[] = settings.accounts
    .filter((a) => a.enabled)
    .flatMap((a): DashboardProvider[] => {
      const m = meta(a.providerId);
      if (!m) return [];
      const usage = PREVIEW_USAGE[a.providerId];
      const base = {
        accountId: a.accountId,
        providerId: a.providerId,
        name: a.label?.trim() || m.name,
        accent: m.accent,
        enabled: true,
      };
      if (usage) {
        return [
          {
            ...base,
            plan: resolvePlan(a.providerId, m.plan, PREVIEW_PLAN[a.providerId]),
            fiveHour: usage.fiveHour,
            fiveHourReset: null,
            weekly: usage.weekly,
            weeklyReset: null,
            balance: null,
            error: null,
          },
        ];
      }
      const balance = PREVIEW_BALANCE[a.providerId];
      if (balance) {
        return [
          {
            ...base,
            plan: resolvePlan(a.providerId, m.plan, undefined),
            fiveHour: null,
            fiveHourReset: null,
            weekly: null,
            weeklyReset: null,
            balance,
            error: null,
          },
        ];
      }
      return [
        {
          ...base,
          plan: resolvePlan(a.providerId, m.plan, undefined),
          fiveHour: null,
          fiveHourReset: null,
          weekly: null,
          weeklyReset: null,
          balance: null,
          error: PREVIEW_ERROR[a.providerId] ?? NOT_INTEGRATED,
        },
      ];
    });

  if (settings.preferences.sortByPressure) {
    const has = (r: DashboardProvider) =>
      r.fiveHour != null || r.weekly != null || r.balance != null ? 1 : 0;
    const press = (r: DashboardProvider) =>
      Math.max(r.fiveHour ?? 0, r.weekly ?? 0);
    rows.sort((a, b) => has(b) - has(a) || press(b) - press(a));
  }

  const readable = rows.filter((r) =>
    r.fiveHour != null || r.weekly != null || r.balance != null
  );
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

  connect_account: async (args: {
    providerId: string;
    authMethod: AuthMethod;
    apiKey?: string;
    label?: string;
    accountId?: string;
  }) => {
    const single = args.providerId === "claude" || args.providerId === "gpt";
    const existing =
      args.accountId && settings.accounts.find((a) => a.accountId === args.accountId);
    const accountId = single
      ? args.providerId
      : existing
        ? args.accountId!
        : `${args.providerId}-${settings.accounts.length + 1}`;
    const label = args.label?.trim() ? args.label.trim() : null;
    const hasSecret =
      (args.authMethod === "key" && !!args.apiKey?.trim()) ||
      (!!existing && existing.hasSecret && args.authMethod === "key");
    const accounts = settings.accounts.filter((a) => a.accountId !== accountId);
    accounts.push({
      accountId,
      providerId: args.providerId,
      label,
      enabled: existing ? existing.enabled : true,
      authMethod: args.authMethod,
      hasSecret,
    });
    settings = { ...settings, accounts };
    return structuredClone(settings);
  },

  disconnect_account: async (args: { accountId: string }) => {
    settings = {
      ...settings,
      accounts: settings.accounts.filter((a) => a.accountId !== args.accountId),
    };
    return structuredClone(settings);
  },

  set_account_enabled: async (args: { accountId: string; enabled: boolean }) => {
    settings = {
      ...settings,
      accounts: settings.accounts.map((a) =>
        a.accountId === args.accountId ? { ...a, enabled: args.enabled } : a,
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
