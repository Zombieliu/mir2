"use client";

import { useEffect, useId, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";

import { R16_GUILD_LAYOUT as G } from "../../lib/r16-guild-layout";
import { GuildConfirmation, GuildDropdown, GuildSourceButton, guildRect } from "./original-client-guild-controls";
import { currentSocialItem, sameSocialItem, SOCIAL_GUILD_SLOT_COUNT, validSocialItem, validSocialItemSlot, type SocialItemSlot } from "../../lib/social-item-window-model";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { SpriteButton } from "./original-client-overlays";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";
import { guildRankPermissionCommand, guildRankRenameCommand } from "../../lib/social-parity-actions";
import type { GuildStatusPresentation } from "../../lib/guild-status-presentation";
import { captureGuildBuffAction, guildBuffActionCurrent, guildBuffRows, guildBuffStatLabel,
  type GuildBuffActionDto, type GuildBuffActionPlan, type GuildBuffDefinition, type GuildBuffPending,
  type GuildBuffRow, type GuildBuffSource } from "../../lib/guild-buff-ui";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type GuildMember = {
  name: string;
  rank?: string;
  rankIndex?: number;
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

/** Host-owned Native rename and option cooldowns; the window never starts timers. */
export type GuildRankEditPending = Readonly<{
  editableRankIndices?: readonly number[];
  renameRankIndices: readonly number[];
  permission: boolean;
  create?: boolean;
}>;

/** A single slot in the guild storage grid (item present or empty). */
export type GuildStorageItem = {
  /** Slot index within the storage grid. */
  slot: number;
  uniqueId?: number;
  icon?: number;
  name?: string;
  /** Optional icon URL when the host can resolve item art. */
  iconUrl?: string;
  count?: number;
  itemIndex?: number;
  /** Received snake UserItem, when available; never synthesized from display fields. */
  userItem?: Readonly<Record<string, unknown>>;
  /** Tooltip / grade hint shown on hover. */
  hint?: string;
};
export type GuildStorageTooltipItem = Readonly<SocialItemSlot & {
  itemIndex: number;
  userItem?: Readonly<Record<string, unknown>>;
}>;

/**
 * Compatible with the `stage5Systems.guild` payload shape
 * (`{ name, members, rank, permissions, chatLog }`) but accepts richer member
 * objects when the caller can provide them.
 */
export type GuildSummary = GuildStatusPresentation & {
  name?: string;
  /** Viewer's own rank within the guild. */
  rank?: string;
  notice?: string;
  /** Members may be plain names (legacy payload) or detailed records. */
  members?: Array<string | GuildMember>;
  permissions?: string[];
  chatLog?: string[];
  // --- Additive, optional richer fields (rendered gracefully when absent) ---
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
  incomingInvite?: Readonly<{ name: string; epoch: number }> | null;
  onReplyInvite?: (epoch: number, acceptInvite: boolean) => void;
  /** Viewer name, used to highlight the player's own roster row. */
  playerName?: string | null;
  viewerRankIndex?: number;
  guildScope?: object;
  onEditNotice?: (notice: string) => void;
  onInviteMember?: (name: string) => void;
  onKickMember?: (name: string) => void;
  onSendGuildChat?: (message: string) => void;
  // --- Additive, optional action callbacks (wired later in page.tsx) ---
  /** Change a member's rank by rank index. */
  onChangeMemberRank?: (name: string, rankIndex: number) => void;
  /** Save only the exact rank-name draft. False preserves the draft for retry. */
  onRenameRank?: (rankIndex: number, name: string) => boolean;
  /** Submit one checkbox immediately; checked state remains server-owned. */
  onToggleRankPermission?: (rankIndex: number, key: GuildPermissionKey, enabled: boolean) => boolean;
  rankEditPending?: GuildRankEditPending;
  onRefreshRanks?: () => void;
  onCreateRank?: () => boolean;
  /** Deposit gold into guild storage. */
  onDepositGold?: (amount: number) => void;
  /** Retrieve gold from guild storage. */
  onWithdrawGold?: (amount: number) => void;
  inventoryItems?: readonly SocialItemSlot[];
  emptyInventorySlots?: readonly number[];
  itemsReady?: boolean;
  itemActionPending?: boolean;
  itemSourceKey?: string;
  canStoreItem?: boolean;
  canRetrieveItem?: boolean;
  onDepositItem?: (from: number, to: number, uniqueId: number) => void;
  onRetrieveItem?: (from: number, to: number, uniqueId: number) => void;
  onMoveItem?: (from: number, to: number, uniqueId: number) => void;
  onRefreshStorage?: () => void;
  onReadStorageItemTooltip?: (item: GuildStorageTooltipItem) => CrystalTooltipDocument | null;
  buffSource?: GuildBuffSource | null;
  buffPending?: GuildBuffPending | null;
  onBuffAction?: (dto: GuildBuffActionDto) => void;
  onClose: () => void;
};

type GuildTab = "members" | "storage" | "ranks" | "notice";

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
  CanChangeRank: { key: "game.final.02", fallback: "Edit ranks" },
  CanRecruit: { key: "game.final.03", fallback: "Recruit member" },
  CanKick: { key: "game.final.04", fallback: "Kick member" },
  CanStoreItem: { key: "game.final.05", fallback: "Store item" },
  CanRetrieveItem: { key: "game.final.06", fallback: "Retrieve item" },
  CanAlterAlliance: { key: "game.final.07", fallback: "Alter alliance" },
  CanChangeNotice: { key: "game.final.08", fallback: "Change notice" },
  CanActivateBuff: { key: "game.final.09", fallback: "Activate Buff" },
};

const FRAME = { width: G.window.width, height: G.window.height, frame: G.assets.background,
  closeButton: { base: G.assets.close.normal, hover: G.assets.close.hover, pressed: G.assets.close.active } };
const TAB_LABELS = {
  notice: { key: "game.114", english: "Notice" }, members: { key: "game.110", english: "Members" },
  storage: { key: "game.063", english: "Storage" }, ranks: { key: "game.115", english: "Ranks" },
  status: { key: "game.guild.stats", english: "Stats" }, buff: { key: "game.complete.022", english: "Buffs" },
} as const;

export function GuildWindow({
  t,
  guild,
  incomingInvite,
  onReplyInvite,
  playerName,
  viewerRankIndex,
  guildScope,
  onEditNotice,
  onInviteMember,
  onKickMember,
  onSendGuildChat,
  onChangeMemberRank,
  onRenameRank,
  onToggleRankPermission,
  rankEditPending,
  onRefreshRanks,
  onCreateRank,
  onDepositGold,
  onWithdrawGold,
  inventoryItems,
  emptyInventorySlots,
  itemsReady,
  itemActionPending,
  itemSourceKey,
  canStoreItem,
  canRetrieveItem,
  onDepositItem,
  onRetrieveItem,
  onMoveItem,
  onRefreshStorage,
  onReadStorageItemTooltip,
  buffSource,
  buffPending,
  onBuffAction,
  onClose,
}: GuildWindowProps) {
  const [tab, setTab] = useState<GuildTab>("notice");
  const [rightTab, setRightTab] = useState<"status" | "buff">("status");
  const [windowPosition, setWindowPosition] = useState<{ left: number; top: number }>({ left: G.window.left, top: G.window.top });
  const windowRef = useRef<HTMLElement>(null);
  const windowDrag = useRef<{ pointer: number; x: number; y: number; left: number; top: number; scale: number } | null>(null);
  const [selectedRankIndex, setSelectedRankIndex] = useState<{ index: number; structure: string } | null>(null);
  const [createRank, setCreateRank] = useState(false);
  const [noticeEditing, setNoticeEditing] = useState(false);
  const [memberAction, setMemberAction] = useState<{ kind: "kick" | "rank"; name: string; rank?: number; rankName?: string; beforeRank?: number; scope?: object } | null>(null);
  const recruitInputId = useId();
  useEffect(() => { setSelectedRankIndex(null); setCreateRank(false); setMemberAction(null); setTab("notice"); setRightTab("status"); setNoticeEditing(false); }, [guild?.name, playerName, guildScope]);
  const refreshRanksRef = useRef(onRefreshRanks);
  refreshRanksRef.current = onRefreshRanks;
  useEffect(() => {
    if (tab === "members" || tab === "ranks") refreshRanksRef.current?.();
  },[tab,guild?.name]);
  const [inviteName, setInviteName] = useState("");
  const [noticeDraft, setNoticeDraft] = useState(guild?.notice ?? "");
  const [showOffline, setShowOffline] = useState(true);
  const [goldDraft, setGoldDraft] = useState("");
  const [itemSelection, setItemSelection] = useState<{ source: "bag" | "storage"; item: SocialItemSlot; key: string } | null>(null);
  const [activeStorageTooltip, setActiveStorageTooltip] = useState<GuildStorageItem | null>(null);
  const [buffStart, setBuffStart] = useState(0);
  const [buffConfirmation, setBuffConfirmation] = useState<{ dto: GuildBuffActionDto; definition: GuildBuffDefinition; plan: GuildBuffActionPlan } | null>(null);
  const buffDialogRef = useRef<HTMLDivElement>(null);
  const buffReturnFocus = useRef<HTMLElement | null>(null);
  const currentBuffSource = buffSource && buffSource.guildName === guild?.name ? buffSource : null;
  const buffRows = currentBuffSource ? guildBuffRows(currentBuffSource) : [];
  const maxBuffStart = Math.max(0, buffRows.length - 8);
  const visibleBuffStart = Math.min(buffStart, maxBuffStart);
  const buffConfirmCurrent = !!buffConfirmation && !!currentBuffSource
    && guildBuffActionCurrent(buffConfirmation.dto, currentBuffSource) && !buffPending && !!onBuffAction;
  useEffect(() => { setBuffStart(0); setBuffConfirmation(null); }, [guild?.name]);
  useEffect(() => {
    if (buffConfirmation) {
      buffReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      buffDialogRef.current?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
    } else { buffReturnFocus.current?.focus(); buffReturnFocus.current = null; }
  }, [!!buffConfirmation]);
  const requestBuffList = () => {
    if (!currentBuffSource || buffPending || !onBuffAction) return;
    const dto = captureGuildBuffAction(currentBuffSource, 0, 0); if (dto) onBuffAction(dto);
  };
  const beginBuff = (buff: GuildBuffRow) => {
    if (!currentBuffSource || buffPending || !onBuffAction || !buff.plan) return;
    const dto = captureGuildBuffAction(currentBuffSource, buff.plan.wire.action, buff.definition.id);
    if (dto) setBuffConfirmation({ dto, definition: buff.definition, plan: buff.plan });
  };
  const confirmBuff = () => {
    if (!buffConfirmation || !buffConfirmCurrent) return;
    onBuffAction?.(buffConfirmation.dto); setBuffConfirmation(null);
  };
  const onBuffDialogKey = (event: KeyboardEvent<HTMLDivElement>) => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); setBuffConfirmation(null); }
    if (event.key === "Enter" && !(event.target instanceof HTMLButtonElement)) { event.preventDefault(); confirmBuff(); }
    if (event.key === "Tab") {
      const buttons = buffDialogRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
      if (!buttons?.length) { event.preventDefault(); return; }
      const first = buttons[0], last = buttons[buttons.length - 1];
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
      if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }
  };
  const storageSlots = buildStorageSlots(guild?.storage, guild?.storageSize);
  const storageItems = storageSlots.flatMap((row, slot) => {
    const item = row && { slot, uniqueId: row.uniqueId, name: row.name, icon: row.icon, count: row.count };
    return item && validSocialItem(item, SOCIAL_GUILD_SLOT_COUNT) ? [item] : [];
  });
  const bagItems = inventoryItems ?? [], emptySlots = emptyInventorySlots ?? [];
  const storageTooltipReady = Array.isArray(guild?.storage)
    && guild.storage.every(row => validSocialItem(row, SOCIAL_GUILD_SLOT_COUNT))
    && guild.storage.length === storageItems.length
    && new Set(guild.storage.map(row => row.slot)).size === guild.storage.length
    && new Set(storageItems.map(row => row.uniqueId)).size === storageItems.length;
  const itemEditing = itemsReady === true && itemActionPending !== true && Boolean(itemSourceKey)
    && inventoryItems !== undefined && emptyInventorySlots !== undefined
    && guild?.storageSize === SOCIAL_GUILD_SLOT_COUNT && Array.isArray(guild.storage)
    && guild.storage.every(row => validSocialItem(row, SOCIAL_GUILD_SLOT_COUNT))
    && guild.storage.length === storageItems.length
    && new Set(storageItems.map(item => item.uniqueId)).size === storageItems.length
    && bagItems.every(item => validSocialItem(item, 256))
    && new Set(bagItems.map(item => item.slot)).size === bagItems.length
    && new Set(bagItems.map(item => item.uniqueId)).size === bagItems.length
    && !storageItems.some(item => bagItems.some(bag => bag.uniqueId === item.uniqueId))
    && emptySlots.every(slot => validSocialItemSlot(slot, 256) && !bagItems.some(item => item.slot === slot))
    && new Set(emptySlots).size === emptySlots.length;
  const selectedItem = itemSelection;
  const selection = itemEditing && selectedItem !== null && selectedItem.key === itemSourceKey
    && currentSocialItem(selectedItem.source === "bag" ? bagItems : storageItems, selectedItem.item)
    ? selectedItem : null;
  useEffect(() => { setItemSelection(null); setActiveStorageTooltip(null); }, [guild?.name, itemSourceKey, itemActionPending, itemsReady, tab]);
  useEffect(() => { if (itemSelection && !selection) setItemSelection(null); }, [itemSelection, selection]);
  const selectStorageSlot = (slot: number) => {
    if (!itemEditing || !validSocialItemSlot(slot, SOCIAL_GUILD_SLOT_COUNT)) return;
    const item = storageItems.find(row => row.slot === slot);
    if (item) {
      if (canRetrieveItem === true) setItemSelection({ source: "storage", item, key: itemSourceKey! });
      return;
    }
    if (selection?.source === "bag" && canStoreItem === true && onDepositItem) {
      onDepositItem(selection.item.slot, slot, selection.item.uniqueId); setItemSelection(null);
    } else if (selection?.source === "storage" && canStoreItem === true && canRetrieveItem === true && onMoveItem) {
      onMoveItem(selection.item.slot, slot, selection.item.uniqueId); setItemSelection(null);
    }
  };

  const members = useMemo(() => normalizeMembers(guild?.members), [guild?.members]);
  const ranks = guild?.ranks ?? [];
  const hasPermission = (option: string, alias: string) => guild?.permissions?.some(value => value === option || value === alias) === true;
  const mayEditRanks = hasPermission("CanChangeRank", "changeRank");
  const mayRecruit = hasPermission("CanRecruit", "recruit");
  const mayOpenStorage = hasPermission("CanStoreItem", "store") || hasPermission("CanRetrieveItem", "retrieve");
  const mayKick = hasPermission("CanKick", "kick");
  const mayEditNotice = hasPermission("CanChangeNotice", "notice");
  const ownRankIndex = viewerRankIndex;
  const memberMayBeManaged = (member: GuildMember) => ownRankIndex !== undefined && member.rankIndex !== undefined
    && ranks.some(rank => rank.index === member.rankIndex && rank.index >= ownRankIndex);
  const rankStructure = ranks.map(rank => rank.index).join(",");
  const selectedRank = (selectedRankIndex?.structure === rankStructure
    ? ranks.find(rank => rank.index === selectedRankIndex.index) : null) ?? ranks[0] ?? null;
  const canCreateRank = mayEditRanks && !!onCreateRank && ranks.length > 0 && ranks.length < 255 && !rankEditPending?.create;
  const rankChoices = [...ranks.map(rank => ({ index: rank.index, name: rank.name })),
    ...(mayEditRanks && onCreateRank && ranks.length > 0 && ranks.length < 255 ? [{ index: -1, name: t("game.guild.newRank", [], "New Rank"), create: true, disabled: !canCreateRank }] : [])];
  const memberActionCurrent = !!memberAction && memberAction.scope === guildScope
    && members.some(member => member.name === memberAction.name && member.rankIndex === memberAction.beforeRank && memberMayBeManaged(member))
    && (memberAction.kind === "kick" ? mayKick && memberAction.name !== playerName && !!onKickMember
      : mayEditRanks && !!onChangeMemberRank && ranks.some(rank => rank.index === memberAction.rank && rank.name === memberAction.rankName && ownRankIndex !== undefined && rank.index >= ownRankIndex));
  const experience = guild?.experience, maxExperience = guild?.maxExperience;
  const guildExperienceRatio = typeof experience === "number" && Number.isSafeInteger(experience) && experience >= 0
    && typeof maxExperience === "number" && Number.isSafeInteger(maxExperience) && maxExperience > 0
    ? Math.max(0, Math.min(1, experience / maxExperience)) : null;
  const visibleMembers = useMemo(
    () => (showOffline ? members : members.filter((member) => member.online)),
    [members, showOffline],
  );


  useEffect(() => {
    setNoticeDraft(guild?.notice ?? "");
  }, [guild?.notice]);

  const invite = incomingInvite && typeof incomingInvite.name === "string"
    && incomingInvite.name.trim().length > 0 && incomingInvite.name.length <= 256
    && !incomingInvite.name.includes("\0") && Number.isSafeInteger(incomingInvite.epoch)
    && incomingInvite.epoch > 0 ? incomingInvite : null;

  if (!guild || !guild.name || invite) {
    return (
      <section aria-label={t("game.111", [], "Guild")} data-ui-interactive="true" style={style.window}>
        <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
        <div style={style.titleText}>{t("game.111", [], "Guild")}</div>
        <div style={style.close}>
          <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
        </div>
        {invite ? (
          <div role="dialog" aria-label={t("ui.guildInvite", [], "Guild invitation")}
            data-guild-invite-epoch={invite.epoch} style={style.incomingInvite}>
            <p style={style.invitePrompt}>
              {t("ui.guildInvitePrompt", [invite.name], `Do you want to join the ${invite.name} guild?`)}
            </p>
            <div style={style.actions}>
              <button type="button" disabled={!onReplyInvite} style={style.inviteReply}
                onClick={() => onReplyInvite?.(invite.epoch, true)}>
                {t("ui.accept", [], "Accept")}
              </button>
              <button type="button" disabled={!onReplyInvite} style={style.inviteReply}
                onClick={() => onReplyInvite?.(invite.epoch, false)}>
                {t("ui.decline", [], "Decline")}
              </button>
            </div>
          </div>
        ) : <div style={style.noGuild}>{t("ui.guildNone", [], "You are not in a guild.")}</div>}
      </section>
    );
  }

  return (
    <section ref={windowRef} aria-label={t("game.111", [], "Guild")} data-ui-interactive="true" data-guild-name={guild.name}
      data-guild-tab={tab} data-guild-right-tab={rightTab} style={{ ...style.window, ...windowPosition }}
      onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()}>
      <style>{'[data-guild-storage-tooltip-cell] > .original-item-tooltip { opacity: 1; visibility: visible; transform: translateY(0); }'}</style>
      <img style={style.frame} src={G.assets.background} alt="" draggable={false} />
      <div style={{ ...guildRect(G.dragRect), cursor: "move", touchAction: "none" }} data-guild-drag-handle="true"
        onPointerDown={event => {
          if (event.button !== 0) return;
          event.preventDefault(); event.stopPropagation(); event.currentTarget.setPointerCapture(event.pointerId);
          windowDrag.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, ...windowPosition,
            scale: G.window.width / Math.max(1, windowRef.current!.getBoundingClientRect().width) };
        }} onPointerMove={event => {
          const drag = windowDrag.current; if (!drag || drag.pointer !== event.pointerId) return;
          event.stopPropagation(); setWindowPosition({ left: drag.left + (event.clientX - drag.x) * drag.scale,
            top: drag.top + (event.clientY - drag.y) * drag.scale });
        }} onPointerUp={() => { windowDrag.current = null; }} onLostPointerCapture={() => { windowDrag.current = null; }} />
      <img style={{ ...guildRect(G.titleRect), pointerEvents: "none" }} src={G.assets.title} alt="" draggable={false} />
      {t("game.111", [], "Guild") !== "Guild" ? <span style={style.nativeTitle}>{t("game.111", [], "Guild")}</span> : null}
      <GuildSourceButton art={G.assets.close} rect={G.closeRect} label={t("ui.close", [], "Close")} onClick={onClose} />
      {(["left", "right"] as const).map(side => <div key={side} role="tablist" aria-label={t(side === "left" ? "game.111" : "ui.guildStatus", [], side === "left" ? "Guild" : "Guild status")}>
        {G.tabs.filter(entry => entry.side === side && (entry.key !== "storage" || mayOpenStorage)
          && (entry.key !== "ranks" || mayEditRanks)).map(entry => {
          const labels = TAB_LABELS[entry.key];
          const active = entry.side === "left" ? entry.key === tab : entry.key === rightTab;
          const label = t(labels.key, [], labels.english);
          return <button key={entry.key} type="button" role="tab" aria-label={label} aria-selected={active} data-guild-tab={entry.key}
            onClick={() => {
              if (entry.side === "left") setTab(entry.key as GuildTab);
              else { setRightTab(entry.key as "status" | "buff"); if (entry.key === "buff" && !currentBuffSource?.catalogReady) requestBuffList(); }
            }} style={{ ...guildRect(entry.rect), border: 0, padding: 0, background: "transparent", color: "white", font: "inherit", cursor: "pointer" }}>
            <img src={active ? entry.assets.active : entry.assets.idle} alt="" draggable={false} style={{ width: "100%", height: "100%", pointerEvents: "none" }} />
            {label !== labels.english ? <span style={style.nativeTabCaption}>{label}</span> : null}
          </button>;
        })}
      </div>)}

      {tab === "notice" ? <div style={style.nativeNotice}>
        <textarea aria-label={t("ui.guildNoticeEdit", [], "Edit Notice")} spellCheck={false} maxLength={400}
          style={style.nativeNoticeText} readOnly={!noticeEditing || !onEditNotice || !mayEditNotice} value={noticeEditing ? noticeDraft : (guild.notice ?? "")}
          onChange={event => setNoticeDraft(event.target.value)} />
        {onEditNotice && mayEditNotice ? <div style={{ position: "absolute", left: 5, top: 340 }}>
          <ActionButton label={noticeEditing ? t("ui.guildSaveNotice", [], "Save Notice") : t("ui.edit", [], "Edit")}
            disabled={noticeEditing && noticeDraft === (guild.notice ?? "")}
            onClick={() => noticeEditing ? onEditNotice(noticeDraft) : setNoticeEditing(true)} />
        </div> : null}
      </div> : null}

      {tab === "members" ? <div style={style.nativeMembers}>
        <img src="/original-ui/Prguse/1852.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 324, height: 332, pointerEvents: "none" }} />
        <div style={{ position: "absolute", left: 11, top: 4, color: "#bbb", fontSize: 10.6667 }}>{t("ui.guildRank", [], "Rank")}</div>
        <div style={{ position: "absolute", left: 112, top: 4, color: "#bbb", fontSize: 10.6667 }}>{t("common.dcd1d5223f73", [], "Name")}</div>
        <div style={{ position: "absolute", left: 212, top: 4, color: "#bbb", fontSize: 10.6667 }}>{t("ui.guildOnline", [], "Status")}</div>
        <div style={{ position: "absolute", left: 0, top: 28, width: 324, height: 270, overflowY: "auto", scrollbarWidth: "thin" }}
          onScroll={() => setMemberAction(null)}>
          {visibleMembers.map((member, row) => <div key={member.name} data-member-name={member.name}
            style={{ position: "relative", height: 15, fontSize: 10.6667, lineHeight: "14px", color: member.online ? "#fff" : "#888" }}>
            <select aria-label={`${t("ui.guildRank", [], "Rank")}: ${member.name}`} value={member.rankIndex ?? ""}
              disabled={!onChangeMemberRank || !mayEditRanks || !memberMayBeManaged(member)} style={{ ...style.nativeMemberRank, color: member.online ? "white" : "#888" }}
              onChange={event => {
                const rank = ranks.find(entry => entry.index === Number(event.target.value));
                if (rank) setMemberAction({ kind: "rank", name: member.name, rank: rank.index, rankName: rank.name, beforeRank: member.rankIndex, scope: guildScope });
              }}>
              {!ranks.some(rank => rank.index === member.rankIndex) ? <option value="">{member.rank ?? ""}</option> : null}
              {ranks.map(rank => <option key={rank.index} value={rank.index} disabled={ownRankIndex === undefined || rank.index < ownRankIndex}>{rank.name}</option>)}
            </select>
            <span style={{ position: "absolute", left: 112, width: 84, overflow: "hidden", whiteSpace: "nowrap", background: member.name === playerName ? "#222" : "#0a0a0a" }}>{member.name}</span>
            {onKickMember && mayKick && memberMayBeManaged(member) && member.name !== playerName ? <button type="button" aria-label={`${t("ui.guildKick", [], "Kick")}: ${member.name}`}
              onClick={() => setMemberAction({ kind: "kick", name: member.name, beforeRank: member.rankIndex, scope: guildScope })} style={{ position: "absolute", left: 197, top: 0, width: 10, height: 12,
                border: 0, padding: 0, background: "transparent", cursor: "pointer" }}><img src="/original-ui/Prguse/917.png" alt="" draggable={false} /></button> : null}
            <span style={{ position: "absolute", left: 212, width: 100 }}>{member.online ? t("ui.guildStateOnline", [], "Online") : (member.lastSeen ?? t("ui.guildStateOffline", [], "Offline"))}</span>
          </div>)}
        </div>
        <label style={{ position: "absolute", left: 11, top: 304, fontSize: 10.6667, display: "flex", alignItems: "center", gap: 4 }}>
          <input type="checkbox" checked={showOffline} onChange={event => setShowOffline(event.target.checked)} />{t("ui.guildShowOffline", [], "Show offline members")}
        </label>
      </div> : null}

      {tab === "storage" ? (
        <div style={style.panel}>
          <div style={style.storageGoldRow}>
            <span style={style.storageGoldLabel}>{t("ui.guildGold", [], "Guild Gold")}</span>
            <span style={style.storageGoldValue}>{formatNumber(guild.gold)}</span>
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

          <div style={{ ...style.noticeLabel, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            {t("ui.guildStorageItems", [], "Storage")}
            {onRefreshStorage ? <button type="button" style={style.pickerButton} disabled={itemActionPending === true}
              onClick={onRefreshStorage}>{t("ui.refresh", [], "Refresh")}</button> : null}
          </div>
          <div style={style.storageGrid} aria-label={t("ui.guildStorageItems", [], "Storage")}>
            {storageSlots.map((item, index) => {
              const tooltipItem = storageTooltipReady && item ? guildStorageTooltipItem(item) : null;
              const document = item === activeStorageTooltip && tooltipItem ? onReadStorageItemTooltip?.(tooltipItem) : null;
              const disabled = !itemEditing || (item ? canRetrieveItem !== true : canStoreItem !== true || !selection);
              return <div key={`slot-${index}`} data-guild-storage-tooltip-cell="true" style={{position:"relative"}}
                tabIndex={tooltipItem && disabled ? 0 : undefined} aria-label={tooltipItem && disabled ? item?.name : undefined}
                onMouseEnter={() => setActiveStorageTooltip(item)} onMouseLeave={() => setActiveStorageTooltip(null)}
                onFocus={() => setActiveStorageTooltip(item)} onBlur={() => setActiveStorageTooltip(null)}>
              <button type="button"
                style={{ ...style.storageSlot, padding: 0, width:"100%", ...(selection?.source === "storage" && selection.item.slot === index
                  ? { border: "1px solid #f4dcaf" } : null) }}
                data-storage-slot={index}
                disabled={disabled}
                aria-label={item?.name ?? t("ui.emptyStorageSlot", [index + 1], `Empty storage ${index + 1}`)}
                aria-pressed={selection?.source === "storage" && selection.item.slot === index}
                onClick={() => selectStorageSlot(index)}
                title={document ? undefined : item?.hint ?? item?.name ?? ""}
              >
                {item?.iconUrl ? (
                  <img style={style.storageIcon} src={item.iconUrl} alt={item.name ?? ""} draggable={false} />
                ) : item?.icon !== undefined ? (
                  <img style={style.storageIcon} src={originalItemIconPath(item.icon)} alt="" draggable={false} />
                ) : item?.name ? (
                  <span style={style.storageName}>{item.name}</span>
                ) : null}
                {item && (item.count ?? 0) > 1 ? <span style={style.storageCount}>{item.count}</span> : null}
              </button>
              {document ? <OriginalCrystalItemTooltip document={document} align={index % 10 > 4 ? "left" : "right"} /> : null}
              </div>;
            })}
          </div>
          {inventoryItems && emptyInventorySlots ? <div style={style.itemPicker} aria-label={t("ui.guildBagItems", [], "Guild storage bag items")}>
            <div style={style.sideHint} aria-live="polite">{itemActionPending
              ? t("ui.itemActionPending", [], "Waiting for item update...")
              : !itemEditing ? t("ui.itemDataUnavailable", [], "Item actions unavailable")
              : selection?.source === "storage" ? t("ui.guildRetrieveHint", [], "Choose an empty bag slot to retrieve, or an empty storage slot to move.")
              : t("ui.guildDepositHint", [], "Choose a bag item, then an empty storage slot.")}</div>
            <div style={style.pickerCells}>
              {bagItems.map(item => <button type="button" key={`bag-${item.slot}-${item.uniqueId}`} style={style.pickerButton}
                disabled={!itemEditing || canStoreItem !== true || !onDepositItem}
                aria-pressed={selection?.source === "bag" && sameSocialItem(selection.item, item)}
                title={`${item.name} ×${item.count} (${item.slot + 1})`}
                onClick={() => setItemSelection({ source: "bag", item, key: itemSourceKey! })}>
                {item.icon !== undefined ? <img style={{ width: 20, height: 20, imageRendering: "pixelated" }}
                  src={originalItemIconPath(item.icon)} alt="" draggable={false} /> : null}
                {item.name} ×{item.count}
              </button>)}
              {emptySlots.map(slot => <button type="button" key={`empty-${slot}`} style={style.pickerButton}
                disabled={!itemEditing || canRetrieveItem !== true || selection?.source !== "storage" || !onRetrieveItem}
                onClick={() => { if (itemEditing && canRetrieveItem === true && selection?.source === "storage" && onRetrieveItem) {
                  onRetrieveItem(selection.item.slot, slot, selection.item.uniqueId); setItemSelection(null);
                } }}>
                {t("ui.emptyBagSlot", [slot + 1], `Empty bag ${slot + 1}`)}
              </button>)}
            </div>
          </div> : null}
        </div>
      ) : null}



      {tab === "ranks" ? <div data-guild-ranks-page="true">
        <span style={style.rankEditLabel}>{t("game.guild.editRank", [], "Edit rank")}</span>
        <span style={style.rankSelectLabel}>{t("game.guild.selectRank", [], "Select rank")}</span>
        <GuildDropdown rect={G.ranks.dropdownRect} choices={rankChoices} selected={selectedRank?.index ?? null}
          label={t("game.guild.selectRank", [], "Select rank")} onSelect={choice => choice.create
            ? (canCreateRank && setCreateRank(true)) : setSelectedRankIndex({ index: choice.index, structure: rankStructure })} />
        {selectedRank ? <RankCard key={`${guild.name}:${rankStructure}:${selectedRank.index}`} t={t} rank={selectedRank}
          onRenameRank={mayEditRanks ? onRenameRank : undefined} onToggleRankPermission={mayEditRanks ? onToggleRankPermission : undefined} rankEditPending={rankEditPending} /> : null}
        {rankEditPending?.create ? <span role="status" style={{ position: "absolute", left: 42, top: 132, width: 285, fontSize: 10.6667 }}>
          {t("ui.waiting", [], "Waiting for the server…")}</span> : null}
      </div> : null}

      {rightTab === "status" ? <div data-guild-status="true">
        <img src={G.assets.statusBackground} alt="" draggable={false} style={{ ...guildRect(G.pages.status.backgroundRect), pointerEvents: "none" }} />
        {t("game.complete.048", [], "Guild Statistics") !== "Guild Statistics" ? <span style={style.nativeStatsTitle}>{t("game.complete.048", [], "Guild Statistics")}</span> : null}
        <span style={{ ...guildRect(G.pages.status.headersRect), height: 75, color: "#808080", fontSize: 10.6667, lineHeight: "13px", textAlign: "right", whiteSpace: "pre" }}>
          {[t("common.dcd1d5223f73", [], "Guild Name"), t("game.141", [], "Level"), t("game.110", [], "Members")].join("\n\n")}</span>
        {[{ key: "name", rect: G.pages.status.nameRect, value: guild.name }, { key: "level", rect: G.pages.status.levelRect, value: formatNumber(guild.level) },
          { key: "members", rect: G.pages.status.membersRect, value: `${formatNumber(guild.memberCount, false)}${guild.maxMembers === 0 ? "" : `/${formatNumber(guild.maxMembers, false)}`}` }].map((row, i) => <span key={i} data-guild-stat={row.key}
            style={{ ...guildRect(row.rect), height: 15, fontSize: 10.6667, lineHeight: "13px", overflow: "hidden", whiteSpace: "nowrap" }}>{row.value}</span>)}
        {onInviteMember && mayRecruit ? <form onSubmit={event => { event.preventDefault(); const name = inviteName.trim(); if (name) { onInviteMember(name); setInviteName(""); } }}>
          <label htmlFor={recruitInputId} style={{ ...guildRect(G.pages.status.recruitLabelRect), fontSize: 10.6667 }}>{t("game.109", [], "Recruit Member")}</label>
          <input id={recruitInputId} style={style.nativeRecruitInput} value={inviteName} maxLength={20} autoComplete="off" spellCheck={false}
            aria-label={t("ui.guildInvite", [], "Invite")} onChange={event => setInviteName(event.target.value)} />
          <GuildSourceButton art={G.assets.recruit} rect={G.pages.status.recruitButtonRect} label={t("shell.ok", [], "OK")} english="OK"
            disabled={!inviteName.trim()} onClick={() => { if (inviteName.trim()) { onInviteMember(inviteName.trim()); setInviteName(""); } }} />
        </form> : null}
      </div> : null}
      {rightTab === "buff" ? (
        <div style={{ ...style.buffPanel, overflowY: "auto" }} role="tabpanel" aria-label={t("ui.guildSkills", [], "Guild skills")}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
            <span>{t("game.141", [], "Guild level")}: {currentBuffSource?.level ?? "—"}</span>
            <span>{t("ui.guildSparePoints", [], "Spare points")}: {currentBuffSource?.sparePoints ?? "—"}</span>
            <span>{t("ui.guildGold", [], "Guild gold")}: {currentBuffSource?.gold.toLocaleString() ?? "—"}</span>
            <button type="button" style={style.buffActionButton} disabled={!currentBuffSource || !!buffPending || !onBuffAction} onClick={requestBuffList}>{t("ui.refresh", [], "Refresh")}</button>
          </div>
          {buffPending ? <div role="status">{t("ui.waiting", [], "Waiting for the server…")}</div> : null}
          {!currentBuffSource || !currentBuffSource.catalogReady ? <div role="status">{t("ui.guildBuffUnavailable", [], "Guild skills are unavailable.")}</div> : null}
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}
            onWheel={event => { if (event.deltaY) { event.preventDefault(); event.stopPropagation(); setBuffStart(Math.max(0, Math.min(maxBuffStart, visibleBuffStart + (event.deltaY > 0 ? 1 : -1)))); } }}>
            {buffRows.slice(visibleBuffStart, visibleBuffStart + 8).map(buff => <div key={buff.definition.id} style={{ display: "flex", alignItems: "flex-start", gap: 4 }}>
              <button type="button"
              data-guild-buff-id={buff.definition.id} data-guild-buff-action={buff.plan?.wire.action}
              title={guildBuffHint(t, buff)} disabled={!buff.plan || !!buffPending || !onBuffAction}
              onClick={() => beginBuff(buff)} style={{ ...style.buffActionButton, flex: 1, display: "flex", alignItems: "center", gap: 8, textAlign: "left" }}>
              {buff.icon >= 0 && buff.icon < 48 ? <img src={`/original-ui/GuildSkill/${buff.icon}.png`} alt="" width={36} height={34} draggable={false} style={{ imageRendering: "pixelated" }} /> : null}
              <span style={{ flex: 1 }}><strong>{buff.definition.name}</strong><br />
                <span>{guildBuffStatus(t, buff)}{buff.state ? ` · ${buff.state.active ? t("ui.active", [], "Active") : t("ui.inactive", [], "Inactive")}` : ""}</span>
                {buff.error && buff.error !== "active" ? <span style={{ display: "block", color: "#e4a284" }}>{guildBuffError(t, buff.error)}</span> : null}
              </span>
              <span>{buff.state ? t("ui.activate", [], "Activate") : t("ui.acquire", [], "Acquire")}</span>
              </button>
              <details style={{ maxWidth: 220 }}>
                <summary style={{ ...style.buffActionButton, display: "flex", alignItems: "center", boxSizing: "border-box" }}>{t("ui.details", [], "Details")}</summary>
                <pre style={{ whiteSpace: "pre-wrap", font: "inherit", margin: "4px 0", color: "#f0d69b" }}>{guildBuffHint(t, buff)}</pre>
              </details>
            </div>)}
          </div>
          <div style={{ display: "flex", gap: 4 }}>
            <button type="button" style={style.buffActionButton} disabled={visibleBuffStart === 0} onClick={() => setBuffStart(Math.max(0, visibleBuffStart - 1))}>{t("ui.previous", [], "Previous")}</button>
            <button type="button" style={style.buffActionButton} disabled={visibleBuffStart >= maxBuffStart} onClick={() => setBuffStart(Math.min(maxBuffStart, visibleBuffStart + 1))}>{t("ui.next", [], "Next")}</button>
          </div>
        </div>
      ) : null}

      <>
        <img src={G.assets.progressBackground} alt="" draggable={false} style={guildRect(G.pages.status.progressRect)} />
        {guildExperienceRatio !== null ? <div style={{ ...guildRect(G.pages.status.progressRect), width: G.pages.status.progressFillWidth * guildExperienceRatio, overflow: "hidden", pointerEvents: "none" }}>
          <img src={G.assets.progress} alt="" draggable={false} style={{ width: G.pages.status.progressRect.width, height: G.pages.status.progressRect.height, maxWidth: "none" }} /></div> : null}
        <span data-guild-stat="experience" style={{ ...guildRect(G.pages.status.progressTextRect), textAlign: "center", fontSize: 10.6667, lineHeight: "15px", pointerEvents: "none" }}>{guildExperienceRatio === null ? "—" : `${Number((guildExperienceRatio * 100).toFixed(2))}%`}</span>
      </>
      {createRank ? <GuildConfirmation title={t("game.guild.createRankConfirm", [], "Create new rank?")}
        confirm={t("common.eebdd24a77d9", [], "Confirm")} cancel={t("common.19766ed6ccb2", [], "Cancel")} disabled={!canCreateRank}
        onCancel={() => setCreateRank(false)} onConfirm={() => { if (canCreateRank && onCreateRank?.()) setCreateRank(false); }} /> : null}
      {memberAction ? <GuildConfirmation title={memberAction.kind === "kick" ? `${t("ui.guildKick", [], "Kick")}: ${memberAction.name}?`
        : `${memberAction.name} → ${memberAction.rankName}?`} confirm={t("common.eebdd24a77d9", [], "Confirm")} cancel={t("common.19766ed6ccb2", [], "Cancel")}
        disabled={!memberActionCurrent} onCancel={() => setMemberAction(null)} onConfirm={() => {
          if (!memberActionCurrent) return;
          if (memberAction.kind === "kick") onKickMember?.(memberAction.name);
          else onChangeMemberRank?.(memberAction.name, memberAction.rank!);
          setMemberAction(null);
        }} /> : null}
      {buffConfirmation ? <div style={style.buffModalBackdrop} onPointerDown={event => event.stopPropagation()} onPointerUp={event => event.stopPropagation()}>
        <div ref={buffDialogRef} role="alertdialog" aria-modal="true" aria-label={t("common.eebdd24a77d9", [], "Confirm")} onKeyDown={onBuffDialogKey} style={style.buffModal}>
          <p>{buffConfirmation.dto.wire.action === 1
            ? t("ui.guildBuffAcquireConfirm", [buffConfirmation.definition.name], `Acquire ${buffConfirmation.definition.name}?`)
            : t("ui.guildBuffActivateConfirm", [buffConfirmation.definition.name], `Activate ${buffConfirmation.definition.name}?`)}</p>
          <p>{t("ui.guildSparePoints", [], "Spare points")}: {buffConfirmation.plan.pointsCost} · {t("ui.guildGold", [], "Guild gold")}: {buffConfirmation.plan.goldCost.toLocaleString()}</p>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" style={style.buffActionButton} disabled={!buffConfirmCurrent} onClick={confirmBuff}>{t("common.eebdd24a77d9", [], "Confirm")}</button>
            <button type="button" style={style.buffActionButton} onClick={() => setBuffConfirmation(null)}>{t("common.19766ed6ccb2", [], "Cancel")}</button>
          </div>
        </div>
      </div> : null}

    </section>
  );
}

