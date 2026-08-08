import Link from "next/link";
import { AdminShell } from "../../../components/admin-shell";
import { Bars } from "../../../components/bars";
import { StatusBadge } from "../../../components/status-badge";
import { SubmitButton } from "../../../components/submit-button";
import { adminGet, type AdminPlayerDetail } from "../../../lib/admin-api";
import { formatMap, formatNumber, formatVersion, statusTone } from "../../../lib/format";
import { getAdminI18n, translateAdminStatus } from "../../../lib/i18n";
import { setCharacterFlagAction, setChatBanAction, updateCharacterAction } from "../../ops-actions";

export default async function PlayerDetailPage({
  params,
  searchParams
}: {
  params: Promise<{ id: string }>;
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}) {
  const { id } = await params;
  const requestedId = decodeRouteId(id);
  const query = (await searchParams) ?? {};
  const error = firstParam(query.error);
  const commandId = firstParam(query.commandId);
  const { t } = await getAdminI18n();
  const detail = await adminGet<AdminPlayerDetail>(
    `/admin/read/players/${encodeURIComponent(requestedId)}`
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
          <h2>{summary?.characterName ?? requestedId}</h2>
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
          <Link
            className="button secondary"
            href={`/service-trace?query=${encodeURIComponent(summary?.playerId ?? requestedId)}`}
          >
            服务节点追踪
          </Link>
        </div>
      </div>
      {error ? <p className="notice">{error}</p> : null}
      {commandId ? <p className="notice">Command queued: {commandId}</p> : null}
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
                ["PK points", formatNumber(summary?.pkPoints)],
                ["Chat ban", summary?.chatBanned ? `Active until ${formatTimestamp(summary.chatBanUntilMs)}` : "-"],
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
          <h3>Crystal player editor</h3>
          <form action={updateCharacterAction} className="form-grid">
            <input name="characterId" type="hidden" value={summary?.playerId ?? requestedId} />
            <input name="redirectPath" type="hidden" value={`/players/${encodeURIComponent(requestedId)}`} />
            <input className="control" name="name" placeholder="new name" />
            <input className="control" name="level" placeholder="level" type="number" />
            <input className="control" name="gold" placeholder="gold" type="number" />
            <input className="control" name="credit" placeholder="credit" type="number" />
            <input className="control" name="pkPoints" placeholder="PK points" type="number" />
            <input className="control" name="mapFileName" placeholder="map file" />
            <input className="control" name="mapTitle" placeholder="map title" />
            <input className="control" name="positionX" placeholder="x" type="number" />
            <input className="control" name="positionY" placeholder="y" type="number" />
            <input className="control" name="hp" placeholder="hp" type="number" />
            <input className="control" name="maxHp" placeholder="max hp" type="number" />
            <input className="control" name="mp" placeholder="mp" type="number" />
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="player detail character update" />
            <div className="field full">
              <SubmitButton idle="Update character" pending="Updating..." />
            </div>
          </form>
          <div className="rune-divider" />
          <h3>Chat ban</h3>
          <form action={setChatBanAction} className="form-stack">
            <input name="characterId" type="hidden" value={summary?.playerId ?? requestedId} />
            <input name="redirectPath" type="hidden" value={`/players/${encodeURIComponent(requestedId)}`} />
            <input name="banned" type="hidden" value="true" />
            <input className="control" name="durationMinutes" placeholder="duration minutes" type="number" defaultValue={5} />
            <input className="control" name="reason" defaultValue="chat ban from admin console" />
            <SubmitButton idle="Apply chat ban" pending="Applying..." />
          </form>
          <form action={setChatBanAction} className="form-stack">
            <input name="characterId" type="hidden" value={summary?.playerId ?? requestedId} />
            <input name="redirectPath" type="hidden" value={`/players/${encodeURIComponent(requestedId)}`} />
            <input className="control" name="reason" defaultValue="clear chat ban from admin console" />
            <SubmitButton idle="Clear chat ban" pending="Clearing..." />
          </form>
          <div className="rune-divider" />
          <h3>NPC flags</h3>
          <p className="muted">
            Active flags:{" "}
            {player?.activeNpcFlags.length
              ? player.activeNpcFlags.map((flag) => flag.index).join(", ")
              : "-"}
          </p>
          <form action={setCharacterFlagAction} className="form-stack">
            <input name="characterId" type="hidden" value={summary?.playerId ?? requestedId} />
            <input className="control" name="flagIndex" placeholder="flag index 1-1999" type="number" />
            <label className="check-row">
              <input name="active" type="checkbox" defaultChecked />
              <span>Active</span>
            </label>
            <input className="control" name="approvalId" placeholder="approval id required" />
            <input className="control" name="reason" defaultValue="set player NPC flag from admin console" />
            <SubmitButton idle="Set flag" pending="Setting..." />
          </form>
          <div className="actions">
            <StatusBadge tone="warn">{t("playerDetail.secondConfirm")}</StatusBadge>
            <StatusBadge>{t("playerDetail.rollbackRequired")}</StatusBadge>
          </div>
        </section>
      </div>
    </AdminShell>
  );
}

function firstParam(value: string | string[] | undefined) {
  return (Array.isArray(value) ? value[0] : value)?.trim() ?? "";
}

function decodeRouteId(value: string) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function formatTimestamp(value: number | undefined) {
  if (!value) {
    return "-";
  }
  return new Date(value).toLocaleString();
}
