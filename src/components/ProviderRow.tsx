import type { DashboardProvider } from "../types";
import { color, font } from "../theme";

const PCT_COL = 38;

function remainingPercent(used: number | null): number {
  return Math.max(0, Math.min(100, 100 - (used ?? 0)));
}

/** Condense a failure message into a short right-aligned status label. */
function shortStatus(error: string): string {
  if (/token|api key|钥匙串/i.test(error)) return "需配置";
  if (/登录|凭据|codex|claude/i.test(error)) return "需登录";
  if (/暂未接入|不支持/.test(error)) return "未接入";
  return "读取失败";
}

/** One provider line in the menu-bar dropdown: chip · name · plan · remaining 5H · remaining week,
 *  or a muted failure state when the provider couldn't be read. */
export function ProviderRow({ provider }: { provider: DashboardProvider }) {
  const failed = provider.error != null;

  return (
    <div
      title={provider.error ?? undefined}
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "8px 4px",
        borderRadius: 9,
        opacity: failed ? 0.72 : 1,
      }}
    >
      <span
        style={{
          width: 9,
          height: 9,
          borderRadius: 2,
          background: failed ? color.toggleOff : provider.accent,
          flex: "none",
        }}
      />
      <div style={{ flex: 1, minWidth: 0, display: "flex", alignItems: "baseline", gap: 7 }}>
        <span style={{ fontWeight: 600, fontSize: 13, color: color.ink }}>{provider.name}</span>
        <span
          style={{
            fontSize: 10.5,
            color: color.faint,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {provider.plan}
        </span>
      </div>

      {failed ? (
        <span
          style={{
            flex: "none",
            display: "inline-flex",
            alignItems: "center",
            gap: 5,
            fontSize: 11,
            fontWeight: 600,
            color: color.warn,
          }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2">
            <path d="M12 9v4" strokeLinecap="round" />
            <path d="M12 17h.01" strokeLinecap="round" />
            <path
              d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"
              strokeLinejoin="round"
            />
          </svg>
          {shortStatus(provider.error!)}
        </span>
      ) : (
        <>
          <span
            style={{
              flex: "none",
              width: PCT_COL,
              textAlign: "right",
              fontFamily: font.mono,
              fontSize: 13.5,
              fontWeight: 600,
              color: provider.accent,
            }}
          >
            {remainingPercent(provider.fiveHour)}%
          </span>
          <span
            style={{
              flex: "none",
              width: PCT_COL,
              textAlign: "right",
              fontFamily: font.mono,
              fontSize: 13.5,
              fontWeight: 600,
              color: "#8a8072",
            }}
          >
            {remainingPercent(provider.weekly)}%
          </span>
        </>
      )}
    </div>
  );
}
