import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import {
  adminGet,
  type AdminCommandRecord,
  type AdminEventsResponse,
  type AuditRecord
} from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function AuditPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const eventQuery = buildEventQuery(params);
  const audit = await adminGet<AuditRecord[]>("/admin/audit");
  const commands = await adminGet<AdminCommandRecord[]>("/admin/commands");
  const events = await adminGet<AdminEventsResponse>(`/admin/events${eventQuery.apiSuffix}`);
  const eventsAvailable = events.ok && !events.data.degraded;
  const eventRecords = events.ok ? events.data.records : [];

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
      <section className="card">
        <div className="section-head">
          <p className="eyebrow">{t("audit.events")}</p>
          <StatusBadge tone={eventsAvailable ? "success" : "warn"}>
            {eventsAvailable ? t("audit.eventsConnected") : t("audit.eventsDegraded")}
          </StatusBadge>
        </div>
        <form className="form-grid" action="/audit">
          <input
            className="control"
            defaultValue={eventQuery.commandId}
            name="commandId"
            placeholder={t("audit.command")}
          />
          <input
            className="control"
            defaultValue={eventQuery.eventType}
            name="eventType"
            placeholder={t("audit.event")}
          />
          <select className="control" defaultValue={eventQuery.status} name="status">
            <option value="">{t("audit.anyStatus")}</option>
            <option value="succeeded">{translateAdminStatus(t, "succeeded")}</option>
            <option value="failed">{translateAdminStatus(t, "failed")}</option>
            <option value="denied">{translateAdminStatus(t, "denied")}</option>
            <option value="dead_letter">{translateAdminStatus(t, "dead_letter")}</option>
          </select>
          <button className="button" type="submit">
            {t("audit.applyFilters")}
          </button>
        </form>
        {events.ok && events.data.degraded ? <p className="notice">{events.data.error}</p> : null}
        {!events.ok ? <p className="notice">{events.error}</p> : null}
        {events.ok ? (
          <>
            <table className="table">
              <thead>
                <tr>
                  <th>{t("audit.event")}</th>
                  <th>{t("audit.command")}</th>
                  <th>{t("audit.operator")}</th>
                  <th>{t("table.status")}</th>
                </tr>
              </thead>
              <tbody>
                {eventRecords.length ? (
                  eventRecords.map((record, index) => (
                    <tr key={eventRecordKey(record, index)}>
                      <td>{record.eventType}</td>
                      <td>{record.commandId}</td>
                      <td>{record.operatorId}</td>
                      <td>
                        <StatusBadge tone={record.status === "succeeded" ? "success" : "warn"}>
                          {translateAdminStatus(t, record.status)}
                        </StatusBadge>
                      </td>
                    </tr>
                  ))
                ) : (
                  <tr>
                    <td colSpan={4}>{t("audit.emptyEvents")}</td>
                  </tr>
                )}
              </tbody>
            </table>
          </>
        ) : (
          <p className="notice">{t("audit.emptyEvents")}</p>
        )}
      </section>
    </AdminShell>
  );
}

function buildEventQuery(params: Record<string, string | string[] | undefined>) {
  const commandId = firstParam(params.commandId);
  const eventType = firstParam(params.eventType);
  const status = firstParam(params.status);
  const apiParams = new URLSearchParams();
  if (commandId) {
    apiParams.set("commandId", commandId);
  }
  if (eventType) {
    apiParams.set("eventType", eventType);
  }
  if (status) {
    apiParams.set("status", status);
  }
  apiParams.set("limit", "100");
  return {
    apiSuffix: `?${apiParams.toString()}`,
    commandId,
    eventType,
    status
  };
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function eventRecordKey(
  record: { eventId: string; eventType: string; commandId: string; occurredAtMs: number },
  index: number
) {
  return `${record.eventId}:${record.eventType}:${record.commandId}:${record.occurredAtMs}:${index}`;
}
