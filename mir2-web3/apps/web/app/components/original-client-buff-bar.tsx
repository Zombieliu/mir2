"use client";

import type { DisplayActiveBuff, TranslateFn } from "./original-client-types";

export type BuffBarProps = {
  t: TranslateFn;
  buffs: DisplayActiveBuff[];
};

// Crystal's BuffDialog shows active buffs as a BuffIcon row. Map the sim's buff
// keys to the Crystal BuffType -> BuffIcon frame index; unmapped buffs fall back
// to a labelled chip.
const BUFF_ICON_FRAMES: Record<string, number> = {
  "temporal-flux": 0,
  hiding: 1,
  "haste": 2,
  "swift-feet": 3,
  "moon-light": 4,
  "ultimate-enhancer": 5,
  "soul-shield": 6,
  "block-movement": 7,
  "energy-shield": 8,
  "magic-booster": 9,
  "concentration": 10,
  "magic-shield": 13,
  "protection-field": 14,
  "battle-focus": 19,
  "elemental-barrier": 24,
};

export function BuffBar({ t, buffs }: BuffBarProps) {
  if (!buffs.length) {
    return null;
  }

  return (
    <section className="buff-bar" aria-label={t("ui.buffs", [], "Buffs")}>
      {buffs.map((buff) => {
        const bonuses: string[] = [];
        if (buff.attackBonus) bonuses.push(`${t("ui.attack", [], "DC")} +${buff.attackBonus}`);
        if (buff.defenceBonus) bonuses.push(`${t("ui.defence", [], "AC")} +${buff.defenceBonus}`);
        const tooltip = [buff.name, buff.description, bonuses.join(", ")].filter(Boolean).join(" - ");
        const iconFrame = BUFF_ICON_FRAMES[buff.key];
        return (
          <div
            key={buff.key}
            className="buff-chip"
            data-buff-key={buff.key}
            title={tooltip}
          >
            {typeof iconFrame === "number" ? (
              <img className="buff-chip-image" src={`/original-ui/BuffIcon/${iconFrame}.png`} alt="" draggable={false} />
            ) : (
              <span className="buff-chip-icon" aria-hidden>
                {buffInitials(buff.name)}
              </span>
            )}
            <span className="buff-chip-time">{formatBuffRemaining(buff.remainingTicks)}</span>
          </div>
        );
      })}
    </section>
  );
}

function buffInitials(name: string) {
  const words = name.trim().split(/\s+/).filter(Boolean);
  if (!words.length) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return words
    .slice(0, 2)
    .map((word) => word[0]?.toUpperCase() ?? "")
    .join("");
}

function formatBuffRemaining(remainingTicks: number) {
  if (remainingTicks <= 0) return "";
  // World ticks are coarse; show the raw remaining count (compacted for large
  // values) so the chip reads like a timer without claiming a precise second
  // value we do not have.
  if (remainingTicks >= 1000) return `${Math.floor(remainingTicks / 1000)}k`;
  return String(remainingTicks);
}
