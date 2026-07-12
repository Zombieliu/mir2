"use client";

import { useEffect, useRef, useState } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { OriginalAudioSettingsControls } from "./original-client-audio-settings";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type ChatFilterKey = "all" | "shout" | "whisper" | "lover" | "mentor" | "group" | "guild";
export type ChatOptionFilterKey =
  | "normal"
  | "whisper"
  | "shout"
  | "system"
  | "lover"
  | "mentor"
  | "group"
  | "guild";

type DisplayLogLineLike = {
  text: string;
  tone: "chat" | "system" | "network";
  channel:
    | "normal"
    | "shout"
    | "trade"
    | "whisper"
    | "group"
    | "guild"
    | "mentor"
    | "relationship"
    | "system"
    | "hint"
    | "server"
    | "line"
    | "announcement"
    | "network";
};

type ItemContainer = "bag1" | "bag2" | "quest" | "belt" | "storage";

type DisplayItemLike = {
  key: string;
  uniqueId: number;
  name: string;
  icon: number;
  slot: number;
  quantity: number;
  container: ItemContainer;
  description?: string;
  durabilityCurrent?: number;
  durabilityMax?: number;
};

type ItemActionRef = Pick<DisplayItemLike, "key" | "uniqueId" | "slot" | "container">;

type EquipmentSlot =
  | "weapon"
  | "armour"
  | "helmet"
  | "mount"
  | "necklace"
  | "torch"
  | "braceletLeft"
  | "braceletRight"
  | "ringLeft"
  | "ringRight"
  | "amulet"
  | "boots"
  | "belt"
  | "stone";

type DisplayEquipmentItemLike = {
  slot: EquipmentSlot;
  durabilityCurrent: number;
  durabilityMax: number;
};

const CHAT_FILTER_BUTTONS: Array<{ key: ChatFilterKey; left: number; labelKey: string }> = [
  { key: "all", left: 12, labelKey: "client.Chat_All" },
  { key: "shout", left: 34, labelKey: "ui.shout" },
  { key: "whisper", left: 56, labelKey: "client.Chat_Whisper" },
  { key: "lover", left: 78, labelKey: "client.Chat_Lover" },
  { key: "mentor", left: 100, labelKey: "client.Chat_Mentor" },
  { key: "group", left: 122, labelKey: "client.Chat_Group" },
  { key: "guild", left: 144, labelKey: "client.Chat_Guild" },
];

export const CHAT_FILTER_PREFIX: Record<ChatFilterKey, string> = {
  all: "",
  shout: "!",
  whisper: "/",
  lover: ":)",
  mentor: "!#",
  group: "!!",
  guild: "!~",
};

const CHAT_OPTION_FILTER_BUTTONS: Array<{
  key: ChatOptionFilterKey;
  labelKey: string;
  fallback: string;
}> = [
  { key: "normal", labelKey: "client.Chat_All", fallback: "General" },
  { key: "whisper", labelKey: "client.Chat_Whisper", fallback: "Whisper" },
  { key: "shout", labelKey: "client.Chat_Short", fallback: "Shout" },
  { key: "system", labelKey: "ui.system", fallback: "System" },
  { key: "lover", labelKey: "client.Chat_Lover", fallback: "Lover" },
  { key: "mentor", labelKey: "client.Chat_Mentor", fallback: "Mentor" },
  { key: "group", labelKey: "client.Chat_Group", fallback: "Group" },
  { key: "guild", labelKey: "client.Chat_Guild", fallback: "Guild" },
];

export function chatPrefixForFilter(filter: ChatFilterKey) {
  return CHAT_FILTER_PREFIX[filter];
}

export function formatChatMessageForFilter(filter: ChatFilterKey, value: string) {
  const prefix = chatPrefixForFilter(filter);
  const message = value.trimEnd();
  if (!message || message === prefix) return "";
  if (!prefix || message.startsWith(prefix)) return message;
  return `${prefix}${message}`;
}

export type ChatFrameProps = {
  t: TranslateFn;
  runtimeMessage: string;
  logs: DisplayLogLineLike[];
  chatMessage: string;
  hints: string[];
  activeFilter: ChatFilterKey;
  hiddenFilters: ChatOptionFilterKey[];
  expanded: boolean;
  showSettings: boolean;
  transparent: boolean;
  onChatMessageChange: (value: string) => void;
  onSendChat: () => void;
  onCloseSettings: () => void;
  onToggleHiddenFilter: (filter: ChatOptionFilterKey) => void;
  onToggleAllHiddenFilters: () => void;
  onToggleTransparent: () => void;
};

