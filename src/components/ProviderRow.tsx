import type { DashboardProvider } from "../types";
import { color, font } from "../theme";

const PCT_COL = 48;

function remainingPercent(used: number | null): number {
  return Math.max(0, Math.min(100, 100 - (used ?? 0)));
}

function formatResetTime(isoTime: string | null): string {
  if (!isoTime) return "";
  const date = new Date(isoTime);
  if (isNaN(date.getTime())) return "";
  const now = new Date();
  const isToday =
    date.getDate() === now.getDate() &&
    date.getMonth() === now.getMonth() &&
    date.getFullYear() === now.getFullYear();
  const hours = date.getHours().toString().padStart(2, "0");
  const minutes = date.getMinutes().toString().padStart(2, "0");
  if (isToday) {
    return `${hours}:${minutes}`;
  }
  const month = date.getMonth() + 1;
  const day = date.getDate();
  return `${month}/${day} ${hours}:${minutes}`;
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
        {provider.plan && (
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
        )}
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
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.2"
          >
            <path d="M12 9v4" strokeLinecap="round" />
            <path d="M12 17h.01" strokeLinecap="round" />
            <path
              d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"
              strokeLinejoin="round"
            />
          </svg>
          {shortStatus(provider.error!)}
        </span>
      ) : provider.balance ? (
        <div
          style={{
            flex: "none",
            width: PCT_COL * 2 + 10,
            textAlign: "right",
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-end",
            justifyContent: "center",
          }}
        >
          <span
            style={{
              fontFamily: font.mono,
              fontSize: 13.5,
              fontWeight: 600,
              color: provider.accent,
            }}
          >
            {provider.balance}
          </span>
        </div>
      ) : (
        <>
          <div
            style={{
              flex: "none",
              width: PCT_COL,
              textAlign: "right",
              display: "flex",
              flexDirection: "column",
              alignItems: "flex-end",
            }}
          >
            <span
              style={{
                fontFamily: font.mono,
                fontSize: 13.5,
                fontWeight: 600,
                color: provider.fiveHour == null ? color.faint : provider.accent,
              }}
            >
              {provider.fiveHour == null ? "—" : `${remainingPercent(provider.fiveHour)}%`}
            </span>
            {provider.fiveHourReset && (
              <span
                style={{ fontSize: 9, color: color.faint, marginTop: -2, whiteSpace: "nowrap" }}
              >
                {formatResetTime(provider.fiveHourReset)}
              </span>
            )}
          </div>
          <div
            style={{
              flex: "none",
              width: PCT_COL,
              textAlign: "right",
              display: "flex",
              flexDirection: "column",
              alignItems: "flex-end",
            }}
          >
            <span
              style={{
                fontFamily: font.mono,
                fontSize: 13.5,
                fontWeight: 600,
                color: "#8a8072",
              }}
            >
              {remainingPercent(provider.weekly)}%
            </span>
            {provider.weeklyReset && (
              <span
                style={{ fontSize: 9, color: color.faint, marginTop: -2, whiteSpace: "nowrap" }}
              >
                {formatResetTime(provider.weeklyReset)}
              </span>
            )}
          </div>
        </>
      )}
    </div>
  );
}
