"use client";

import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import type { BagBeltButtonBinding } from "../../lib/bag-belt-gesture";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import type { DisplayItem } from "./original-client-types";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";

import type { ChatUiControls } from "../../lib/client-core-runtime";

import {
  CrystalChatHistory,
  CrystalChatType,
  type CrystalChatType as CrystalChatTypeValue,
} from "../../lib/crystal-chat-history";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { IS_PLATINUM_176_PROFILE } from "../../lib/content-profile";
import { OriginalAudioSettingsControls } from "./original-client-audio-settings";
import { CrystalGdiTextImage, findCrystalGdiTextAsset } from "./crystal-gdi-text";
import { originalItemIconPath } from "./original-client-inventory-utils";
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
  crystalChatType?: number;
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

type DisplayItemLike = DisplayItem;

type ItemActionRef = Pick<DisplayItemLike, "key" | "uniqueId" | "authoritativeUniqueId" | "slot" | "container">;

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
const VISIBLE_CHAT_FILTER_BUTTONS = CHAT_FILTER_BUTTONS.filter(
  ({ key }) => !IS_PLATINUM_176_PROFILE || (key !== "lover" && key !== "mentor"),
);

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
const VISIBLE_CHAT_OPTION_FILTER_BUTTONS = CHAT_OPTION_FILTER_BUTTONS.filter(
  ({ key }) => !IS_PLATINUM_176_PROFILE || (key !== "lover" && key !== "mentor"),
);

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


type CrystalChatDragLease = Readonly<{source:object;epoch:number;pointerId:number;grabY:number;capture:HTMLDivElement}>;
export function crystalChatLocalY(clientY:number,rect:Readonly<{top:number;width:number;height:number}>,height:number):number|null {
  if (![clientY,rect.top,rect.width,rect.height,height].every(Number.isFinite)
    ||rect.width<=0||rect.height<=0||height<=0
    ||Math.abs(rect.width/632-rect.height/height)>0.002) return null;
  return (clientY-rect.top)*height/rect.height;
}
export function crystalChatDragCurrent(lease:CrystalChatDragLease|null,source:object,epoch:number,pointerId:number,visible:boolean,settingsOpen:boolean):boolean {
  return Boolean(lease&&lease.source===source&&lease.epoch===epoch&&lease.pointerId===pointerId&&visible&&!settingsOpen);
}
const CHAT_MASK_CHANNELS:ChatOptionFilterKey[]=["normal","whisper","shout","system","lover","mentor","group","guild"];
export function crystalChatHiddenFilters(mask:number):ChatOptionFilterKey[] {
  return CHAT_MASK_CHANNELS.filter((_key,index)=>(mask&(1<<index))!==0);
}

