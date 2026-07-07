import { useCallback, useEffect, useRef, useState } from "react";
import { bridge } from "../api/bridge";
import { ProviderRow } from "../components/ProviderRow";
import { useAutoResize } from "../hooks/useAutoResize";
import { color, font } from "../theme";
import type { Dashboard } from "../types";

const PCT_COL = 38;

/**
 * The menu-bar dropdown. Loads the dashboard on mount and re-fetches whenever
 * the window regains focus (i.e. each time the tray icon reopens it).
 */
export function Panel() {
  const cardRef = useRef<HTMLDivElement>(null);
  const settingsLinkRef = useRef<HTMLAnchorElement>(null);
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      // Non-forcing: serves the 60s cache so reopening the panel never hammers
      // provider endpoints. The tray's "刷新用量" menu item forces a fresh fetch.
      setDashboard(await bridge.getDashboard());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  }, []);

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

  useAutoResize(cardRef, [dashboard, error]);

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
      ) : !dashboard ? (
        <PanelSkeleton />
      ) : dashboard.providers.length === 0 ? (
        <div style={{ padding: "18px 6px", fontSize: 12.5, color: color.faint }}>
          还没有启用的账户，去设置里添加 →
        </div>
      ) : (
        dashboard.providers.map((p) => <ProviderRow key={p.id} provider={p} />)
      )}

      {/* footer */}
      <footer
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          padding: "9px 6px 4px",
          marginTop: 5,
          borderTop: `1px solid ${color.hair}`,
          fontSize: 12,
        }}
      >
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
