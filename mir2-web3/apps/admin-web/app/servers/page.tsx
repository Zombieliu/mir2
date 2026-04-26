import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { servers } from "../../lib/mock-data";

export default async function ServersPage() {
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/servers">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("servers.eyebrow")}</p>
          <h2>{t("servers.title")}</h2>
          <p className="muted">{t("servers.subtitle")}</p>
        </div>
        <StatusBadge tone="success">{t("servers.zonesOnline")}</StatusBadge>
      </div>
      <div className="grid three">
        <section className="card">
          <p className="eyebrow">{t("servers.cpuMemory")}</p>
          <Bars
            rows={[
              { label: t("servers.gateway"), value: 32 },
              { label: t("servers.worldS1"), value: 68 },
              { label: t("servers.worldS2"), value: 54 },
              { label: "Admin API", value: 12 }
            ]}
          />
        </section>
        <section className="card">
          <p className="eyebrow">{t("servers.latencyTitle")}</p>
          <p className="metric-value">41ms</p>
          <p className="muted">{t("servers.latencyNote")}</p>
        </section>
        <section className="card">
          <p className="eyebrow">{t("servers.alerts")}</p>
          <p className="metric-value">3</p>
          <p className="muted">{t("servers.alertNote")}</p>
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <p className="eyebrow">{t("servers.realmList")}</p>
        <table className="table">
          <tbody>
            {servers.map(([realm, online, latency, status]) => (
              <tr key={realm}>
                <td>{realm}</td>
                <td>
                  {online} {t("table.online").toLowerCase()}
                </td>
                <td>{latency}</td>
                <td>
                  <StatusBadge tone={status === "Warn" ? "warn" : "success"}>
                    {translateAdminStatus(t, status)}
                  </StatusBadge>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
