import Link from "next/link";
import { AdminShell } from "../../../components/admin-shell";
import { Bars } from "../../../components/bars";
import { StatusBadge } from "../../../components/status-badge";
import { adminGet, type AdminPlayerDetail } from "../../../lib/admin-api";
import { formatMap, formatNumber, formatVersion, statusTone } from "../../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../../lib/i18n";

export default async function PlayerDetailPage({
  params
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const { t } = await getAdminI18n();
  const detail = await adminGet<AdminPlayerDetail>(
    `/admin/read/players/${encodeURIComponent(id)}`
  );
  const player = detail.ok ? detail.data : undefined;
  const summary = player?.summary;
  const hpPercent =
    summary && summary.maxHp > 0 ? Math.min(100, Math.ceil((summary.hp / summary.maxHp) * 100)) : 0;

  return (
    <AdminShell active="/players">
      <div className="page-head">
        <div>
          <p className="eyebrow">{t("playerDetail.eyebrow")}</p>
          <h2>{summary?.characterName ?? id}</h2>
          <p className="muted">
            {summary
              ? t("playerDetail.identity", {
                  id: summary.playerId,
                  className: summary.className,
                  map: formatMap(summary.mapTitle, summary.mapFileName)
                })
              : t("common.unavailable")}
          </p>
        </div>
        <div className="actions">
          <StatusBadge tone={summary ? statusTone(summary.status) : "warn"}>
            {summary ? translateAdminStatus(t, summary.status) : t("common.unavailable")}
          </StatusBadge>
          <Link className="button" href="/gm-tools">
            {t("playerDetail.sendMail")}
          </Link>
        </div>
      </div>
      {!detail.ok ? <p className="notice">{detail.error}</p> : null}

      <div className="grid three">
        <section className="card">
          <p className="eyebrow">{t("playerDetail.character")}</p>
          <h3>{t("playerDetail.combatProfile")}</h3>
          <Bars
            rows={[
              { label: "HP", value: hpPercent },
              { label: "MP", value: Math.min(100, summary?.mp ?? 0), suffix: "" },
              { label: t("players.gold"), value: Math.min(100, summary?.gold ?? 0), suffix: "" },
              { label: t("players.credit"), value: Math.min(100, summary?.credit ?? 0), suffix: "" }
            ]}
          />
        </section>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.inventory")}</p>
          <h3>{t("playerDetail.equipmentBags")}</h3>
          <p className="muted">{t("playerDetail.inventoryCount", { value: player?.inventoryCount ?? 0 })}</p>
          <p className="muted">{t("playerDetail.storageCount", { value: player?.storageCount ?? 0 })}</p>
          <p className="muted">{t("playerDetail.equipmentCount", { value: player?.equipmentCount ?? 0 })}</p>
          <p className="muted">{t("playerDetail.questSkillCount", { quest: player?.questStateCount ?? 0, skill: player?.skillStateCount ?? 0 })}</p>
        </section>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.riskSignals")}</p>
          <h3>{player?.activeBanReason ? t("common.status.banned") : t("common.status.normal")}</h3>
          <p className="muted">{player?.activeBanReason ?? t("playerDetail.noActiveBan")}</p>
          <StatusBadge tone={player?.activeBanReason ? "danger" : "success"}>
            {player?.activeBanReason ? t("common.status.banned") : t("common.status.lowRisk")}
          </StatusBadge>
        </section>
      </div>

      <div className="grid two" style={{ marginTop: 16 }}>
        <section className="card">
          <p className="eyebrow">{t("playerDetail.records")}</p>
          <table className="table">
            <tbody>
              {[
                [t("playerDetail.account"), summary?.accountId ?? "-"],
                [t("playerDetail.map"), summary ? `${formatMap(summary.mapTitle, summary.mapFileName)} (${summary.positionX}, ${summary.positionY})` : "-"],
                [t("players.gold"), formatNumber(summary?.gold)],
                [t("players.credit"), formatNumber(summary?.credit)],
                [t("playerDetail.mail"), `${formatNumber(player?.unclaimedMailCount)} / ${formatNumber(player?.mailCount)}`],
                [
                  t("playerDetail.runtime"),
                  summary?.online
                    ? `${summary.onlineSource ?? "gateway_session_cache"} / tick ${formatVersion(summary.runtimeTick)}`
                    : "-"
                ],
                [t("playerDetail.versions"), `${formatVersion(summary?.storeVersion)} / ${formatVersion(summary?.saveVersion)}`]
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
