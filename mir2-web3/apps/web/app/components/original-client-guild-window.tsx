"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type GuildMember = {
  name: string;
  rank?: string;
  online?: boolean;
  /** Contribution / donation points. */
  contribution?: number;
  lastSeen?: string;
};

/**
 * One of the eight Crystal `GuildRankOptions` permission flags. Used by both
 * the per-rank permission editor and the overview permission badges.
 */
export type GuildPermissionKey =
  | "CanChangeRank"
  | "CanRecruit"
  | "CanKick"
  | "CanStoreItem"
  | "CanRetrieveItem"
  | "CanAlterAlliance"
  | "CanChangeNotice"
  | "CanActivateBuff";

/** A configurable rank with its permission flags + assigned members. */
export type GuildRank = {
  /** Stable index used as the rank identifier in callbacks. */
  index: number;
  name: string;
  /** Active permission flags for this rank. */
  permissions?: GuildPermissionKey[];
};

/** A single slot in the guild storage grid (item present or empty). */
export type GuildStorageItem = {
  /** Slot index within the storage grid. */
  slot: number;
  name?: string;
  /** Optional icon URL when the host can resolve item art. */
  iconUrl?: string;
  count?: number;
  /** Tooltip / grade hint shown on hover. */
  hint?: string;
};

/**
 * Compatible with the `stage5Systems.guild` payload shape
 * (`{ name, members, rank, permissions, chatLog }`) but accepts richer member
 * objects when the caller can provide them.
 */
export type GuildSummary = {
  name?: string;
  /** Viewer's own rank within the guild. */
  rank?: string;
  level?: number;
  experience?: number;
  maxExperience?: number;
  notice?: string;
  gold?: number;
  /** Members may be plain names (legacy payload) or detailed records. */
  members?: Array<string | GuildMember>;
  permissions?: string[];
  chatLog?: string[];
  // --- Additive, optional richer fields (rendered gracefully when absent) ---
  /** Hard member cap; when present shows "N/Max" in the roster header. */
  maxMembers?: number;
  /** Unspent guild buff points (Crystal `SparePoints`). */
  sparePoints?: number;
  /** Guilds this guild is at war with (Crystal `WarringGuilds`). */
  warGuilds?: string[];
  /** Allied guilds (Crystal `AllyGuilds`). */
  allyGuilds?: string[];
  /** Configurable ranks with their permission flags (for the Ranks tab). */
  ranks?: GuildRank[];
  /** Guild storage grid (gold lives in `gold`). */
  storage?: GuildStorageItem[];
  /** Total storage capacity, used to pad the grid with empty slots. */
  storageSize?: number;
};

export type GuildWindowProps = {
  t: TranslateFn;
  guild: GuildSummary | null;
  /** Viewer name, used to highlight the player's own roster row. */
  playerName?: string | null;
  onEditNotice?: (notice: string) => void;
  onInviteMember?: (name: string) => void;
  onKickMember?: (name: string) => void;
  onSendGuildChat?: (message: string) => void;
  // --- Additive, optional action callbacks (wired later in page.tsx) ---
  /** Change a member's rank by rank index. */
  onChangeMemberRank?: (name: string, rankIndex: number) => void;
  /** Persist a rank's name + permission flag set. */
  onSaveRank?: (rankIndex: number, name: string, permissions: GuildPermissionKey[]) => void;
  /** Deposit gold into guild storage. */
  onDepositGold?: (amount: number) => void;
  /** Retrieve gold from guild storage. */
  onWithdrawGold?: (amount: number) => void;
  onClose: () => void;
};

type GuildTab = "overview" | "members" | "storage" | "ranks" | "notice";

const PERMISSION_KEYS: GuildPermissionKey[] = [
  "CanChangeRank",
  "CanRecruit",
  "CanKick",
  "CanStoreItem",
  "CanRetrieveItem",
  "CanAlterAlliance",
  "CanChangeNotice",
  "CanActivateBuff",
];

