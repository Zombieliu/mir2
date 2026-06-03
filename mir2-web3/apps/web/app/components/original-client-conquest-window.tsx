"use client";

import { useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

/** A named defensive structure with current HP and repair cost. */
export type ConquestStructure = {
  name?: string;
  /** HP as a percentage 0-100. */
  hpPercent?: number;
  /** Whether it is currently alive/standing. */
  alive?: boolean;
  /** Cost to repair (gold). */
  repairCost?: number;
};

/** A capture flag / control point and which guild currently holds it. */
export type ConquestFlag = {
  name?: string;
  /** Guild currently controlling the point, if any. */
  controlledBy?: string;
};

/** Mirrors the stage-5 `conquest` slice. */
export type ConquestSummary = {
  castleOwner?: string;
  activeWars?: string[];
  eventLog?: string[];
  taxRatePercent?: number;
  gold?: number;
  /** Per-structure HP percentages (0-100), one entry per structure. */
  guards?: number[];
  walls?: number[];
  gates?: number[];
  /** Indices of gates that are currently open. */
  openGates?: number[];
  // --- Additive optional fields (rendered gracefully when absent) ---
  /** Castle / palace name. */
  name?: string;
  /** Whether a war is currently underway (Crystal `WarIsOn`). */
  warActive?: boolean;
  /** Daily war start time as a "HH:MM" string. */
  warStartTime?: string;
  /** Daily war end time as a "HH:MM" string. */
  warEndTime?: string;
  /** War length in minutes (Crystal `WarLength`). */
  warLengthMinutes?: number;
  /** Days the war runs (e.g. "Saturday"). */
  warDays?: string[];
  /** Game mode label (CapturePalace / KingOfHill / Classic / ControlPoints). */
  gameType?: string;
  /** How the war is triggered (Request / Auto / Forced). */
  startType?: string;
  /** Defending archer NPCs (Crystal `ArcherList`). */
  archers?: ConquestStructure[];
  /** Capture flags / control points (Crystal `FlagList`). */
  flags?: ConquestFlag[];
};

/** Mirrors the stage-5 `guildTerritory` slice. */
export type GuildTerritorySummary = {
  owned?: boolean;
  mapFileName?: string;
  rentalDaysLeft?: number;
  recallLog?: string[];
};

export type ConquestWindowProps = {
  t: TranslateFn;
  conquest: ConquestSummary | null;
  territory?: GuildTerritorySummary | null;
  /** Viewer's guild name, used to decide whether they own the castle. */
  guildName?: string | null;
  onStartWar?: () => void;
  onToggleGate?: (gateIndex: number) => void;
  onSetTaxRate?: (percent: number) => void;
  /** Repair a named defending structure ("archer" | "gate" | "wall" | "guard" + index). */
  onRepairStructure?: (kind: "archer" | "gate" | "wall" | "guard", index: number) => void;
  onClose: () => void;
};

type ConquestTab = "castle" | "defences" | "log";

const FRAME = ORIGINAL_UI.gameShop;

const CONQUEST_TABS: { key: ConquestTab; labelKey: string; fallback: string }[] = [
  { key: "castle", labelKey: "ui.conquestCastle", fallback: "Castle" },
  { key: "defences", labelKey: "ui.conquestDefences", fallback: "Defences" },
  { key: "log", labelKey: "ui.conquestLog", fallback: "War Log" },
];

const TAX_PRESETS = [0, 5, 10, 15, 20, 30, 50];

export function ConquestWindow({
  t,
  conquest,
  territory,
  guildName,
  onStartWar,
  onToggleGate,
  onSetTaxRate,
  onRepairStructure,
  onClose,
}: ConquestWindowProps) {
  const [tab, setTab] = useState<ConquestTab>("castle");

  const owner = conquest?.castleOwner?.trim() ?? "";
  const ownedByViewer = owner.length > 0 && guildName != null && owner === guildName;
  const activeWars = conquest?.activeWars ?? [];
  const eventLog = conquest?.eventLog ?? [];
  const taxRate = clampPercent(conquest?.taxRatePercent ?? 0);
  const openGates = new Set(conquest?.openGates ?? []);
  const warActive = conquest?.warActive ?? false;
  const archers = conquest?.archers ?? [];
  const flags = conquest?.flags ?? [];
  const scheduleText = buildScheduleText(conquest);

  return (
    <section
      aria-label={t("ui.conquest", [], "Conquest")}
      data-conquest-tab={tab}
      data-conquest-owner={owner}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{conquest?.name?.trim() || t("ui.conquest", [], "Conquest")}</div>
      <div style={style.subtitle}>
        {owner
          ? t("ui.conquestHeldBy", [owner], `Held by ${owner}`)
          : t("ui.conquestUnclaimed", [], "Castle unclaimed")}
        {warActive ? <span style={style.warBadge}>{t("ui.conquestWarLive", [], "WAR LIVE")}</span> : null}
      </div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.conquest", [], "Conquest")}>
        {CONQUEST_TABS.map((entry) => {
          const active = entry.key === tab;
          return (
            <button
              key={entry.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-conquest-tab={entry.key}
              onClick={() => setTab(entry.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(entry.labelKey, [], entry.fallback)}
            </button>
          );
        })}
      </div>

      {tab === "castle" ? (
        <div style={style.panel}>
          <div style={style.grid}>
            <Info label={t("ui.conquestOwner", [], "Owner")} value={owner || t("ui.conquestUnclaimedShort", [], "Unclaimed")} />
            <Info label={t("ui.conquestTax", [], "Tax Rate")} value={`${taxRate}%`} />
            <Info label={t("ui.conquestTreasury", [], "Treasury")} value={formatNumber(conquest?.gold ?? 0)} />
            <Info
              label={t("ui.conquestWarState", [], "War State")}
              value={warActive ? t("ui.conquestWarOn", [], "In Progress") : t("ui.conquestWarOff", [], "Peacetime")}
            />
            <Info
              label={t("ui.conquestGameType", [], "Game Type")}
              value={conquest?.gameType?.trim() || "-"}
            />
            <Info
              label={t("ui.conquestStartType", [], "Trigger")}
              value={conquest?.startType?.trim() || "-"}
            />
            <Info
              label={t("ui.conquestTerritory", [], "Sabuk Wall")}
              value={territory?.owned ? t("ui.on", [], "Owned") : t("ui.off", [], "Free")}
            />
            <Info
              label={t("ui.conquestRental", [], "Rental Days")}
              value={typeof territory?.rentalDaysLeft === "number" ? String(territory.rentalDaysLeft) : "-"}
            />
            <Info label={t("ui.conquestSchedule", [], "Schedule")} value={scheduleText} />
          </div>

          <div style={style.subhead}>{t("ui.conquestWars", [], "Active Wars")}</div>
          <div style={style.warList}>
            {activeWars.length === 0 ? (
              <div style={style.empty}>{t("ui.conquestNoWars", [], "No wars scheduled.")}</div>
            ) : (
              activeWars.map((war, index) => (
                <div key={`war-${index}`} style={style.warRow}>
                  <span style={style.warDot} aria-hidden="true" />
                  {war}
                </div>
              ))
            )}
          </div>

          <div style={style.taxRow}>
            <span style={style.taxLabel}>{t("ui.conquestSetTax", [], "Set Tax")}</span>
            {TAX_PRESETS.map((value) => (
              <button
                key={`tax-${value}`}
                type="button"
                disabled={!onSetTaxRate || !ownedByViewer}
                aria-pressed={taxRate === value}
                style={{
                  ...style.taxButton,
                  ...(taxRate === value ? style.taxButtonActive : null),
                  ...(!onSetTaxRate || !ownedByViewer ? style.actionButtonDisabled : null),
                }}
                onClick={() => onSetTaxRate?.(value)}
              >
                {`${value}%`}
              </button>
            ))}
          </div>

          <button
            type="button"
            disabled={!onStartWar || ownedByViewer}
            style={{ ...style.actionButton, width: "100%", ...(!onStartWar || ownedByViewer ? style.actionButtonDisabled : null) }}
            onClick={() => onStartWar?.()}
          >
            {ownedByViewer ? t("ui.conquestDefend", [], "You hold this castle") : t("ui.conquestDeclare", [], "Declare War")}
          </button>
        </div>
      ) : null}

      {tab === "defences" ? (
        <div style={{ ...style.panel, overflowY: "auto" }}>
          <StructureGroup
            t={t}
            title={t("ui.conquestWalls", [], "Walls")}
            values={conquest?.walls ?? []}
            color="#9c8d6f"
          />
          <StructureGroup
            t={t}
            title={t("ui.conquestGuards", [], "Guard Towers")}
            values={conquest?.guards ?? []}
            color="#d8552f"
          />
          <div style={style.subhead}>{t("ui.conquestArchers", [], "Defending Archers")}</div>
          {archers.length === 0 ? (
            <div style={style.empty}>{t("ui.conquestNoArchers", [], "No defending NPCs.")}</div>
          ) : (
            <div style={style.structureRows}>
              {archers.map((archer, index) => {
                const hp = clampPercent(archer.hpPercent ?? (archer.alive === false ? 0 : 100));
                const dead = archer.alive === false || hp <= 0;
                return (
                  <div key={`archer-${index}`} style={style.archerRow} data-archer-alive={dead ? "0" : "1"}>
                    <span style={style.archerName}>{archer.name?.trim() || t("ui.conquestArcherNo", [index + 1], `Archer ${index + 1}`)}</span>
                    <Bar value={hp} color={dead ? "#5c5648" : "#8be07a"} />
                    {dead ? (
                      <button
                        type="button"
                        disabled={!onRepairStructure || !ownedByViewer}
                        title={archer.repairCost ? t("ui.conquestRepairCost", [archer.repairCost], `Repair (${archer.repairCost}g)`) : undefined}
                        style={{ ...style.repairButton, ...(!onRepairStructure || !ownedByViewer ? style.actionButtonDisabled : null) }}
                        onClick={() => onRepairStructure?.("archer", index)}
                      >
                        {t("ui.conquestRepair", [], "Repair")}
                      </button>
                    ) : (
                      <span style={style.archerState}>{t("ui.conquestAlive", [], "Alive")}</span>
                    )}
                  </div>
                );
              })}
            </div>
          )}

          <div style={style.subhead}>{t("ui.conquestFlags", [], "Control Points")}</div>
          {flags.length === 0 ? (
            <div style={style.empty}>{t("ui.conquestNoFlags", [], "No control points.")}</div>
          ) : (
            <div style={style.structureRows}>
              {flags.map((flag, index) => {
                const held = flag.controlledBy?.trim() ?? "";
                const mine = held.length > 0 && guildName != null && held === guildName;
                return (
                  <div key={`flag-${index}`} style={style.flagRow} data-flag-owner={held}>
                    <span style={style.archerName}>{flag.name?.trim() || t("ui.conquestFlagNo", [index + 1], `Flag ${index + 1}`)}</span>
                    <span style={{ ...style.flagOwner, color: mine ? "#8be07a" : held ? "#f0d69b" : "#9c8d6f" }}>
                      {held || t("ui.conquestNeutral", [], "Neutral")}
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          <div style={style.subhead}>{t("ui.conquestGates", [], "Gates")}</div>
          <div style={style.gateGrid}>
            {(conquest?.gates ?? []).length === 0 ? (
              <div style={style.empty}>{t("ui.conquestNoGates", [], "No gates registered.")}</div>
            ) : (
              (conquest?.gates ?? []).map((hp, index) => {
                const open = openGates.has(index);
                return (
                  <div key={`gate-${index}`} style={style.gateCell}>
                    <div style={style.gateHead}>
                      <span>{t("ui.conquestGateNo", [index + 1], `Gate ${index + 1}`)}</span>
                      <span style={{ color: open ? "#8be07a" : "#cbb38a" }}>
                        {open ? t("ui.conquestGateOpen", [], "Open") : t("ui.conquestGateClosed", [], "Closed")}
                      </span>
                    </div>
                    <Bar value={clampPercent(hp)} color="#caa64a" />
                    <button
                      type="button"
                      disabled={!onToggleGate || !ownedByViewer}
                      style={{ ...style.gateButton, ...(!onToggleGate || !ownedByViewer ? style.actionButtonDisabled : null) }}
                      onClick={() => onToggleGate?.(index)}
                    >
                      {open ? t("ui.conquestCloseGate", [], "Close") : t("ui.conquestOpenGate", [], "Open")}
                    </button>
                  </div>
                );
              })
            )}
          </div>
        </div>
      ) : null}

      {tab === "log" ? (
        <div style={style.panel}>
          <div style={style.subhead}>{t("ui.conquestLog", [], "War Log")}</div>
          <div style={style.log} aria-label={t("ui.conquestLog", [], "War Log")}>
            {eventLog.length === 0 ? (
              <div style={style.empty}>{t("ui.conquestNoEvents", [], "No conquest events yet.")}</div>
            ) : (
              eventLog.slice(-60).map((line, index) => (
                <div key={`event-${index}`} style={style.logLine}>
                  {line}
                </div>
              ))
            )}
          </div>
          {territory?.recallLog?.length ? (
            <>
              <div style={style.subhead}>{t("ui.conquestRecallLog", [], "Recall Log")}</div>
              <div style={style.log} aria-label={t("ui.conquestRecallLog", [], "Recall Log")}>
                {territory.recallLog.slice(-30).map((line, index) => (
                  <div key={`recall-${index}`} style={style.logLine}>
                    {line}
                  </div>
                ))}
              </div>
            </>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

function StructureGroup({
  t,
  title,
  values,
  color,
}: {
  t: TranslateFn;
  title: string;
  values: number[];
  color: string;
}) {
  return (
    <div style={style.structureGroup}>
      <div style={style.subhead}>{title}</div>
      {values.length === 0 ? (
        <div style={style.empty}>{t("ui.conquestNoStructures", [], "None registered.")}</div>
      ) : (
        <div style={style.structureRows}>
          {values.map((hp, index) => (
            <div key={`${title}-${index}`} style={style.structureRow}>
              <span style={style.structureLabel}>{index + 1}</span>
              <Bar value={clampPercent(hp)} color={color} />
              <span style={style.structureValue}>{`${clampPercent(hp)}%`}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Bar({ value, color }: { value: number; color: string }) {
  return (
    <div style={style.barTrack} role="progressbar" aria-valuenow={value} aria-valuemax={100}>
      <span style={{ ...style.barFill, width: `${value}%`, background: color }} />
    </div>
  );
}

function Info({ label, value }: { label: string; value: string }) {
  return (
    <div style={style.info}>
      <span style={style.infoLabel}>{label}</span>
      <span style={style.infoValue}>{value}</span>
    </div>
  );
}

function clampPercent(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, Math.round(value)));
}

function formatNumber(value: number) {
  return value.toLocaleString("en-US");
}

/** Compose a human-readable "Sat 20:00–21:00 (60m)" war schedule line. */
function buildScheduleText(conquest: ConquestSummary | null): string {
  if (!conquest) return "-";
  const days = (conquest.warDays ?? []).filter((day) => day && day.trim().length > 0);
  const start = conquest.warStartTime?.trim();
  const end = conquest.warEndTime?.trim();
  const length = typeof conquest.warLengthMinutes === "number" ? conquest.warLengthMinutes : undefined;

  const parts: string[] = [];
  if (days.length) parts.push(days.join(", "));
  if (start) parts.push(end ? `${start}-${end}` : start);
  if (!start && length != null) parts.push(`${length}m`);
  return parts.length ? parts.join(" · ") : "-";
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 164,
    top: 146,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 32,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  titleText: {
    position: "absolute",
    left: 22,
    top: 10,
    fontSize: 14,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  subtitle: { position: "absolute", left: 22, top: 30, fontSize: 11, color: "#cbb38a", display: "flex", alignItems: "center", gap: 8 },
  warBadge: {
    fontSize: 9,
    fontWeight: 700,
    color: "#fff",
    background: "#b5322a",
    border: "1px solid #e0746c",
    padding: "1px 6px",
    letterSpacing: 0.6,
  },
  close: { position: "absolute", left: 666, top: 6 },
  tabs: { position: "absolute", left: 22, top: 50, display: "flex", gap: 4 },
  tab: {
    minWidth: 96,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "4px 12px",
    fontSize: 12,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  panel: {
    position: "absolute",
    left: 22,
    top: 80,
    width: 652,
    height: 372,
    display: "flex",
    flexDirection: "column",
    gap: 8,
    overflow: "hidden",
  },
  grid: { display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 8 },
  info: {
    display: "flex",
    justifyContent: "space-between",
    gap: 8,
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.45)",
    padding: "5px 9px",
    fontSize: 12,
  },
  infoLabel: { color: "#a89568" },
  infoValue: { color: "#f0d69b", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  subhead: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.6 },
  warList: {
    maxHeight: 96,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.45)",
    padding: "4px 8px",
    display: "flex",
    flexDirection: "column",
    gap: 3,
  },
  warRow: { display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "#e3d3af" },
  warDot: { width: 7, height: 7, borderRadius: "50%", background: "#d8552f", flex: "0 0 auto" },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  taxRow: { display: "flex", alignItems: "center", gap: 6 },
  taxLabel: { fontSize: 10, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5, flex: "0 0 auto" },
  taxButton: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "3px 14px",
    fontSize: 11,
    cursor: "pointer",
  },
  taxButtonActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    borderColor: "rgba(214, 180, 110, 0.85)",
    color: "#f8e6bb",
  },
  structureGroup: { display: "flex", flexDirection: "column", gap: 4 },
  structureRows: { display: "flex", flexDirection: "column", gap: 3 },
  structureRow: { display: "flex", alignItems: "center", gap: 8 },
  archerRow: { display: "flex", alignItems: "center", gap: 8 },
  archerName: { flex: "0 0 150px", fontSize: 11, color: "#e3d3af", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  archerState: { flex: "0 0 60px", fontSize: 10, color: "#8be07a", textAlign: "right" },
  repairButton: {
    flex: "0 0 60px",
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "2px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  flagRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(20, 13, 7, 0.4)",
    padding: "3px 8px",
  },
  flagOwner: { fontSize: 11, flex: "0 0 auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 200 },
  structureLabel: { flex: "0 0 20px", fontSize: 11, color: "#a89568", textAlign: "right" },
  structureValue: { flex: "0 0 44px", fontSize: 11, color: "#e3d3af", textAlign: "right" },
  gateGrid: { display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 8, overflowY: "auto" },
  gateCell: {
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(20, 13, 7, 0.45)",
    padding: "6px 8px",
    display: "flex",
    flexDirection: "column",
    gap: 4,
  },
  gateHead: { display: "flex", justifyContent: "space-between", fontSize: 11, color: "#cbb38a" },
  gateButton: {
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "rgba(52, 32, 18, 0.86)",
    color: "#f4dcaf",
    padding: "3px 0",
    fontSize: 10,
    cursor: "pointer",
  },
  barTrack: {
    position: "relative",
    flex: 1,
    height: 7,
    background: "rgba(0, 0, 0, 0.55)",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    overflow: "hidden",
  },
  barFill: { position: "absolute", left: 0, top: 0, bottom: 0, display: "block" },
  log: {
    flex: 1,
    minHeight: 0,
    overflowY: "auto",
    border: "1px solid rgba(190, 157, 99, 0.28)",
    background: "rgba(11, 8, 5, 0.55)",
    padding: "6px 8px",
    fontSize: 12,
    lineHeight: 1.4,
    color: "#d6c6a5",
  },
  logLine: { marginBottom: 2 },
  actionButton: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 16px",
    fontSize: 12,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
