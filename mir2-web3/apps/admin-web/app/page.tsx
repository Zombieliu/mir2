import { AdminShell } from "../components/admin-shell";
import { Bars } from "../components/bars";
import { MetricCard } from "../components/metric-card";
import { StatusBadge } from "../components/status-badge";
import {
  adminGet,
  type AdminDashboardReadModel,
  type GameplayEventCommandSummary,
  type GameplayEventSummaryResponse
} from "../lib/admin-api";
import { formatNumber, serviceConfigStatusKey, statusTone } from "../lib/format";
import { getAdminI18n, translateAdminStatus } from "../lib/i18n";

export default async function DashboardPage() {
  const { t } = await getAdminI18n();
  const [dashboard, gameplaySummary] = await Promise.all([
    adminGet<AdminDashboardReadModel>("/admin/read/dashboard"),
    adminGet<GameplayEventSummaryResponse>(
      "/admin/gameplay-events/summary?windowSeconds=300&limit=5&maxLagSeconds=180&minEvents=1"
    )
  ]);
  const data = dashboard.ok ? dashboard.data : undefined;
  const gameplay = gameplaySummary.ok ? gameplaySummary.data : undefined;
  const gameplayHealthy = gameplaySummary.ok && Boolean(gameplay?.ready) && !gameplay?.degraded;
  const gameplayAlerts = gameplay?.alerts ?? [];
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
          <div className="section-head">
            <div>
              <p className="eyebrow">{t("dashboard.gameplayEvents")}</p>
              <h3>{t("dashboard.gameplayReadiness")}</h3>
            </div>
            <StatusBadge tone={gameplayHealthy ? "success" : "warn"}>
              {gameplayHealthy
                ? t("dashboard.eventStreamReady")
                : !gameplaySummary.ok || gameplay?.degraded
                  ? t("dashboard.eventStreamDegraded")
                  : t("dashboard.eventStreamAlert")}
            </StatusBadge>
          </div>
          <div className="event-summary-grid">
            <div>
              <p className="metric-title">{t("dashboard.commandVolume")}</p>
              <p className="metric-value">{formatNumber(gameplay?.totalCount)}</p>
            </div>
            <div>
              <p className="metric-title">{t("dashboard.eventLag")}</p>
              <p className="metric-value">{formatLag(gameplay?.lagMs)}</p>
            </div>
          </div>
          <p className="muted">
            {gameplay?.lastOccurredAtMs
              ? t("dashboard.lastGameplayEvent", { value: formatTimestamp(gameplay.lastOccurredAtMs) })
              : t("dashboard.noGameplayEvents")}
          </p>
          {!gameplaySummary.ok ? <p className="notice">{gameplaySummary.error}</p> : null}
          {gameplay?.degraded && gameplay.error ? <p className="notice">{gameplay.error}</p> : null}
          {gameplayAlerts.length ? (
            <div className="alert-list">
              {gameplayAlerts.map((alert) => (
                <p className="notice" key={`${alert.level}-${alert.code}`}>
                  {alert.message}
                </p>
              ))}
            </div>
          ) : null}
          {gameplay?.commands.length ? (
            <table className="table compact-table">
              <thead>
                <tr>
                  <th>{t("table.command")}</th>
                  <th>{t("table.events")}</th>
                  <th>{t("table.tick")}</th>
                </tr>
              </thead>
              <tbody>
                {gameplay.commands.map((command) => (
                  <tr key={command.commandKind}>
                    <td>{formatCommandKind(command)}</td>
                    <td>{formatNumber(command.eventCount)}</td>
                    <td>{formatNumber(command.maxSnapshotTick)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
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

function formatLag(value: number | undefined) {
  if (value === undefined) {
    return "-";
  }
  if (value < 1_000) {
    return `${value}ms`;
  }
  if (value < 60_000) {
    return `${Math.round(value / 1_000)}s`;
  }
  return `${Math.round(value / 60_000)}m`;
}

function formatTimestamp(value: number) {
  return new Date(value).toLocaleString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

function formatCommandKind(command: GameplayEventCommandSummary) {
  return command.commandKind.replace(/^client\./, "");
}
