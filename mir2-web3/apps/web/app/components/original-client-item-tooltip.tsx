"use client";

import type { TranslateFn } from "./original-client-types";

export type ItemTooltipAlign = "right" | "left" | "top";

export type OriginalItemTooltipProps = {
  t: TranslateFn;
  name: string;
  description?: string;
  quantity?: number;
  durabilityCurrent?: number;
  durabilityMax?: number;
  attack?: number;
  defence?: number;
  addedAttack?: number;
  addedDefence?: number;
  weight?: number;
  grade?: string;
  align?: ItemTooltipAlign;
};

// Crystal item-grade name colours.
const GRADE_COLORS: Record<string, string> = {
  common: "#ffffff",
  rare: "#5aa9ff",
  legendary: "#ff9a3c",
  mythical: "#c56bff",
  heroic: "#ff5a4d",
};

export function OriginalItemTooltip({
  t,
  name,
  description,
  quantity,
  durabilityCurrent,
  durabilityMax,
  attack,
  defence,
  addedAttack,
  addedDefence,
  weight,
  grade,
  align = "right",
}: OriginalItemTooltipProps) {
  const descriptionLines = description
    ? description
        .split(/\r?\n/u)
        .map((line) => line.trim())
        .filter(Boolean)
    : [];
  const rows: Array<{ label: string; value: string }> = [];

  if (quantity && quantity > 1) {
    rows.push({ label: t("ui.quantity", [], "Quantity"), value: String(quantity) });
  }
  if (durabilityCurrent !== undefined && durabilityMax !== undefined && durabilityMax > 0) {
    rows.push({
      label: t("ui.durability", [], "Durability"),
      value: `${durabilityCurrent}/${durabilityMax}`,
    });
  }
  const baseAttack = attack ?? 0;
  const bonusAttack = addedAttack ?? 0;
  if (baseAttack > 0 || bonusAttack > 0) {
    rows.push({ label: t("ui.attack", [], "Attack"), value: statValue(baseAttack, bonusAttack) });
  }
  const baseDefence = defence ?? 0;
  const bonusDefence = addedDefence ?? 0;
  if (baseDefence > 0 || bonusDefence > 0) {
    rows.push({ label: t("ui.defence", [], "Defence"), value: statValue(baseDefence, bonusDefence) });
  }
  if (weight !== undefined && weight > 0) {
    rows.push({ label: t("ui.weight", [], "Weight"), value: String(weight) });
  }

  const gradeColor = grade ? GRADE_COLORS[grade] : undefined;

  return (
    <div className={`original-item-tooltip align-${align}`} role="tooltip">
      <strong style={gradeColor ? { color: gradeColor } : undefined}>{name}</strong>
      {descriptionLines.length ? (
        <div className="original-item-tooltip-description">
          {descriptionLines.map((line) => (
            <span key={line}>{line}</span>
          ))}
        </div>
      ) : null}
      {rows.length ? (
        <div className="original-item-tooltip-stats">
          {rows.map((row) => (
            <span key={row.label}>
              <em>{row.label}</em>
              <b>{row.value}</b>
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function statValue(base: number, bonus: number) {
  if (bonus > 0) {
    return `${base} (+${bonus})`;
  }
  return String(base);
}
