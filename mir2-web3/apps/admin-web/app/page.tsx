import { AdminShell } from "../components/admin-shell";
import { Bars } from "../components/bars";
import { MetricCard } from "../components/metric-card";
import { StatusBadge } from "../components/status-badge";
import { adminGet, type AuditRecord, type SystemMailReceipt } from "../lib/admin-api";
import { hotMaps, metrics, servers } from "../lib/mock-data";

export default async function DashboardPage() {
  const audit = await adminGet<AuditRecord[]>("/admin/audit");
  const outbox = await adminGet<SystemMailReceipt[]>("/admin/system-mail/outbox");

  return (
    <AdminShell active="/">
      <div className="page-head">
        <div>
          <p className="eyebrow">Global Operations</p>
          <h2>Realm command center</h2>
          <p className="muted">
            Real-time operational posture across gameplay, economy, risk, and GM workflow.
          </p>
        </div>
        <StatusBadge tone={audit.ok ? "success" : "warn"}>
          {audit.ok ? "Admin API connected" : "Admin API offline"}
        </StatusBadge>
      </div>

      <div className="grid metrics">
        {metrics.map((metric) => (
          <MetricCard key={metric.title} {...metric} />
        ))}
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">Population Heat</p>
          <h3>Hot maps and dungeons</h3>
          <Bars rows={hotMaps} />
        </section>
        <section className="card">
          <p className="eyebrow">World Boss</p>
          <h3>Ancient Wooma Lord</h3>
          <p className="metric-value">17:42</p>
          <p className="muted">Next spawn window. S2 Jade Ruins is predicted to overflow.</p>
          <div className="rune-divider" />
          <div className="actions">
            <StatusBadge tone="warn">High attention</StatusBadge>
            <StatusBadge>Instance split ready</StatusBadge>
          </div>
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">Server Status</p>
          <table className="table">
            <thead>
              <tr>
                <th>Realm</th>
                <th>Online</th>
                <th>Latency</th>
                <th>Status</th>
              </tr>
            </thead>
            <tbody>
              {servers.map(([realm, online, latency, status]) => (
                <tr key={realm}>
                  <td>{realm}</td>
                  <td>{online}</td>
                  <td>{latency}</td>
                  <td>
                    <StatusBadge tone={status === "Warn" ? "warn" : "success"}>
                      {status}
                    </StatusBadge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">Command Evidence</p>
          <h3>Audit and outbox</h3>
          <p className="metric-value">
            {audit.ok ? audit.data.length : 0} / {outbox.ok ? outbox.data.length : 0}
          </p>
          <p className="muted">Recent audit records / queued system mail receipts.</p>
          {!audit.ok ? <p className="notice">{audit.error}</p> : null}
        </section>
      </div>
    </AdminShell>
  );
}
