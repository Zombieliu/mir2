import { AdminShell } from "../../../components/admin-shell";
import { Bars } from "../../../components/bars";
import { StatusBadge } from "../../../components/status-badge";
import { getAdminI18n } from "../../../lib/i18n";

export default async function PlayerDetailPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/players/AZ-1048">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("playerDetail.eyebrow")}</p>
          <h2>MoonBlade</h2>
          <p className="muted">{t("playerDetail.lastSeen", { id })}</p>
        </div>
        <div className="actions">
          <StatusBadge tone="success">{t("common.status.normal")}</StatusBadge>
          <button className="button secondary">{t("playerDetail.mute")}</button>
          <button className="button">{t("playerDetail.sendMail")}</button>
          <button className="button secondary">{t("playerDetail.banReview")}</button>
        </div>
      </div>

      <div className="grid three">
        <section className="card">
          <p className="eyebrow">{t("playerDetail.character")}</p>
          <h3>{t("playerDetail.combatProfile")}</h3>
          <Bars
            rows={[
              { label: "HP", value: 86 },
              { label: "MP", value: 52 },
              { label: t("playerDetail.statAttack"), value: 74 },
              { label: t("playerDetail.statDefense"), value: 68 }
            ]}
          />
        </section>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.inventory")}</p>
          <h3>{t("playerDetail.equipmentBags")}</h3>
          <p className="muted">{t("playerDetail.weapon")}</p>
          <p className="muted">{t("playerDetail.warehouse")}</p>
          <p className="muted">{t("playerDetail.questItems")}</p>
        </section>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.riskSignals")}</p>
          <h3>{t("playerDetail.deviceIp")}</h3>
          <p className="muted">{t("playerDetail.devices")}</p>
          <p className="muted">{t("playerDetail.tradeScore")}</p>
          <StatusBadge tone="success">{t("common.status.lowRisk")}</StatusBadge>
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.records")}</p>
          <table className="table">
            <tbody>
              {[
                ["Recharge", "$2,184 lifetime / last $49.99"],
                ["Trade", "83 player trades / 0 chargeback risk"],
                ["Chat", "2 warning keywords in last 7 days"],
                ["Quest", "Mainline chapter 4 / Wooma temple"],
                ["GM Ops", "1 compensation mail, 0 disciplinary actions"]
              ].map(([label, value]) => (
                <tr key={label}>
                  <th>{label}</th>
                  <td>{value}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.actionDrawer")}</p>
          <h3>{t("playerDetail.requirements")}</h3>
          <p className="muted">{t("playerDetail.requirementsBody")}</p>
          <div className="actions">
            <StatusBadge tone="warn">{t("playerDetail.secondConfirm")}</StatusBadge>
            <StatusBadge>{t("playerDetail.rollbackRequired")}</StatusBadge>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}
