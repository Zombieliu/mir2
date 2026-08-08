import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminActivitiesReadModel } from "../../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { upsertActivityAction } from "./actions";

export default async function ActivitiesPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
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
      {error ? <p className="notice">{error}</p> : null}
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
                  <td>{activity.signal || activity.reward || "-"}</td>
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
          <p className="notice">
            {data?.configured ? `${data.activities.length} ${t("activities.list")}` : t("activities.noConfigStore")}
          </p>
          <div className="rune-divider" />
          <form action={upsertActivityAction} className="form-stack">
            <div className="form-grid">
              <label className="field">
                <span>{t("activities.activityId")}</span>
                <input className="control" name="activityId" placeholder="activity-r250" />
              </label>
              <label className="field">
                <span>{t("activities.activityName")}</span>
                <input className="control" name="name" required />
              </label>
              <label className="field">
                <span>{t("activities.type")}</span>
                <input className="control" name="activityType" defaultValue="seasonal" required />
              </label>
              <label className="field">
                <span>{t("table.status")}</span>
                <select className="control" name="status" defaultValue="Draft">
                  <option>Draft</option>
                  <option>Scheduled</option>
                  <option>Running</option>
                  <option>Paused</option>
                </select>
              </label>
              <label className="field">
                <span>{t("activities.start")}</span>
                <input className="control" name="startAtMs" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("activities.signal")}</span>
                <input className="control" name="signal" defaultValue="operator_configured" />
              </label>
              <label className="field">
                <span>{t("activities.reward")}</span>
                <input className="control" name="reward" />
              </label>
              <label className="field">
                <span>{t("activities.realms")}</span>
                <input className="control" name="realms" placeholder="s1,s2" />
              </label>
              <label className="field full">
                <span>{t("activities.condition")}</span>
                <textarea className="control" name="condition" />
              </label>
            </div>
            <SubmitButton idle={t("common.submit")} pending={t("common.submitting")} />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}
