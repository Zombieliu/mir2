import { AdminShell } from "../../components/admin-shell";
import { Bars } from "../../components/bars";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminEconomyReadModel } from "../../lib/admin-api";
import { formatNumber, statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { upsertPriceFeedAction } from "./actions";

export default async function EconomyPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
  const economy = await adminGet<AdminEconomyReadModel>("/admin/read/economy");
  const data = economy.ok ? economy.data : undefined;

  return (
    <AdminShell active="/economy">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("economy.eyebrow")}</p>
          <h2>{t("economy.title")}</h2>
          <p className="muted">{t("economy.subtitle")}</p>
        </div>
        <StatusBadge tone={economy.ok ? "success" : "warn"}>
          {economy.ok ? economy.data.source : t("common.unavailable")}
        </StatusBadge>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {!economy.ok ? <p className="notice">{economy.error}</p> : null}
      <div className="grid two">
        <section className="card">
          <p className="eyebrow">{t("economy.mainCurrencies")}</p>
          <table className="table">
            <thead>
              <tr>
                <th>{t("economy.asset")}</th>
                <th>{t("economy.currentTotal")}</th>
                <th>{t("economy.holders")}</th>
                <th>{t("economy.average")}</th>
                <th>{t("economy.state")}</th>
              </tr>
            </thead>
            <tbody>
              {(data?.assets ?? []).map((asset) => (
                <tr key={asset.asset}>
                  <td>{asset.asset}</td>
                  <td>{formatNumber(asset.total)}</td>
                  <td>{formatNumber(asset.holders)}</td>
                  <td>{formatNumber(asset.average)}</td>
                  <td>
                    <StatusBadge tone={statusTone(asset.state)}>
                      {translateAdminStatus(t, asset.state)}
                    </StatusBadge>
                  </td>
                </tr>
              ))}
              {!data?.assets.length ? (
                <tr>
                  <td colSpan={5}>
                    <p className="notice">{t("economy.empty")}</p>
                  </td>
                </tr>
              ) : null}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("economy.resourceDistribution")}</p>
          {data?.goldDistribution.length ? (
            <Bars
              rows={data.goldDistribution.map((bucket) => ({
                label: t(`economy.bucket.${bucket.key}`),
                value: bucket.value,
                suffix: `%, ${formatNumber(bucket.amount)}`
              }))}
            />
          ) : (
            <p className="notice">{t("economy.emptyDistribution")}</p>
          )}
        </section>
      </div>
      <section className="card" style={{ marginTop: 16 }}>
        <div className="section-head">
          <p className="eyebrow">{t("economy.priceMovement")}</p>
          <StatusBadge tone={data?.priceFeedConfigured ? "success" : "warn"}>
            {data?.priceFeedConfigured ? t("common.connected") : t("common.unconfigured")}
          </StatusBadge>
        </div>
        <div className="grid two">
          <div>
            {data?.priceFeeds.length ? (
              <div className="grid three">
                {data.priceFeeds.map((feed) => (
                  <div className="side-card" key={feed.item} style={{ marginTop: 0 }}>
                    <h3>{feed.item}</h3>
                    <p className="metric-value">{formatNumber(feed.latestPrice)}</p>
                    <p className="delta">
                      {feed.source} / {formatNumber(feed.sampleCount)}
                    </p>
                  </div>
                ))}
              </div>
            ) : (
              <p className="notice">{t("economy.noPriceFeed")}</p>
            )}
          </div>
          <form action={upsertPriceFeedAction} className="form-stack">
            <div className="form-grid">
              <label className="field">
                <span>{t("economy.item")}</span>
                <input className="control" name="item" required />
              </label>
              <label className="field">
                <span>{t("economy.latestPrice")}</span>
                <input className="control" name="latestPrice" inputMode="numeric" required />
              </label>
              <label className="field">
                <span>{t("economy.sampleCount")}</span>
                <input className="control" name="sampleCount" defaultValue="1" inputMode="numeric" />
              </label>
              <label className="field">
                <span>{t("economy.source")}</span>
                <input className="control" name="source" defaultValue="operator_projection" required />
              </label>
            </div>
            <SubmitButton idle={t("common.submit")} pending={t("common.submitting")} />
          </form>
        </div>
      </section>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}
