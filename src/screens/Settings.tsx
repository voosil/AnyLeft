import { useEffect, useMemo, useState, type ReactNode } from "react";
import { bridge, setAutostart } from "../api/bridge";
import { AddAccountModal } from "../components/AddAccountModal";
import { Kbd } from "../components/Kbd";
import { ProviderBadge } from "../components/ProviderBadge";
import { Toggle } from "../components/Toggle";
import { isSingleInstance } from "../providerCaps";
import { color, font } from "../theme";
import type { Account, AppSettings, CatalogProvider, Preferences } from "../types";

export function Settings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [catalog, setCatalog] = useState<CatalogProvider[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
  const [configuring, setConfiguring] = useState<Account | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [s, c] = await Promise.all([bridge.getSettings(), bridge.getCatalog()]);
        setSettings(s);
        setCatalog(c);
      } catch (err) {
        setError(String(err));
      }
    })();
  }, []);

  const byId = useMemo(() => {
    const map: Record<string, CatalogProvider> = {};
    catalog.forEach((p) => (map[p.id] = p));
    return map;
  }, [catalog]);

  const guard = async (op: () => Promise<AppSettings>) => {
    try {
      setSettings(await op());
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  const toggleAccount = (accountId: string, enabled: boolean) =>
    guard(() => bridge.setAccountEnabled(accountId, enabled));

  const disconnectAccount = (accountId: string) =>
    guard(() => bridge.disconnectAccount(accountId));

  const updatePref = <K extends keyof Preferences>(key: K, value: Preferences[K]) => {
    if (!settings) return;
    const next: Preferences = { ...settings.preferences, [key]: value };
    if (key === "launchAtLogin") void setAutostart(value as boolean);
    void guard(() => bridge.setPreferences(next));
  };

  return (
    <div
      style={{
        position: "relative",
        width: "100vw",
        height: "100vh",
        background: color.card,
        border: "1px solid rgba(255,255,255,.6)",
        borderRadius: 16,
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
        animation: "panelIn .5s cubic-bezier(.2,.85,.25,1) both",
      }}
    >
      <TitleBar onClose={() => void bridge.closeSettings()} />

      <div style={{ padding: "20px 22px 22px", overflowY: "auto", flex: 1 }}>
        {error && (
          <div style={{ fontSize: 12, color: color.accent, marginBottom: 12 }}>{error}</div>
        )}

        <SectionLabel>已连接账户</SectionLabel>
        <div
          style={{
            border: `1px solid ${color.hair}`,
            borderRadius: 12,
            overflow: "hidden",
            marginBottom: 22,
            background: color.inner,
          }}
        >
          {settings?.accounts.map((account) => {
            const meta = byId[account.providerId];
            if (!meta) return null;
            const label = account.label?.trim();
            const displayName = label || meta.name;
            const subtitle = label ? `${meta.name} · ${meta.company}` : meta.company;
            // Custom naming / re-configuring is for multi-account providers;
            // Claude & ChatGPT read a fixed local login, so they're excluded.
            const canConfigure = !isSingleInstance(account.providerId);
            return (
              <div
                key={account.accountId}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 12,
                  padding: "11px 14px",
                  borderBottom: `1px solid ${color.hairSoft}`,
                  opacity: account.enabled ? 1 : 0.5,
                }}
              >
                <ProviderBadge mono={meta.mono} accent={meta.accent} tint={meta.tint} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div
                    style={{
                      fontWeight: 600,
                      fontSize: 13.5,
                      color: color.ink,
                      whiteSpace: "nowrap",
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                    }}
                  >
                    {displayName}
                  </div>
                  <div style={{ fontSize: 11, color: color.faint }}>{subtitle}</div>
                </div>
                {canConfigure && (
                  <button
                    onClick={() => {
                      setConfiguring(account);
                      setModalOpen(true);
                    }}
                    style={{
                      border: `1px solid ${color.hairStrong}`,
                      background: color.card,
                      color: color.brown,
                      borderRadius: 7,
                      padding: "5px 8px",
                      fontSize: 11,
                      fontWeight: 600,
                      cursor: "pointer",
                    }}
                  >
                    配置
                  </button>
                )}
                <StatusPill enabled={account.enabled} />
                <Toggle
                  on={account.enabled}
                  onToggle={() => toggleAccount(account.accountId, !account.enabled)}
                  label={`${displayName} 开关`}
                />
                <button
                  onClick={() => disconnectAccount(account.accountId)}
                  style={{
                    background: "none",
                    border: "none",
                    padding: 4,
                    cursor: "pointer",
                    color: color.faint,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    marginLeft: 4,
                  }}
                  title="删除账户"
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M3 6h18" />
                    <path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" />
                    <path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" />
                  </svg>
                </button>
              </div>
            );
          })}

          <div
            onClick={() => setModalOpen(true)}
            style={{
              padding: "12px 14px",
              display: "flex",
              alignItems: "center",
              gap: 10,
              cursor: "pointer",
            }}
          >
            <span
              style={{
                width: 30,
                height: 30,
                borderRadius: 9,
                border: `1.5px dashed rgba(51,48,42,.28)`,
                color: color.brown,
                display: "grid",
                placeItems: "center",
                fontSize: 17,
                flex: "none",
              }}
            >
              +
            </span>
            <span style={{ fontWeight: 600, fontSize: 13.5, color: color.brown }}>添加账户</span>
            <span style={{ marginLeft: "auto", color: "#c3b8a6", fontSize: 16 }}>›</span>
          </div>
        </div>

        <SectionLabel>偏好设置</SectionLabel>
        <div
          style={{
            border: `1px solid ${color.hair}`,
            borderRadius: 12,
            overflow: "hidden",
            background: color.inner,
          }}
        >
          <PrefRow title="唤出面板快捷键" subtitle="从任意位置呼出菜单栏面板" divider>
            <span style={{ display: "inline-flex", gap: 4 }}>
              <Kbd>⌘</Kbd>
              <Kbd>⇧</Kbd>
              <Kbd>U</Kbd>
            </span>
          </PrefRow>
          <PrefRow title="接近上限提醒" subtitle="任一窗口剩余低于 15% 时通知" divider>
            <Toggle
              on={settings?.preferences.nearLimitAlert ?? false}
              onToggle={() => updatePref("nearLimitAlert", !settings?.preferences.nearLimitAlert)}
              label="接近上限提醒"
            />
          </PrefRow>
          <PrefRow title="开机自动启动" subtitle="登录时自动运行 AnyLeft">
            <Toggle
              on={settings?.preferences.launchAtLogin ?? true}
              onToggle={() => updatePref("launchAtLogin", !settings?.preferences.launchAtLogin)}
              label="开机自动启动"
            />
          </PrefRow>
        </div>
      </div>

      <AddAccountModal
        open={modalOpen}
        catalog={catalog}
        connected={settings?.accounts.map((a) => a.providerId) ?? []}
        mode={configuring ? "configure" : "add"}
        initialProviderId={configuring?.providerId}
        initialAccountId={configuring?.accountId}
        initialLabel={configuring?.label ?? undefined}
        onClose={() => {
          setModalOpen(false);
          setConfiguring(null);
        }}
        onConnected={(next) => {
          setSettings(next);
          setModalOpen(false);
          setConfiguring(null);
        }}
      />
    </div>
  );
}

