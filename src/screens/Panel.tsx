import { useCallback, useEffect, useRef, useState } from "react";
import { bridge } from "../api/bridge";
import { ProviderRow } from "../components/ProviderRow";
import { useAutoResize } from "../hooks/useAutoResize";
import { color, font } from "../theme";
import type { DashboardProvider } from "../types";

const PCT_COL = 38;

/**
 * The menu-bar dropdown. Loads the dashboard on mount and re-fetches whenever
 * the window regains focus (i.e. each time the tray icon reopens it).
 * Provider rows are rendered as soon as they arrive, first-finished first-displayed.
 */
export function Panel() {
  const cardRef = useRef<HTMLDivElement>(null);
  const settingsLinkRef = useRef<HTMLAnchorElement>(null);
  const [providers, setProviders] = useState<DashboardProvider[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const startFetch = useCallback((force: boolean) => {
    setLoading(true);
    setError(null);
    setProviders([]);
    bridge
      .watchDashboard(
        force,
        (provider) => setProviders((prev) => [...prev, provider]),
        () => setLoading(false),
      )
      .catch((err) => {
        setError(String(err));
        setLoading(false);
      });
  }, []);

  const load = useCallback(() => startFetch(false), [startFetch]);
  const refresh = useCallback(() => startFetch(true), [startFetch]);

  useEffect(() => {
    void load();
    const clearSettingsFocus = () => settingsLinkRef.current?.blur();
    const onFocus = () => {
      clearSettingsFocus();
      void load();
    };
    clearSettingsFocus();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [load]);

  useAutoResize(cardRef, [providers.length, error, loading]);

  const showSkeleton = loading && providers.length === 0;
  const hasProviders = providers.length > 0;

  return (
    <div
      ref={cardRef}
      style={{
        animation: "panelIn .5s cubic-bezier(.2,.85,.25,1) both",
        background: color.cardGlass,
        // backdropFilter: "blur(24px) saturate(1.3)",
        // WebkitBackdropFilter: "blur(24px) saturate(1.3)",
        border: `1px solid rgba(255,255,255,.62)`,
        borderRadius: 15,
        boxShadow: "inset 0 1px 0 rgba(255,255,255,.75)",
        padding: "13px 13px 8px",
        width: 340,
      }}
    >
      {/* header */}
      <header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 4px 9px",
          borderBottom: `1px solid ${color.hair}`,
          marginBottom: 5,
        }}
      >
        <span style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <span style={{ fontWeight: 700, fontSize: 15, color: color.inkStrong }}>剩了么</span>
          <span
            style={{
              fontFamily: font.mono,
              fontSize: 9,
              letterSpacing: ".05em",
              color: color.faint,
              textTransform: "uppercase",
            }}
          >
            Reset: 5h & week
          </span>
        </span>
        <div
          style={{
            display: "flex",
            gap: 14,
            fontFamily: font.mono,
            fontSize: 9,
            letterSpacing: ".06em",
            color: color.faint,
          }}
        >
          <span style={{ width: PCT_COL, textAlign: "right" }}>5H</span>
          <span style={{ width: PCT_COL, textAlign: "right" }}>Week</span>
        </div>
      </header>

      {/* provider rows */}
      {error ? (
        <div style={{ padding: "16px 6px", fontSize: 12, color: color.brown }}>
          无法读取用量 · {error}
        </div>
      ) : showSkeleton ? (
        <PanelSkeleton />
      ) : !hasProviders ? (
        <div style={{ padding: "18px 6px", fontSize: 12.5, color: color.faint }}>
          还没有启用的账户，去设置里添加 →
        </div>
      ) : (
        <ProviderRows providers={providers} />
      )}

      {/* footer */}
      <footer
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "9px 6px 4px",
          marginTop: 5,
          borderTop: `1px solid ${color.hair}`,
          fontSize: 12,
        }}
      >
        <button
          onClick={() => void refresh()}
          disabled={loading}
          aria-label="刷新用量"
          title="刷新用量"
          style={{
            background: "none",
            border: "none",
            padding: 4,
            marginLeft: -4,
            cursor: loading ? "default" : "pointer",
            color: loading ? color.faint : color.muted,
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            borderRadius: 5,
            outline: "none",
            transition: "color .12s ease",
          }}
        >
          <svg
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            style={{
              animation: loading ? "spin 0.8s linear infinite" : undefined,
            }}
          >
            <path d="M21 12a9 9 0 1 1-3.5-7.1" />
            <path d="M21 4v5h-5" />
          </svg>
        </button>
        <a
          ref={settingsLinkRef}
          href="#settings"
          tabIndex={-1}
          onPointerDown={(e) => e.preventDefault()}
          onClick={(e) => {
            e.preventDefault();
            e.currentTarget.blur();
            void bridge.openSettings();
          }}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            color: color.muted,
            fontWeight: 500,
            outline: "none",
          }}
        >
          设置
          <span
            style={{
              fontFamily: font.mono,
              fontSize: 10,
              color: color.faint,
              background: color.chip,
              borderRadius: 5,
              padding: "2px 6px",
            }}
          >
            ⌘,
          </span>
        </a>
      </footer>
    </div>
  );
}

/** Split rows by data type: package-quota providers first, then a divider, then API-credit balance providers. */
function ProviderRows({ providers }: { providers: DashboardProvider[] }) {
  const quota = providers.filter((p) => !isBalanceProvider(p));
  const balances = providers.filter((p) => isBalanceProvider(p));
  const showDivider = quota.length > 0 && balances.length > 0;

  return (
    <>
      {quota.map((p) => (
        <ProviderRow key={p.accountId} provider={p} />
      ))}
      {showDivider && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "6px 4px",
          }}
        >
          <div style={{ flex: 1, height: 1, background: color.hair }} />
          <span style={{ fontSize: 9, color: color.faint, whiteSpace: "nowrap" }}>API 余额</span>
          <div style={{ flex: 1, height: 1, background: color.hair }} />
        </div>
      )}
      {balances.map((p) => (
        <ProviderRow key={p.accountId} provider={p} />
      ))}
    </>
  );
}

function isBalanceProvider(provider: DashboardProvider): boolean {
  return provider.providerId === "deepseek" || provider.balance != null;
}

/** Placeholder rows shown before the first fetch resolves. */
function PanelSkeleton() {
  return (
    <>
      {Array.from({ length: 5 }).map((_, i) => (
        <div key={i} style={{ display: "flex", alignItems: "center", gap: 10, padding: "8px 4px" }}>
          <span style={{ width: 9, height: 9, borderRadius: 2, background: color.hair }} />
          <span
            style={{
              flex: 1,
              height: 10,
              borderRadius: 4,
              background: color.hairSoft,
              maxWidth: 120,
            }}
          />
          <span
            style={{ width: PCT_COL, height: 10, borderRadius: 4, background: color.hairSoft }}
          />
          <span
            style={{ width: PCT_COL, height: 10, borderRadius: 4, background: color.hairSoft }}
          />
        </div>
      ))}
    </>
  );
}
