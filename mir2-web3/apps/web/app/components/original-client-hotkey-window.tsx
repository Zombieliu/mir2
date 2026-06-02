"use client";

import { type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single key binding row. */
export type HotkeyBinding = {
  /** Stable id (also used as the React key). */
  id: string;
  /** The key combination, e.g. "Alt+C" or "F1". */
  keys: string;
  /** Localization key for the action label. */
  labelKey: string;
  /** English fallback for the action label. */
  labelFallback: string;
};

/** A labelled group of bindings. */
export type HotkeyGroup = {
  titleKey: string;
  titleFallback: string;
  bindings: HotkeyBinding[];
};

export type HotkeyWindowProps = {
  t: TranslateFn;
  /** Optional override; when omitted the built-in default layout is shown. */
  groups?: HotkeyGroup[];
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.gameShop;

/**
 * The default Crystal / Legend of Mir 2 key layout. Kept as data so a host can
 * swap in user-rebound keys later without touching the presentation.
 */
export const DEFAULT_HOTKEY_GROUPS: HotkeyGroup[] = [
  {
    titleKey: "ui.hotkeyGroupWindows",
    titleFallback: "Windows",
    bindings: [
      { id: "character", keys: "Alt + C", labelKey: "ui.character", labelFallback: "Character" },
      { id: "inventory", keys: "Alt + E", labelKey: "ui.inventory", labelFallback: "Inventory" },
      { id: "skills", keys: "Alt + S", labelKey: "ui.skills", labelFallback: "Skills" },
      { id: "quest", keys: "Alt + Q", labelKey: "ui.quest", labelFallback: "Quest Log" },
      { id: "friends", keys: "Alt + F", labelKey: "ui.friend", labelFallback: "Friends" },
      { id: "group", keys: "Alt + G", labelKey: "ui.group", labelFallback: "Group" },
      { id: "guild", keys: "Alt + L", labelKey: "ui.guild", labelFallback: "Guild" },
      { id: "ranking", keys: "Alt + R", labelKey: "ui.ranking", labelFallback: "Ranking" },
    ],
  },
  {
    titleKey: "ui.hotkeyGroupGameplay",
    titleFallback: "Gameplay",
    bindings: [
      { id: "pickup", keys: "Space", labelKey: "ui.hotkeyPickup", labelFallback: "Pick Up Item" },
      { id: "attack", keys: "Ctrl", labelKey: "ui.hotkeyAttack", labelFallback: "Attack In Place" },
      { id: "run", keys: "Shift", labelKey: "ui.hotkeyRun", labelFallback: "Run / Walk Toggle" },
      { id: "sit", keys: "Alt + Z", labelKey: "ui.hotkeySit", labelFallback: "Sit / Rest" },
      { id: "map", keys: "Tab", labelKey: "ui.hotkeyMap", labelFallback: "World Map" },
      { id: "target", keys: "Alt + A", labelKey: "ui.hotkeyTarget", labelFallback: "Target Nearest" },
    ],
  },
  {
    titleKey: "ui.hotkeyGroupBelt",
    titleFallback: "Belt & Skills",
    bindings: [
      { id: "belt1", keys: "F1 - F8", labelKey: "ui.hotkeyBeltSkills", labelFallback: "Skill Belt 1-8" },
      { id: "potions", keys: "1 - 6", labelKey: "ui.hotkeyBeltItems", labelFallback: "Item Belt 1-6" },
      { id: "rotate", keys: "F9", labelKey: "ui.hotkeyBeltRotate", labelFallback: "Rotate Belt" },
    ],
  },
  {
    titleKey: "ui.hotkeyGroupChat",
    titleFallback: "Chat & System",
    bindings: [
      { id: "chat", keys: "Enter", labelKey: "ui.hotkeyChat", labelFallback: "Open Chat" },
      { id: "whisper", keys: "Ctrl + W", labelKey: "ui.hotkeyWhisper", labelFallback: "Reply Whisper" },
      { id: "menu", keys: "Esc", labelKey: "ui.hotkeyMenu", labelFallback: "System Menu" },
      { id: "screenshot", keys: "F12", labelKey: "ui.hotkeyScreenshot", labelFallback: "Screenshot" },
    ],
  },
];

export function HotkeyWindow({ t, groups = DEFAULT_HOTKEY_GROUPS, onClose }: HotkeyWindowProps) {
  return (
    <section aria-label={t("ui.hotkeys", [], "Hotkeys")} style={style.window}>
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.hotkeys", [], "Hotkeys")}</div>
      <div style={style.subtitle}>{t("ui.hotkeySubtitle", [], "Default keyboard layout")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.columns}>
        {groups.map((group) => (
          <section key={group.titleKey} style={style.group}>
            <h3 style={style.groupTitle}>{t(group.titleKey, [], group.titleFallback)}</h3>
            <div style={style.bindings}>
              {group.bindings.map((binding) => (
                <div key={binding.id} style={style.bindingRow} data-hotkey-id={binding.id}>
                  <span style={style.bindingLabel}>{t(binding.labelKey, [], binding.labelFallback)}</span>
                  <kbd style={style.keycap}>{binding.keys}</kbd>
                </div>
              ))}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 164,
    top: 146,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 33,
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
  columns: {
    position: "absolute",
    left: 22,
    top: 52,
    width: 652,
    height: 400,
    display: "grid",
    gridTemplateColumns: "1fr 1fr",
    gap: 12,
    overflowY: "auto",
    alignContent: "start",
  },
  group: {
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: "8px 10px",
  },
  groupTitle: {
    margin: "0 0 6px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    borderBottom: "1px solid rgba(190, 157, 99, 0.24)",
    paddingBottom: 4,
  },
  bindings: { display: "flex", flexDirection: "column", gap: 4 },
  bindingRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    fontSize: 12,
  },
  bindingLabel: { color: "#d6c6a5", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  keycap: {
    flex: "0 0 auto",
    fontFamily: "inherit",
    fontSize: 11,
    color: "#f8e6bb",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.95), rgba(28, 17, 9, 0.95))",
    border: "1px solid rgba(214, 180, 110, 0.6)",
    borderRadius: 3,
    padding: "1px 7px",
    minWidth: 48,
    textAlign: "center",
  },
};
