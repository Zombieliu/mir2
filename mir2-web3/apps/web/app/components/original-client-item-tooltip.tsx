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
  addedStats?: Array<{ stat: number; value: number }>;
  requiredType?: number;
  requiredClass?: number;
  requiredAmount?: number;
  align?: ItemTooltipAlign;
};

// Crystal RequiredType enum -> label.
const REQUIRED_TYPE_LABELS: Record<number, string> = {
  0: "Level",
  1: "AC",
  2: "MAC",
  3: "DC",
  4: "MC",
  5: "SC",
  6: "Max Level",
  7: "AC",
  8: "MAC",
  9: "DC",
  10: "MC",
  11: "SC",
};

// Crystal RequiredClass bitmask.
const REQUIRED_CLASS_FLAGS: Array<{ flag: number; label: string }> = [
  { flag: 1, label: "War" },
  { flag: 2, label: "Wiz" },
  { flag: 4, label: "Tao" },
  { flag: 8, label: "Sin" },
  { flag: 16, label: "Archer" },
];

function requiredClassLabel(mask: number): string | null {
  if (!mask || mask === 31) return null; // 0 or all-classes => no restriction
  const names = REQUIRED_CLASS_FLAGS.filter((entry) => (mask & entry.flag) !== 0).map((entry) => entry.label);
  return names.length ? names.join("/") : null;
}

// Crystal item-grade name colours.
const GRADE_COLORS: Record<string, string> = {
  common: "#ffffff",
  rare: "#5aa9ff",
  legendary: "#ff9a3c",
  mythical: "#c56bff",
  heroic: "#ff5a4d",
};

// Crystal Stat enum index -> label for item bonus stats. DC (4-6) and AC (0-1)
// are intentionally omitted: the Attack/Defence rows already show those.
const BONUS_STAT_LABELS: Record<number, string> = {
  2: "MAC",
  3: "MAC",
  7: "MC",
  8: "MC",
  9: "MC",
  10: "SC",
  11: "SC",
  12: "SC",
  13: "HP",
  14: "MP",
  15: "Accuracy",
  16: "Agility",
  17: "Luck",
  19: "Haste",
  38: "A.Speed",
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
  addedStats,
  requiredType,
  requiredClass,
  requiredAmount,
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
  if (addedStats?.length) {
    const bonusByLabel = new Map<string, number>();
    for (const entry of addedStats) {
      if (!entry || !entry.value) continue;
      const label = BONUS_STAT_LABELS[entry.stat];
      if (!label) continue;
      const previous = bonusByLabel.get(label) ?? 0;
      if (Math.abs(entry.value) > Math.abs(previous)) {
        bonusByLabel.set(label, entry.value);
      }
    }
    for (const [label, value] of bonusByLabel) {
      rows.push({ label, value: value > 0 ? `+${value}` : String(value) });
    }
  }
  if (weight !== undefined && weight > 0) {
    rows.push({ label: t("ui.weight", [], "Weight"), value: String(weight) });
  }

  // Equip requirements render as their own (red-tinted) rows below the stats.
  const requirementRows: Array<{ label: string; value: string }> = [];
  if (requiredAmount !== undefined && requiredAmount > 0) {
    const reqLabel = requiredType !== undefined ? REQUIRED_TYPE_LABELS[requiredType] : undefined;
    requirementRows.push({
      label: t("ui.require", [], "Requires"),
      value: `${reqLabel ?? t("ui.level", [], "Level")} ${requiredAmount}`,
    });
  }
  const classLabel = requiredClass !== undefined ? requiredClassLabel(requiredClass) : null;
  if (classLabel) {
    requirementRows.push({ label: t("ui.class", [], "Class"), value: classLabel });
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
      {requirementRows.length ? (
        <div className="original-item-tooltip-requirements">
          {requirementRows.map((row) => (
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
