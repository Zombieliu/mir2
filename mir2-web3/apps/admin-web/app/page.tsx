import { AdminShell } from "../components/admin-shell";
import { Bars } from "../components/bars";
import { MetricCard } from "../components/metric-card";
import { StatusBadge } from "../components/status-badge";
import { adminGet, type AdminDashboardReadModel } from "../lib/admin-api";
import { formatNumber, serviceConfigStatusKey, statusTone } from "../lib/format";
import { getAdminI18n, translateAdminStatus } from "../lib/i18n";

export default async function DashboardPage() {
  const { t } = await getAdminI18n();
  const dashboard = await adminGet<AdminDashboardReadModel>("/admin/read/dashboard");
  const data = dashboard.ok ? dashboard.data : undefined;
  const metrics = [
    {
      title: t("metric.accounts"),
      value: formatNumber(data?.accountCount),
      delta: data ? data.source : t("common.unavailable")
    },
    {
      title: t("metric.characters"),
      value: formatNumber(data?.characterCount),
      delta: t("metric.charactersNote")
    },
    {
      title: t("metric.totalGold"),
      value: formatNumber(data?.totalGold),
      delta: t("metric.totalCredit", { value: formatNumber(data?.totalCredit) })
    },
    {
      title: t("metric.riskQueue"),
      value: formatNumber(data?.activeBanCount),
      delta: t("metric.riskDeltaReal"),
      negative: Boolean(data?.activeBanCount)
    }
  ];

  return (
    <AdminShell active="/">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("dashboard.eyebrow")}</p>
          <h2>{t("dashboard.title")}</h2>
          <p className="muted">{t("dashboard.subtitle")}</p>
        </div>
        <StatusBadge tone={dashboard.ok ? "success" : "warn"}>
          {dashboard.ok ? t("dashboard.adminConnected") : t("dashboard.adminOffline")}
        </StatusBadge>
      </div>
      {!dashboard.ok ? <p className="notice">{dashboard.error}</p> : null}

      <div className="grid metrics">
        {metrics.map((metric) => (
          <MetricCard key={metric.title} {...metric} />
        ))}
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">{t("dashboard.populationHeat")}</p>
          <h3>{t("dashboard.hotMaps")}</h3>
          {data?.hotMaps.length ? (
            <Bars
              rows={data.hotMaps.map((row) => ({
                label: row.mapTitle,
                value: row.percent,
                suffix: `%, ${formatNumber(row.characterCount)}`
              }))}
            />
          ) : (
            <p className="notice">{t("dashboard.emptyHotMaps")}</p>
          )}
        </section>
        <section className="card">
          <p className="eyebrow">{t("dashboard.source")}</p>
          <h3>{data?.source ?? t("common.unavailable")}</h3>
          <p className="metric-value">{formatNumber(data?.onlineNow)}</p>
          <p className="muted">{data?.onlineSource ?? t("dashboard.noOnlineSource")}</p>
          <div className="rune-divider" />
          <div className="actions">
            <StatusBadge>{t("dashboard.realReadModel")}</StatusBadge>
          </div>
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">{t("dashboard.serverStatus")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("table.realm")}</th>
                <th>{t("table.online")}</th>
                <th>{t("table.latency")}</th>
                <th>{t("table.status")}</th>
              </tr>
            </thead>
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
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("dashboard.commandEvidence")}</p>
          <h3>{t("dashboard.auditOutbox")}</h3>
          <p className="metric-value">
            {formatNumber(data?.auditRecordCount)} / {formatNumber(data?.outboxReceiptCount)}
          </p>
          <p className="muted">{t("dashboard.auditOutboxNote")}</p>
        </section>
      </div>
    </AdminShell>
  );
}
