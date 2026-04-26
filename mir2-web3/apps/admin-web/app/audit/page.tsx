import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminCommandRecord, type AuditRecord } from "../../lib/admin-api";

export default async function AuditPage() {
  const audit = await adminGet<AuditRecord[]>("/admin/audit");
  const commands = await adminGet<AdminCommandRecord[]>("/admin/commands");

  return (
    <AdminShell active="/audit">
      <div className="page-head">
        <div>
          <p className="eyebrow">Audit</p>
          <h2>Command and audit ledger</h2>
          <p className="muted">Append-only operational evidence from the Rust control plane.</p>
        </div>
        <StatusBadge tone={audit.ok && commands.ok ? "success" : "warn"}>
          {audit.ok && commands.ok ? "Ledger connected" : "API unavailable"}
        </StatusBadge>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Audit Records</p>
          {audit.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>Audit</th>
                  <th>Operator</th>
                  <th>Permission</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {audit.data.length ? (
                  audit.data.map((record) => (
                    <tr key={record.auditId}>
                      <td>{record.auditId}</td>
                      <td>{record.operatorEmail}</td>
                      <td>{record.permission}</td>
                      <td>
                        <StatusBadge tone={record.status === "succeeded" ? "success" : "warn"}>
                          {record.status}
                        </StatusBadge>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>No audit records in this process yet.</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{audit.error}</p>
          )}
        </section>
        <section className="card">
          <p className="eyebrow">Command Records</p>
          {commands.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>Command</th>
                  <th>Target</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                {commands.data.length ? (
                  commands.data.map((record) => (
                    <tr key={record.envelope.commandId}>
                      <td>{record.envelope.commandId}</td>
                      <td>{record.envelope.target.targetId}</td>
                      <td>
                        <StatusBadge tone={record.status === "succeeded" ? "success" : "warn"}>
                          {record.status}
                        </StatusBadge>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={3}>No command records in this process yet.</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{commands.error}</p>
          )}
        </section>
      </div>
    </AdminShell>
  );
}
