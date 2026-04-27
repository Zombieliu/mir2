import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminServersReadModel } from "../../lib/admin-api";
import { formatNumber, serviceConfigStatusKey, statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { upsertZoneRuntimeAction } from "./actions";

export default async function ServersPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const servers = await adminGet<AdminServersReadModel>("/admin/read/servers");
  const data = servers.ok ? servers.data : undefined;
  const healthy = data?.services.filter((service) => service.status === "Healthy").length ?? 0;

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
      {error ? <p className="notice">{error}</p> : null}
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
          <p className="eyebrow">{t("servers.zonesOnline")}</p>
          <p className="metric-value">{formatNumber(data?.zonesOnline)}</p>
          <p className="muted">
            {data?.zoneRuntimeConfigured ? t("common.connected") : t("common.unconfigured")} /{" "}
            {data?.zonesSource ?? "-"}
          </p>
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <div className="section-head">
          <p className="eyebrow">{t("servers.zoneRuntime")}</p>
          <StatusBadge tone={data?.zoneRuntimeConfigured ? "success" : "warn"}>
            {data?.zoneRuntimeConfigured ? t("common.connected") : t("common.unconfigured")}
          </StatusBadge>
        </div>
        <div className="grid two">
          <div>
            <table className="table">
              <thead>
                <tr>
                  <th>{t("servers.zone")}</th>
                  <th>{t("table.status")}</th>
                  <th>{t("servers.players")}</th>
                  <th>{t("servers.maps")}</th>
                  <th>{t("servers.tickRate")}</th>
                </tr>
              </thead>
              <tbody>
                {(data?.zones ?? []).map((zone) => (
                  <tr key={zone.zoneId}>
                    <td>{zone.name}</td>
                    <td>
                      <StatusBadge tone={statusTone(zone.status)}>
                        {translateAdminStatus(t, zone.status)}
                      </StatusBadge>
                    </td>
                    <td>{formatNumber(zone.playerCount)}</td>
                    <td>{formatNumber(zone.mapCount)}</td>
                    <td>{formatNumber(zone.tickRate)}</td>
                  </tr>
                ))}
                {!(data?.zones?.length ?? 0) ? (
                  <tr>
                    <td colSpan={5}>
                      <p className="notice">{t("servers.noZones")}</p>
                    </td>
                  </tr>
                ) : null}
              </tbody>
            </table>
          </div>
          <form action={upsertZoneRuntimeAction} className="form-stack">
            <div className="form-grid">
              <label className="field">
                <span>{t("servers.zoneId")}</span>
                <input className="control" name="zoneId" />
              </label>
              <label className="field">
                <span>{t("servers.zone")}</span>
                <input className="control" name="name" required />
              </label>
              <label className="field">
                <span>{t("table.status")}</span>
                <select className="control" name="status" defaultValue="Healthy">
                  <option>Healthy</option>
                  <option>Warn</option>
                  <option>Unavailable</option>
                </select>
              </label>
              <label className="field">
                <span>{t("servers.host")}</span>
                <input className="control" name="host" defaultValue="127.0.0.1" />
              </label>
              <label className="field">
                <span>{t("servers.processId")}</span>
                <input className="control" name="processId" />
              </label>
              <label className="field">
                <span>{t("servers.players")}</span>
                <input className="control" name="playerCount" defaultValue="0" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("servers.maps")}</span>
                <input className="control" name="mapCount" defaultValue="1" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("servers.tickRate")}</span>
                <input className="control" name="tickRate" defaultValue="20" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("servers.uptime")}</span>
                <input className="control" name="uptimeSeconds" defaultValue="0" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("economy.source")}</span>
                <input className="control" name="source" defaultValue="operator_projection" />
              </label>
            </div>
            <SubmitButton idle={t("common.submit")} pending={t("common.submitting")} />
          </form>
        </div>
      </section>
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

function firstParam(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}
