import { useEffect, useMemo, useState, type ReactNode } from "react";
import { bridge, setAutostart } from "../api/bridge";
import { AddAccountModal } from "../components/AddAccountModal";
import { Kbd } from "../components/Kbd";
import { ProviderBadge } from "../components/ProviderBadge";
import { Toggle } from "../components/Toggle";
import { color, font } from "../theme";
import type { AppSettings, CatalogProvider, Preferences } from "../types";

export function Settings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [catalog, setCatalog] = useState<CatalogProvider[]>([]);
  const [modalOpen, setModalOpen] = useState(false);
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

  const toggleAccount = (id: string, enabled: boolean) =>
    guard(() => bridge.setAccountEnabled(id, enabled));

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
            const meta = byId[account.id];
            if (!meta) return null;
            return (
              <div
                key={account.id}
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
                  <div style={{ fontWeight: 600, fontSize: 13.5, color: color.ink }}>
                    {meta.name}
                  </div>
                  <div style={{ fontSize: 11, color: color.faint }}>
                    {meta.plan} · {meta.company}
                  </div>
                </div>
                <StatusPill enabled={account.enabled} />
                <Toggle
                  on={account.enabled}
                  onToggle={() => toggleAccount(account.id, !account.enabled)}
                  label={`${meta.name} 开关`}
                />
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
          <PrefRow title="菜单栏显示剩余额度" subtitle="在时钟旁显示最低剩余额度" divider>
            <Toggle
              on={settings?.preferences.menubarPercent ?? true}
              onToggle={() => updatePref("menubarPercent", !settings?.preferences.menubarPercent)}
              label="菜单栏显示剩余额度"
            />
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
        connected={settings?.accounts.map((a) => a.id) ?? []}
        onClose={() => setModalOpen(false)}
        onConnected={(next) => {
          setSettings(next);
          setModalOpen(false);
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