export function ChatFrame({
  t,
  logs,
  chatMessage,
  activeFilter,
  hiddenFilters,
  expanded,
  showSettings,
  transparent,
  onChatMessageChange,
  onSendChat,
  onCloseSettings,
  onToggleHiddenFilter,
  onToggleAllHiddenFilters,
  onToggleTransparent,
}: ChatFrameProps) {
  const lines = playerFacingChatLines(logs, hiddenFilters);
  const activePrefix = chatPrefixForFilter(activeFilter);
  const hiddenFilterSet = new Set(hiddenFilters);
  const [scrollOffset, setScrollOffset] = useState(0);
  const previousMaxScrollOffsetRef = useRef(0);
  const previousActiveFilterRef = useRef(activeFilter);
  const previousExpandedRef = useRef(expanded);
  const visibleLineCount = 4;
  const maxScrollOffset = Math.max(lines.length - visibleLineCount, 0);
  const visibleLines = lines.slice(scrollOffset, scrollOffset + visibleLineCount);
  const knobTop = maxScrollOffset === 0 ? 16 : 16 + Math.round((scrollOffset / maxScrollOffset) * 28);
  const chatTextBoxVisible = chatMessage.length > 0;

  useEffect(() => {
    setScrollOffset((current) => {
      const previousMaxScrollOffset = previousMaxScrollOffsetRef.current;
      const filterChanged = previousActiveFilterRef.current !== activeFilter;
      const expandedChanged = previousExpandedRef.current !== expanded;
      previousMaxScrollOffsetRef.current = maxScrollOffset;
      previousActiveFilterRef.current = activeFilter;
      previousExpandedRef.current = expanded;

      if (filterChanged || expandedChanged || current >= previousMaxScrollOffset) {
        return maxScrollOffset;
      }

      return Math.min(current, maxScrollOffset);
    });
  }, [activeFilter, expanded, maxScrollOffset]);

  return (
    <section className={`chat-frame ${expanded ? "" : "collapsed"} ${transparent ? "transparent" : ""}`}>
      <img className="chat-frame-bg" src={ORIGINAL_UI.game.chatDialog} alt="" draggable={false} />
      <div className="chat-scroll-buttons">
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.home} label={t("ui.home")} onClick={() => setScrollOffset(0)} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.up} label={t("ui.up")} onClick={() => setScrollOffset((current) => Math.max(current - 1, 0))} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.down} label={t("ui.down")} onClick={() => setScrollOffset((current) => Math.min(current + 1, maxScrollOffset))} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.end} label={t("ui.end")} onClick={() => setScrollOffset(maxScrollOffset)} />
      </div>
      <img className="chat-count-bar" src={ORIGINAL_UI.game.chatCountBar} alt="" draggable={false} />
      <div className="chat-position-knob" style={{ top: knobTop }}>
        <img src={ORIGINAL_UI.game.chatScrollButtons.knob.base} alt="" draggable={false} />
      </div>
      <div className={`chat-feed ${expanded ? "" : "hidden"}`}>
        {visibleLines.map((line, index) => (
          <div
            key={`chat-line-${activeFilter}-${index}`}
            className={`chat-feed-line ${line.tone === "system" ? "system" : ""} channel-${line.channel}`}
          >
            {line.text}
          </div>
        ))}
      </div>
      {showSettings ? (
        <div className="chat-settings-panel">
          <div className="chat-settings-title">{t("ui.settings")}</div>
          <OriginalAudioSettingsControls t={t} className="chat-audio-settings" />
          <div className="chat-settings-tabs">
            <button type="button" className="active">
              {t("ui.filter", [], "Filter")}
            </button>
            <button type="button" onClick={onToggleTransparent} data-chat-transparent={transparent}>
              {transparent ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
            </button>
          </div>
          <div className="chat-settings-grid">
            <button
              type="button"
              className="chat-settings-option"
              data-chat-option-filter="all"
              data-chat-option-hidden={hiddenFilters.length === CHAT_OPTION_FILTER_BUTTONS.length}
              onClick={onToggleAllHiddenFilters}
            >
              {t("client.Chat_All", [], "All")}
            </button>
            {CHAT_OPTION_FILTER_BUTTONS.map(({ key, labelKey, fallback }) => (
              <button
                key={key}
                type="button"
                className="chat-settings-option"
                data-chat-option-filter={key}
                data-chat-option-hidden={hiddenFilterSet.has(key)}
                onClick={() => onToggleHiddenFilter(key)}
              >
                {t(labelKey, [], fallback)}
              </button>
            ))}
          </div>
          <div className="chat-settings-copy">{`${t("ui.size")}: ${expanded ? t("ui.down") : t("ui.up")}`}</div>
          <button type="button" className="chat-settings-close" onClick={onCloseSettings}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      <input
        className="chat-textbox"
        value={chatMessage}
        data-chat-prefix={activePrefix}
        data-chat-visible={chatTextBoxVisible}
        aria-hidden={!chatTextBoxVisible}
        aria-label={t("ui.worldChatPlaceholder")}
        onChange={(event) => onChatMessageChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            onSendChat();
          }
        }}
      />
    </section>
  );
}