function guildBuffStatus(t: TranslateFn, buff: GuildBuffRow): string {
  const labels = { insufficientLevel: "Insufficient level", available: "Available", countingDown: "Counting down", expired: "Expired", obtained: "Obtained" };
  const status = t(`ui.guildBuff${buff.status[0].toUpperCase()}${buff.status.slice(1)}`, [], labels[buff.status]);
  return buff.status === "countingDown" ? `${status} · ${t("ui.minutes", [buff.state?.activeTimeRemaining ?? 0], `${buff.state?.activeTimeRemaining ?? 0} minutes`)}` : status;
}
function guildBuffError(t: TranslateFn, error: NonNullable<GuildBuffRow["error"]>): string {
  const labels = { unavailable: "Guild skill information is unavailable.", permission: "Your rank cannot activate guild skills.", level: "Guild level is too low.", points: "Insufficient spare points.", funds: "Insufficient guild gold.", active: "This skill is already active." };
  return t(`ui.guildBuffError${error[0].toUpperCase()}${error.slice(1)}`, [], labels[error]);
}
function guildBuffHint(t: TranslateFn, buff: GuildBuffRow): string {
  const d = buff.definition;
  const lines = [d.name, `${t("game.141", [], "Guild level")}: ${d.levelRequirement}`,
    `${t("ui.guildSparePoints", [], "Spare points")}: ${d.pointsRequirement}`,
    `${t("ui.guildGold", [], "Guild gold")}: ${d.activationCost}`];
  if (d.timeLimit > 0) lines.push(t("ui.minutes", [buff.state?.active ? buff.state.activeTimeRemaining : d.timeLimit], `${buff.state?.active ? buff.state.activeTimeRemaining : d.timeLimit} minutes`));
  for (const stat of d.stats) {
    const label = guildBuffStatLabel(stat.stat);
    lines.push(`${t(`client.${label}`, [], label)} ${stat.value > 0 ? "+" : ""}${stat.value}${label.includes("Percent") ? "%" : ""}`);
  }
  return lines.join("\n");
}