export type ChatFrameProps = {
  t: TranslateFn;
  runtimeMessage: string;
  logs: DisplayLogLineLike[];
  chatMessage: string;
  hints: string[];
  chatUi?: ChatUiControls;
  onInputHoldChange?: (held:boolean)=>void;
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
  chatUi,onInputHoldChange,
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
  const shared=chatUi?.document;
  const appliedFilters=shared?crystalChatHiddenFilters(shared.appliedMask):hiddenFilters;
  const lines = playerFacingChatLines(logs,appliedFilters,!shared);
  const activePrefix = chatPrefixForFilter(activeFilter);
  const settingsMask=shared?.draftMask??shared?.appliedMask;
  const displayHiddenFilters=settingsMask===undefined?hiddenFilters:crystalChatHiddenFilters(settingsMask);
  const hiddenFilterSet = new Set(displayHiddenFilters);
  const [scrollOffset, setScrollOffset] = useState(0);
  const previousMaxScrollOffsetRef = useRef(0);
  const previousActiveFilterRef = useRef(activeFilter);
  const previousExpandedRef = useRef(expanded);
  const visibleLineCount = shared?.lineCount??4;
  const maxScrollOffset = Math.max(lines.length - visibleLineCount, 0);
  const liveScroll=shared?.index??scrollOffset;
  const visibleLines = lines.slice(liveScroll,liveScroll+visibleLineCount);
  const knobTop = shared?.knobTop??(maxScrollOffset === 0 ? 16 : 16 + Math.round((scrollOffset / maxScrollOffset) * 28));
  const chatTextBoxVisible = chatMessage.length > 0;
  const settingsVisible=shared?.open??showSettings;
  const backgroundTransparent=shared?Boolean(shared.appliedMask&512):transparent;
  const draftTransparent=settingsMask===undefined?transparent:Boolean(settingsMask&512);
  const frameRef=useRef<HTMLElement>(null),settingsRef=useRef<HTMLDivElement>(null);
  const dragRef=useRef<CrystalChatDragLease|null>(null);
  const liveRef=useRef({chatUi,expanded,settingsVisible,onInputHoldChange});
  liveRef.current={chatUi,expanded,settingsVisible,onInputHoldChange};
  function endChatDrag() {
    const held=dragRef.current;dragRef.current=null;
    if(held){
      try {if(held.capture.hasPointerCapture(held.pointerId))held.capture.releasePointerCapture(held.pointerId);}catch{}
      if(!dragRef.current)liveRef.current.onInputHoldChange?.(false);
    }
  }
  function closeChatSettings() {if(chatUi&&shared)chatUi.cancel(shared.epoch);else onCloseSettings();}
  function scroll(action:"home"|"up"|"down"|"end") {
    if(chatUi&&shared) {chatUi.scroll(action,shared.epoch);return;}
    setScrollOffset(current=>action==="home"?0:action==="end"?maxScrollOffset:action==="up"?Math.max(current-1,0):Math.min(current+1,maxScrollOffset));
  }
  function chatDragDown(event:ReactPointerEvent<HTMLDivElement>) {
    if(!chatUi||!shared||shared.open||!expanded||shared.historyCount!==lines.length||event.button!==0||dragRef.current) return;
    const live=liveRef.current;
    if(!live.chatUi||live.chatUi.source!==chatUi.source||live.chatUi.document.epoch!==shared.epoch||!live.expanded||live.settingsVisible)return;
    const rect=frameRef.current?.getBoundingClientRect();
    const y=rect?crystalChatLocalY(event.clientY,rect,shared.height):null;
    if(y===null||document.visibilityState!=="visible")return;
    const lease={source:chatUi.source,epoch:shared.epoch,pointerId:event.pointerId,grabY:y-shared.knobTop,capture:event.currentTarget};
    try {event.currentTarget.setPointerCapture(event.pointerId);}catch{return;}
    dragRef.current=lease;onInputHoldChange?.(true);event.preventDefault();event.stopPropagation();
  }
  function chatDragMove(event:ReactPointerEvent<HTMLDivElement>) {
    const live=liveRef.current,ui=live.chatUi,lease=dragRef.current;
    if(!ui||!crystalChatDragCurrent(lease,ui.source,ui.document.epoch,event.pointerId,
      live.expanded&&document.visibilityState==="visible",live.settingsVisible)) {if(lease?.pointerId===event.pointerId)endChatDrag();return;}
    const rect=frameRef.current?.getBoundingClientRect();
    const y=rect?crystalChatLocalY(event.clientY,rect,ui.document.height):null;
    if(y===null){endChatDrag();return;}
    event.preventDefault();event.stopPropagation();ui.drag(y,lease!.grabY,lease!.epoch);
  }
  function chatDragEnd(event:ReactPointerEvent<HTMLDivElement>) {
    if(dragRef.current?.pointerId!==event.pointerId)return;
    event.preventDefault();event.stopPropagation();endChatDrag();
    if(event.currentTarget.hasPointerCapture(event.pointerId))event.currentTarget.releasePointerCapture(event.pointerId);
  }
  useEffect(()=>{
    chatUi?.observe(lines.length);
  },[chatUi?.source,lines.length,shared?.appliedMask,shared?.epoch]);
  useEffect(()=>{
    if(dragRef.current&&(!chatUi||dragRef.current.source!==chatUi.source||dragRef.current.epoch!==shared?.epoch||!expanded||settingsVisible))endChatDrag();
  },[chatUi?.source,shared?.epoch,expanded,settingsVisible]);
  useEffect(()=>{
    const clear=()=>endChatDrag(),visibility=()=>{if(document.visibilityState!=="visible")clear();};
    window.addEventListener("blur",clear);document.addEventListener("visibilitychange",visibility);
    return()=>{window.removeEventListener("blur",clear);document.removeEventListener("visibilitychange",visibility);clear();};
  },[]);
  useEffect(()=>{
    if(!settingsVisible)return;
    const previous=document.activeElement instanceof HTMLElement?document.activeElement:null;
    const panel=settingsRef.current;panel?.querySelector<HTMLButtonElement>("button")?.focus();
    return()=>{if(previous?.isConnected)previous.focus();};
  },[settingsVisible,shared?.epoch]);
  useEffect(() => {
    if(shared)return;
    setScrollOffset((current) => {
      const previousMaxScrollOffset = previousMaxScrollOffsetRef.current;
      const filterChanged = previousActiveFilterRef.current !== activeFilter;
      const expandedChanged = previousExpandedRef.current !== expanded;
      previousMaxScrollOffsetRef.current = maxScrollOffset;
      previousActiveFilterRef.current = activeFilter;
      previousExpandedRef.current = expanded;
      if (filterChanged || expandedChanged || current >= previousMaxScrollOffset) return maxScrollOffset;
      return Math.min(current, maxScrollOffset);
    });
  }, [activeFilter, expanded, maxScrollOffset,Boolean(shared)]);

  return (
    <section ref={frameRef} data-chat-size={shared?.size} data-chat-epoch={shared?.epoch}
      className={`chat-frame ${expanded ? "" : "collapsed"} ${backgroundTransparent ? "transparent" : ""}`}
      style={shared?{top:shared.top,height:shared.height}:undefined}>
      <img className="chat-frame-bg" src={shared?`/original-ui/Prguse/${shared.frameIndex}.png`:ORIGINAL_UI.game.chatDialog} alt="" draggable={false} />
      <div className="chat-scroll-buttons">
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.home} label={t("ui.home")} onClick={() => scroll("home")} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.up} label={t("ui.up")} onClick={() => scroll("up")} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.down} label={t("ui.down")} onClick={() => scroll("down")} />
        <SpriteButton sprite={ORIGINAL_UI.game.chatScrollButtons.end} label={t("ui.end")} onClick={() => scroll("end")} />
      </div>
      <img className="chat-count-bar" src={shared?`/original-ui/Prguse/${shared.countBarIndex}.png`:ORIGINAL_UI.game.chatCountBar} alt="" draggable={false} />
      <div className="chat-position-knob" style={{top:knobTop,touchAction:"none"}}
        role={shared?"slider":undefined} tabIndex={shared?0:undefined} aria-label={t("ui.chatHistory",[],"Chat history")}
        aria-valuemin={0} aria-valuemax={shared?Math.max(shared.historyCount-1,0):undefined} aria-valuenow={shared?.index}
        onPointerDown={chatDragDown} onPointerMove={chatDragMove} onPointerUp={chatDragEnd} onPointerCancel={chatDragEnd} onLostPointerCapture={chatDragEnd}
        onKeyDown={event=>{const action=event.key==="Home"?"home":event.key==="End"?"end":event.key==="ArrowUp"?"up":event.key==="ArrowDown"?"down":null;
          if(action&&shared){event.preventDefault();event.stopPropagation();scroll(action);}}}>
        <img src={ORIGINAL_UI.game.chatScrollButtons.knob.base} alt="" draggable={false} />
      </div>
      <div className={`chat-feed ${expanded ? "" : "hidden"}`}>
        {visibleLines.map((line, index) => {
          const gdiText = findCrystalGdiTextAsset({
            text: line.text,
            foreground: line.foreground,
            background: line.background,
            outline: false,
          });
          return (
            <div
              key={`chat-line-${activeFilter}-${liveScroll + index}-${line.text}`}
              className={`chat-feed-line ${line.tone === "system" ? "system" : ""} channel-${line.channel}`}
              style={{ color: line.foreground, backgroundColor: line.background }}
            >
              {gdiText ? (
                <CrystalGdiTextImage asset={gdiText} accessibleText={line.text} />
              ) : line.text}
            </div>
          );
        })}
      </div>
      {settingsVisible ? (
        <div ref={settingsRef} className="chat-settings-panel" role="dialog" aria-modal="true" aria-label={t("ui.chatSettings",[],"Chat Settings")}
          onKeyDown={event=>{if(event.key==="Escape"){event.preventDefault();event.stopPropagation();closeChatSettings();}
            else if(event.key==="Tab"){const buttons=Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"));
              const first=buttons[0],last=buttons.at(-1);if(event.shiftKey&&document.activeElement===first&&last){event.preventDefault();last.focus();}
              else if(!event.shiftKey&&document.activeElement===last&&first){event.preventDefault();first.focus();}}}}>
          <div className="chat-settings-title">{t("ui.settings")}</div>
          <OriginalAudioSettingsControls t={t} className="chat-audio-settings" />
          <div className="chat-settings-tabs">
            <button type="button" className="active">
              {t("ui.filter", [], "Filter")}
            </button>
            <button type="button" onClick={()=>shared&&chatUi?chatUi.editTransparent(!draftTransparent,shared.epoch):onToggleTransparent()} data-chat-transparent={draftTransparent}>
              {draftTransparent ? t("ui.on", [], "On") : t("ui.off", [], "Off")}
            </button>
          </div>
          <div className="chat-settings-grid">
            <button
              type="button"
              className="chat-settings-option"
              data-chat-option-filter="all"
              data-chat-option-hidden={displayHiddenFilters.length === CHAT_OPTION_FILTER_BUTTONS.length}
              onClick={()=>shared&&chatUi?chatUi.editAll(displayHiddenFilters.length>0,shared.epoch):onToggleAllHiddenFilters()}
            >
              {t("client.Chat_All", [], "All")}
            </button>
            {VISIBLE_CHAT_OPTION_FILTER_BUTTONS.map(({ key, labelKey, fallback }) => (
              <button
                key={key}
                type="button"
                className="chat-settings-option"
                data-chat-option-filter={key}
                data-chat-option-hidden={hiddenFilterSet.has(key)}
                onClick={()=>shared&&chatUi?chatUi.editFilter(CHAT_MASK_CHANNELS.indexOf(key),hiddenFilterSet.has(key),shared.epoch):onToggleHiddenFilter(key)}
              >
                {t(labelKey, [], fallback)}
              </button>
            ))}
          </div>
          <div className="chat-settings-copy">{`${t("ui.size")}: ${expanded ? t("ui.down") : t("ui.up")}`}</div>
          {shared&&chatUi?<div className="chat-settings-actions">
            <button type="button" data-chat-settings-action="defaults" onClick={()=>chatUi.defaults(shared.epoch)}>{t("ui.defaults",[],"Defaults")}</button>
            <button type="button" data-chat-settings-action="cancel" onClick={closeChatSettings}>{t("ui.cancel",[],"Cancel")}</button>
            <button type="button" data-chat-settings-action="apply" onClick={()=>chatUi.apply(shared.epoch)}>{t("ui.apply",[],"Apply")}</button>
          </div>:null}
          <button type="button" className="chat-settings-close" onClick={closeChatSettings}>
            {t("ui.close")}
          </button>
        </div>
      ) : null}
      <input
        className="chat-textbox"
        style={shared?{top:shared.inputTop}:undefined}
        value={chatMessage}
        data-chat-prefix={activePrefix}
        data-chat-visible={chatTextBoxVisible}
        aria-hidden={!chatTextBoxVisible}
        aria-label={t("ui.worldChatPlaceholder")}
        onChange={(event) => onChatMessageChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key !== "Enter") return;
          if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
          onSendChat();
        }}
      />
    </section>
  );
}

