import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminRiskReadModel } from "../../lib/admin-api";
import { statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function RiskPage() {
  const { t } = await getAdminI18n();
  const risk = await adminGet<AdminRiskReadModel>("/admin/read/risk");
  const data = risk.ok ? risk.data : undefined;

  return (
    <AdminShell active="/risk">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("risk.eyebrow")}</p>
          <h2>{t("risk.title")}</h2>
          <p className="muted">{t("risk.subtitle")}</p>
        </div>
        <StatusBadge tone={risk.ok ? "success" : "warn"}>
          {risk.ok ? risk.data.source : t("common.unavailable")}
        </StatusBadge>
      </div>
      {!risk.ok ? <p className="notice">{risk.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("risk.caseQueue")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("risk.player")}</th>
                <th>{t("risk.signal")}</th>
                <th>{t("risk.risk")}</th>
                <th>{t("risk.evidence")}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.cases ?? []).map((item) => (
                <tr key={item.playerId}>
                  <td>{item.characterName}</td>
                  <td>{item.signal}</td>
                  <td>
                    <StatusBadge tone={statusTone(item.risk)}>
                      {translateAdminStatus(t, item.risk)}
                    </StatusBadge>
                  </td>
                  <td>{item.evidence}</td>
                </tr>
              ))}
              {!data?.cases.length ? (
                <tr>
                  <td colSpan={4}>
                    <p className="notice">{t("risk.empty")}</p>
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card relationship-map">
          <p className="eyebrow">{t("risk.graph")}</p>
          <p className="notice">{t("risk.noGraph")}</p>
        </section>
      </div>
    </AdminShell>
  );
}