function TitleBar({ onClose }: { onClose: () => void }) {
  const light = (bg: string, onClick?: () => void) => (
    <span
      onClick={onClick}
      style={{
        width: 12,
        height: 12,
        borderRadius: "50%",
        background: bg,
        cursor: onClick ? "pointer" : "default",
      }}
    />
  );
  return (
    <div
      data-tauri-drag-region=""
      style={{
        height: 40,
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "0 14px",
        background: `linear-gradient(180deg,${color.titlebarTop},${color.titlebarBottom})`,
        borderBottom: `1px solid rgba(51,48,42,.09)`,
      }}
    >
      {light(color.trafficRed, onClose)}
      {light(color.trafficAmber)}
      {light(color.trafficGreen)}
      <span
        data-tauri-drag-region=""
        style={{
          flex: 1,
          textAlign: "center",
          fontSize: 13,
          fontWeight: 600,
          color: color.inkSoft,
          marginLeft: -46,
          pointerEvents: "none",
        }}
      >
        AnyLeft · 设置
      </span>
    </div>
  );
}

function SectionLabel({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        fontFamily: font.mono,
        fontSize: 10.5,
        letterSpacing: ".1em",
        color: color.faint,
        marginBottom: 10,
      }}
    >
      {children}
    </div>
  );
}

function StatusPill({ enabled }: { enabled: boolean }) {
  const c = enabled ? color.green : color.faint;
  return (
    <span
      style={{
        fontFamily: font.mono,
        fontSize: 10,
        color: c,
        fontWeight: 600,
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
      }}
    >
      <span style={{ width: 6, height: 6, borderRadius: "50%", background: c }} />
      {enabled ? "使用中" : "已暂停"}
    </span>
  );
}

function PrefRow({
  title,
  subtitle,
  divider,
  children,
}: {
  title: string;
  subtitle: string;
  divider?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "12px 14px",
        borderBottom: divider ? `1px solid ${color.hairSoft}` : undefined,
      }}
    >
      <div>
        <div style={{ fontWeight: 600, fontSize: 13, color: color.ink }}>{title}</div>
        <div style={{ fontSize: 11, color: color.faint }}>{subtitle}</div>
      </div>
      {children}
    </div>
  );
}