export type ChatFilterBarProps = {
  t: TranslateFn;
  activeFilter: ChatFilterKey;
  chatExpanded: boolean;
  chatUi?: ChatUiControls;
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
  chatUi,
  showSettings,
  onSelectFilter,
  onRequestTrade,
  onToggleExpanded,
  onToggleSettings,
  onToggleReport,
}: ChatFilterBarProps) {
  return (
    <section className="chat-filter-bar" style={chatUi?{top:chatUi.document.controlTop}:undefined}>
      <img className="chat-filter-bg" src={ORIGINAL_UI.game.chatControlBar} alt="" draggable={false} />
      {VISIBLE_CHAT_FILTER_BUTTONS.map(({ key, left, labelKey }) => (
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


type ActiveItemTooltipLease<Item> = Readonly<{item: Item; node: HTMLElement;
  reader: (item: Readonly<Item>) => CrystalTooltipDocument | null}>;

/** One displayed item owns one read-only refresh timer; old sources never borrow a new reader. */
export function useActiveItemTooltip<Item>(items: readonly Item[],
  reader?: (item: Readonly<Item>) => CrystalTooltipDocument | null, visible = true) {
  const latest = useRef({items, reader, visible}); latest.current = {items, reader, visible};
  const active = useRef<ActiveItemTooltipLease<Item> | null>(null);
  const [lease, setLease] = useState<ActiveItemTooltipLease<Item> | null>(null);
  const [result, setResult] = useState<{lease: ActiveItemTooltipLease<Item>; document: CrystalTooltipDocument | null} | null>(null);
  function current(capture: ActiveItemTooltipLease<Item>) {
    const live = latest.current;
    return active.current === capture && live.visible && live.reader === capture.reader && live.items.includes(capture.item)
      && capture.node.isConnected && document.visibilityState !== "hidden";
  }
  function clear(capture: ActiveItemTooltipLease<Item> | null) {
    if (!capture || active.current !== capture) return;
    active.current = null; setLease(null); setResult(null);
  }
  function refresh(capture: ActiveItemTooltipLease<Item>) {
    if (!current(capture)) return;
    let document: CrystalTooltipDocument | null = null;
    try { document = capture.reader(capture.item); } catch { /* Missing/retired source retains the basic label. */ }
    if (current(capture)) setResult({lease: capture, document});
  }
  function activate(item: Item, node: HTMLElement) {
    if (!reader || latest.current.reader !== reader || !latest.current.visible || !latest.current.items.includes(item) || !node.isConnected) return;
    const capture = {item, node, reader}; active.current = capture; setLease(capture); refresh(capture);
  }
  function release(item: Item, node: HTMLElement) {
    const capture = active.current;
    if (capture && capture.item === item && capture.node === node && capture.reader === reader) clear(capture);
  }
  const present = !!lease && visible && reader === lease.reader && items.includes(lease.item);
  useEffect(() => {
    if (!lease || !present || !current(lease)) return;
    const timer = window.setInterval(() => refresh(lease), 1000);
    const cancel = () => clear(lease), hidden = () => { if (document.visibilityState === "hidden") cancel(); };
    window.addEventListener("blur", cancel); window.addEventListener("resize", cancel);
    window.addEventListener("pointercancel", cancel); window.addEventListener("pagehide", cancel);
    document.addEventListener("visibilitychange", hidden);
    return () => {
      window.clearInterval(timer); window.removeEventListener("blur", cancel); window.removeEventListener("resize", cancel);
      window.removeEventListener("pointercancel", cancel); window.removeEventListener("pagehide", cancel);
      document.removeEventListener("visibilitychange", hidden);
      if (active.current === lease) active.current = null;
    };
  }, [lease, reader, visible, present]);
  return {activate, release, document(item: Item) {
    return lease && result && lease.item === item && result.lease === lease && current(lease) ? result.document : null;
  }};
}

export type BeltDialogProps = {
  onRegisterBeltTargets?: (targets: readonly BagBeltButtonBinding[]) => () => void;
  onFenceBeltMouse?: (event: { target: EventTarget | null; detail?: number; timeStamp: number; preventDefault(): void; stopPropagation(): void }) => boolean;
  t: TranslateFn;
  items: DisplayItemLike[];
  onReadItemTooltip?: (item: Readonly<DisplayItem>) => CrystalTooltipDocument | null;
  vertical: boolean;
  onClose: () => void;
  onRotate: () => void;
  onUseItem: (item: ItemActionRef) => void;
};

export function BeltDialog({ t, items, vertical, onClose, onRotate, onUseItem, onReadItemTooltip, onRegisterBeltTargets, onFenceBeltMouse }: BeltDialogProps) {
  const tooltip = useActiveItemTooltip(items, onReadItemTooltip);
  const itemBySlot = new Map(items.map((item) => [item.slot, item]));
  const beltButtons = useRef<Array<HTMLButtonElement | null>>([]);
  const beltBindingKey = JSON.stringify(Array.from({ length: 6 }, (_, slot) => {
    const item = itemBySlot.get(slot);
    return [slot, !!item, item ? item.authoritativeUniqueId ?? "unknown" : null];
  }));
  useLayoutEffect(() => {
    const targets = Array.from({ length: 6 }, (_, slot) => {
      const item = itemBySlot.get(slot);
      return { slot, node: beltButtons.current[slot], item: item
        ? Object.freeze({ authoritativeUniqueId: item.authoritativeUniqueId }) : null };
    });
    if (targets.every(target => target.node !== null && target.node !== undefined)) return onRegisterBeltTargets?.(targets.map(target => ({ ...target, node: target.node! })));
  }, [beltBindingKey, vertical, onRegisterBeltTargets]);
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
      authoritativeUniqueId: item.authoritativeUniqueId,
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
        const tooltipDocument = item ? tooltip.document(item) : null;

        return (
          <div
            key={slot.key}
            className="belt-slot"
            style={{
              left: vertical ? slot.verticalX : slot.horizontalX,
              top: vertical ? slot.verticalY : slot.horizontalY,
            }}
          >
              <button
                type="button"
                ref={node => { beltButtons.current[index] = node; }}
                className={`belt-item ${vertical ? "vertical" : "horizontal"}`}
                aria-label={item?.name ?? `${t("ui.belt", [], "Belt")} ${index + 1}`}
                onPointerEnter={event => { if (item && event.pointerType !== "touch") tooltip.activate(item, event.currentTarget); }}
                onPointerLeave={event => { if (item && (event.pointerType === "touch" || document.activeElement !== event.currentTarget)) tooltip.release(item, event.currentTarget); }}
                onFocus={event => { if (item) tooltip.activate(item, event.currentTarget); }}
                onBlur={event => { if (item && !event.currentTarget.matches(":hover")) tooltip.release(item, event.currentTarget); }}
                onPointerCancel={event => { if (item) tooltip.release(item, event.currentTarget); }}
                onPointerDown={event => { if (item && event.pointerType === "touch") { tooltip.activate(item, event.currentTarget); event.stopPropagation(); } }}
                onMouseDown={(event) => {
                  if (onFenceBeltMouse?.(event) || !item || event.button !== 0) return;
                  event.preventDefault();
                  useBeltItem(item);
                }}
                onClick={(event) => {
                  if (onFenceBeltMouse?.(event) || !item || event.detail !== 0) return;
                  useBeltItem(item);
                }}
              >
                {item ? <>
                <img
                  className="original-item-icon belt-item-icon"
                  src={originalItemIconPath(item.icon)}
                  alt=""
                  draggable={false}
                />
                {item.quantity > 0 ? <span className="item-stack-count belt-item-count">{item.quantity}</span> : null}
                {tooltipDocument ? <OriginalCrystalItemTooltip document={tooltipDocument} align={vertical ? "right" : "top"} /> : (
                <OriginalItemTooltip
                  t={t}
                  name={item.name}
                  description={item.description}
                  quantity={item.quantity}
                  durabilityCurrent={item.durabilityCurrent}
                  durabilityMax={item.durabilityMax}
                  align={vertical ? "right" : "top"}
                />
                )}
                </> : null}
              </button>
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

function playerFacingChatLines(logs: DisplayLogLineLike[], hiddenFilters: ChatOptionFilterKey[], emptyPadding=true) {
  const history = new CrystalChatHistory(measureCrystalChatText, {
    FilterNormalChat: hiddenFilters.includes("normal"),
    FilterWhisperChat: hiddenFilters.includes("whisper"),
    FilterShoutChat: hiddenFilters.includes("shout"),
    FilterSystemChat: hiddenFilters.includes("system"),
    FilterGroupChat: hiddenFilters.includes("group"),
    FilterGuildChat: hiddenFilters.includes("guild"),
  });

  for (const line of [...logs].reverse()) {
    if (line.tone === "network" || !matchesChatVisibility(line, hiddenFilters)) continue;
    history.receiveChat(trimLogTimestamp(line.text), crystalChatTypeForDisplayLine(line));
  }

  const lines = history.History.map((line) => ({
    text: line.Text,
    tone: line.Type === CrystalChatType.Normal ? ("chat" as const) : ("system" as const),
    channel: line.Channel,
    foreground: argbToCss(line.ForeColour),
    background: argbToCss(line.BackColour),
  }));

  return lines.length || !emptyPadding
    ? lines
    : Array.from({ length: 6 }, () => ({
        text: "",
        tone: "chat" as const,
        channel: "normal" as const,
        foreground: "rgba(0, 0, 0, 1)",
        background: "rgba(255, 255, 255, 1)",
      }));
}

function crystalChatTypeForDisplayLine(line: DisplayLogLineLike): CrystalChatTypeValue {
  if (
    Number.isInteger(line.crystalChatType) &&
    line.crystalChatType! >= CrystalChatType.Normal &&
    line.crystalChatType! <= CrystalChatType.LineMessage
  ) {
    return line.crystalChatType as CrystalChatTypeValue;
  }

  switch (line.channel) {
    case "shout":
      return CrystalChatType.Shout;
    case "whisper":
      return CrystalChatType.WhisperIn;
    case "group":
      return CrystalChatType.Group;
    case "guild":
      return CrystalChatType.Guild;
    case "mentor":
      return CrystalChatType.Mentor;
    case "relationship":
      return CrystalChatType.Relationship;
    case "hint":
      return CrystalChatType.Hint;
    case "line":
      return CrystalChatType.LineMessage;
    case "announcement":
      return CrystalChatType.Announcement;
    case "system":
    case "server":
      return CrystalChatType.System;
    default:
      return line.tone === "system" ? CrystalChatType.System : CrystalChatType.Normal;
  }
}

let crystalChatMeasureContext: CanvasRenderingContext2D | null | undefined;

function measureCrystalChatText(text: string) {
  if (crystalChatMeasureContext === undefined) {
    const canvas = typeof document === "undefined" ? null : document.createElement("canvas");
    crystalChatMeasureContext = canvas?.getContext("2d") ?? null;
    if (crystalChatMeasureContext) crystalChatMeasureContext.font = "11px Arial";
  }
  return Math.ceil(crystalChatMeasureContext?.measureText(text).width ?? text.length * 6);
}

function argbToCss(argb: string) {
  const value = argb.replace(/^#/, "");
  if (value.length !== 8) return argb;
  const alpha = Number.parseInt(value.slice(0, 2), 16) / 255;
  const red = Number.parseInt(value.slice(2, 4), 16);
  const green = Number.parseInt(value.slice(4, 6), 16);
  const blue = Number.parseInt(value.slice(6, 8), 16);
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
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
