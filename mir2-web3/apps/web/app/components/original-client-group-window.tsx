"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single party member row. */
export type GroupMember = {
  name: string;
  /** True for the leader (first member in the Crystal group payload). */
  leader?: boolean;
  online?: boolean;
  level?: number;
  classKey?: "warrior" | "wizard" | "taoist" | "assassin" | "archer";
  hp?: number;
  maxHp?: number;
};

/**
 * Mirrors the stage-5 `group` slice (`{ members?: string[]; lootMode?: string }`)
 * but accepts richer member records when the host can provide them.
 */
export type GroupSummary = {
  members?: Array<string | GroupMember>;
  /** "group" => shared loot, "solo"/anything else => personal loot. */
  lootMode?: string;
  /** Whether the viewer currently allows incoming group invites. */
  allowInvites?: boolean;
};

export type GroupWindowProps = {
  t: TranslateFn;
  group: GroupSummary | null;
  /** Viewer name, used to highlight their own roster row + gate leader actions. */
  playerName?: string | null;
  onInviteMember?: (name: string) => void;
  onKickMember?: (name: string) => void;
  onLeaveGroup?: () => void;
  onToggleLootMode?: () => void;
  onToggleAllowInvites?: (allow: boolean) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.character;

const CLASS_ICONS = FRAME.classIcons;

export function GroupWindow({
  t,
  group,
  playerName,
  onInviteMember,
  onKickMember,
  onLeaveGroup,
  onToggleLootMode,
  onToggleAllowInvites,
  onClose,
}: GroupWindowProps) {
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [inviteName, setInviteName] = useState("");

  const members = useMemo(() => normalizeMembers(group?.members), [group?.members]);
  const onlineCount = members.filter((member) => member.online).length;
  const leader = members.find((member) => member.leader) ?? members[0] ?? null;
  const viewerIsLeader =
    playerName != null && leader != null && leader.name === playerName;
  const sharedLoot = (group?.lootMode ?? "").toLowerCase() === "group";
  const allowInvites = group?.allowInvites ?? true;

  const selected = useMemo(() => {
    if (selectedName) {
      const match = members.find((member) => member.name === selectedName);
      if (match) return match;
    }
    return members[0] ?? null;
  }, [members, selectedName]);

  useEffect(() => {
    if (selectedName && !members.some((member) => member.name === selectedName)) {
      setSelectedName(null);
    }
  }, [members, selectedName]);

  const hasGroup = members.length > 0;

  return (
    <section
      aria-label={t("ui.group", [], "Party")}
      data-group-size={members.length}
      data-group-loot={sharedLoot ? "group" : "solo"}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.page} src={FRAME.pages.char} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.group", [], "Party")}</div>
      <div style={style.subtitle}>
        {t("ui.groupMemberCount", [members.length, onlineCount], `${members.length} members · ${onlineCount} online`)}
      </div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.lootRow}>
        <span style={style.lootLabel}>{t("ui.groupLoot", [], "Loot")}</span>
        <button
          type="button"
          disabled={!onToggleLootMode || !viewerIsLeader}
          aria-pressed={sharedLoot}
          onClick={() => onToggleLootMode?.()}
          style={{ ...style.lootButton, ...(!onToggleLootMode || !viewerIsLeader ? style.actionButtonDisabled : null) }}
        >
          {sharedLoot ? t("ui.groupLootShared", [], "Shared") : t("ui.groupLootPersonal", [], "Personal")}
        </button>
        <button
          type="button"
          disabled={!onToggleAllowInvites}
          aria-pressed={allowInvites}
          onClick={() => onToggleAllowInvites?.(!allowInvites)}
          style={{ ...style.lootButton, ...(!onToggleAllowInvites ? style.actionButtonDisabled : null) }}
        >
          {allowInvites ? t("ui.groupAllowOn", [], "Invites On") : t("ui.groupAllowOff", [], "Invites Off")}
        </button>
      </div>

      <div style={style.list} aria-label={t("ui.group", [], "Party")}>
        {!hasGroup ? (
          <div style={style.empty}>{t("ui.groupNone", [], "You are not in a party.")}</div>
        ) : (
          members.map((member) => {
            const isSelected = selected?.name === member.name;
            const isSelf = playerName != null && member.name === playerName;
            const icon = member.classKey ? CLASS_ICONS[member.classKey] : undefined;
            return (
              <button
                key={member.name}
                type="button"
                data-member-name={member.name}
                aria-pressed={isSelected}
                onClick={() => setSelectedName(member.name)}
                style={{
                  ...style.row,
                  ...(isSelected ? style.rowSelected : null),
                  ...(isSelf ? style.rowSelf : null),
                }}
              >
                {icon ? (
                  <img style={style.rowIcon} src={icon} alt="" draggable={false} />
                ) : (
                  <span style={style.rowIconFallback} aria-hidden="true" />
                )}
                <span style={style.rowName}>
                  {member.name}
                  {member.leader ? <span style={style.leaderTag}>{t("ui.groupLeader", [], "Leader")}</span> : null}
                </span>
                <span style={{ ...style.rowState, color: member.online ? "#8be07a" : "#9c8d6f" }}>
                  {member.online ? t("ui.guildStateOnline", [], "Online") : t("ui.guildStateOffline", [], "Offline")}
                </span>
              </button>
            );
          })
        )}
      </div>

      <div style={style.detail} data-member-detail={selected?.name ?? ""}>
        {selected ? (
          <>
            <div style={style.detailName}>
              {selected.name}
              {selected.leader ? <span style={style.leaderTag}>{t("ui.groupLeader", [], "Leader")}</span> : null}
            </div>
            <div style={style.detailMeta}>
              {selected.level ? t("ui.heroLevel", [selected.level], `Level ${selected.level}`) : t("ui.group", [], "Party")}
              {selected.classKey ? ` · ${classLabel(t, selected.classKey)}` : ""}
            </div>
            {selected.maxHp ? (
              <div style={style.gauge}>
                <div style={style.gaugeHead}>
                  <span style={style.gaugeLabel}>{t("ui.hp", [], "HP")}</span>
                  <span style={style.gaugeValue}>{`${selected.hp ?? 0}/${selected.maxHp}`}</span>
                </div>
                <div
                  style={style.gaugeTrack}
                  role="progressbar"
                  aria-valuenow={selected.hp ?? 0}
                  aria-valuemax={selected.maxHp}
                >
                  <span
                    style={{
                      ...style.gaugeFill,
                      width: `${Math.min(100, Math.max(0, ((selected.hp ?? 0) / Math.max(selected.maxHp, 1)) * 100))}%`,
                    }}
                  />
                </div>
              </div>
            ) : null}
            <button
              type="button"
              disabled={
                !onKickMember ||
                !viewerIsLeader ||
                Boolean(selected.leader) ||
                (playerName != null && selected.name === playerName)
              }
              style={{
                ...style.actionButton,
                ...(!onKickMember || !viewerIsLeader || selected.leader || (playerName != null && selected.name === playerName)
                  ? style.actionButtonDisabled
                  : null),
              }}
              onClick={() => onKickMember?.(selected.name)}
            >
              {t("ui.groupKick", [], "Remove")}
            </button>
          </>
        ) : (
          <div style={style.empty}>{t("ui.groupSelectHint", [], "Select a member.")}</div>
        )}
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
          placeholder={t("ui.groupInvitePlaceholder", [], "Player name")}
          aria-label={t("ui.groupInvite", [], "Invite")}
          autoComplete="off"
          spellCheck={false}
        />
        <button
          type="submit"
          disabled={!onInviteMember || inviteName.trim().length === 0}
          style={{
            ...style.actionButton,
            flex: "0 0 78px",
            ...(!onInviteMember || inviteName.trim().length === 0 ? style.actionButtonDisabled : null),
          }}
        >
          {t("ui.groupInvite", [], "Invite")}
        </button>
      </form>

      <div style={style.footer}>
        <button
          type="button"
          disabled={!onLeaveGroup || !hasGroup}
          style={{ ...style.actionButton, flex: 1, ...(!onLeaveGroup || !hasGroup ? style.actionButtonDisabled : null) }}
          onClick={() => onLeaveGroup?.()}
        >
          {t("ui.groupLeave", [], "Leave Party")}
        </button>
      </div>
    </section>
  );
}

