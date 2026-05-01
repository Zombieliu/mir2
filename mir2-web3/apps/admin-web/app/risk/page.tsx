import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { SubmitButton } from "../../components/submit-button";
import { adminGet, type AdminRiskReadModel } from "../../lib/admin-api";
import { statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { upsertTradeGraphEdgeAction } from "./actions";

export default async function RiskPage({
  searchParams
}: {
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { t } = await getAdminI18n();
  const params = (await searchParams) ?? {};
  const error = firstParam(params.error);
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
      {error ? <p className="notice">{error}</p> : null}
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
          {data?.graph.length ? (
            <table className="table">
              <thead>
                <tr>
                  <th>{t("risk.from")}</th>
                  <th>{t("risk.to")}</th>
                  <th>{t("risk.risk")}</th>
                  <th>{t("risk.signal")}</th>
                </tr>
              </thead>
              <tbody>
                {data.graph.map((edge) => (
                  <tr key={edge.edgeId}>
                    <td>{edge.from}</td>
                    <td>{edge.to}</td>
                    <td>
                      <StatusBadge tone={statusTone(edge.risk)}>
                        {translateAdminStatus(t, edge.risk)}
                      </StatusBadge>
                    </td>
                    <td>{edge.signal}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="notice">{t("risk.noGraph")}</p>
          )}
          <div className="rune-divider" />
          <form action={upsertTradeGraphEdgeAction} className="form-stack">
            <div className="form-grid">
              <label className="field">
                <span>{t("risk.edgeId")}</span>
                <input className="control" name="edgeId" />
              </label>
              <label className="field">
                <span>{t("risk.from")}</span>
                <input className="control" name="from" required />
              </label>
              <label className="field">
                <span>{t("risk.to")}</span>
                <input className="control" name="to" required />
              </label>
              <label className="field">
                <span>{t("risk.risk")}</span>
                <select className="control" name="risk" defaultValue="Medium">
                  <option>Low</option>
                  <option>Medium</option>
                  <option>High</option>
                  <option>Critical</option>
                </select>
              </label>
              <label className="field">
                <span>{t("risk.signal")}</span>
                <input className="control" name="signal" defaultValue="manual_review" required />
              </label>
              <label className="field full">
                <span>{t("risk.evidence")}</span>
                <textarea className="control" name="evidence" />
              </label>
            </div>
            <SubmitButton idle={t("common.submit")} pending={t("common.submitting")} />
          </form>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}
