"use client";

import { useRef, useState } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { clampCrystalHelpPosition, crystalHelpPage, defaultCrystalKeyBindings,
  displayCrystalKeyBinding, stepCrystalHelpPage, type CrystalHelpWindowPosition, type CrystalKeyBinding } from "../../lib/player-ui-preferences";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (key: string, params?: Array<string | number>, fallback?: string) => string;
export type HelpWindowProps = {
  t: TranslateFn; onClose: () => void;
  /** Root retains page and position across Hide/Show, matching HelpDialogUi. */
  page?: number; position?: CrystalHelpWindowPosition; bindings?: readonly CrystalKeyBinding[];
  onPageChange?: (page: number) => void; onPositionChange?: (position: CrystalHelpWindowPosition) => void;
};
const HELP_PAGE_TITLES = ["Shortcut Information","Shortcut Information","Chat Shortcuts","Movements","Attacking","Collecting Items","Health","Skills","Skills","Mana","Chatting","Groups","Durability","Purchasing","Selling","Repairing","Trading","Inspecting","Statistics","Statistics","Statistics","Statistics","Statistics","Statistics","Quests","Quests","Quests","Quests","Mounts","Mounts","Fishing","Gems and Orbs","Heroes","Heroes","Heroes","Heroes","Heroes","Guild Buffs","Guild Buffs","Guild Buffs","Awakening","Awakening","Awakening","Awakening","Awakening"];
const SHORTCUTS: readonly (readonly (readonly [string, string])[])[] = [[["Exit","Exit the game"],["Logout","Log out"],["Bar1Skill1","Skill buttons"],["Inventory","Inventory window (open / close)"],["Equipment","Status window (open / close)"],["Skills","Skill window (open / close)"],["Group","Group window (open / close)"],["Trade","Trade window (open / close)"],["Friends","Friend window (open / close)"],["Minimap","Minimap window (open / close)"],["Guilds","Guild window (open / close)"],["GameShop","Gameshop window (open / close)"],["Relationship","Engagement window (open / close)"],["Belt","Belt window (open / close)"],["Options","Option window (open / close)"],["Help","Help window (open / close)"],["Mount","Mount / Dismount ride"],["TargetSpellLockOn","Lock spell onto target not cursor location"]],[["ChangePetmode","Toggle pet attack pet"],["ChangeAttackmode","Toggle player attack mode"],["AttackmodePeace","Peace Mode - Attack monsters only"],["AttackmodeGroup","Group Mode - Attack all subjects except your group members"],["AttackmodeGuild","Guild Mode - Attack all subjects except your guild members"],["AttackmodeRedbrown","Good/Evil Mode - Attack PK players and monsters only"],["AttackmodeAll","All Attack Mode - Attack all subjects"],["Bigmap","Show the field map"],["Skillbar","Show the skill bar"],["Autorun","Auto run on / off"],["Cameramode","Show / Hide interface"],["Pickup","Highlight / Pickup Items"],["","Show other players kits"],["Screenshot","Screen Capture"],["Fishing","Open / Close fishing window"],["Mentor","Mentor window (open / close)"],["CreaturePickup","Creature Pickup (Multi Mouse Target)"],["CreatureAutoPickup","Creature Pickup (Single Mouse Target)"]]];
const CHAT_SHORTCUTS = [["/(username)", "Command to whisper to others"], ["!(text)", "Command to shout to others nearby"], ["!~(text)", "Command to guild chat"]] as const;
const DEFAULT_BINDINGS = defaultCrystalKeyBindings();
/** Crystal's three text pages use current bindings; pages 3..44 use original Help/0..41. */
export function HelpWindow({ t, onClose, page, position, bindings = DEFAULT_BINDINGS, onPageChange, onPositionChange }: HelpWindowProps) {
  const [localPage, setLocalPage] = useState(0);
  const [localPosition, setLocalPosition] = useState<CrystalHelpWindowPosition>({ left: 244, top: 129 });
  const shownPage = crystalHelpPage(page ?? localPage);
  const shownPosition = clampCrystalHelpPosition(position ?? localPosition);
  const dragRef = useRef<{ pointerId: number; x: number; y: number; left: number; top: number; scale: number } | null>(null);
  const changePage = (direction: -1 | 1) => {
    const next = stepCrystalHelpPage(shownPage, direction);
    setLocalPage(next); onPageChange?.(next);
  };
  const boundKey = (functionId: string) => {
    const binding = bindings.find(row => row.function === functionId);
    return binding ? displayCrystalKeyBinding(binding) : "";
  };
  const imageIndex = shownPage - 3;
  const imageWidth = imageIndex <= 28 ? 512 : imageIndex === 30 || imageIndex >= 34 ? 508 : 509;
  const imageHeight = imageIndex === 30 || imageIndex >= 34 ? 395 : 396;
  const rows = shownPage < 2 ? SHORTCUTS[shownPage].map(([functionId, label], index) => ({
    key: functionId ? shownPage === 0 && index === 2 ? boundKey(functionId) + "-" + boundKey("Bar1Skill8") : boundKey(functionId) : "Ctrl + Right Click", label,
  })) : CHAT_SHORTCUTS.map(([key, label]) => ({ key, label }));
  return <div className="original-help-window" role="dialog" aria-label={t("ui.help", undefined, "Help")} data-ui-interactive="true"
    style={{ position: "absolute", left: shownPosition.left, top: shownPosition.top, width: 536, height: 509, zIndex: 45 }}>
    <img src="/original-ui/Prguse/920.png" alt="" draggable={false} style={{ position: "absolute", inset: 0, width: 536, height: 509, pointerEvents: "none" }} />
    <img src="/original-ui/Title/57.png" alt={t("ui.help", undefined, "Help")} draggable={false} style={{ position: "absolute", left: 18, top: 9, width: 45, height: 14, pointerEvents: "none" }} />
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
    <div style={{ position: "absolute", left: 147, top: 39, width: 242, height: 30, textAlign: "center", fontSize: 10, color: "#eee" }}>{shownPage + 1}. {HELP_PAGE_TITLES[shownPage]}</div>
    {shownPage < 3 ? <>
      <div style={{ position: "absolute", left: 13, top: 75, fontSize: 10, color: "#eee" }}>{t("ui.shortcuts", undefined, "Shortcuts")}</div>
      <div style={{ position: "absolute", left: 114, top: 75, fontSize: 10, color: "#eee" }}>{t("ui.information", undefined, "Information")}</div>
      {rows.map((row, index) => <div key={index} style={{ position: "absolute", left: 18, top: 107 + index * 20, display: "flex", width: 501, height: 23, fontSize: 9 }}>
        <span style={{ width: 101, flexShrink: 0, color: "#d8c47c" }}>{row.key}</span><span style={{ width: 400, color: "#eee" }}>{row.label}</span>
      </div>)}
    </> : <img src={"/original-ui/Help/" + imageIndex + ".png"} alt={HELP_PAGE_TITLES[shownPage]} draggable={false}
      style={{ position: "absolute", left: 12, top: 75, width: imageWidth, height: imageHeight, pointerEvents: "none" }} />}
    <div style={{ position: "absolute", left: 210, top: 485 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.previousButton} label={t("ui.previous", undefined, "Previous")} onClick={() => changePage(-1)} /></div>
    <div style={{ position: "absolute", left: 230, top: 480, width: 80, height: 20, textAlign: "center", color: "#eee", fontSize: 9 }}>{shownPage + 1} / 45</div>
    <div style={{ position: "absolute", left: 310, top: 485 }}><SpriteButton sprite={ORIGINAL_UI.gameShop.nextButton} label={t("ui.next", undefined, "Next")} onClick={() => changePage(1)} /></div>
  </div>;
}
