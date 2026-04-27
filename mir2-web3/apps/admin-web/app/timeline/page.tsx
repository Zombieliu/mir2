import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminTimelineResponse } from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function TimelinePage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const query = buildTimelineQuery(params);
  const timeline = await adminGet<AdminTimelineResponse>(
    `/admin/timeline${query.apiSuffix}`
  );
  const records = timeline.ok ? timeline.data.records : [];
  const available = timeline.ok && !timeline.data.degraded;

  return (
    <AdminShell active="/timeline">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("timeline.eyebrow")}</p>
          <h2>{t("timeline.title")}</h2>
          <p className="muted">{t("timeline.subtitle")}</p>
        </div>
        <StatusBadge tone={available ? "success" : "warn"}>
          {available ? t("timeline.connected") : t("timeline.degraded")}
        </StatusBadge>
      </div>
      <section className="card">
        <form className="form-grid" action="/timeline">
          <input
            className="control"
            defaultValue={query.commandId}
            name="commandId"
            placeholder={t("audit.command")}
          />
          <input
            className="control"
            defaultValue={query.targetId}
            name="targetId"
            placeholder={t("gm.target")}
          />
          <button className="button" type="submit">
            {t("audit.applyFilters")}
          </button>
        </form>
        {timeline.ok && timeline.data.degraded ? (
          <p className="notice">{timeline.data.error}</p>
        ) : null}
        {!timeline.ok ? <p className="notice">{timeline.error}</p> : null}
        <table className="table">
          <thead>
            <tr>
              <th>{t("timeline.source")}</th>
              <th>{t("audit.command")}</th>
              <th>{t("gm.target")}</th>
              <th>{t("audit.event")}</th>
              <th>{t("table.status")}</th>
              <th>{t("timeline.actor")}</th>
              <th>{t("timeline.summary")}</th>
            </tr>
          </thead>
          <tbody>
            {records.length ? (
              records.map((record) => (
                <tr key={`${record.source}-${record.recordId}`}>
                  <td>{record.source}</td>
                  <td>{record.commandId ?? "-"}</td>
                  <td>{record.targetId ?? "-"}</td>
                  <td>{record.eventType}</td>
                  <td>
                    <StatusBadge tone={record.status === "succeeded" ? "success" : "warn"}>
                      {translateAdminStatus(t, record.status)}
                    </StatusBadge>
                  </td>
                  <td>{record.actorId ?? "-"}</td>
                  <td>{record.summary}</td>
                </tr>
              ))
            ) : (
              <tr>
                <td colSpan={7}>{t("timeline.empty")}</td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}

function buildTimelineQuery(params: Record<string, string | string[] | undefined>) {
  const commandId = firstParam(params.commandId);
  const targetId = firstParam(params.targetId);
  const apiParams = new URLSearchParams();
  if (commandId) {
    apiParams.set("commandId", commandId);
  }
  if (targetId) {
    apiParams.set("targetId", targetId);
  }
  apiParams.set("limit", "100");
  return {
    apiSuffix: `?${apiParams.toString()}`,
    commandId,
    targetId
  };
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}
