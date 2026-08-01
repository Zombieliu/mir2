"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { requestSuiLoginToken, type SuiLoginKind } from "../../lib/client-login-runtime";

export type BrowserIdentitySession = {
  sessionId: string;
  accountId: string;
  authMethod: string;
  issuedAtMs: number;
  lastSeenAtMs: number;
  expiresAtMs: number;
  revokedAtMs?: number | null;
  revokedReason?: string | null;
  userAgentSummary: string;
  gatewayId: string;
  current: boolean;
};

type Credential = {
  credentialId: string;
  credentialKind: string;
  credentialSubject: string;
  displayName: string;
  createdAtMs: number;
  lastUsedAtMs?: number | null;
  revokedAtMs?: number | null;
};

type Overview = {
  accountId: string;
  currentSessionId: string;
  sessions: BrowserIdentitySession[];
  credentials: Credential[];
};

type Props = {
  token: string | null;
  accountId: string;
  language: string;
  onCurrentSessionRevoked: () => void;
};

export function IdentitySecurityPanel({ token, accountId, language, onCurrentSessionRevoked }: Props) {
  const zh = language.toLowerCase().startsWith("zh");
  const [open, setOpen] = useState(false);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [codes, setCodes] = useState<string[] | null>(null);
  const [recoverAccount, setRecoverAccount] = useState(accountId);
  const [recoveryCode, setRecoveryCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const onCurrentSessionRevokedRef = useRef(onCurrentSessionRevoked);
  useEffect(() => {
    onCurrentSessionRevokedRef.current = onCurrentSessionRevoked;
  }, [onCurrentSessionRevoked]);

  const call = useCallback(async (path: string, init?: RequestInit) => {
    const response = await fetch(`/api/identity/${path}`, {
      ...init,
      headers: {
        ...(init?.body ? { "content-type": "application/json" } : {}),
        ...(token ? { authorization: `Bearer ${token}` } : {}),
      },
      cache: "no-store",
    });
    const payload = (await response.json().catch(() => null)) as Record<string, unknown> | null;
    if (!response.ok) {
      if (response.status === 401 && token) {
        setOverview(null);
        setCodes(null);
        setOpen(false);
        onCurrentSessionRevokedRef.current();
      }
      throw new Error(typeof payload?.error === "string" ? payload.error : `HTTP ${response.status}`);
    }
    return payload;
  }, [token]);

  const refresh = useCallback(async () => {
    if (!token) return;
    setBusy("refresh");
    setError(null);
    try {
      setOverview((await call("me")) as Overview);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  }, [call, token]);

  useEffect(() => {
    if (open && token) void refresh();
  }, [open, refresh, token]);

  async function mutate(key: string, path: string, body?: unknown) {
    setBusy(key);
    setError(null);
    try {
      await call(path, { method: "POST", body: body === undefined ? "{}" : JSON.stringify(body) });
      if (key === "current-session") {
        onCurrentSessionRevoked();
        setOpen(false);
      } else {
        await refresh();
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  }

  async function rotateCodes() {
    setBusy("codes");
    setError(null);
    try {
      const payload = await call("recovery-codes/rotate", { method: "POST", body: "{}" });
      setCodes(Array.isArray(payload?.recoveryCodes) ? payload.recoveryCodes.map(String) : []);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  }

  async function bindSuiCredential(kind: SuiLoginKind) {
    setBusy(`bind-${kind}`);
    setError(null);
    try {
      const proof = await requestSuiLoginToken(kind);
      const address = proof.accountId.startsWith("sui:") ? proof.accountId.slice(4) : proof.accountId;
      await call("credentials/bind-sui", {
        method: "POST",
        body: JSON.stringify({ address, proofToken: proof.token }),
      });
      await refresh();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  }

  async function recover() {
    setBusy("recover");
    setError(null);
    try {
      await call("recover", { method: "POST", body: JSON.stringify({ accountId: recoverAccount, recoveryCode, newPassword }) });
      setRecoveryCode("");
      setNewPassword("");
      setError(zh ? "密码已重置，请使用新密码登录。" : "Password reset. Sign in with the new password.");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(null);
    }
  }

  return (
    <>
      <button type="button" className="identity-security-trigger" onClick={() => setOpen(true)}>
        {token ? (zh ? "账号安全" : "Account security") : (zh ? "恢复账号" : "Recover account")}
      </button>
      {open ? (
        <div className="identity-security-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setOpen(false); }}>
          <section className="identity-security-dialog" role="dialog" aria-modal="true" aria-label={zh ? "账号安全中心" : "Account security center"}>
            <header><div><small>DUBHE IDENTITY</small><h2>{zh ? "账号安全中心" : "Account security center"}</h2></div><button type="button" onClick={() => setOpen(false)} aria-label="Close">×</button></header>
            {error ? <p className="identity-security-message" role="status">{error}</p> : null}
            {!token ? (
              <div className="identity-security-form">
                <label>{zh ? "账号" : "Account"}<input value={recoverAccount} onChange={(event) => setRecoverAccount(event.target.value)} autoComplete="username" /></label>
                <label>{zh ? "恢复码" : "Recovery code"}<input value={recoveryCode} onChange={(event) => setRecoveryCode(event.target.value)} autoComplete="one-time-code" /></label>
                <label>{zh ? "新密码" : "New password"}<input type="password" value={newPassword} onChange={(event) => setNewPassword(event.target.value)} autoComplete="new-password" /></label>
                <button type="button" disabled={busy !== null} onClick={() => void recover()}>{busy === "recover" ? (zh ? "处理中…" : "Recovering…") : (zh ? "使用恢复码重置" : "Reset with recovery code")}</button>
              </div>
            ) : (
              <>
                <div className="identity-security-actions"><button type="button" disabled={busy !== null} onClick={() => void refresh()}>{zh ? "刷新" : "Refresh"}</button><button type="button" disabled={busy !== null} onClick={() => void mutate("others", "sessions/revoke-others")}>{zh ? "退出其他设备" : "Sign out other devices"}</button><button type="button" disabled={busy !== null} onClick={() => void rotateCodes()}>{zh ? "生成新恢复码" : "Rotate recovery codes"}</button><button type="button" disabled={busy !== null} onClick={() => void bindSuiCredential("passkey")}>{busy === "bind-passkey" ? (zh ? "绑定中…" : "Binding…") : (zh ? "绑定 Passkey" : "Bind Passkey")}</button><button type="button" disabled={busy !== null} onClick={() => void bindSuiCredential("wallet")}>{busy === "bind-wallet" ? (zh ? "连接中…" : "Connecting…") : (zh ? "绑定 Sui 钱包" : "Bind Sui wallet")}</button></div>
                <h3>{zh ? "登录设备" : "Sessions"}</h3>
                <div className="identity-security-list">{overview?.sessions.map((session) => <article key={session.sessionId} className={session.revokedAtMs ? "revoked" : ""}><div><strong>{session.current ? (zh ? "当前设备" : "Current device") : session.userAgentSummary || (zh ? "未知设备" : "Unknown device")}</strong><span>{session.authMethod} · {session.gatewayId}</span><span>{new Date(session.lastSeenAtMs).toLocaleString()}</span></div>{!session.revokedAtMs ? <button type="button" disabled={busy !== null} onClick={() => void mutate(session.current ? "current-session" : session.sessionId, "sessions/revoke", { sessionId: session.sessionId, reason: "player_security_center" })}>{zh ? "撤销" : "Revoke"}</button> : <em>{zh ? "已撤销" : "Revoked"}</em>}</article>) ?? (busy ? <p>{zh ? "加载中…" : "Loading…"}</p> : null)}</div>
                <h3>{zh ? "登录凭证" : "Credentials"}</h3>
                <div className="identity-security-list">{overview?.credentials.map((credential) => <article key={credential.credentialId} className={credential.revokedAtMs ? "revoked" : ""}><div><strong>{credential.displayName}</strong><span>{credential.credentialSubject}</span></div>{credential.revokedAtMs ? <em>{zh ? "已撤销" : "Revoked"}</em> : <button type="button" disabled={busy !== null} onClick={() => void mutate(credential.credentialId, "credentials/revoke", { credentialId: credential.credentialId, reason: "player_security_center" })}>{zh ? "解绑" : "Unbind"}</button>}</article>)}</div>
                {codes ? <div className="identity-recovery-codes"><strong>{zh ? "仅显示一次，请离线保存" : "Shown once — store offline"}</strong><code>{codes.join("\n")}</code><button type="button" onClick={() => navigator.clipboard.writeText(codes.join("\n"))}>{zh ? "复制" : "Copy"}</button></div> : null}
              </>
            )}
          </section>
        </div>
      ) : null}
    </>
  );
}
