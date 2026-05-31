"use client";

import type { DisplayActiveBuff, TranslateFn } from "./original-client-types";

export type BuffBarProps = {
  t: TranslateFn;
  buffs: DisplayActiveBuff[];
};

// Crystal's BuffDialog shows active buffs as an icon row under the minimap.
// The web client has no extracted buff-icon library, so each buff renders as a
// labelled chip with its remaining duration and a tooltip describing its
// bonuses - a functional stand-in for the icon row.
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
        return (
          <div
            key={buff.key}
            className="buff-chip"
            data-buff-key={buff.key}
            title={tooltip}
          >
            <span className="buff-chip-icon" aria-hidden>
              {buffInitials(buff.name)}
            </span>
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
