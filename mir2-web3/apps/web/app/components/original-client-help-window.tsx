"use client";

import { useCallback, useRef, useState } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { clampCrystalHelpPosition, crystalHelpPage, defaultCrystalKeyBindings,
  displayCrystalKeyBinding, stepCrystalHelpPage, type CrystalHelpWindowPosition, type CrystalKeyBinding } from "../../lib/player-ui-preferences";
import type { Mir2Language } from "../../lib/localization";
import { nativeLanguageInfo, nativeText } from "../../lib/r16-native-localization";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (key: string, params?: Array<string | number>, fallback?: string) => string;
export type HelpWindowProps = {
  t: TranslateFn; onClose: () => void; language: Mir2Language;
  /** Root retains page and position across Hide/Show, matching HelpDialogUi. */
  page?: number; position?: CrystalHelpWindowPosition; bindings?: readonly CrystalKeyBinding[];
  onPageChange?: (page: number) => void; onPositionChange?: (position: CrystalHelpWindowPosition) => void;
  scrollOffsets: Map<string, number>;
};

// Native Help titles follow the original page order. Use their specific catalog
// keys since generic aliases such as "Statistics" also occur in other windows.
const HEADINGS = {
  shortcuts: ["game.014", "Shortcut Information"], chatShortcuts: ["game.015", "Chat Shortcuts"],
  movement: ["game.018", "Movements"], attack: ["game.019", "Attacking"],
  collect: ["game.020", "Collecting Items"], health: ["game.021", "Health"],
  skills: ["game.007", "Skills"], mana: ["game.022", "Mana"], chat: ["game.023", "Chatting"],
  group: ["game.024", "Groups"], durability: ["game.025", "Durability"], purchase: ["game.026", "Purchasing"],
  sell: ["game.027", "Selling"], repair: ["game.028", "Repairing"], trade: ["game.029", "Trading"],
  inspect: ["game.030", "Inspecting"], stats: ["game.009", "Statistics"], quests: ["game.012", "Quests"],
  mounts: ["game.031", "Mounts"], fishing: ["help.page.27.title", "Fishing"],
  gems: ["game.033", "Gems and Orbs"], heroes: ["game.034", "Heroes"], guild: ["game.035", "Guild Buffs"],
  awakening: ["client.ItemTypeAwakening", "Awakening"],
} as const;
const HELP_PAGES: readonly (keyof typeof HEADINGS)[] = [
  "shortcuts", "shortcuts", "chatShortcuts", "movement", "attack", "collect", "health", "skills", "skills",
  "mana", "chat", "group", "durability", "purchase", "sell", "repair", "trade", "inspect",
  "stats", "stats", "stats", "stats", "stats", "stats", "quests", "quests", "quests", "quests", "mounts", "mounts",
  "fishing", "gems", "heroes", "heroes", "heroes", "heroes", "heroes", "guild", "guild", "guild",
  "awakening", "awakening", "awakening", "awakening", "awakening",
];
// The first 36 help.shortcut keys describe these bindings in Native order.
const SHORTCUT_FUNCTIONS = [
  ["Exit", "Logout", "Bar1Skill1", "Inventory", "Equipment", "Skills", "Group", "Trade", "Friends",
    "Minimap", "Guilds", "GameShop", "Relationship", "Belt", "Options", "Help", "Mount", "TargetSpellLockOn"],
  ["ChangePetmode", "ChangeAttackmode", "AttackmodePeace", "AttackmodeGroup", "AttackmodeGuild",
    "AttackmodeRedbrown", "AttackmodeAll", "Bigmap", "Skillbar", "Autorun", "Cameramode", "Pickup", "",
    "Screenshot", "Fishing", "Mentor", "CreaturePickup", "CreatureAutoPickup"],
] as const;
const CHAT_SHORTCUTS = ["/(username)", "!(text)", "!~(text)"] as const;
const DEFAULT_BINDINGS = defaultCrystalKeyBindings();
const twoDigits = (number: number) => String(number).padStart(2, "0");

