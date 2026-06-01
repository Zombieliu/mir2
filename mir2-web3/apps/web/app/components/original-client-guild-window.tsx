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
  onClose: () => void;
};

type GuildTab = "overview" | "members" | "notice";

const FRAME = ORIGINAL_UI.gameShop;

const GUILD_TABS: { key: GuildTab; labelKey: string; fallback: string }[] = [
  { key: "overview", labelKey: "ui.guildOverview", fallback: "Overview" },
  { key: "members", labelKey: "ui.guildMembers", fallback: "Members" },
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
  onClose,
}: GuildWindowProps) {
  const [tab, setTab] = useState<GuildTab>("overview");
  const [selectedMember, setSelectedMember] = useState<string | null>(null);
  const [inviteName, setInviteName] = useState("");
  const [chatDraft, setChatDraft] = useState("");
  const [noticeDraft, setNoticeDraft] = useState(guild?.notice ?? "");

  const members = useMemo(() => normalizeMembers(guild?.members), [guild?.members]);
  const onlineCount = members.filter((member) => member.online).length;
  const selected = useMemo(() => {
    if (selectedMember) {
      const match = members.find((member) => member.name === selectedMember);
      if (match) return match;
    }
    return members[0] ?? null;
  }, [members, selectedMember]);

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
        {t("ui.guildMemberCount", [members.length, onlineCount], `${members.length} members · ${onlineCount} online`)}
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
            <Info label={t("ui.guildMembers", [], "Members")} value={String(members.length)} />
            <Info label={t("ui.guildOnline", [], "Online")} value={String(onlineCount)} />
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
              <div style={style.noticeLabel}>{t("ui.guildPermissions", [], "Permissions")}</div>
              <div style={style.permissions}>
                {guild.permissions.map((permission) => (
                  <span key={permission} style={style.permissionTag}>
                    {permission}
                  </span>
                ))}
              </div>
            </>
          ) : null}
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
                {members.length === 0 ? (
                  <div style={style.empty}>{t("ui.guildNoMembers", [], "No members.")}</div>
                ) : (
                  members.map((member) => {
                    const isSelected = selected?.name === member.name;
                    const isSelf = playerName != null && member.name === playerName;
                    return (
                      <button
                        key={member.name}
                        type="button"
                        data-member-name={member.name}
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
