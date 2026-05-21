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
  align?: ItemTooltipAlign;
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
  if (attack !== undefined && attack > 0) {
    rows.push({ label: t("ui.attack", [], "Attack"), value: String(attack) });
  }
  if (defence !== undefined && defence > 0) {
    rows.push({ label: t("ui.defence", [], "Defence"), value: String(defence) });
  }

  return (
    <div className={`original-item-tooltip align-${align}`} role="tooltip">
      <strong>{name}</strong>
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
