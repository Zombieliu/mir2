"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** Mirrors a single entry of the world's `activeBuffs` list. */
export type BuffEntry = {
  key: string;
  name: string;
  description?: string;
  /** Remaining duration in server ticks; <= 0 (or omitted) means permanent. */
  remainingTicks?: number;
  attackBonus?: number;
  defenceBonus?: number;
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

export function BuffWindow({ t, buffs, ticksPerSecond = 10, onRemoveBuff, onClose }: BuffWindowProps) {
  const ordered = useMemo(() => buffs.filter((buff) => buff && buff.name), [buffs]);
  const [selectedKey, setSelectedKey] = useState<string | null>(ordered[0]?.key ?? null);

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

      <div style={style.list} aria-label={t("ui.buffs", [], "Buffs")}>
        {ordered.length === 0 ? (
          <div style={style.empty}>{t("ui.buffEmpty", [], "No active buffs.")}</div>
        ) : (
          ordered.map((buff) => {
            const isSelected = selected?.key === buff.key;
            return (
              <button
                key={buff.key}
                type="button"
                data-buff-key={buff.key}
                aria-pressed={isSelected}
                onClick={() => setSelectedKey(buff.key)}
                style={{ ...style.row, ...(isSelected ? style.rowSelected : null) }}
              >
                <span style={style.rowName}>{buff.name}</span>
                <span style={style.rowTime}>{formatRemaining(t, buff.remainingTicks, ticksPerSecond)}</span>
              </button>
            );
          })
        )}
      </div>

      <div style={style.detail} data-buff-detail={selected?.key ?? ""}>
        {selected ? (
          <>
            <div style={style.detailName}>{selected.name}</div>
            <div style={style.detailTime}>{formatRemaining(t, selected.remainingTicks, ticksPerSecond)}</div>
            {selected.description ? <p style={style.detailDesc}>{selected.description}</p> : null}
            <div style={style.bonusRow}>
              <Bonus label={t("ui.buffAttack", [], "Attack")} value={selected.attackBonus} />
              <Bonus label={t("ui.buffDefence", [], "Defence")} value={selected.defenceBonus} />
            </div>
          </>
        ) : (
          <div style={style.empty}>{t("ui.buffSelectHint", [], "Select a buff to see its effect.")}</div>
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

function Bonus({ label, value }: { label: string; value?: number }) {
  const amount = typeof value === "number" ? value : 0;
  if (amount === 0) return null;
  const positive = amount > 0;
  return (
    <span style={{ ...style.bonus, color: positive ? "#8be07a" : "#d8552f" }}>
      {`${label} ${positive ? "+" : ""}${amount}`}
    </span>
  );
}

function formatRemaining(t: TranslateFn, ticks: number | undefined, ticksPerSecond: number): string {
  if (ticks === undefined || ticks <= 0) {
    return t("ui.buffPermanent", [], "Permanent");
  }
  const totalSeconds = Math.max(0, Math.round(ticks / Math.max(1, ticksPerSecond)));
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
  list: {
    position: "absolute",
    left: 12,
    top: 46,
    width: 240,
    height: 184,
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.3)",
    background: "rgba(11, 8, 5, 0.42)",
    padding: 4,
  },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  row: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 7,
    width: "100%",
    height: 22,
    padding: "0 7px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  rowSelected: { background: "rgba(95, 53, 24, 0.5)", borderColor: "rgba(214, 180, 110, 0.7)" },
  rowName: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowTime: { fontSize: 10, color: "#cbb38a", flex: "0 0 auto" },
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
  detailName: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, marginBottom: 2 },
  detailTime: { fontSize: 10, color: "#cbb38a", marginBottom: 4 },
  detailDesc: { margin: "0 0 4px", fontSize: 11, color: "#d6c6a5", lineHeight: 1.3 },
  bonusRow: { display: "flex", gap: 10, fontSize: 11, fontWeight: 700 },
  bonus: {},
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
