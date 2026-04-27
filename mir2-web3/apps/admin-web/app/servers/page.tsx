import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminServersReadModel } from "../../lib/admin-api";
import { formatNumber, serviceConfigStatusKey, statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function ServersPage() {
  const { t } = await getAdminI18n();
  const servers = await adminGet<AdminServersReadModel>("/admin/read/servers");
  const data = servers.ok ? servers.data : undefined;
  const healthy = data?.services.filter((service) => service.status === "Healthy").length ?? 0;
  const unavailable =
    data?.services.filter((service) => service.status === "Unavailable").length ?? 0;

  return (
    <AdminShell active="/servers">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("servers.eyebrow")}</p>
          <h2>{t("servers.title")}</h2>
          <p className="muted">{t("servers.subtitle")}</p>
        </div>
        <StatusBadge tone={servers.ok ? "success" : "warn"}>
          {servers.ok ? data?.accountStoreSource : t("common.unavailable")}
        </StatusBadge>
      </div>
      {!servers.ok ? <p className="notice">{servers.error}</p> : null}
      <div className="grid three">
        <section className="card">
          <p className="eyebrow">{t("servers.serviceHealth")}</p>
          <Bars
            rows={(data?.services ?? []).map((service) => ({
              label: service.name,
              value: service.status === "Healthy" ? 100 : service.status === "Unavailable" ? 0 : 35,
              suffix: "%"
            }))}
          />
        </section>
        <section className="card">
          <p className="eyebrow">{t("servers.accountStore")}</p>
          <p className="metric-value">{formatNumber(data?.characterCount)}</p>
          <p className="muted">
            {formatNumber(data?.accountCount)} {t("servers.accounts")} / {data?.accountStoreSource ?? "-"}
          </p>
        </section>
        <section className="card">
          <p className="eyebrow">{t("servers.alerts")}</p>
          <p className="metric-value">{formatNumber(unavailable)}</p>
          <p className="muted">
            {formatNumber(healthy)} {t("servers.healthyServices")} / {data?.zonesSource ?? "-"}
          </p>
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <p className="eyebrow">{t("servers.serviceList")}</p>
        <table className="table">
          <tbody>
            {(data?.services ?? []).map((service) => (
              <tr key={service.name}>
                <td>{service.name}</td>
                <td>{t(serviceConfigStatusKey(service.configured, service.status))}</td>
                <td>{service.latencyMs === undefined ? "-" : `${service.latencyMs}ms`}</td>
                <td>
                  <StatusBadge tone={statusTone(service.status)}>
                    {translateAdminStatus(t, service.status)}
                  </StatusBadge>
                </td>
                <td>{service.detail}</td>
              </tr>
            ))}
            {!data?.services.length ? (
              <tr>
                <td colSpan={5}>
                  <p className="notice">{t("servers.empty")}</p>
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
