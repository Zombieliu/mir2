"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A single stat modifier line for a buff's tooltip (e.g. "+15 AC"). */
export type BuffStat = {
  /** Localized stat label, e.g. "AC" / "Max DC" / "Attack Speed". */
  label: string;
  value: number;
  /** Optional suffix the original client appends ("%" for Percent, "x" for Multiplier). */
  suffix?: string;
};

/**
 * Mirrors a single entry of the world's `activeBuffs` list, plus the richer
 * fields the Crystal {@link BuffDialog} renders (icon, infinite/paused state,
 * caster, structured stat modifiers). Every extra field is optional so the
 * existing `{ key, name, remainingTicks, attackBonus, defenceBonus }` payload
 * keeps working unchanged.
 */
export type BuffEntry = {
  key: string;
  name: string;
  description?: string;
  /** Remaining duration in server ticks; <= 0 (or omitted) means permanent. */
  remainingTicks?: number;
  attackBonus?: number;
  defenceBonus?: number;
  /** Icon index into `/original-ui/BuffIcon/<icon>.png` (or the chosen library). */
  icon?: number;
  /** Library the icon belongs to (Crystal switches by image range). */
  iconLibrary?: "buff" | "magic" | "prguse2";
  /** Never-expiring buff (Crystal `Infinite`) — overrides remaining time. */
  infinite?: boolean;
  /** Expiry timer paused (Crystal `Paused`). */
  paused?: boolean;
  /** Name of the caster/source ("Caster: X"). */
  caster?: string;
  /** Whether this is a beneficial buff or a harmful debuff/poison. */
  kind?: "buff" | "debuff";
  /** Structured stat modifiers for the tooltip, when the host can supply them. */
  stats?: BuffStat[];
};