const PERMISSION_LABELS: Record<GuildPermissionKey, { key: string; fallback: string }> = {
  CanChangeRank: { key: "ui.guildPermChangeRank", fallback: "Change Rank" },
  CanRecruit: { key: "ui.guildPermRecruit", fallback: "Recruit" },
  CanKick: { key: "ui.guildPermKick", fallback: "Kick" },
  CanStoreItem: { key: "ui.guildPermStore", fallback: "Store Item" },
  CanRetrieveItem: { key: "ui.guildPermRetrieve", fallback: "Retrieve Item" },
  CanAlterAlliance: { key: "ui.guildPermAlliance", fallback: "Alter Alliance" },
  CanChangeNotice: { key: "ui.guildPermNotice", fallback: "Change Notice" },
  CanActivateBuff: { key: "ui.guildPermBuff", fallback: "Activate Buff" },
};

const FRAME = ORIGINAL_UI.gameShop;

const GUILD_TABS: { key: GuildTab; labelKey: string; fallback: string }[] = [
  { key: "overview", labelKey: "ui.guildOverview", fallback: "Overview" },
  { key: "members", labelKey: "ui.guildMembers", fallback: "Members" },
  { key: "storage", labelKey: "ui.guildStorage", fallback: "Storage" },
  { key: "ranks", labelKey: "ui.guildRanks", fallback: "Ranks" },
  { key: "notice", labelKey: "ui.guildNotice", fallback: "Notice" },
];