export type ChatFilterBarProps = {
  t: TranslateFn;
  activeFilter: ChatFilterKey;
  chatExpanded: boolean;
  showSettings: boolean;
  onSelectFilter: (filter: ChatFilterKey) => void;
  onRequestTrade: () => void;
  onToggleExpanded: () => void;
  onToggleSettings: () => void;
  onToggleReport: () => void;
};

export function ChatFilterBar({
  t,
  activeFilter,
  chatExpanded,
  showSettings,
  onSelectFilter,
  onRequestTrade,
  onToggleExpanded,
  onToggleSettings,
  onToggleReport,
}: ChatFilterBarProps) {
  return (
    <section className="chat-filter-bar">
      <img className="chat-filter-bg" src={ORIGINAL_UI.game.chatControlBar} alt="" draggable={false} />
      {CHAT_FILTER_BUTTONS.map(({ key, left, labelKey }) => (
        <div
          key={key}
          className="chat-filter-button"
          data-chat-filter-key={key}
          data-chat-filter-active={activeFilter === key}
          style={{ left }}
        >
          <SpriteButton
            sprite={ORIGINAL_UI.game.chatFilterButtons[key]}
            label={t(labelKey, [], labelKey)}
            onClick={() => onSelectFilter(key)}
            onPointerActivate={() => onSelectFilter(key)}
            active={activeFilter === key}
          />
        </div>
      ))}
      <div className="chat-filter-button trade" data-chat-filter-key="trade" data-chat-filter-active="false">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.trade}
          label={t("ui.trade")}
          onClick={onRequestTrade}
          onPointerActivate={onRequestTrade}
        />
      </div>
      <div className="chat-filter-button size">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.size}
          label={t("ui.size")}
          onClick={onToggleExpanded}
          onPointerActivate={onToggleExpanded}
          active={!chatExpanded}
        />
      </div>
      <div className="chat-filter-button settings">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.settings}
          label={t("ui.settings")}
          onClick={onToggleSettings}
          onPointerActivate={onToggleSettings}
          active={showSettings}
        />
      </div>
      <div className="chat-filter-button report">
        <SpriteButton
          sprite={ORIGINAL_UI.game.chatFilterButtons.report}
          label={t("ui.report")}
          onClick={onToggleReport}
          onPointerActivate={onToggleReport}
        />
      </div>
    </section>
  );
}

export type BeltDialogProps = {
  t: TranslateFn;
  items: DisplayItemLike[];
  vertical: boolean;
  onClose: () => void;
  onRotate: () => void;
  onUseItem: (item: ItemActionRef) => void;
};

