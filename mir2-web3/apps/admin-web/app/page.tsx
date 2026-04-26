import { AdminShell } from "../components/admin-shell";
import { Bars } from "../components/bars";
import { MetricCard } from "../components/metric-card";
import { StatusBadge } from "../components/status-badge";
import { adminGet, type AuditRecord, type SystemMailReceipt } from "../lib/admin-api";
import { getAdminI18n, translateAdminStatus } from "../lib/i18n";
import { hotMaps, metrics, servers } from "../lib/mock-data";

export default async function DashboardPage() {
  const { t } = await getAdminI18n();
  const audit = await adminGet<AuditRecord[]>("/admin/audit");
  const outbox = await adminGet<SystemMailReceipt[]>("/admin/system-mail/outbox");
  const metricText = [
    { title: t("metric.onlineNow"), delta: t("metric.onlineDelta") },
    { title: t("metric.dauMau"), delta: t("metric.dauMauDelta") },
    { title: t("metric.revenueToday"), delta: t("metric.revenueDelta") },
    { title: t("metric.riskQueue"), delta: t("metric.riskDelta") }
  ];

  return (
    <AdminShell active="/">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("dashboard.eyebrow")}</p>
          <h2>{t("dashboard.title")}</h2>
          <p className="muted">{t("dashboard.subtitle")}</p>
        </div>
        <StatusBadge tone={audit.ok ? "success" : "warn"}>
          {audit.ok ? t("dashboard.adminConnected") : t("dashboard.adminOffline")}
        </StatusBadge>
      </div>

      <div className="grid metrics">
        {metrics.map((metric, index) => (
          <MetricCard
            key={metric.title}
            {...metric}
            delta={metricText[index]?.delta ?? metric.delta}
            title={metricText[index]?.title ?? metric.title}
          />
        ))}
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">{t("dashboard.populationHeat")}</p>
          <h3>{t("dashboard.hotMaps")}</h3>
          <Bars
            rows={hotMaps.map((row, index) => ({
              ...row,
              label:
                [
                  t("map.bichonProvince"),
                  t("map.ancientMine3f"),
                  t("map.woomaTemple"),
                  t("map.redMoonValley")
                ][index] ?? row.label
            }))}
          />
        </section>
        <section className="card">
          <p className="eyebrow">{t("dashboard.worldBoss")}</p>
          <h3>{t("dashboard.bossName")}</h3>
          <p className="metric-value">17:42</p>
          <p className="muted">{t("dashboard.bossNote")}</p>
          <div className="rune-divider" />
          <div className="actions">
            <StatusBadge tone="warn">{t("dashboard.highAttention")}</StatusBadge>
            <StatusBadge>{t("dashboard.instanceReady")}</StatusBadge>
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
              {servers.map(([realm, online, latency, status]) => (
                <tr key={realm}>
                  <td>{realm}</td>
                  <td>{online}</td>
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
        <section className="card">
          <p className="eyebrow">{t("dashboard.commandEvidence")}</p>
          <h3>{t("dashboard.auditOutbox")}</h3>
          <p className="metric-value">
            {audit.ok ? audit.data.length : 0} / {outbox.ok ? outbox.data.length : 0}
          </p>
          <p className="muted">{t("dashboard.auditOutboxNote")}</p>
          {!audit.ok ? <p className="notice">{audit.error}</p> : null}
        </section>
      </div>
    </AdminShell>
  );
}
