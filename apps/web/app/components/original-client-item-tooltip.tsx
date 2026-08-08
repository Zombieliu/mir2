"use client";

import type { CSSProperties } from "react";

import type { TranslateFn } from "./original-client-types";

export type ItemTooltipAlign = "right" | "left" | "top";

/**
 * Crystal item grades. The colour of the item name in the original client is
 * driven by the grade, so callers that know the grade get a faithful coloured
 * header for free; callers that do not pass a grade fall back to "common".
 */
export type ItemTooltipGrade =
  | "common"
  | "rare"
  | "heroic"
  | "legendary"
  | "mythical";

/** A min~max stat range, e.g. DC 3~5. */
export type StatRange = { min?: number; max?: number };

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

  // --- Additive Crystal tooltip fields (all optional) ---------------------
  /** Item rarity/grade; colours the name like the original client. */
  grade?: ItemTooltipGrade;
  /** Localised item type label, e.g. "Weapon" / "Armour". */
  itemType?: string;
  /** Item weight in the same units the client shows in the weight bar. */
  weight?: number;

  // Equipment stat ranges (rendered as "AC + min~max" like the client).
  ac?: StatRange;
  mac?: StatRange;
  dc?: StatRange;
  mc?: StatRange;
  sc?: StatRange;

  // Flat secondary bonuses.
  accuracy?: number;
  agility?: number;
  luck?: number;
  holy?: number;
  magicResist?: number;
  poisonResist?: number;
  maxHpBonus?: number;
  maxMpBonus?: number;
  attackSpeed?: number;
  handWeightBonus?: number;
  bagWeightBonus?: number;

  // Requirements to equip / use the item.
  requiredLevel?: number;
  requiredClass?: string;
  requiredGender?: string;
  requiredStat?: { label: string; value: number };

  // Bind / seal state.
  /** Name the item is soulbound to (shows "Soulbound to: <name>"). */
  boundTo?: string;
  /** When true, shows the generic "SoulBinds on equip" line. */
  bindsOnEquip?: boolean;
  /** Localised remaining-seal duration, e.g. "Sealed for 3 days". */
  sealedFor?: string;
  /** When true, shows the "Hold CTRL and left click to seal" hint. */
  canSeal?: boolean;
};

const GRADE_COLOUR: Record<ItemTooltipGrade, string> = {
  common: "#f4ecd6",
  rare: "#6fc3ff",
  heroic: "#7be07a",
  legendary: "#f0b94a",
  mythical: "#e07ad0",
};

const GRADE_LABEL_KEY: Record<ItemTooltipGrade, { key: string; fallback: string }> = {
  common: { key: "client.ItemGradeCommon", fallback: "Common" },
  rare: { key: "client.ItemGradeRare", fallback: "Rare" },
  heroic: { key: "client.ItemGradeHeroic", fallback: "Heroic" },
  legendary: { key: "client.ItemGradeLegendary", fallback: "Legendary" },
  mythical: { key: "client.ItemGradeMythical", fallback: "Mythical" },
};