export function BeltDialog({ t, items, vertical, onClose, onRotate, onUseItem }: BeltDialogProps) {
  const itemBySlot = new Map(items.map((item) => [item.slot, item]));
  const useBeltItem = (item: DisplayItemLike) => {
    (window as typeof window & { __mir2LastBeltActivation?: Record<string, unknown> }).__mir2LastBeltActivation = {
      key: item.key,
      name: item.name,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
      at: Date.now(),
    };
    onUseItem({
      key: item.key,
      uniqueId: item.uniqueId,
      slot: item.slot,
      container: item.container,
    });
  };

  return (
    <section className={`belt-dialog ${vertical ? "vertical" : "horizontal"}`}>
      <img
        className="belt-dialog-bg belt-dialog-overlay"
        src={vertical ? ORIGINAL_UI.game.belt.verticalOverlay : ORIGINAL_UI.game.belt.horizontalOverlay}
        alt=""
        draggable={false}
      />
      <img
        className="belt-dialog-bg"
        src={vertical ? ORIGINAL_UI.game.belt.vertical : ORIGINAL_UI.game.belt.horizontal}
        alt=""
        draggable={false}
      />
      {ORIGINAL_UI.game.belt.slots.map((slot, index) => (
        <span
          key={`${slot.key}-label`}
          className="belt-slot-label"
          style={{
            left: vertical ? slot.verticalLabelX : slot.labelX + index * 35,
            top: vertical ? slot.verticalLabelY + index * 35 : slot.labelY,
          }}
        >
          {index + 1}
        </span>
      ))}
      {ORIGINAL_UI.game.belt.slots.map((slot, index) => {
        const item = itemBySlot.get(index) ?? null;

        return (
          <div
            key={slot.key}
            className="belt-slot"
            style={{
              left: vertical ? slot.verticalX : slot.horizontalX,
              top: vertical ? slot.verticalY : slot.horizontalY,
            }}
          >
            {item ? (
              <button
                type="button"
                className={`belt-item ${vertical ? "vertical" : "horizontal"}`}
                aria-label={item.name}
                onMouseDown={(event) => {
                  if (event.button !== 0) return;
                  event.preventDefault();
                  useBeltItem(item);
                }}
                onClick={(event) => {
                  if (event.detail !== 0) return;
                  useBeltItem(item);
                }}
              >
                <img
                  className="original-item-icon belt-item-icon"
                  src={originalItemIconPath(item.icon)}
                  alt=""
                  draggable={false}
                />
                {item.quantity > 0 ? <span className="item-stack-count belt-item-count">{item.quantity}</span> : null}
                <OriginalItemTooltip
                  t={t}
                  name={item.name}
                  description={item.description}
                  quantity={item.quantity}
                  durabilityCurrent={item.durabilityCurrent}
                  durabilityMax={item.durabilityMax}
                  align={vertical ? "right" : "top"}
                />
              </button>
            ) : null}
          </div>
        );
      })}
      <div className={`belt-button ${vertical ? "rotate-vertical" : "rotate-horizontal"}`}>
        <SpriteButton
          sprite={vertical ? ORIGINAL_UI.game.belt.rotateVertical : ORIGINAL_UI.game.belt.rotateHorizontal}
          label={t("ui.rotateBelt")}
          onClick={onRotate}
        />
      </div>
      <div className={`belt-button ${vertical ? "close-vertical" : "close-horizontal"}`}>
        <SpriteButton
          sprite={vertical ? ORIGINAL_UI.game.belt.closeVertical : ORIGINAL_UI.game.belt.closeHorizontal}
          label={t("ui.closeBelt")}
          onClick={onClose}
        />
      </div>
    </section>
  );
}

export type DuraPanelProps = {
  t: TranslateFn;
  visible: boolean;
  equipmentItems: DisplayEquipmentItemLike[];
  onToggle: () => void;
};

const DURA_ICON_LAYOUT = [
  { className: "helmet", slot: "helmet" as EquipmentSlot },
  { className: "belt", slot: "belt" as EquipmentSlot },
  { className: "armour", slot: "armour" as EquipmentSlot },
  { className: "boots", slot: "boots" as EquipmentSlot },
  { className: "weapon", slot: "weapon" as EquipmentSlot },
  { className: "necklace", slot: "necklace" as EquipmentSlot },
  { className: "bracelet-left", slot: "braceletLeft" as EquipmentSlot },
  { className: "bracelet-right", slot: "braceletRight" as EquipmentSlot },
  { className: "ring-left", slot: "ringLeft" as EquipmentSlot },
  { className: "ring-right", slot: "ringRight" as EquipmentSlot },
  { className: "torch", slot: "torch" as EquipmentSlot },
  { className: "stone", slot: "stone" as EquipmentSlot },
  { className: "amulet", slot: "amulet" as EquipmentSlot },
  { className: "mount", slot: "mount" as EquipmentSlot },
];

