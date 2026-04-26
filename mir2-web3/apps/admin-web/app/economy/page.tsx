import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { economyRows } from "../../lib/mock-data";

export default async function EconomyPage() {
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/economy">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("economy.eyebrow")}</p>
          <h2>{t("economy.title")}</h2>
          <p className="muted">{t("economy.subtitle")}</p>
        </div>
        <StatusBadge tone="warn">{t("economy.inflationBadge")}</StatusBadge>
      </div>
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("economy.mainCurrencies")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("economy.asset")}</th>
                <th>{t("economy.produced")}</th>
                <th>{t("economy.consumed")}</th>
                <th>{t("economy.net")}</th>
                <th>{t("economy.state")}</th>
              </tr>
            </thead>
            <tbody>
              {economyRows.map(([asset, produced, consumed, net, state]) => (
                <tr key={asset}>
                  <td>{asset}</td>
                  <td>{produced}</td>
                  <td>{consumed}</td>
                  <td>{net}</td>
                  <td>
                    <StatusBadge tone={state === "Stable" ? "success" : "warn"}>
                      {translateAdminStatus(t, state)}
                    </StatusBadge>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("economy.resourceDistribution")}</p>
          <Bars
            rows={[
              { label: t("economy.topOne"), value: 46 },
              { label: t("economy.guildBanks"), value: 22 },
              { label: t("economy.market"), value: 19 },
              { label: t("economy.dormant"), value: 13 }
            ]}
          />
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <p className="eyebrow">{t("economy.priceMovement")}</p>
        <div className="grid three">
          {["Dragon Scale", "Blessed Oil", "Wooma Horn"].map((item, index) => (
            <div className="side-card" key={item} style={{ marginTop: 0 }}>
              <h3>{item}</h3>
              <p className="metric-value">{[12800, 8420, 22100][index].toLocaleString()}</p>
              <p className={index === 1 ? "delta negative" : "delta"}>{index === 1 ? "+18.4% abnormal" : "+3.2% normal"}</p>
            </div>
          ))}
        </div>
      </section>
    </AdminShell>
  );
}
