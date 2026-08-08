import Link from "next/link";
import { AdminShell } from "../../components/admin-shell";
import { StatusBadge } from "../../components/status-badge";
import { adminGet, type AdminPlayersReadModel } from "../../lib/admin-api";
import { formatMap, formatNumber, statusTone } from "../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../lib/i18n";

export default async function PlayersPage() {
  const { t } = await getAdminI18n();
  const players = await adminGet<AdminPlayersReadModel>("/admin/read/players");
  const rows = players.ok ? players.data.players : [];

  return (
    <AdminShell active="/players">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("players.eyebrow")}</p>
          <h2>{t("players.title")}</h2>
          <p className="muted">{t("players.subtitle")}</p>
        </div>
        <StatusBadge tone={players.ok ? "success" : "warn"}>
          {players.ok ? players.data.source : t("common.unavailable")}
        </StatusBadge>
      </div>
      {!players.ok ? <p className="notice">{players.error}</p> : null}
      <section className="card">
        <table className="table">
          <thead>
            <tr>
              <th>{t("players.player")}</th>
              <th>{t("players.class")}</th>
              <th>{t("players.level")}</th>
              <th>{t("players.map")}</th>
              <th>{t("players.gold")}</th>
              <th>{t("players.credit")}</th>
              <th>{t("table.status")}</th>
              <th>{t("players.action")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((player) => (
              <tr key={player.playerId}>
                <td>
                  <strong>{player.characterName}</strong>
                  <div className="muted">{player.playerId}</div>
                </td>
                <td>{player.className}</td>
                <td>{player.level}</td>
                <td>{formatMap(player.mapTitle, player.mapFileName)}</td>
                <td>{formatNumber(player.gold)}</td>
                <td>{formatNumber(player.credit)}</td>
                <td>
                  <StatusBadge tone={statusTone(player.status)}>
                    {translateAdminStatus(t, player.status)}
                  </StatusBadge>
                </td>
                <td>
                  <Link className="badge" href={`/players/${encodeURIComponent(player.playerId)}`}>
                    {t("players.viewDetail")}
                  </Link>
                </td>
              </tr>
            ))}
            {!rows.length ? (
              <tr>
                <td colSpan={8}>
                  <p className="notice">{t("players.empty")}</p>
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </section>
    </AdminShell>
  );
}
