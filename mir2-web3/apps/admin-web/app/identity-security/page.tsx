import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { identityAdminGet } from "../../lib/identity-api";
import {
  revokeAllIdentitySessionsAction,
  revokeIdentitySessionAction,
} from "./actions";

export const dynamic = "force-dynamic";

export default async function IdentitySecurityPage({
  searchParams,
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = (await searchParams) ?? {};
  const accountId = first(params.accountId);
  const success = first(params.success);
  const error = first(params.error);
  const result = accountId ? await identityAdminGet(accountId) : null;
  const data = result?.ok ? result.data : null;

  return (
    <AdminShell active="/identity-security">
      <div className="page-head">
        <div>
          <p className="eyebrow">Commercial Identity</p>
          <h2>账号安全与会话控制</h2>
          <p className="muted">查询账号绑定凭证、登录设备和安全审计；所有撤销动作会同步到 Redis，并在 5 秒内断开在线连接。</p>
        </div>
        <StatusBadge tone={data ? "success" : result ? "warn" : "default"}>
          {data ? data.source : result ? "unavailable" : "ready"}
        </StatusBadge>
      </div>

      <section className="card">
        <form className="form-grid" action="/identity-security">
          <input className="control" name="accountId" defaultValue={accountId} placeholder="账号 ID" minLength={1} maxLength={160} required />
          <button className="button" type="submit">查询账号</button>
        </form>
        {success ? <p className="notice">{success}</p> : null}
        {error ? <p className="notice">{error}</p> : null}
        {result && !result.ok ? <p className="notice">{result.error}</p> : null}
      </section>

      {data ? (
        <div className="grid two">
          <section className="card">
            <div className="page-head compact">
              <div><p className="eyebrow">Sessions</p><h3>登录设备</h3></div>
              <StatusBadge tone="default">{data.sessions.length}</StatusBadge>
            </div>
            <table className="table">
              <thead><tr><th>设备</th><th>方式</th><th>最后活动</th><th>状态</th><th>操作</th></tr></thead>
              <tbody>
                {data.sessions.map((session) => (
                  <tr key={session.sessionId}>
                    <td><strong>{session.userAgentSummary || "未知设备"}</strong><div className="muted">{short(session.sessionId)} · {session.gatewayId}</div></td>
                    <td>{session.authMethod}</td>
                    <td>{date(session.lastSeenAtMs)}</td>
                    <td><StatusBadge tone={session.revokedAtMs ? "danger" : session.expiresAtMs <= Date.now() ? "warn" : "success"}>{session.revokedAtMs ? "已撤销" : session.expiresAtMs <= Date.now() ? "已过期" : "在线凭证"}</StatusBadge></td>
                    <td>{!session.revokedAtMs && session.expiresAtMs > Date.now() ? <form action={revokeIdentitySessionAction}><input type="hidden" name="accountId" value={data.accountId} /><input type="hidden" name="sessionId" value={session.sessionId} /><input type="hidden" name="reason" value="operator_single_session_revoke" /><SubmitButton idle="踢下线" pending="撤销中…" /></form> : "—"}</td>
                  </tr>
                ))}
                {!data.sessions.length ? <tr><td colSpan={5}>暂无会话。</td></tr> : null}
              </tbody>
            </table>
            <form action={revokeAllIdentitySessionsAction} className="form-stack">
              <input type="hidden" name="accountId" value={data.accountId} />
              <input className="control" name="reason" defaultValue="operator_account_security_logout" minLength={4} maxLength={160} required />
              <SubmitButton idle="全端登出" pending="正在撤销…" />
            </form>
          </section>

          <section className="card">
            <div className="page-head compact"><div><p className="eyebrow">Credentials</p><h3>绑定凭证</h3></div><StatusBadge tone="default">{data.credentials.length}</StatusBadge></div>
            <table className="table"><thead><tr><th>类型</th><th>标识</th><th>最近使用</th><th>状态</th></tr></thead><tbody>
              {data.credentials.map((credential) => <tr key={credential.credentialId}><td>{credential.displayName}</td><td><code>{credential.credentialSubject}</code></td><td>{date(credential.lastUsedAtMs)}</td><td><StatusBadge tone={credential.revokedAtMs ? "danger" : "success"}>{credential.revokedAtMs ? "已撤销" : "有效"}</StatusBadge></td></tr>)}
              {!data.credentials.length ? <tr><td colSpan={4}>暂无凭证。</td></tr> : null}
            </tbody></table>
          </section>

          <section className="card span-two">
            <div className="page-head compact"><div><p className="eyebrow">Security Audit</p><h3>身份安全事件</h3></div><StatusBadge tone="default">最近 {data.auditEvents.length} 条</StatusBadge></div>
            <table className="table"><thead><tr><th>时间</th><th>事件</th><th>结果</th><th>原因</th><th>Trace</th></tr></thead><tbody>
              {data.auditEvents.map((event) => <tr key={event.eventId}><td>{date(event.occurredAtMs)}</td><td>{event.eventType}</td><td><StatusBadge tone={event.outcome === "success" ? "success" : event.outcome === "blocked" ? "danger" : "warn"}>{event.outcome}</StatusBadge></td><td>{event.reasonCode || "—"}</td><td><code>{event.traceId}</code></td></tr>)}
              {!data.auditEvents.length ? <tr><td colSpan={5}>暂无安全事件。</td></tr> : null}
            </tbody></table>
          </section>
        </div>
      ) : null}
    </AdminShell>
  );
}

function first(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function short(value: string) {
  return value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-6)}` : value;
}

function date(value?: number | null) {
  return value ? new Date(value).toLocaleString("zh-CN", { hour12: false }) : "—";
}