function classLabel(t: TranslateFn, classKey: NonNullable<GroupMember["classKey"]>) {
  switch (classKey) {
    case "wizard":
      return t("ui.classWizard", [], "Wizard");
    case "taoist":
      return t("ui.classTaoist", [], "Taoist");
    case "assassin":
      return t("ui.classAssassin", [], "Assassin");
    case "archer":
      return t("ui.classArcher", [], "Archer");
    case "warrior":
    default:
      return t("ui.classWarrior", [], "Warrior");
  }
}

function normalizeMembers(members?: Array<string | GroupMember>): GroupMember[] {
  if (!members) return [];
  return members.map((member, index) =>
    typeof member === "string" ? { name: member, leader: index === 0 } : member,
  );
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 120,
    top: 130,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 30,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  page: { position: "absolute", left: 8, top: 90, pointerEvents: "none", opacity: 0.25 },
  titleText: {
    position: "absolute",
    left: 16,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 16, top: 24, fontSize: 10, color: "#cbb38a" },
  close: { position: "absolute", left: 238, top: 4 },
  lootRow: {
    position: "absolute",
    left: 12,
    top: 42,
    width: 240,
    display: "flex",
    alignItems: "center",
    gap: 5,
  },
  lootLabel: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5, flex: "0 0 auto" },
  lootButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "3px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  list: {
    position: "absolute",
    left: 12,
    top: 66,
    width: 240,
    height: 170,
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: 3,
  },
  empty: { color: "#cbb38a", padding: "10px 4px", fontSize: 11 },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    width: "100%",
    height: 24,
    padding: "0 5px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  rowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  rowSelf: { color: "#f8e6bb" },
  rowIcon: { width: 18, height: 18, imageRendering: "pixelated", flex: "0 0 auto" },
  rowIconFallback: {
    width: 18,
    height: 18,
    flex: "0 0 auto",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    background: "rgba(0, 0, 0, 0.4)",
  },
  rowName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", display: "flex", alignItems: "center", gap: 5 },
  leaderTag: { fontSize: 9, color: "#f0d69b", border: "1px solid rgba(214, 180, 110, 0.6)", padding: "0 4px", borderRadius: 2 },
  rowState: { fontSize: 9, flex: "0 0 auto" },
  detail: {
    position: "absolute",
    left: 12,
    top: 240,
    width: 240,
    height: 86,
    display: "flex",
    flexDirection: "column",
    gap: 4,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
    overflow: "hidden",
  },
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, display: "flex", alignItems: "center", gap: 6 },
  detailMeta: { fontSize: 10, color: "#cbb38a" },
  gauge: { display: "flex", flexDirection: "column", gap: 2 },
  gaugeHead: { display: "flex", justifyContent: "space-between", fontSize: 10, color: "#cbb38a" },
  gaugeLabel: { color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  gaugeValue: { color: "#e3d3af" },
  gaugeTrack: {
    position: "relative",
    height: 7,
    background: "rgba(0, 0, 0, 0.55)",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    overflow: "hidden",
  },
  gaugeFill: { position: "absolute", left: 0, top: 0, bottom: 0, display: "block", background: "#d8552f" },
  inviteRow: { position: "absolute", left: 12, top: 332, width: 240, display: "flex", gap: 6 },
  input: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(11, 8, 5, 0.7)",
    color: "#f0eee8",
    padding: "4px 8px",
    fontSize: 11,
  },
  footer: { position: "absolute", left: 12, top: 360, width: 240, display: "flex", gap: 6 },
  actionButton: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "4px 10px",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