export type BuffWindowProps = {
  t: TranslateFn;
  buffs: BuffEntry[];
  /** Server ticks per second, used to render the remaining time. Defaults to 10. */
  ticksPerSecond?: number;
  /** Remove / cancel a (removable) buff. Hidden entirely when not supplied. */
  onRemoveBuff?: (key: string) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.character;

const ICON_LIBRARY_PATH: Record<NonNullable<BuffEntry["iconLibrary"]>, string> = {
  buff: "/original-ui/BuffIcon",
  magic: "/original-ui/MagIcon",
  prguse2: "/original-ui/Prguse2",
};

export function BuffWindow({ t, buffs, ticksPerSecond = 10, onRemoveBuff, onClose }: BuffWindowProps) {
  const ordered = useMemo(() => buffs.filter((buff) => buff && buff.name), [buffs]);
  const positive = useMemo(() => ordered.filter((buff) => buff.kind !== "debuff"), [ordered]);
  const negative = useMemo(() => ordered.filter((buff) => buff.kind === "debuff"), [ordered]);
  const [selectedKey, setSelectedKey] = useState<string | null>(ordered[0]?.key ?? null);
  const [hoverKey, setHoverKey] = useState<string | null>(null);

  useEffect(() => {
    if (ordered.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (!selectedKey || !ordered.some((buff) => buff.key === selectedKey)) {
      setSelectedKey(ordered[0].key);
    }
  }, [ordered, selectedKey]);

  const selected = ordered.find((buff) => buff.key === selectedKey) ?? null;
  const hovered = ordered.find((buff) => buff.key === hoverKey) ?? null;
  const tooltip = hovered ?? selected;

  return (
    <section
      aria-label={t("ui.buffs", [], "Buffs")}
      data-buff-count={ordered.length}
      data-buff-selected={selected?.key ?? ""}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.buffs", [], "Buffs")}</div>
      <div style={style.subtitle}>{t("ui.buffActiveCount", [ordered.length], `${ordered.length} active`)}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.grids}>
        {ordered.length === 0 ? (
          <div style={style.empty}>{t("ui.buffEmpty", [], "No active buffs.")}</div>
        ) : (
          <>
            <BuffSection
              t={t}
              title={t("ui.buffBeneficial", [], "Buffs")}
              buffs={positive}
              ticksPerSecond={ticksPerSecond}
              selectedKey={selected?.key ?? null}
              onSelect={setSelectedKey}
              onHover={setHoverKey}
            />
            {negative.length > 0 ? (
              <BuffSection
                t={t}
                title={t("ui.buffHarmful", [], "Debuffs")}
                buffs={negative}
                ticksPerSecond={ticksPerSecond}
                selectedKey={selected?.key ?? null}
                onSelect={setSelectedKey}
                onHover={setHoverKey}
                debuff
              />
            ) : null}
          </>
        )}
      </div>

      <div style={style.detail} data-buff-detail={tooltip?.key ?? ""}>
        {tooltip ? (
          <>
            <div style={style.detailHead}>
              <span style={style.detailName}>{tooltip.name}</span>
              {tooltip.kind === "debuff" ? (
                <span style={style.detailKindDebuff}>{t("ui.buffHarmfulTag", [], "Debuff")}</span>
              ) : null}
            </div>
            <div style={style.detailTime}>{formatRemaining(t, tooltip, ticksPerSecond)}</div>
            {tooltip.description ? <p style={style.detailDesc}>{tooltip.description}</p> : null}
            <div style={style.bonusRow}>{renderStatLines(t, tooltip)}</div>
            {tooltip.caster ? (
              <div style={style.caster}>{t("ui.buffCaster", [tooltip.caster], `Caster: ${tooltip.caster}`)}</div>
            ) : null}
          </>
        ) : (
          <div style={style.empty}>{t("ui.buffSelectHint", [], "Hover a buff to see its effect.")}</div>
        )}
      </div>

      {onRemoveBuff ? (
        <div style={style.actions}>
          <button
            type="button"
            disabled={!selected}
            style={{ ...style.actionButton, ...(!selected ? style.actionButtonDisabled : null) }}
            onClick={() => selected && onRemoveBuff(selected.key)}
          >
            {t("ui.buffRemove", [], "Remove")}
          </button>
        </div>
      ) : null}
    </section>
  );
}

function BuffSection({
  t,
  title,
  buffs,
  ticksPerSecond,
  selectedKey,
  onSelect,
  onHover,
  debuff,
}: {
  t: TranslateFn;
  title: string;
  buffs: BuffEntry[];
  ticksPerSecond: number;
  selectedKey: string | null;
  onSelect: (key: string) => void;
  onHover: (key: string | null) => void;
  debuff?: boolean;
}) {
  if (buffs.length === 0) return null;
  return (
    <div style={style.section}>
      <div style={style.sectionTitle}>{title}</div>
      <div style={style.iconGrid} role="list">
        {buffs.map((buff) => {
          const isSelected = selectedKey === buff.key;
          const iconSrc = buffIconPath(buff);
          const timeLabel = shortRemaining(buff, ticksPerSecond);
          return (
            <button
              key={buff.key}
              type="button"
              role="listitem"
              data-buff-key={buff.key}
              aria-pressed={isSelected}
              title={tooltipText(t, buff, ticksPerSecond)}
              onClick={() => onSelect(buff.key)}
              onMouseEnter={() => onHover(buff.key)}
              onMouseLeave={() => onHover(null)}
              style={{
                ...style.iconCell,
                ...(debuff ? style.iconCellDebuff : null),
                ...(isSelected ? style.iconCellSelected : null),
                ...(buff.paused ? style.iconCellPaused : null),
              }}
            >
              {iconSrc ? (
                <img style={style.iconImg} src={iconSrc} alt="" draggable={false} />
              ) : (
                <span style={style.iconLabel}>{initials(buff.name)}</span>
              )}
              <span style={style.iconTime}>
                {buff.infinite ? "∞" : buff.paused ? "∥" : timeLabel}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function renderStatLines(t: TranslateFn, buff: BuffEntry) {
  const lines: BuffStat[] = [];
  if (Array.isArray(buff.stats) && buff.stats.length > 0) {
    lines.push(...buff.stats.filter((stat) => stat && typeof stat.value === "number" && stat.value !== 0));
  } else {
    if (typeof buff.attackBonus === "number" && buff.attackBonus !== 0) {
      lines.push({ label: t("ui.attack", [], "Attack"), value: buff.attackBonus });
    }
    if (typeof buff.defenceBonus === "number" && buff.defenceBonus !== 0) {
      lines.push({ label: t("ui.defence", [], "Defence"), value: buff.defenceBonus });
    }
  }
  if (lines.length === 0) return null;
  return lines.map((stat, index) => {
    const isPositive = stat.value > 0;
    return (
      <span key={`${stat.label}-${index}`} style={{ ...style.bonus, color: isPositive ? "#8be07a" : "#d8552f" }}>
        {`${stat.label} ${isPositive ? "+" : ""}${stat.value}${stat.suffix ?? ""}`}
      </span>
    );
  });
}

function buffIconPath(buff: BuffEntry): string | null {
  if (typeof buff.icon !== "number" || buff.icon < 0) return null;
  const base = ICON_LIBRARY_PATH[buff.iconLibrary ?? "buff"];
  return `${base}/${buff.icon}.png`;
}

function initials(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "?";
  const words = trimmed.split(/\s+/);
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}

/** Builds the multi-line hover hint that mirrors Crystal's `BuffString`. */
function tooltipText(t: TranslateFn, buff: BuffEntry, ticksPerSecond: number): string {
  const lines: string[] = [buff.name];
  if (buff.description) lines.push(buff.description);
  if (Array.isArray(buff.stats)) {
    for (const stat of buff.stats) {
      if (!stat || typeof stat.value !== "number" || stat.value === 0) continue;
      lines.push(`${stat.value > 0 ? "+" : ""}${stat.value}${stat.suffix ?? ""} ${stat.label}`);
    }
  } else {
    if (typeof buff.attackBonus === "number" && buff.attackBonus !== 0) {
      lines.push(`${buff.attackBonus > 0 ? "+" : ""}${buff.attackBonus} ${t("ui.attack", [], "Attack")}`);
    }
    if (typeof buff.defenceBonus === "number" && buff.defenceBonus !== 0) {
      lines.push(`${buff.defenceBonus > 0 ? "+" : ""}${buff.defenceBonus} ${t("ui.defence", [], "Defence")}`);
    }
  }
  lines.push(formatRemaining(t, buff, ticksPerSecond));
  if (buff.caster) lines.push(t("ui.buffCaster", [buff.caster], `Caster: ${buff.caster}`));
  return lines.join("\n");
}

function formatRemaining(t: TranslateFn, buff: BuffEntry, ticksPerSecond: number): string {
  if (buff.paused) return t("ui.buffPaused", [], "Paused");
  // No explicit timer (or infinite flag) means the buff never expires.
  if (buff.infinite || buff.remainingTicks === undefined || buff.remainingTicks <= 0) {
    return t("ui.buffPermanent", [], "Permanent");
  }
  const label = secondsLabel(buff.remainingTicks, ticksPerSecond);
  return t("ui.buffExpire", [label], `Expires in ${label}`);
}

function secondsLabel(ticks: number, ticksPerSecond: number): string {
  const totalSeconds = Math.max(0, Math.round(ticks / Math.max(1, ticksPerSecond)));
  return printTimeSpan(totalSeconds);
}

/** Short label for the icon corner badge. */
function shortRemaining(buff: BuffEntry, ticksPerSecond: number): string {
  if (buff.infinite || buff.remainingTicks === undefined || buff.remainingTicks <= 0) return "";
  const totalSeconds = Math.max(0, Math.round(buff.remainingTicks / Math.max(1, ticksPerSecond)));
  if (totalSeconds >= 3600) return `${Math.floor(totalSeconds / 3600)}h`;
  if (totalSeconds >= 60) return `${Math.floor(totalSeconds / 60)}m`;
  return `${totalSeconds}s`;
}

function printTimeSpan(totalSeconds: number): string {
  if (totalSeconds >= 3600) {
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    return `${hours}h ${minutes}m`;
  }
  if (totalSeconds >= 60) {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${minutes}m ${seconds}s`;
  }
  return `${totalSeconds}s`;
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 560,
    top: 150,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 31,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  titleText: {
    position: "absolute",
    left: 16,
    top: 9,
    fontSize: 13,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 16, top: 27, fontSize: 10, color: "#cbb38a" },
  close: { position: "absolute", left: 236, top: 6 },
  grids: {
    position: "absolute",
    left: 12,
    top: 46,
    width: 240,
    height: 184,
    display: "flex",
    flexDirection: "column",
    gap: 6,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.3)",
    background: "rgba(11, 8, 5, 0.42)",
    padding: 6,
  },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  section: { display: "flex", flexDirection: "column", gap: 4 },
  sectionTitle: {
    fontSize: 9,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
    borderBottom: "1px solid rgba(190, 157, 99, 0.24)",
    paddingBottom: 2,
  },
  iconGrid: { display: "flex", flexWrap: "wrap", gap: 4 },
  iconCell: {
    position: "relative",
    width: 34,
    height: 34,
    padding: 0,
    border: "1px solid rgba(190, 157, 99, 0.45)",
    background: "rgba(20, 13, 7, 0.55)",
    color: "#e3d3af",
    cursor: "pointer",
    overflow: "hidden",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  iconCellDebuff: { borderColor: "rgba(181, 86, 58, 0.7)", background: "rgba(40, 16, 11, 0.55)" },
  iconCellSelected: { borderColor: "rgba(214, 180, 110, 0.95)", boxShadow: "0 0 0 1px rgba(214, 180, 110, 0.6) inset" },
  iconCellPaused: { opacity: 0.6 },
  iconImg: { width: 30, height: 30, imageRendering: "pixelated" },
  iconLabel: { fontSize: 11, fontWeight: 700, color: "#f0d69b" },
  iconTime: {
    position: "absolute",
    right: 1,
    bottom: 0,
    fontSize: 8,
    lineHeight: "9px",
    padding: "0 1px",
    color: "#f8e6bb",
    background: "rgba(0, 0, 0, 0.6)",
  },
  detail: {
    position: "absolute",
    left: 12,
    top: 238,
    width: 240,
    height: 96,
    overflow: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  detailHead: { display: "flex", alignItems: "baseline", justifyContent: "space-between", gap: 6, marginBottom: 2 },
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700 },
  detailKindDebuff: { fontSize: 9, color: "#e08a6a", flex: "0 0 auto" },
  detailTime: { fontSize: 10, color: "#cbb38a", marginBottom: 4 },
  detailDesc: { margin: "0 0 4px", fontSize: 11, color: "#d6c6a5", lineHeight: 1.3 },
  bonusRow: { display: "flex", flexWrap: "wrap", gap: 8, fontSize: 11, fontWeight: 700 },
  bonus: {},
  caster: { marginTop: 4, fontSize: 10, color: "#b7a884" },
  actions: { position: "absolute", left: 12, top: 340, width: 240, display: "flex", gap: 6 },
  actionButton: {
    flex: 1,
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "4px 0",
    fontSize: 11,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
