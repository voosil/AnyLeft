import { useMemo, useState } from "react";
import { bridge, openExternal } from "../api/bridge";
import { ProviderBadge } from "./ProviderBadge";
import { color, font } from "../theme";
import type { AppSettings, AuthMethod, CatalogProvider } from "../types";

/** Best-effort login pages for the browser-login flow. */
const LOGIN_URLS: Record<string, string> = {
  claude: "https://claude.ai/login",
  gpt: "https://chatgpt.com/",
  glm: "https://open.bigmodel.cn/",
  kimi: "https://kimi.moonshot.cn/",
  minimax: "https://platform.minimaxi.com/",
  gemini: "https://gemini.google.com/",
  grok: "https://grok.com/",
  cursor: "https://www.cursor.com/",
  deepseek: "https://platform.deepseek.com/",
};

interface AddAccountModalProps {
  open: boolean;
  catalog: CatalogProvider[];
  connected: string[];
  mode?: "add" | "configure";
  initialProviderId?: string;
  onClose: () => void;
  onConnected: (settings: AppSettings) => void;
}

export function AddAccountModal({
  open,
  catalog,
  connected,
  mode = "add",
  initialProviderId,
  onClose,
  onConnected,
}: AddAccountModalProps) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [auth, setAuth] = useState<AuthMethod>("key");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const addable = useMemo(
    () => catalog.filter((p) => !connected.includes(p.id)),
    [catalog, connected],
  );
  const activeId = initialProviderId ?? selectedId;
  const selected = activeId ? catalog.find((p) => p.id === activeId) ?? null : null;
  const configuring = mode === "configure";

  if (!open) return null;

  const reset = () => {
    setSelectedId(null);
    setAuth("key");
    setApiKey("");
    setError(null);
    setBusy(false);
  };
  const handleClose = () => {
    reset();
    onClose();
  };
  const pick = (id: string) => {
    setSelectedId(id);
    setAuth("key");
    setApiKey("");
    setError(null);
  };

  const connect = async () => {
    if (!selected) return;
    setBusy(true);
    setError(null);
    try {
      const nextAuth = selected.id === "minimax" ? "key" : auth;
      const next = await bridge.connectAccount(
        selected.id,
        nextAuth,
        nextAuth === "key" ? apiKey : undefined,
      );
      onConnected(next);
      reset();
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  };

  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        background: "rgba(46,42,34,.34)",
        backdropFilter: "blur(3px)",
        WebkitBackdropFilter: "blur(3px)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 26,
        zIndex: 20,
        animation: "fadeIn .18s both",
      }}
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) handleClose();
      }}
    >
      <div
        style={{
          width: 430,
          background: color.card,
          border: "1px solid rgba(255,255,255,.7)",
          borderRadius: 14,
          boxShadow: "0 26px 56px -18px rgba(30,20,6,.6)",
          overflow: "hidden",
          animation: "popIn .22s cubic-bezier(.2,.85,.25,1) both",
        }}
      >
        {/* modal header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "14px 16px",
            borderBottom: `1px solid ${color.hair}`,
          }}
        >
          <span style={{ fontWeight: 700, fontSize: 15, color: color.inkStrong }}>
            {configuring && selected ? `配置 ${selected.name}` : "添加账户"}
          </span>
          <span
            onClick={handleClose}
            style={{
              width: 24,
              height: 24,
              borderRadius: 7,
              display: "grid",
              placeItems: "center",
              cursor: "pointer",
              color: color.muted,
              background: color.chip,
              fontSize: 15,
            }}
          >
            ✕
          </span>
        </div>

        {selected ? (
          <ConnectForm
            provider={selected}
            auth={auth}
            apiKey={apiKey}
            busy={busy}
            error={error}
            mode={mode}
            onBack={configuring ? undefined : () => setSelectedId(null)}
            onAuth={setAuth}
            onKey={setApiKey}
            onCancel={handleClose}
            onConnect={connect}
          />
        ) : (
          <PickProvider addable={addable} onPick={pick} />
        )}
      </div>
    </div>
  );
}

function PickProvider({
  addable,
  onPick,
}: {
  addable: CatalogProvider[];
  onPick: (id: string) => void;
}) {
  return (
    <div style={{ padding: "16px 18px 18px" }}>
      <div style={{ fontSize: 12.5, color: color.muted, marginBottom: 13 }}>
        选择要连接的服务商
      </div>
      {addable.length === 0 ? (
        <div style={{ textAlign: "center", padding: "28px 10px", color: color.faint, fontSize: 13 }}>
          全部服务商都已连接 🎉
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 9 }}>
          {addable.map((p) => (
            <button
              key={p.id}
              onClick={() => onPick(p.id)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "11px 12px",
                border: `1px solid ${color.hairStrong}`,
                borderRadius: 11,
                background: color.inner,
                cursor: "pointer",
                textAlign: "left",
              }}
            >
              <ProviderBadge mono={p.mono} accent={p.accent} tint={p.tint} />
              <span style={{ minWidth: 0 }}>
                <span
                  style={{
                    display: "block",
                    fontWeight: 600,
                    fontSize: 13,
                    color: color.ink,
                    lineHeight: 1.2,
                  }}
                >
                  {p.name}
                </span>
                <span style={{ display: "block", fontSize: 10.5, color: color.faint }}>
                  {p.company}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

interface ConnectFormProps {
  provider: CatalogProvider;
  auth: AuthMethod;
  apiKey: string;
  busy: boolean;
  error: string | null;
  mode: "add" | "configure";
  onBack?: () => void;
  onAuth: (a: AuthMethod) => void;
  onKey: (k: string) => void;
  onCancel: () => void;
  onConnect: () => void;
}

function ConnectForm({
  provider,
  auth,
  apiKey,
  busy,
  error,
  mode,
  onBack,
  onAuth,
  onKey,
  onCancel,
  onConnect,
}: ConnectFormProps) {
  const keyOnly = provider.id === "minimax";
  const effectiveAuth = keyOnly ? "key" : auth;
  const configuring = mode === "configure";
  const segStyle = (active: boolean) => ({
    flex: 1,
    border: "none",
    cursor: "pointer",
    fontSize: 12.5,
    fontWeight: 600,
    padding: 8,
    borderRadius: 8,
    color: active ? color.ink : color.muted,
    background: active ? color.card : "transparent",
    boxShadow: active ? "0 1px 3px rgba(0,0,0,.12)" : "none",
  });

  return (
    <div style={{ padding: "14px 18px 18px" }}>
      {onBack && (
        <button
          onClick={onBack}
          style={{
            border: "none",
            background: "none",
            padding: 0,
            marginBottom: 14,
            cursor: "pointer",
            color: color.brown,
            fontSize: 12,
            fontWeight: 600,
            display: "inline-flex",
            alignItems: "center",
            gap: 5,
          }}
        >
          ‹ 返回选择
        </button>
      )}

      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "12px 13px",
          background: color.inner,
          border: `1px solid ${color.hair}`,
          borderRadius: 11,
          marginBottom: 18,
        }}
      >
        <ProviderBadge mono={provider.mono} accent={provider.accent} tint={provider.tint} size={38} />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontWeight: 600, fontSize: 14, color: color.ink }}>{provider.name}</div>
          <div style={{ fontSize: 11, color: color.faint }}>
            {provider.plan} · {provider.company}
          </div>
        </div>
      </div>

      {!keyOnly && (
        <>
          <div style={{ fontSize: 12, fontWeight: 600, color: color.inkSoft, marginBottom: 8 }}>
            授权方式
          </div>
          <div
            style={{
              display: "flex",
              gap: 6,
              background: color.chip,
              padding: 3,
              borderRadius: 10,
              marginBottom: 16,
            }}
          >
            <button onClick={() => onAuth("key")} style={segStyle(auth === "key")}>
              API Key
            </button>
            <button onClick={() => onAuth("login")} style={segStyle(auth === "login")}>
              浏览器登录
            </button>
          </div>
        </>
      )}

      {effectiveAuth === "key" ? (
        <>
          <div style={{ marginBottom: 6, fontSize: 12, color: color.muted }}>
            {provider.id === "minimax" ? "粘贴 MiniMax token" : "粘贴 API Key"}
          </div>
          <input
            type="text"
            value={apiKey}
            onChange={(e) => onKey(e.target.value)}
            placeholder={provider.id === "minimax" ? "MiniMax token" : "sk-..."}
            spellCheck={false}
            autoComplete="off"
            style={{
              width: "100%",
              padding: "11px 13px",
              border: `1px solid rgba(51,48,42,.16)`,
              borderRadius: 10,
              background: color.inner,
              fontSize: 13,
              color: color.ink,
              outline: "none",
              fontFamily: font.mono,
            }}
          />
          <div style={{ fontSize: 11, color: color.faint, marginTop: 8 }}>
            {provider.id === "minimax"
              ? "Token 仅保存在本机钥匙串，用于读取 MiniMax Token Plan 用量。"
              : "密钥仅保存在本机钥匙串，用于读取用量。"}
          </div>
        </>
      ) : (
        <div
          style={{
            padding: "16px 14px",
            textAlign: "center",
            border: `1px dashed rgba(51,48,42,.2)`,
            borderRadius: 10,
            background: color.inner,
          }}
        >
          <div style={{ fontSize: 12.5, color: color.inkSoft, marginBottom: 10 }}>
            在浏览器中登录 {provider.name} 以授权读取用量
          </div>
          <span
            onClick={() => void openExternal(LOGIN_URLS[provider.id] ?? `https://${provider.company}`)}
            style={{
              display: "inline-block",
              fontSize: 12.5,
              fontWeight: 600,
              color: "#fff",
              background: provider.accent,
              borderRadius: 8,
              padding: "8px 16px",
              cursor: "pointer",
            }}
          >
            打开登录页 ↗
          </span>
        </div>
      )}

      {error && (
        <div style={{ fontSize: 11.5, color: color.accent, marginTop: 10 }}>{error}</div>
      )}

      <div style={{ display: "flex", gap: 10, marginTop: 20 }}>
        <button
          onClick={onCancel}
          style={{
            flex: 1,
            border: `1px solid rgba(51,48,42,.16)`,
            background: color.card,
            cursor: "pointer",
            fontSize: 13,
            fontWeight: 600,
            color: color.inkSoft,
            padding: 11,
            borderRadius: 10,
          }}
        >
          取消
        </button>
        <button
          onClick={onConnect}
          disabled={busy}
          style={{
            flex: 2,
            border: "none",
            cursor: busy ? "default" : "pointer",
            fontSize: 13,
            fontWeight: 600,
            color: "#fff",
            background: provider.accent,
            padding: 11,
            borderRadius: 10,
            opacity: busy ? 0.7 : 1,
          }}
        >
          {busy ? (configuring ? "保存中…" : "连接中…") : configuring ? "保存 token" : "连接账户"}
        </button>
      </div>
    </div>
  );
}
