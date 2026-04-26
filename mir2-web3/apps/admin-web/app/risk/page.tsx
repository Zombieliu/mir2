import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { riskRows } from "../../lib/mock-data";

export default async function RiskPage() {
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/risk">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("risk.eyebrow")}</p>
          <h2>{t("risk.title")}</h2>
          <p className="muted">{t("risk.subtitle")}</p>
        </div>
        <div className="actions">
          <button className="button secondary">{t("risk.batchReview")}</button>
          <button className="button">{t("risk.openQueue")}</button>
        </div>
      </div>
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
              {riskRows.map(([player, signal, risk, evidence]) => (
                <tr key={player}>
                  <td>{player}</td>
                  <td>{signal}</td>
                  <td>
                    <StatusBadge tone={risk === "Critical" ? "danger" : risk === "High" ? "warn" : "default"}>
                      {translateAdminStatus(t, risk)}
                    </StatusBadge>
                  </td>
                  <td>{evidence}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card relationship-map">
          <p className="eyebrow">{t("risk.graph")}</p>
          <div className="node hot" style={{ left: "38%", top: "34%" }}>
            SilentCart
          </div>
          <div className="node" style={{ left: "8%", top: "12%" }}>
            Mule 07
          </div>
          <div className="node" style={{ right: "8%", top: "14%" }}>
            Broker
          </div>
          <div className="node" style={{ left: "14%", bottom: "8%" }}>
            Miner
          </div>
          <div className="node" style={{ right: "16%", bottom: "10%" }}>
            Auction
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
