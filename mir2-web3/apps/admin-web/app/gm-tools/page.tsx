import { AdminShell } from "../../components/admin-shell";
import { MailCommandForm } from "../../components/mail-command-form";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type SystemMailReceipt } from "../../lib/admin-api";

export default async function GmToolsPage() {
  const outbox = await adminGet<SystemMailReceipt[]>("/admin/system-mail/outbox");

  return (
    <AdminShell active="/gm-tools">
      <div className="page-head">
        <div>
          <p className="eyebrow">GM Tools</p>
          <h2>Mail, announcements, and commands</h2>
          <p className="muted">This page is wired to the Rust Admin API for SendSystemMail.</p>
        </div>
        <StatusBadge tone={outbox.ok ? "success" : "warn"}>
          {outbox.ok ? "API ready" : "API offline"}
        </StatusBadge>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">Audited System Mail</p>
          <h3>Queue command</h3>
          <MailCommandForm />
        </section>
        <section className="card">
          <p className="eyebrow">Outbox Receipts</p>
          {outbox.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>Outbox</th>
                  <th>Target</th>
                  <th>Delivery</th>
                  <th>Mail IDs</th>
                </tr>
              </thead>
              <tbody>
                {outbox.data.length ? (
                  outbox.data.map((receipt) => (
                    <tr key={receipt.outboxId}>
                      <td>{receipt.outboxId}</td>
                      <td>
                        {receipt.targetKind} / {receipt.targetId}
                      </td>
                      <td>
                        {receipt.deliveryMode} / {receipt.deliveredCount}
                      </td>
                      <td>{receipt.mailIds.length ? receipt.mailIds.join(", ") : "-"}</td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>No queued mail in this process yet.</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{outbox.error}</p>
          )}
          <div className="rune-divider" />
          <div className="actions">
            <button className="button secondary">Announcement</button>
            <button className="button secondary">Sensitive Words</button>
            <button className="button secondary">Hotfix Notice</button>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
