import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { activities } from "../../lib/mock-data";

export default async function ActivitiesPage() {
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/activities">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("activities.eyebrow")}</p>
          <h2>{t("activities.title")}</h2>
          <p className="muted">{t("activities.subtitle")}</p>
        </div>
        <button className="button">{t("activities.create")}</button>
      </div>
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
              {activities.map(([name, start, type, status, signal]) => (
                <tr key={name}>
                  <td>{name}</td>
                  <td>{start}</td>
                  <td>{type}</td>
                  <td>
                    <StatusBadge tone={status === "Running" ? "success" : "warn"}>
                      {translateAdminStatus(t, status)}
                    </StatusBadge>
                  </td>
                  <td>{signal}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("activities.preview")}</p>
          <div className="form-grid">
            <div className="field full">
              <label>{t("activities.activityName")}</label>
              <input className="control" defaultValue={t("activities.defaultName")} />
            </div>
            <div className="field">
              <label>{t("activities.reward")}</label>
              <input className="control" defaultValue="red-moon-token x12" />
            </div>
            <div className="field">
              <label>{t("activities.condition")}</label>
              <input className="control" defaultValue="level >= 35" />
            </div>
            <div className="field full">
              <label>{t("activities.realms")}</label>
              <input className="control" defaultValue={t("activities.defaultRealms")} />
            </div>
          </div>
          <div className="rune-divider" />
          <div className="actions">
            <button className="button secondary">{t("activities.saveDraft")}</button>
            <button className="button">{t("activities.submitReview")}</button>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