function ActionButton({ label, disabled, onClick, nativeSave }: { label: string; disabled?: boolean; onClick: () => void; nativeSave?: boolean }) {
  if (nativeSave) return <GuildSourceButton art={G.assets.save} rect={{ left: 0, top: 0, width: G.ranks.saveRect.width, height: G.ranks.saveRect.height }}
    label={label} english="Save" disabled={disabled} onClick={onClick} />;
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

/** Name is a local draft. Each permission checkbox reflects only the server rank. */
function RankCard({
  t,
  rank,
  onRenameRank,
  onToggleRankPermission,
  rankEditPending,
}: {
  t: TranslateFn;
  rank: GuildRank;
  onRenameRank?: GuildWindowProps["onRenameRank"];
  onToggleRankPermission?: GuildWindowProps["onToggleRankPermission"];
  rankEditPending?: GuildRankEditPending;
}) {
  const [draft, setDraft] = useState({ value: rank.name, dirty: false });
  useEffect(() => {
    setDraft(current => current.value === rank.name ? { value: rank.name, dirty: false }
      : current.dirty ? current : { value: rank.name, dirty: false });
  }, [rank.name]);
  const name = draft.value;
  const renamePending = rankEditPending?.renameRankIndices.includes(rank.index) === true;
  const authorityDisabled = rankEditPending?.editableRankIndices !== undefined
    && !rankEditPending.editableRankIndices.includes(rank.index);
  const renameDisabled = !onRenameRank || renamePending || authorityDisabled
    || guildRankRenameCommand(rank.index, name) === null;
  return <div data-rank-index={rank.index}>
    <input style={style.rankNameInput} value={name} aria-label={t("ui.guildRankName", [], "Rank Name")}
      onChange={event => setDraft({ value: event.target.value, dirty: true })} spellCheck={false} maxLength={20}
      disabled={!onRenameRank || renamePending || authorityDisabled}
      onKeyDown={event => {
        event.stopPropagation();
        if (event.nativeEvent?.isComposing || event.keyCode === 229) return;
        if (event.key === "Enter") { event.preventDefault(); if (!renameDisabled) onRenameRank?.(rank.index, name); }
        else if (event.key === "Escape") { event.preventDefault(); setDraft({ value: "", dirty: true }); event.currentTarget.blur(); }
      }} />
    {PERMISSION_KEYS.map((key, bit) => {
      const on = rank.permissions?.includes(key) === true;
      const disabled = !onToggleRankPermission || rankEditPending?.permission === true || authorityDisabled
        || guildRankPermissionCommand(rank.index, key, !on) === null;
      const box = G.ranks.permissions[bit];
      return <label key={key} style={{ ...guildRect(box.rect), width: 152, height: 20, fontSize: 10.6667, color: "white", cursor: disabled ? "default" : "pointer" }}>
        <input type="checkbox" checked={on} disabled={disabled} data-guild-rank-permission={key}
          onChange={event => { if (!disabled) onToggleRankPermission?.(rank.index, key, event.target.checked); }}
          style={{ appearance: "none", position: "absolute", left: 0, top: 0, width: on ? box.checkedRect.width : box.rect.width,
            height: box.rect.height, margin: 0, padding: 0, border: 0,
            background: `url(${on ? G.assets.permissionChecked : G.assets.permissionUnchecked}) no-repeat`,
            opacity: disabled ? 0.55 : 1, cursor: disabled ? "default" : "pointer" }} />
        <span style={{ position: "absolute", left: 17, top: -2, width: 135, height: 20, lineHeight: "13px", overflow: "hidden" }}>{permissionLabel(t, key)}</span>
      </label>;
    })}
    <div style={guildRect(G.ranks.saveRect)}><ActionButton label={t("game.complete.004", [], "Save")} nativeSave
      disabled={renameDisabled} onClick={() => { if (!renameDisabled) onRenameRank?.(rank.index, name); }} /></div>
  </div>;
}

/** Localise a permission key, falling back to the raw key for legacy strings. */
function permissionLabel(t: TranslateFn, permission: string): string {
  const entry = PERMISSION_LABELS[permission as GuildPermissionKey];
  return entry ? t(entry.key, [], entry.fallback) : permission;
}

/** Read-only source identity; missing catalogue identity preserves the basic hint. */
function guildStorageTooltipItem(item: GuildStorageItem): GuildStorageTooltipItem | null {
  const itemIndex = item.itemIndex, raw = item.userItem;
  if (!validSocialItem(item, SOCIAL_GUILD_SLOT_COUNT) || typeof itemIndex !== "number"
    || !Number.isSafeInteger(itemIndex) || itemIndex < 0 || itemIndex > 0x7fff_ffff) return null;
  if (raw !== undefined && (!raw || typeof raw !== "object" || Array.isArray(raw)
    || raw.item_index !== itemIndex || raw.unique_id !== item.uniqueId || raw.count !== item.count)) return null;
  return {slot:item.slot,uniqueId:item.uniqueId,name:item.name,icon:item.icon,count:item.count,itemIndex,
    ...(raw !== undefined ? {userItem:raw} : {})};
}

/** Pad the storage payload out to `storageSize` slots so empty cells render. */
function buildStorageSlots(
  storage?: GuildStorageItem[],
  storageSize?: number,
): Array<GuildStorageItem | null> {
  const bySlot = new Map<number, GuildStorageItem>();
  let maxSlot = -1;
  for (const item of storage ?? []) {
    if (!item || !validSocialItemSlot(item.slot, SOCIAL_GUILD_SLOT_COUNT)) continue;
    bySlot.set(item.slot, item);
    if (item.slot > maxSlot) maxSlot = item.slot;
  }
  const size = Math.min(SOCIAL_GUILD_SLOT_COUNT, Math.max(
    Number.isSafeInteger(storageSize) && storageSize! >= 0 ? storageSize! : 0, maxSlot + 1, storage?.length ? 16 : 0));
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

function formatNumber(value: number | undefined, grouping = true) {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
    ? grouping ? value.toLocaleString("en-US") : String(value) : "—";
}

const style: Record<string, CSSProperties> = {
  window: { position: "absolute", ...G.window, zIndex: 32, color: "white", fontSize: 10.6667, fontFamily: "inherit" },
  frame: { position: "absolute", inset: 0, width: G.window.width, height: G.window.height, maxWidth: "none", pointerEvents: "none", imageRendering: "pixelated" },
  titleText: { ...guildRect(G.titleRect), color: "white", fontSize: 10.6667 },
  close: guildRect(G.closeRect),
  nativeTitle: { ...guildRect(G.titleRect), background: "#17120a", color: "white", fontSize: 10.6667, pointerEvents: "none", whiteSpace: "nowrap" },
  nativeTabCaption: { position: "absolute", inset: "3px 3px 4px", pointerEvents: "none", display: "grid", placeItems: "center", fontSize: 10.6667,
    lineHeight: "13px", background: "linear-gradient(#322716,#19160e)", color: "white", overflow: "hidden", whiteSpace: "nowrap" },
  nativeNotice: { position: "absolute", left: 13, top: 61, width: 322, height: 365 },
  nativeNoticeText: { position: "absolute", left: 0, top: 0, width: 322, height: 330, padding: 0, margin: 0, resize: "none", boxSizing: "border-box",
    background: "transparent", border: 0, color: "white", font: "inherit", fontSize: 10.6667, lineHeight: "13px", scrollbarWidth: "thin" },
  nativeMembers: { position: "absolute", left: 13, top: 61, width: 324, height: 332 },
  nativeMemberRank: { position: "absolute", left: 11, top: 0, width: 100, height: 14, font: "inherit", fontSize: 10.6667,
    lineHeight: "14px", margin: 0, padding: "0 12px 0 0", border: "1px solid #303030", background: "#141414", appearance: "none",
    backgroundImage: `url(${G.assets.dropdownArrow.normal})`, backgroundRepeat: "no-repeat", backgroundPosition: "right center", backgroundSize: "10px 10px" },
  nativeRecruitInput: { ...guildRect(G.pages.status.recruitNameRect), minWidth: 0, boxSizing: "border-box", border: "1px solid #232323", padding: "1px 2px",
    color: "white", font: "inherit", fontSize: 10.6667, background: "transparent" },
  nativeStatsTitle: { ...guildRect(G.pages.status.titleRect), color: "#dfcfa9", background: "#080808", fontSize: 10.6667, textAlign: "center", lineHeight: "24px" },
  rankEditLabel: { ...guildRect(G.ranks.editLabelRect), color: "white", fontSize: 10.6667, lineHeight: "13px" },
  rankSelectLabel: { ...guildRect(G.ranks.selectLabelRect), color: "white", fontSize: 10.6667, lineHeight: "13px" },
  rankNameInput: { ...guildRect(G.ranks.nameRect), minWidth: 0, boxSizing: "border-box", border: 0, padding: "0 2px", color: "white",
    background: "#232323", font: "inherit", fontSize: 10.6667, lineHeight: "16px" },
  panel: { position: "absolute", left: 13, top: 61, width: 324, height: 332, display: "flex", flexDirection: "column", gap: 6, overflowY: "auto", scrollbarWidth: "thin" },
  buffPanel: { position: "absolute", left: 360, top: 61, width: 216, height: 332, display: "flex", flexDirection: "column", gap: 4, scrollbarWidth: "thin" },
  noGuild: { position: "absolute", left: 22, top: 90, color: "#cbb38a", fontSize: 12 },
  incomingInvite: { position: "absolute", left: 65, top: 110, width: 460, padding: 16,
    boxSizing: "border-box", border: "1px solid #9c8d6f", background: "#17120a" },
  invitePrompt: { margin: "0 0 16px", whiteSpace: "pre-wrap", overflowWrap: "anywhere", color: "#f4dcaf" },
  inviteReply: { minWidth: 100, minHeight: 25, border: "1px solid #9c8d6f", background: "#24180c", color: "#f4dcaf", fontSize: 11, fontFamily: "inherit", cursor: "pointer" },
  actions: { display: "flex", gap: 8, flexWrap: "wrap" },
  actionButton: { border: "1px solid #9c8d6f", background: "linear-gradient(#49321a,#20170d)", color: "#f4dcaf", padding: "3px 6px", font: "inherit", cursor: "pointer", whiteSpace: "nowrap" },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
  inviteRow: { display: "flex", gap: 4, alignItems: "center", flex: "0 0 auto" },
  input: { flex: 1, minWidth: 0, color: "white", background: "#141414", border: "1px solid #555", font: "inherit", padding: "3px" },
  noticeLabel: { fontSize: 10.6667, color: "#cbb38a" },
  storageGoldRow: { display: "flex", justifyContent: "space-between", alignItems: "center", border: "1px solid #625236", padding: "3px 5px", flex: "0 0 auto" },
  storageGoldLabel: { color: "#cbb38a" },
  storageGoldValue: { color: "#f4dcaf" },
  storageGrid: { display: "grid", gridTemplateColumns: "repeat(10, 1fr)", gap: 2, border: "1px solid #625236", background: "#080808", padding: 3, flex: "0 0 auto" },
  storageSlot: { position: "relative", aspectRatio: "1 / 1", border: "1px solid #625236", background: "#141414", display: "flex", alignItems: "center", justifyContent: "center", overflow: "hidden" },
  storageIcon: { maxWidth: "100%", maxHeight: "100%", imageRendering: "pixelated" },
  storageName: { fontSize: 8, color: "#d6c6a5", textAlign: "center", lineHeight: 1.1, padding: 1, overflow: "hidden" },
  storageCount: { position: "absolute", right: 1, bottom: 0, fontSize: 9, color: "#f8e6bb", textShadow: "1px 1px #000" },
  itemPicker: { flex: "0 0 auto", maxHeight: 100, overflowY: "auto", padding: 4, border: "1px solid #625236", background: "#141414" },
  sideHint: { color: "#cbb38a", fontSize: 10 },
  pickerCells: { display: "flex", flexWrap: "wrap", gap: 4, marginTop: 4 },
  pickerButton: { display: "flex", alignItems: "center", gap: 3, minHeight: 20, maxWidth: 180, border: "1px solid #625236", background: "#24180c", color: "white", fontSize: 10, fontFamily: "inherit" },
  buffActionButton: { minWidth: 24, minHeight: 24, padding: "3px 4px", border: "1px solid #625236", background: "#24180c", color: "white", fontSize: 10.6667, fontFamily: "inherit", cursor: "pointer" },
  buffModalBackdrop: { position: "absolute", inset: 0, zIndex: 15, background: "#000a", display: "grid", placeItems: "center", padding: 18 },
  buffModal: { background: "#24180c", border: "2px solid #9c8d6f", padding: 18, maxWidth: 400 },
};
