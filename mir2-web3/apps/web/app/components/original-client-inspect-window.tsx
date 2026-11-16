"use client";

import { useMemo } from "react";
import { ORIGINAL_UI } from "../../lib/original-ui";
import type { RankingInspectEquipment, RankingPlayerInspect } from "../../lib/shared-ranking-inspect";
import type { CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import { originalItemIconPath } from "./original-client-inventory-utils";
import { OriginalCrystalItemTooltip } from "./original-client-crystal-item-tooltip";
import { OriginalItemTooltip } from "./original-client-item-tooltip";
import { SpriteButton } from "./original-client-overlays";
import { useActiveItemTooltip } from "./original-client-panels";

export type OriginalInspectWindowProps = {
  t: (key: string, params?: Array<string | number>, fallback?: string) => string;
  info: RankingPlayerInspect;
  onClose: () => void;
  onReadItemTooltip?: (item: RankingInspectEquipment) => CrystalTooltipDocument | null;
};

// Crystal/Shared/Enums.cs EquipmentSlot, distinct from the visual layout order.
const slotIndexes = { Weapon: 0, Armour: 1, Helmet: 2, Torch: 3, Necklace: 4, BraceletL: 5,
  BraceletR: 6, RingL: 7, RingR: 8, Amulet: 9, Belt: 10, Boots: 11, Stone: 12, Mount: 13 } as const;

/** Read-only InspectDialog; the displayed items never acquire inventory actions. */
export function OriginalInspectWindow({ t, info, onClose, onReadItemTooltip }: OriginalInspectWindowProps) {
  const items = useMemo(() => info.equipment.filter((item): item is RankingInspectEquipment => item !== null), [info]);
  const tooltip = useActiveItemTooltip(items, onReadItemTooltip);
  const classKey = info.class.toLowerCase() as keyof typeof ORIGINAL_UI.character.classIcons;
  return <section className="window-shell character-window inspect-window" role="dialog"
    aria-label={t("ui.inspectPlayer", [info.name], `Inspect ${info.name}`)}
    data-inspect-name={info.name} data-inspect-class={info.class} data-inspect-gender={info.gender}
    data-inspect-hair={info.hair} data-inspect-allow-observe={info.allowObserve} style={{ zIndex: 35 }}>
    <img className="window-frame" src="/original-ui/Prguse/430.png" alt="" draggable={false} />
    <img className="character-page" src={info.gender === "Female" ? "/original-ui/Prguse/341.png" : ORIGINAL_UI.character.pages.char}
      style={{ top: 70 }} alt="" draggable={false} />
    <div className="character-close"><SpriteButton sprite={ORIGINAL_UI.character.closeButton}
      label={t("ui.close", [], "Close")} onClick={onClose} /></div>
    <div className="character-name">{info.name}</div>
    <div className="character-guild">
      <img src={ORIGINAL_UI.character.classIcons[classKey]} alt={info.class} draggable={false}
        style={{ width: 16, height: 16, marginRight: 5, verticalAlign: "middle", imageRendering: "pixelated" }} />
      <span>{info.class} · {t("ui.heroLevel", [info.level], `Level ${info.level}`)}</span>
    </div>
    <div style={{ position: "absolute", top: 51, left: 0, width: 264, textAlign: "center", fontSize: 10,
      color: "#caa64a", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
      <span data-inspect-guild>{info.guildName}</span>{info.guildRank ? " · " : ""}<span data-inspect-rank>{info.guildRank}</span>
    </div>
    {info.loverName ? <span data-inspect-lover title={info.loverName} style={{ position: "absolute", left: 8, top: 337,
      width: 248, textAlign: "center", color: "#e0abc8", fontSize: 10 }}>
      {t("ui.lover", [], "Lover")}: {info.loverName}
    </span> : null}
    {ORIGINAL_UI.character.equipmentSlots.map(slot => {
      if (!Object.hasOwn(slotIndexes, slot.label)) return null;
      const slotIndex = slotIndexes[slot.label as keyof typeof slotIndexes], item = info.equipment[slotIndex];
      const tooltipDocument = item ? tooltip.document(item) : null;
      return <div key={slotIndex} className="character-slot" data-inspect-slot={slotIndex}
        style={{ left: slot.x + 8, top: slot.y + 70 }} aria-label={slot.label} title={slot.label}>
        {item ? <button type="button" className="character-slot-card" aria-label={item.name}
          onPointerEnter={event => { if (event.pointerType !== "touch") tooltip.activate(item, event.currentTarget); }}
          onPointerLeave={event => { if (event.pointerType === "touch" || document.activeElement !== event.currentTarget) tooltip.release(item, event.currentTarget); }}
          onFocus={event => tooltip.activate(item, event.currentTarget)}
          onBlur={event => { if (!event.currentTarget.matches(":hover")) tooltip.release(item, event.currentTarget); }}
          onPointerCancel={event => tooltip.release(item, event.currentTarget)}
          onPointerDown={event => { if (event.pointerType === "touch") { tooltip.activate(item, event.currentTarget); event.stopPropagation(); } }}>
          <img className="original-item-icon character-item-icon" src={originalItemIconPath(item.icon)} alt="" draggable={false} />
          {tooltipDocument ? <OriginalCrystalItemTooltip document={tooltipDocument} align={slot.x > 110 ? "left" : "right"} />
            : <OriginalItemTooltip t={t} name={item.name} align={slot.x > 110 ? "left" : "right"} />}
        </button> : null}
      </div>;
    })}
  </section>;
}