export function DuraPanel({ t, visible, equipmentItems, onToggle }: DuraPanelProps) {
  return (
    <>
      <section className="dura-status-panel">
        <div className="dura-button">
          <SpriteButton
            sprite={ORIGINAL_UI.game.miniMapButtons.dura}
            label={t("ui.duraPanel")}
            onClick={onToggle}
            active={visible}
          />
        </div>
      </section>
      {visible ? (
        <section className="dura-panel">
          <img className="dura-panel-bg" src={ORIGINAL_UI.game.duraPanel} alt="" draggable={false} />
          <img className="dura-panel-gray" src={ORIGINAL_UI.game.duraGray} alt="" draggable={false} />
          <img className="dura-panel-overlay" src={ORIGINAL_UI.game.duraBg} alt="" draggable={false} />
          {DURA_ICON_LAYOUT.map((icon) => (
            <img
              key={icon.className}
              className={`dura-piece ${icon.className}`}
              src={duraIconForSlot(icon.slot, equipmentItems)}
              alt=""
              draggable={false}
            />
          ))}
        </section>
      ) : null}
    </>
  );
}

function playerFacingChatLines(logs: DisplayLogLineLike[], hiddenFilters: ChatOptionFilterKey[]) {
  const lines = logs
    .filter((line) => line.tone !== "network")
    .filter((line) => matchesChatVisibility(line, hiddenFilters))
    .map((line) => ({
      text: trimLogTimestamp(line.text),
      tone: line.tone === "chat" ? ("chat" as const) : ("system" as const),
      channel: line.channel,
    }))
    .slice(0, 24)
    .reverse();

  return lines.length
    ? lines
    : Array.from({ length: 6 }, () => ({
        text: "",
        tone: "chat" as const,
        channel: "normal" as const,
      }));
}

function trimLogTimestamp(text: string) {
  return text.replace(/^\[\d{1,2}:\d{2}:\d{2}(?:\s?[AP]M)?\]\s*/i, "");
}

function matchesChatVisibility(line: DisplayLogLineLike, hiddenFilters: ChatOptionFilterKey[]) {
  const hidden = new Set(hiddenFilters);
  switch (line.channel) {
    case "normal":
      return !hidden.has("normal");
    case "shout":
    case "announcement":
      return !hidden.has("shout");
    case "whisper":
      return !hidden.has("whisper");
    case "group":
      return !hidden.has("group");
    case "guild":
      return !hidden.has("guild");
    case "relationship":
      return !hidden.has("lover");
    case "mentor":
      return !hidden.has("mentor");
    case "system":
    case "hint":
    case "server":
      return !hidden.has("system");
    default:
      return true;
  }
}

function originalItemIconPath(icon: number) {
  return `/original-ui/Items/${icon}.png`;
}

function duraIconForSlot(slot: EquipmentSlot, equipmentItems: DisplayEquipmentItemLike[]) {
  const item = equipmentItems.find((entry) => entry.slot === slot);
  const ratio = item ? item.durabilityCurrent / Math.max(item.durabilityMax, 1) : 0;
  const level = !item ? "empty" : ratio <= 0.33 ? "danger" : ratio <= 0.66 ? "warning" : "healthy";

  switch (slot) {
    case "weapon":
      return ORIGINAL_UI.game.duraIcons.weapon[level === "empty" ? "healthy" : level];
    case "armour":
      return ORIGINAL_UI.game.duraIcons.armour[level === "empty" ? "healthy" : level];
    case "helmet":
      return ORIGINAL_UI.game.duraIcons.helmet[level === "empty" ? "healthy" : level];
    case "mount":
      return ORIGINAL_UI.game.duraIcons.mount[level === "empty" ? "healthy" : level];
    case "necklace":
      return ORIGINAL_UI.game.duraIcons.necklace[level === "empty" ? "healthy" : level];
    case "torch":
      return ORIGINAL_UI.game.duraIcons.torch[level === "empty" ? "healthy" : level];
    case "braceletLeft":
    case "braceletRight":
      return ORIGINAL_UI.game.duraIcons.bracelet[level === "empty" ? "healthy" : level];
    case "ringLeft":
    case "ringRight":
      return ORIGINAL_UI.game.duraIcons.ring[level === "empty" ? "healthy" : level];
    case "amulet":
      return ORIGINAL_UI.game.duraIcons.amulet[level === "empty" ? "healthy" : level];
    case "boots":
      return ORIGINAL_UI.game.duraIcons.boots[level === "empty" ? "healthy" : level];
    case "belt":
      return ORIGINAL_UI.game.duraIcons.belt[level === "empty" ? "healthy" : level];
    case "stone":
      return ORIGINAL_UI.game.duraIcons.stone.empty;
  }
}