/** As in Native R16, English uses the original Help images and the other R16 locales use scrollable text. */
export function HelpWindow({ t, onClose, language, page, position, bindings = DEFAULT_BINDINGS,
  onPageChange, onPositionChange, scrollOffsets }: HelpWindowProps) {
  const [localPage, setLocalPage] = useState(0);
  const [localPosition, setLocalPosition] = useState<CrystalHelpWindowPosition>({ left: 244, top: 129 });
  const shownPage = crystalHelpPage(page ?? localPage);
  const shownPosition = clampCrystalHelpPosition(position ?? localPosition);
  const scrollKey = `${language}/${shownPage}`;
  const restoreScroll = useCallback((element: HTMLDivElement | null) => {
    if (element) element.scrollTop = scrollOffsets.get(scrollKey) ?? 0;
  }, [scrollKey, scrollOffsets]);
  const dragRef = useRef<{ pointerId: number; x: number; y: number; left: number; top: number; scale: number } | null>(null);
  const changePage = (direction: -1 | 1) => {
    const next = stepCrystalHelpPage(shownPage, direction);
    setLocalPage(next); onPageChange?.(next);
  };
  const boundKey = (functionId: string) => {
    const binding = bindings.find(row => row.function === functionId);
    return binding ? displayCrystalKeyBinding(binding) : "";
  };
  const [headingKey, englishHeading] = HEADINGS[HELP_PAGES[shownPage]];
  const heading = nativeText(language, headingKey, englishHeading);
  const locale = nativeLanguageInfo(language);
  const direction = locale?.dir ?? "ltr";
  const caption = nativeText(language, "game.013", "Help");
  const imageIndex = shownPage - 3;
  const imageWidth = imageIndex <= 28 ? 512 : imageIndex === 30 || imageIndex >= 34 ? 508 : 509;
  const imageHeight = imageIndex === 30 || imageIndex >= 34 ? 395 : 396;
  const useLocalizedPage = imageIndex >= 0 && locale && locale.code !== "en";
  const title = useLocalizedPage ? nativeText(language, `help.page.${twoDigits(imageIndex)}.title`, "") : "";
  const body = useLocalizedPage ? nativeText(language, `help.page.${twoDigits(imageIndex)}.body`, "") : "";
  const bodySize = Array.from(body).length > 900 ? 13 : 14;
  const firstShortcut = shownPage * SHORTCUT_FUNCTIONS[0].length;
  const shortcutKeys = shownPage < 2 ? SHORTCUT_FUNCTIONS[shownPage].map((functionId, index) => {
    if (!functionId) return "Ctrl + Right Click";
    if (shownPage === 0 && index === 2) return `${boundKey(functionId)}-${boundKey("Bar1Skill8")}`;
    return boundKey(functionId);
  }) : CHAT_SHORTCUTS;
  return <div className="original-help-window" role="dialog" aria-label={caption} data-ui-interactive="true"
    style={{ position: "absolute", left: shownPosition.left, top: shownPosition.top, width: 536, height: 509, zIndex: 45,
      fontFamily: "var(--mir2-ui-font)" }}>
    <img src="/original-ui/Prguse/920.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 536, height: 509, pointerEvents: "none" }} />
    {locale ? <span aria-hidden="true" dir={direction} style={{ position: "absolute", left: 18, top: 9, width: 45, height: 14,
      pointerEvents: "none", display: "grid", placeItems: "center", overflow: "hidden", background: "#0e0b08", fontSize: 9, color: "#eee" }}>
      {caption}
    </span> : <img src="/original-ui/Title/57.png" alt="" draggable={false} style={{ position: "absolute", left: 18, top: 9, width: 45, height: 14, pointerEvents: "none" }} />}
    <div aria-label={t("ui.moveWindow", undefined, "Move window")} style={{ position: "absolute", left: 0, top: 0, width: 509, height: 35, cursor: "move", touchAction: "none" }}
      onPointerDown={event => {
        if (event.button !== 0) return;
        event.preventDefault(); event.stopPropagation();
        const width = event.currentTarget.getBoundingClientRect().width;
        dragRef.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY,
          left: shownPosition.left, top: shownPosition.top, scale: width > 0 ? width / 509 : 1 };
        event.currentTarget.setPointerCapture(event.pointerId);
      }}
      onPointerMove={event => {
        const drag = dragRef.current; if (!drag || drag.pointerId !== event.pointerId) return;
        event.preventDefault(); event.stopPropagation();
        const next = clampCrystalHelpPosition({ left: drag.left + (event.clientX - drag.x) / drag.scale,
          top: drag.top + (event.clientY - drag.y) / drag.scale });
        setLocalPosition(next); onPositionChange?.(next);
      }}
      onPointerUp={event => { if (dragRef.current?.pointerId === event.pointerId) dragRef.current = null; }}
      onPointerCancel={() => { dragRef.current = null; }} onLostPointerCapture={() => { dragRef.current = null; }} />
    <div style={{ position: "absolute", left: 509, top: 3 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.closeButton} label={t("ui.close", undefined, "Close")} onClick={onClose} /></div>
    <div style={{ position: "absolute", left: 147, top: 39, width: 242, height: 30, textAlign: "center", fontSize: 10, color: "#eee" }}>{shownPage + 1}. <bdi dir={direction}>{heading}</bdi></div>
    {shownPage < 3 ? <>
      <div style={{ position: "absolute", left: 13, top: 75, fontSize: 10, color: "#eee" }}>{nativeText(language, "game.016", "Shortcuts")}</div>
      <div style={{ position: "absolute", left: 114, top: 75, fontSize: 10, color: "#eee" }}>{nativeText(language, "game.017", "Information")}</div>
      {shortcutKeys.map((shortcut, index) => {
        const key = `help.shortcut.${twoDigits(firstShortcut + index)}`;
        return <div key={key} style={{ position: "absolute", left: 18, top: 107 + index * 20, display: "flex", width: 501, height: 23, fontSize: 9 }}>
          <span dir="ltr" style={{ width: 101, flexShrink: 0, color: "#d8c47c" }}>{shortcut}</span>
          <span dir={direction} style={{ width: 400, color: "#eee" }}>{nativeText(language, key, nativeText("en", key, ""))}</span>
        </div>;
      })}
    </> : title && body ? <div style={{ position: "absolute", left: 12, top: 75, width: 512, height: 396, background: "rgba(6, 5, 3, .98)" }}>
      <div key={scrollKey} ref={restoreScroll} role="region" aria-label={title} dir={direction} tabIndex={0}
        onScroll={event => scrollOffsets.set(scrollKey, Math.max(0, event.currentTarget.scrollTop))}
        onWheel={event => event.stopPropagation()}
        style={{ height: 374, padding: 12, overflowY: "auto", overscrollBehavior: "contain", overflowWrap: "anywhere" }}>
        <div style={{ fontSize: 16, color: "#ffd133", marginBottom: 12 }}>{title}</div>
        <div style={{ fontSize: bodySize, lineHeight: 1.45, whiteSpace: "pre-wrap", color: "#f0f0e0" }}>{body}</div>
      </div>
      <div dir={direction} style={{ position: "absolute", left: 12, top: 378, width: 488, fontSize: 11, color: "#bfbaa3" }}>
        {nativeText(language, "help.scroll_hint", "Mouse wheel: scroll this page.")}
      </div>
    </div> : <img src={"/original-ui/Help/" + imageIndex + ".png"} alt={heading} draggable={false}
      style={{ position: "absolute", left: 12, top: 75, width: imageWidth, height: imageHeight, pointerEvents: "none" }} />}
    <div style={{ position: "absolute", left: 210, top: 485 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.previous", undefined, "Previous")} onClick={() => changePage(-1)} /></div>
    <div style={{ position: "absolute", left: 230, top: 480, width: 80, height: 20, textAlign: "center", color: "#eee", fontSize: 9 }}>{shownPage + 1} / 45</div>
    <div style={{ position: "absolute", left: 310, top: 485 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.next", undefined, "Next")} onClick={() => changePage(1)} /></div>
  </div>;
}