export function GuildWindow({
  t,
  guild,
  playerName,
  onEditNotice,
  onInviteMember,
  onKickMember,
  onSendGuildChat,
  onChangeMemberRank,
  onSaveRank,
  onDepositGold,
  onWithdrawGold,
  onClose,
}: GuildWindowProps) {
  const [tab, setTab] = useState<GuildTab>("overview");
  const [selectedMember, setSelectedMember] = useState<string | null>(null);
  const [inviteName, setInviteName] = useState("");
  const [chatDraft, setChatDraft] = useState("");
  const [noticeDraft, setNoticeDraft] = useState(guild?.notice ?? "");
  const [showOffline, setShowOffline] = useState(true);
  const [goldDraft, setGoldDraft] = useState("");

  const members = useMemo(() => normalizeMembers(guild?.members), [guild?.members]);
  const ranks = guild?.ranks ?? [];
  const warGuilds = guild?.warGuilds ?? [];
  const allyGuilds = guild?.allyGuilds ?? [];
  const onlineCount = members.filter((member) => member.online).length;
  const visibleMembers = useMemo(
    () => (showOffline ? members : members.filter((member) => member.online)),
    [members, showOffline],
  );
  const selected = useMemo(() => {
    if (selectedMember) {
      const match = members.find((member) => member.name === selectedMember);
      if (match) return match;
    }
    return visibleMembers[0] ?? members[0] ?? null;
  }, [members, visibleMembers, selectedMember]);

  useEffect(() => {
    setNoticeDraft(guild?.notice ?? "");
  }, [guild?.notice]);

  if (!guild || !guild.name) {
    return (
      <section aria-label={t("ui.guild", [], "Guild")} style={style.window}>
        <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
        <div style={style.titleText}>{t("ui.guild", [], "Guild")}</div>
        <div style={style.close}>
          <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
        </div>
        <div style={style.noGuild}>{t("ui.guildNone", [], "You are not in a guild.")}</div>
      </section>
    );
  }

  return (
    <section
      aria-label={t("ui.guild", [], "Guild")}
      data-guild-name={guild.name}
      data-guild-tab={tab}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{guild.name}</div>
      <div style={style.subtitle}>
        {guild.maxMembers
          ? t(
              "ui.guildMemberCountMax",
              [members.length, guild.maxMembers, onlineCount],
              `${members.length}/${guild.maxMembers} members · ${onlineCount} online`,
            )
          : t("ui.guildMemberCount", [members.length, onlineCount], `${members.length} members · ${onlineCount} online`)}
      </div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.guild", [], "Guild")}>
        {GUILD_TABS.map((entry) => {
          const active = entry.key === tab;
          return (
            <button
              key={entry.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-guild-tab={entry.key}
              onClick={() => setTab(entry.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(entry.labelKey, [], entry.fallback)}
            </button>
          );
        })}
      </div>

      {tab === "overview" ? (
        <div style={style.panel}>
          <div style={style.overviewGrid}>
            <Info label={t("ui.guildName", [], "Name")} value={guild.name} />
            <Info label={t("ui.guildRank", [], "Your Rank")} value={guild.rank ?? t("ui.guildMember", [], "Member")} />
            <Info label={t("ui.guildLevel", [], "Level")} value={guild.level ? String(guild.level) : "1"} />
            <Info label={t("ui.guildGold", [], "Guild Gold")} value={formatNumber(guild.gold ?? 0)} />
            <Info
              label={t("ui.guildMembers", [], "Members")}
              value={guild.maxMembers ? `${members.length}/${guild.maxMembers}` : String(members.length)}
            />
            <Info label={t("ui.guildOnline", [], "Online")} value={String(onlineCount)} />
            {typeof guild.sparePoints === "number" ? (
              <Info label={t("ui.guildSparePoints", [], "Buff Points")} value={String(Math.max(0, guild.sparePoints))} />
            ) : null}
          </div>
          {guild.maxExperience ? (
            <div style={style.expBlock}>
              <div style={style.gaugeHead}>
                <span style={style.gaugeLabel}>{t("ui.experience", [], "Guild EXP")}</span>
                <span style={style.gaugeValue}>{`${guild.experience ?? 0}/${guild.maxExperience}`}</span>
              </div>
              <div style={style.gaugeTrack}>
                <span
                  style={{
                    ...style.gaugeFill,
                    width: `${Math.min(100, Math.max(0, ((guild.experience ?? 0) / Math.max(guild.maxExperience, 1)) * 100))}%`,
                  }}
                />
              </div>
            </div>
          ) : null}
          <div style={style.noticeLabel}>{t("ui.guildNotice", [], "Notice")}</div>
          <div style={style.noticeReadonly}>{guild.notice || t("ui.guildNoticeEmpty", [], "No notice posted.")}</div>
          {guild.permissions?.length ? (
            <>
              <div style={style.noticeLabel}>{t("ui.guildPermissions", [], "Your Permissions")}</div>
              <div style={style.permissions}>
                {guild.permissions.map((permission) => (
                  <span key={permission} style={style.permissionTag}>
                    {permissionLabel(t, permission)}
                  </span>
                ))}
              </div>
            </>
          ) : null}
          <div style={style.diplomacyGrid}>
            <div style={style.diplomacyCol}>
              <div style={{ ...style.noticeLabel, color: "#d8552f" }}>
                {t("ui.guildWars", [warGuilds.length], `At War (${warGuilds.length})`)}
              </div>
              <div style={style.diplomacyList} data-guild-wars={warGuilds.length}>
                {warGuilds.length === 0 ? (
                  <div style={style.empty}>{t("ui.guildNoWars", [], "No active wars.")}</div>
                ) : (
                  warGuilds.map((name) => (
                    <div key={`war-${name}`} style={style.diplomacyRow}>
                      <span style={{ ...style.diplomacyDot, background: "#d8552f" }} aria-hidden="true" />
                      {name}
                    </div>
                  ))
                )}
              </div>
            </div>
            <div style={style.diplomacyCol}>
              <div style={{ ...style.noticeLabel, color: "#8be07a" }}>
                {t("ui.guildAllies", [allyGuilds.length], `Allies (${allyGuilds.length})`)}
              </div>
              <div style={style.diplomacyList} data-guild-allies={allyGuilds.length}>
                {allyGuilds.length === 0 ? (
                  <div style={style.empty}>{t("ui.guildNoAllies", [], "No allies.")}</div>
                ) : (
                  allyGuilds.map((name) => (
                    <div key={`ally-${name}`} style={style.diplomacyRow}>
                      <span style={{ ...style.diplomacyDot, background: "#8be07a" }} aria-hidden="true" />
                      {name}
                    </div>
                  ))
                )}
              </div>
            </div>
          </div>
        </div>
      ) : null}

      {tab === "members" ? (
        <div style={style.panel}>
          <div style={style.membersLayout}>
            <div style={style.memberList} aria-label={t("ui.guildMembers", [], "Members")}>
              <div style={style.memberListHead}>
                <span style={style.memberColName}>{t("ui.guildName", [], "Name")}</span>
                <span style={style.memberColRank}>{t("ui.guildRank", [], "Rank")}</span>
                <span style={style.memberColState}>{t("ui.guildOnline", [], "State")}</span>
              </div>
              <div style={style.memberRows}>
                {visibleMembers.length === 0 ? (
                  <div style={style.empty}>
                    {members.length === 0
                      ? t("ui.guildNoMembers", [], "No members.")
                      : t("ui.guildNoneOnline", [], "No members online.")}
                  </div>
                ) : (
                  visibleMembers.map((member) => {
                    const isSelected = selected?.name === member.name;
                    const isSelf = playerName != null && member.name === playerName;
                    return (
                      <button
                        key={member.name}
                        type="button"
                        data-member-name={member.name}
                        data-member-online={member.online ? "1" : "0"}
                        aria-pressed={isSelected}
                        onClick={() => setSelectedMember(member.name)}
                        style={{
                          ...style.memberRow,
                          ...(isSelected ? style.memberRowSelected : null),
                          ...(isSelf ? style.memberRowSelf : null),
                        }}
                      >
                        <span style={style.memberColName}>{member.name}</span>
                        <span style={style.memberColRank}>{member.rank ?? "-"}</span>
                        <span style={{ ...style.memberColState, color: member.online ? "#8be07a" : "#9c8d6f" }}>
                          {member.online ? t("ui.guildStateOnline", [], "Online") : t("ui.guildStateOffline", [], "Offline")}
                        </span>
                      </button>
                    );
                  })
                )}
              </div>
              <label style={style.showOfflineRow}>
                <input
                  type="checkbox"
                  checked={showOffline}
                  onChange={(event) => setShowOffline(event.target.checked)}
                  style={style.checkbox}
                />
                {t("ui.guildShowOffline", [], "Show offline members")}
              </label>
            </div>

            <div style={style.memberDetail} data-member-detail={selected?.name ?? ""}>
              {selected ? (
                <>
                  <div style={style.memberDetailName}>{selected.name}</div>
                  <Info label={t("ui.guildRank", [], "Rank")} value={selected.rank ?? t("ui.guildMember", [], "Member")} />
                  <Info
                    label={t("ui.guildOnline", [], "State")}
                    value={selected.online ? t("ui.guildStateOnline", [], "Online") : t("ui.guildStateOffline", [], "Offline")}
                  />
                  <Info label={t("ui.guildContribution", [], "Contribution")} value={formatNumber(selected.contribution ?? 0)} />
                  {selected.lastSeen ? <Info label={t("ui.guildLastSeen", [], "Last Seen")} value={selected.lastSeen} /> : null}
                  {ranks.length > 0 ? (
                    <label style={style.assignRankRow}>
                      <span style={style.infoLabel}>{t("ui.guildAssignRank", [], "Assign Rank")}</span>
                      <select
                        value={ranks.find((rank) => rank.name === selected.rank)?.index ?? ""}
                        disabled={!onChangeMemberRank || (playerName != null && selected.name === playerName)}
                        onChange={(event) => {
                          if (event.target.value === "") return;
                          const idx = Number(event.target.value);
                          if (Number.isFinite(idx)) onChangeMemberRank?.(selected.name, idx);
                        }}
                        aria-label={t("ui.guildAssignRank", [], "Assign Rank")}
                        style={{
                          ...style.select,
                          ...(!onChangeMemberRank || (playerName != null && selected.name === playerName)
                            ? style.actionButtonDisabled
                            : null),
                        }}
                      >
                        {ranks.find((rank) => rank.name === selected.rank) ? null : <option value="">-</option>}
                        {ranks.map((rank) => (
                          <option key={rank.index} value={rank.index}>
                            {rank.name}
                          </option>
                        ))}
                      </select>
                    </label>
                  ) : null}
                  <div style={style.actions}>
                    <ActionButton
                      label={t("ui.guildKick", [], "Kick")}
                      disabled={!onKickMember || (playerName != null && selected.name === playerName)}
                      onClick={() => onKickMember?.(selected.name)}
                    />
                  </div>
                </>
              ) : (
                <div style={style.empty}>{t("ui.guildSelectMember", [], "Select a member.")}</div>
              )}
            </div>
          </div>

          <form
            style={style.inviteRow}
            onSubmit={(event) => {
              event.preventDefault();
              const trimmed = inviteName.trim();
              if (trimmed) {
                onInviteMember?.(trimmed);
                setInviteName("");
              }
            }}
          >
            <input
              style={style.input}
              value={inviteName}
              onChange={(event) => setInviteName(event.target.value)}
              placeholder={t("ui.guildInvitePlaceholder", [], "Player name")}
              aria-label={t("ui.guildInvite", [], "Invite")}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              type="submit"
              disabled={!onInviteMember || inviteName.trim().length === 0}
              style={{
                ...style.actionButton,
                flex: "0 0 110px",
                ...(!onInviteMember || inviteName.trim().length === 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.guildInvite", [], "Invite")}
            </button>
          </form>
        </div>
      ) : null}

      {tab === "storage" ? (
        <div style={style.panel}>
          <div style={style.storageGoldRow}>
            <span style={style.storageGoldLabel}>{t("ui.guildGold", [], "Guild Gold")}</span>
            <span style={style.storageGoldValue}>{formatNumber(guild.gold ?? 0)}</span>
          </div>
          <form
            style={style.inviteRow}
            onSubmit={(event) => {
              event.preventDefault();
              const amount = Math.floor(Number(goldDraft));
              if (Number.isFinite(amount) && amount > 0 && onDepositGold) {
                onDepositGold(amount);
                setGoldDraft("");
              }
            }}
          >
            <input
              style={style.input}
              value={goldDraft}
              onChange={(event) => setGoldDraft(event.target.value.replace(/[^0-9]/g, ""))}
              placeholder={t("ui.guildGoldAmount", [], "Amount")}
              aria-label={t("ui.guildGoldAmount", [], "Amount")}
              inputMode="numeric"
              autoComplete="off"
              spellCheck={false}
            />
            <button
              type="submit"
              disabled={!onDepositGold || Math.floor(Number(goldDraft)) <= 0}
              style={{
                ...style.actionButton,
                flex: "0 0 100px",
                ...(!onDepositGold || Math.floor(Number(goldDraft)) <= 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.guildDeposit", [], "Deposit")}
            </button>
            <button
              type="button"
              disabled={!onWithdrawGold || Math.floor(Number(goldDraft)) <= 0}
              onClick={() => {
                const amount = Math.floor(Number(goldDraft));
                if (Number.isFinite(amount) && amount > 0) {
                  onWithdrawGold?.(amount);
                  setGoldDraft("");
                }
              }}
              style={{
                ...style.actionButton,
                flex: "0 0 100px",
                ...(!onWithdrawGold || Math.floor(Number(goldDraft)) <= 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.guildWithdraw", [], "Retrieve")}
            </button>
          </form>

          <div style={style.noticeLabel}>{t("ui.guildStorageItems", [], "Storage")}</div>
          <div style={style.storageGrid} aria-label={t("ui.guildStorageItems", [], "Storage")}>
            {buildStorageSlots(guild.storage, guild.storageSize).map((item, index) => (
              <div
                key={`slot-${index}`}
                style={style.storageSlot}
                data-storage-slot={index}
                title={item?.hint ?? item?.name ?? ""}
              >
                {item?.iconUrl ? (
                  <img style={style.storageIcon} src={item.iconUrl} alt={item.name ?? ""} draggable={false} />
                ) : item?.name ? (
                  <span style={style.storageName}>{item.name}</span>
                ) : null}
                {item && (item.count ?? 0) > 1 ? <span style={style.storageCount}>{item.count}</span> : null}
              </div>
            ))}
          </div>
        </div>
      ) : null}

      {tab === "ranks" ? (
        <div style={style.panel}>
          {ranks.length === 0 ? (
            <div style={style.empty}>{t("ui.guildNoRanks", [], "No ranks configured.")}</div>
          ) : (
            <div style={style.ranksScroll}>
              {ranks.map((rank) => (
                <RankCard key={rank.index} t={t} rank={rank} disabled={!onSaveRank} onSaveRank={onSaveRank} />
              ))}
            </div>
          )}
        </div>
      ) : null}

      {tab === "notice" ? (
        <div style={style.panel}>
          <div style={style.noticeLabel}>{t("ui.guildNoticeEdit", [], "Edit Notice")}</div>
          <textarea
            style={style.noticeTextarea}
            value={noticeDraft}
            onChange={(event) => setNoticeDraft(event.target.value)}
            aria-label={t("ui.guildNoticeEdit", [], "Edit Notice")}
            spellCheck={false}
            maxLength={400}
          />
          <div style={style.actions}>
            <ActionButton
              label={t("ui.guildSaveNotice", [], "Save Notice")}
              disabled={!onEditNotice || noticeDraft === (guild.notice ?? "")}
              onClick={() => onEditNotice?.(noticeDraft.trim())}
            />
          </div>

          <div style={style.noticeLabel}>{t("ui.guildChat", [], "Guild Chat")}</div>
          <div style={style.chatLog} aria-label={t("ui.guildChat", [], "Guild Chat")}>
            {guild.chatLog?.length ? (
              guild.chatLog.slice(-30).map((line, index) => (
                <div key={`chat-${index}`} style={style.chatLine}>
                  {line}
                </div>
              ))
            ) : (
              <div style={style.empty}>{t("ui.guildChatEmpty", [], "No guild messages yet.")}</div>
            )}
          </div>
          <form
            style={style.inviteRow}
            onSubmit={(event) => {
              event.preventDefault();
              const trimmed = chatDraft.trim();
              if (trimmed) {
                onSendGuildChat?.(trimmed);
                setChatDraft("");
              }
            }}
          >
            <input
              style={style.input}
              value={chatDraft}
              onChange={(event) => setChatDraft(event.target.value)}
              placeholder={t("ui.guildChatPlaceholder", [], "Message guild...")}
              aria-label={t("ui.guildChat", [], "Guild Chat")}
              autoComplete="off"
              spellCheck={false}
            />
            <button
              type="submit"
              disabled={!onSendGuildChat || chatDraft.trim().length === 0}
              style={{
                ...style.actionButton,
                flex: "0 0 110px",
                ...(!onSendGuildChat || chatDraft.trim().length === 0 ? style.actionButtonDisabled : null),
              }}
            >
              {t("ui.send", [], "Send")}
            </button>
          </form>
        </div>
      ) : null}
    </section>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div style={style.info}>
      <span style={style.infoLabel}>{label}</span>
      <span style={style.infoValue}>{value}</span>
    </div>
  );
}

function ActionButton({ label, disabled, onClick }: { label: string; disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      style={{ ...style.actionButton, ...(disabled ? style.actionButtonDisabled : null) }}
    >
      {label}
    </button>
  );
}

/** Editable card for a single guild rank: name + permission flag toggles. */
function RankCard({
  t,
  rank,
  disabled,
  onSaveRank,
}: {
  t: TranslateFn;
  rank: GuildRank;
  disabled?: boolean;
  onSaveRank?: (rankIndex: number, name: string, permissions: GuildPermissionKey[]) => void;
}) {
  const [name, setName] = useState(rank.name);
  const [perms, setPerms] = useState<Set<GuildPermissionKey>>(() => new Set(rank.permissions ?? []));

  useEffect(() => {
    setName(rank.name);
    setPerms(new Set(rank.permissions ?? []));
  }, [rank.name, rank.permissions]);

  const original = new Set(rank.permissions ?? []);
  const dirty =
    name !== rank.name ||
    perms.size !== original.size ||
    PERMISSION_KEYS.some((key) => perms.has(key) !== original.has(key));

  return (
    <div style={style.rankCard} data-rank-index={rank.index}>
      <input
        style={style.rankNameInput}
        value={name}
        onChange={(event) => setName(event.target.value)}
        aria-label={t("ui.guildRankName", [], "Rank Name")}
        spellCheck={false}
        maxLength={20}
        disabled={disabled}
      />
      <div style={style.rankPerms}>
        {PERMISSION_KEYS.map((key) => {
          const on = perms.has(key);
          return (
            <label key={key} style={{ ...style.permCheck, ...(on ? style.permCheckOn : null) }}>
              <input
                type="checkbox"
                checked={on}
                disabled={disabled}
                onChange={(event) => {
                  setPerms((prev) => {
                    const next = new Set(prev);
                    if (event.target.checked) next.add(key);
                    else next.delete(key);
                    return next;
                  });
                }}
                style={style.checkbox}
              />
              {permissionLabel(t, key)}
            </label>
          );
        })}
      </div>
      <div style={style.actions}>
        <ActionButton
          label={t("ui.guildSaveRank", [], "Save Rank")}
          disabled={disabled || !dirty}
          onClick={() => onSaveRank?.(rank.index, name.trim() || rank.name, PERMISSION_KEYS.filter((key) => perms.has(key)))}
        />
      </div>
    </div>
  );
}

/** Localise a permission key, falling back to the raw key for legacy strings. */
function permissionLabel(t: TranslateFn, permission: string): string {
  const entry = PERMISSION_LABELS[permission as GuildPermissionKey];
  return entry ? t(entry.key, [], entry.fallback) : permission;
}

/** Pad the storage payload out to `storageSize` slots so empty cells render. */
function buildStorageSlots(
  storage?: GuildStorageItem[],
  storageSize?: number,
): Array<GuildStorageItem | null> {
  const bySlot = new Map<number, GuildStorageItem>();
  let maxSlot = -1;
  for (const item of storage ?? []) {
    bySlot.set(item.slot, item);
    if (item.slot > maxSlot) maxSlot = item.slot;
  }
  const size = Math.max(storageSize ?? 0, maxSlot + 1, storage?.length ? 16 : 0);
  const slots: Array<GuildStorageItem | null> = [];
  for (let i = 0; i < size; i += 1) {
    slots.push(bySlot.get(i) ?? null);
  }
  return slots;
}

function normalizeMembers(members?: Array<string | GuildMember>): GuildMember[] {
  if (!members) return [];
  return members.map((member) =>
    typeof member === "string" ? { name: member } : member,
  );
}

function formatNumber(value: number) {
  return value.toLocaleString("en-US");
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 164,
    top: 146,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 32,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  titleText: {
    position: "absolute",
    left: 22,
    top: 10,
    fontSize: 14,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 22, top: 30, fontSize: 11, color: "#cbb38a" },
  close: { position: "absolute", left: 666, top: 6 },
  tabs: { position: "absolute", left: 22, top: 50, display: "flex", gap: 4 },
  tab: {
    minWidth: 96,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "4px 12px",
    fontSize: 12,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  panel: {
    position: "absolute",
    left: 22,
    top: 80,
    width: 652,
    height: 372,
    display: "flex",
    flexDirection: "column",
    gap: 8,
  },
  noGuild: { position: "absolute", left: 22, top: 90, color: "#cbb38a", fontSize: 13 },
  overviewGrid: { display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 8 },
  info: {
    display: "flex",
    justifyContent: "space-between",
    gap: 8,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.45)",
    padding: "5px 9px",
    fontSize: 12,
  },
  infoLabel: { color: "#a89568" },
  infoValue: { color: "#f0d69b", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  expBlock: { display: "flex", flexDirection: "column", gap: 3 },
  gaugeHead: { display: "flex", justifyContent: "space-between", fontSize: 11, color: "#cbb38a" },
  gaugeLabel: { color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  gaugeValue: { color: "#e3d3af" },
  gaugeTrack: {
    position: "relative",
    height: 8,
    background: "rgba(0, 0, 0, 0.55)",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    overflow: "hidden",
  },
  gaugeFill: { position: "absolute", left: 0, top: 0, bottom: 0, display: "block", background: "#caa64a" },
  noticeLabel: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.6 },
  noticeReadonly: {
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.55)",
    padding: "8px 10px",
    fontSize: 12,
    color: "#e3d3af",
    minHeight: 48,
    lineHeight: 1.35,
    whiteSpace: "pre-wrap",
  },
  permissions: { display: "flex", flexWrap: "wrap", gap: 5 },
  permissionTag: {
    border: "1px solid rgba(190, 157, 99, 0.45)",
    background: "rgba(52, 32, 18, 0.7)",
    color: "#f0d69b",
    padding: "2px 8px",
    fontSize: 11,
  },
  diplomacyGrid: { display: "flex", gap: 10, minHeight: 0 },
  diplomacyCol: { flex: 1, display: "flex", flexDirection: "column", gap: 3, minWidth: 0 },
  diplomacyList: {
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: "4px 8px",
    maxHeight: 84,
    overflowY: "auto",
    display: "flex",
    flexDirection: "column",
    gap: 2,
    fontSize: 12,
    color: "#e3d3af",
  },
  diplomacyRow: { display: "flex", alignItems: "center", gap: 6, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  diplomacyDot: { width: 7, height: 7, borderRadius: "50%", flex: "0 0 auto" },
  showOfflineRow: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: "4px 8px",
    borderTop: "1px solid rgba(190, 157, 99, 0.28)",
    fontSize: 11,
    color: "#cbb38a",
    cursor: "pointer",
  },
  checkbox: { accentColor: "#caa64a", cursor: "pointer" },
  assignRankRow: { display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, fontSize: 12 },
  select: {
    flex: "0 0 130px",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.8)",
    color: "#f0eee8",
    padding: "3px 6px",
    fontSize: 12,
    fontFamily: "inherit",
  },
  storageGoldRow: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.4), rgba(28, 17, 9, 0.55))",
    padding: "6px 12px",
  },
  storageGoldLabel: { fontSize: 11, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  storageGoldValue: { fontSize: 16, color: "#f4d979", fontWeight: 700 },
  storageGrid: {
    flex: 1,
    minHeight: 0,
    overflowY: "auto",
    display: "grid",
    gridTemplateColumns: "repeat(10, 1fr)",
    gap: 3,
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.5)",
    padding: 6,
    alignContent: "start",
  },
  storageSlot: {
    position: "relative",
    aspectRatio: "1 / 1",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(0, 0, 0, 0.35)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  storageIcon: { maxWidth: "100%", maxHeight: "100%", imageRendering: "pixelated" },
  storageName: { fontSize: 8, color: "#d6c6a5", textAlign: "center", lineHeight: 1.1, padding: 1, overflow: "hidden" },
  storageCount: {
    position: "absolute",
    right: 1,
    bottom: 0,
    fontSize: 9,
    color: "#f8e6bb",
    textShadow: "1px 1px 0 #000",
  },
  ranksScroll: { flex: 1, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 8 },
  rankCard: {
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.5)",
    padding: "8px 10px",
    display: "flex",
    flexDirection: "column",
    gap: 6,
  },
  rankNameInput: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f8e6bb",
    padding: "4px 8px",
    fontSize: 13,
    fontWeight: 700,
    fontFamily: "inherit",
  },
  rankPerms: { display: "grid", gridTemplateColumns: "repeat(4, 1fr)", gap: 4 },
  permCheck: {
    display: "flex",
    alignItems: "center",
    gap: 5,
    fontSize: 11,
    color: "#cbb38a",
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(0, 0, 0, 0.25)",
    padding: "3px 6px",
    cursor: "pointer",
  },
  permCheckOn: { color: "#f0d69b", borderColor: "rgba(214, 180, 110, 0.6)", background: "rgba(52, 32, 18, 0.6)" },
  membersLayout: { flex: 1, display: "flex", gap: 10, minHeight: 0 },
  memberList: {
    flex: "1 1 60%",
    display: "flex",
    flexDirection: "column",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.45)",
    minHeight: 0,
  },
  memberListHead: {
    display: "flex",
    padding: "4px 8px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.32)",
    fontSize: 10,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  memberRows: { flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" },
  memberRow: {
    display: "flex",
    padding: "4px 8px",
    border: "1px solid transparent",
    background: "transparent",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
    fontSize: 12,
  },
  memberRowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  memberRowSelf: { color: "#f8e6bb", fontWeight: 700 },
  memberColName: { flex: "1 1 auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  memberColRank: { flex: "0 0 110px" },
  memberColState: { flex: "0 0 70px", textAlign: "right" },
  memberDetail: {
    flex: "1 1 40%",
    display: "flex",
    flexDirection: "column",
    gap: 6,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "8px 10px",
  },
  memberDetailName: { color: "#f8e6bb", fontSize: 13, fontWeight: 700, marginBottom: 2 },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  inviteRow: { display: "flex", gap: 6 },
  input: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "4px 8px",
    fontSize: 12,
  },
  noticeTextarea: {
    width: "100%",
    height: 96,
    resize: "none",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "6px 8px",
    fontSize: 12,
    lineHeight: 1.35,
    fontFamily: "inherit",
  },
  chatLog: {
    flex: 1,
    minHeight: 80,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.55)",
    padding: "6px 8px",
    fontSize: 12,
    lineHeight: 1.4,
    color: "#d6c6a5",
  },
  chatLine: { marginBottom: 2 },
  actions: { display: "flex", gap: 6 },
  actionButton: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 16px",
    fontSize: 12,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