export function OriginalItemTooltip(props: OriginalItemTooltipProps) {
  const {
    t,
    name,
    description,
    quantity,
    durabilityCurrent,
    durabilityMax,
    attack,
    defence,
    align = "right",
    grade,
    itemType,
    weight,
  } = props;

  const descriptionLines = description
    ? description
        .split(/\r?\n/u)
        .map((line) => line.trim())
        .filter(Boolean)
    : [];

  // Primary stat rows (combat / defence ranges). These mirror the labels the
  // original Crystal client renders, falling back to the bundled English text.
  const statRows = buildStatRows(props);

  // Secondary "header" facts (type / weight) shown directly under the name.
  const facts: string[] = [];
  if (itemType) {
    facts.push(t("client.ItemTooltipType", [itemType], `Type: ${itemType}`));
  }
  if (typeof weight === "number" && weight > 0) {
    facts.push(t("client.ItemTooltipWeight", [weight], `Weight: ${weight}`));
  }

  // Backward-compatible simple stat rows (legacy callers pass attack/defence).
  const legacyRows: Array<{ label: string; value: string }> = [];
  if (quantity && quantity > 1) {
    legacyRows.push({ label: t("ui.quantity", [], "Quantity"), value: String(quantity) });
  }
  if (statRows.length === 0 && attack !== undefined && attack > 0) {
    legacyRows.push({ label: t("ui.attack", [], "Attack"), value: String(attack) });
  }
  if (statRows.length === 0 && defence !== undefined && defence > 0) {
    legacyRows.push({ label: t("ui.defence", [], "Defence"), value: String(defence) });
  }

  const durabilityRow =
    durabilityCurrent !== undefined && durabilityMax !== undefined && durabilityMax > 0
      ? `${t("client.Durability", [], "Durability:")} ${durabilityCurrent}/${durabilityMax}`
      : null;

  const requirementRows = buildRequirementRows(props);
  const bindRows = buildBindRows(props);

  const headerColour = grade ? GRADE_COLOUR[grade] : GRADE_COLOUR.common;
  const gradeLabel = grade ? t(GRADE_LABEL_KEY[grade].key, [], GRADE_LABEL_KEY[grade].fallback) : null;

  return (
    <div className={`original-item-tooltip align-${align}`} role="tooltip" data-item-grade={grade ?? "common"}>
      <strong style={{ color: headerColour }}>{name}</strong>
      {gradeLabel ? (
        <span className="original-item-tooltip-grade" style={{ ...GRADE_STYLE, color: headerColour }}>
          {gradeLabel}
        </span>
      ) : null}

      {facts.length ? (
        <div className="original-item-tooltip-facts" style={SECTION_STYLE}>
          {facts.map((fact) => (
            <span key={fact} style={BLOCK_LINE_STYLE}>
              {fact}
            </span>
          ))}
        </div>
      ) : null}

      {descriptionLines.length ? (
        <div className="original-item-tooltip-description">
          {descriptionLines.map((line) => (
            <span key={line}>{line}</span>
          ))}
        </div>
      ) : null}

      {statRows.length ? (
        <div className="original-item-tooltip-statlines" style={DIVIDED_SECTION_STYLE}>
          {statRows.map((line) => (
            <span key={line} style={{ ...BLOCK_LINE_STYLE, ...STAT_LINE_STYLE }}>
              {line}
            </span>
          ))}
        </div>
      ) : null}

      {legacyRows.length ? (
        <div className="original-item-tooltip-stats">
          {legacyRows.map((row) => (
            <span key={row.label}>
              <em>{row.label}</em>
              <b>{row.value}</b>
            </span>
          ))}
        </div>
      ) : null}

      {durabilityRow ? (
        <div className="original-item-tooltip-durability" style={{ ...SECTION_STYLE, color: "#c9b98f" }}>
          {durabilityRow}
        </div>
      ) : null}

      {requirementRows.length ? (
        <div className="original-item-tooltip-requirements" style={DIVIDED_SECTION_STYLE}>
          {requirementRows.map((line) => (
            <span key={line} style={{ ...BLOCK_LINE_STYLE, ...REQUIREMENT_LINE_STYLE }}>
              {line}
            </span>
          ))}
        </div>
      ) : null}

      {bindRows.length ? (
        <div className="original-item-tooltip-bind" style={SECTION_STYLE}>
          {bindRows.map((line) => (
            <span key={line} style={{ ...BLOCK_LINE_STYLE, ...BIND_LINE_STYLE }}>
              {line}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

const SECTION_STYLE: CSSProperties = { display: "grid", gap: 2, marginTop: 4 };
const DIVIDED_SECTION_STYLE: CSSProperties = {
  display: "grid",
  gap: 2,
  marginTop: 6,
  paddingTop: 5,
  borderTop: "1px solid rgba(185, 150, 88, 0.34)",
};
const BLOCK_LINE_STYLE: CSSProperties = { display: "block" };
const GRADE_STYLE: CSSProperties = {
  display: "block",
  marginTop: -2,
  marginBottom: 2,
  fontSize: 10,
  fontWeight: 700,
  letterSpacing: 0.4,
  textTransform: "uppercase",
};
const STAT_LINE_STYLE: CSSProperties = { color: "#9be6ff" };
const REQUIREMENT_LINE_STYLE: CSSProperties = { color: "#e6b86f" };
const BIND_LINE_STYLE: CSSProperties = { color: "#cf8de0" };

function rangeText(t: TranslateFn, key: string, fallbackLabel: string, range?: StatRange): string | null {
  if (!range) return null;
  const min = range.min ?? 0;
  const max = range.max ?? min;
  if (min <= 0 && max <= 0) return null;
  return t(key, [min, max], `${fallbackLabel} + ${min}~${max}`);
}

function flatText(t: TranslateFn, key: string, fallback: (value: number) => string, value?: number): string | null {
  if (value === undefined || value === 0) return null;
  return t(key, [value], fallback(value));
}

function buildStatRows(props: OriginalItemTooltipProps): string[] {
  const { t } = props;
  const rows: Array<string | null> = [
    rangeText(t, "client.DC", "DC", props.dc),
    rangeText(t, "client.MC", "MC", props.mc),
    rangeText(t, "client.SC", "SC", props.sc),
    rangeText(t, "client.AC", "AC", props.ac),
    rangeText(t, "client.MAC", "MAC", props.mac),
    flatText(t, "client.Accuracy", (v) => `Accuracy: + ${v}`, props.accuracy),
    flatText(t, "client.Agility", (v) => `Agility: + ${v}`, props.agility),
    flatText(t, "client.Luck", (v) => `Luck + ${v}`, props.luck),
    flatText(t, "client.Holy", (v) => `Holy: + ${v}`, props.holy),
    flatText(t, "client.MagicResistPlus", (v) => `Magic Resist + ${v}`, props.magicResist),
    flatText(t, "client.PoisonResistPlus", (v) => `Poison Resist + ${v}`, props.poisonResist),
    flatText(t, "client.MaxHpPlus", (v) => `Max HP + ${v}`, props.maxHpBonus),
    flatText(t, "client.MaxMpPlus", (v) => `Max MP + ${v}`, props.maxMpBonus),
    flatText(t, "client.AddAttackSpeed", (v) => `Adds +${v} A.Speed`, props.attackSpeed),
    flatText(t, "client.HandWeightPlus", (v) => `Hand Weight + ${v}`, props.handWeightBonus),
    flatText(t, "client.BagWeightPlus", (v) => `Bag Weight + ${v}`, props.bagWeightBonus),
  ];
  return rows.filter((row): row is string => row !== null);
}

function buildRequirementRows(props: OriginalItemTooltipProps): string[] {
  const { t } = props;
  const rows: string[] = [];
  if (props.requiredLevel !== undefined && props.requiredLevel > 0) {
    rows.push(t("client.RequiredLevel", [props.requiredLevel], `Required Level : ${props.requiredLevel}`));
  }
  if (props.requiredClass) {
    rows.push(t("client.ClassRequired", [props.requiredClass], `Class Required : ${props.requiredClass}`));
  }
  if (props.requiredGender) {
    rows.push(t("ui.genderRequired", [props.requiredGender], `Gender Required : ${props.requiredGender}`));
  }
  if (props.requiredStat && props.requiredStat.value > 0) {
    rows.push(`${props.requiredStat.label} : ${props.requiredStat.value}`);
  }
  return rows;
}

function buildBindRows(props: OriginalItemTooltipProps): string[] {
  const { t } = props;
  const rows: string[] = [];
  if (props.boundTo) {
    rows.push(`${t("client.SoulboundTo", [], "Soulbound to: ")}${props.boundTo}`);
  } else if (props.bindsOnEquip) {
    rows.push(t("client.SoulBindsOnEquip", [], "SoulBinds on equip"));
  }
  if (props.sealedFor) {
    rows.push(t("client.SealedFor", [props.sealedFor], `Sealed for ${props.sealedFor}`));
  } else if (props.canSeal) {
    rows.push(t("client.HoldCtrlSealItem", [], "Hold CTRL and left click to seal an item."));
  }
  return rows;
}
