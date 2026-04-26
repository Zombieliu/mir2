import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminCommandRecord, type AuditRecord } from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function AuditPage() {
  const { t } = await getAdminI18n();
  const audit = await adminGet<AuditRecord[]>("/admin/audit");
  const commands = await adminGet<AdminCommandRecord[]>("/admin/commands");

  return (
    <AdminShell active="/audit">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("audit.eyebrow")}</p>
          <h2>{t("audit.title")}</h2>
          <p className="muted">{t("audit.subtitle")}</p>
        </div>
        <StatusBadge tone={audit.ok && commands.ok ? "success" : "warn"}>
          {audit.ok && commands.ok ? t("audit.connected") : t("common.unavailable")}
        </StatusBadge>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("audit.records")}</p>
          {audit.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("audit.audit")}</th>
                  <th>{t("audit.operator")}</th>
                  <th>{t("audit.permission")}</th>
                  <th>{t("table.status")}</th>
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
                          {translateAdminStatus(t, record.status)}
                        </StatusBadge>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>{t("audit.emptyAudit")}</td>
                  </tr>
                )}
              </tbody>
            </table>
          ) : (
            <p className="notice">{audit.error}</p>
          )}
        </section>
        <section className="card">
          <p className="eyebrow">{t("audit.commands")}</p>
          {commands.ok ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("audit.command")}</th>
                  <th>{t("gm.target")}</th>
                  <th>{t("table.status")}</th>
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
                          {translateAdminStatus(t, record.status)}
                        </StatusBadge>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={3}>{t("audit.emptyCommands")}</td>
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
