import Link from "next/link";
import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";
import { players } from "../../lib/mock-data";

export default async function PlayersPage() {
  const { t } = await getAdminI18n();

  return (
    <AdminShell active="/players">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("players.eyebrow")}</p>
          <h2>{t("players.title")}</h2>
          <p className="muted">{t("players.subtitle")}</p>
        </div>
        <div className="actions">
          <button className="button secondary">{t("players.export")}</button>
          <button className="button">{t("players.openDrawer")}</button>
        </div>
      </div>
      <section className="card">
        <div className="form-grid" style={{ marginBottom: 16 }}>
          <input className="control" placeholder={t("players.searchPlaceholder")} />
          <select className="control" defaultValue="all">
            <option value="all">{t("players.allRealms")}</option>
            <option value="risk">{t("players.riskOnly")}</option>
            <option value="paid">{t("players.paidPlayers")}</option>
          </select>
        </div>
        <table className="table">
          <thead>
            <tr>
              <th>{t("players.player")}</th>
              <th>{t("players.class")}</th>
              <th>{t("players.level")}</th>
              <th>{t("players.power")}</th>
              <th>{t("players.vip")}</th>
              <th>{t("table.status")}</th>
              <th>{t("players.action")}</th>
            </tr>
          </thead>
          <tbody>
            {players.map(([id, name, role, level, power, vip, status]) => (
              <tr key={id}>
                <td>
                  <strong>{name}</strong>
                  <div className="muted">{id}</div>
                </td>
                <td>{role}</td>
                <td>{level}</td>
                <td>{power}</td>
                <td>{vip}</td>
                <td>
                  <StatusBadge
                    tone={
                      status === "Normal" ? "success" : status === "Risk" ? "warn" : "danger"
                    }
                  >
                    {translateAdminStatus(t, status)}
                  </StatusBadge>
                </td>
                <td>
                  <Link className="badge" href={`/players/${id}`}>
                    {t("players.viewDetail")}
                  </Link>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
