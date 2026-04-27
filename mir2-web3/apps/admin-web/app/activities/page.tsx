import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminActivitiesReadModel } from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function ActivitiesPage() {
  const { t } = await getAdminI18n();
  const activities = await adminGet<AdminActivitiesReadModel>("/admin/read/activities");
  const data = activities.ok ? activities.data : undefined;

  return (
    <AdminShell active="/activities">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("activities.eyebrow")}</p>
          <h2>{t("activities.title")}</h2>
          <p className="muted">{t("activities.subtitle")}</p>
        </div>
        <StatusBadge tone={data?.configured ? "success" : "warn"}>
          {data?.configured ? t("common.connected") : t("common.unconfigured")}
        </StatusBadge>
      </div>
      {!activities.ok ? <p className="notice">{activities.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("activities.list")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("activities.name")}</th>
                <th>{t("activities.start")}</th>
                <th>{t("activities.type")}</th>
                <th>{t("table.status")}</th>
                <th>{t("activities.signal")}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.activities ?? []).map((activity) => (
                <tr key={activity.activityId}>
                  <td>{activity.name}</td>
                  <td>{activity.startAtMs ?? "-"}</td>
                  <td>{activity.activityType}</td>
                  <td>
                    <StatusBadge tone={activity.status === "Running" ? "success" : "warn"}>
                      {translateAdminStatus(t, activity.status)}
                    </StatusBadge>
                  </td>
                  <td>{activity.signal}</td>
                </tr>
              ))}
              {!data?.activities.length ? (
                <tr>
                  <td colSpan={5}>
                    <p className="notice">{t("activities.empty")}</p>
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("activities.source")}</p>
          <h3>{data?.source ?? t("common.unavailable")}</h3>
          <p className="notice">{t("activities.noConfigStore")}</p>
        </section>
      </div>
    </AdminShell>
  );
}
