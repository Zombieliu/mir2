"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { ORIGINAL_UI } from "../../lib/original-ui";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type RankingClassKey = "warrior" | "wizard" | "taoist" | "assassin" | "archer";

/** One ranked character (mirrors the page's `RankingEntry`). */
export type RankingEntry = {
  rank: number;
  playerId: number;
  name: string;
  level: number;
  classKey: RankingClassKey;
  /** Whether the character is currently online (drives the status dot). */
  online?: boolean;
  /**
   * Optional ranked value for value-based boards (PK points, guild score, …).
   * When present a Value column is shown in place of (or beside) Level.
   */
  value?: number;
  /** Optional preformatted value label, overriding the numeric `value`. */
  valueLabel?: string;
};

/** One ranking page (mirrors the page's `RankingState`). */
export type RankingPage = {
  rankType: number;
  onlineOnly: boolean;
  myRank: number;
  count: number;
  entries: RankingEntry[];
};

/** A selectable ranking board tab. */
export type RankingTabKey =
  | "overall"
  | "warrior"
  | "wizard"
  | "taoist"
  | "assassin"
  | "archer"
  | "online"
  // Additive value-based boards. The host opts in by listing them in `tabs`.
  | "pk"
  | "guild";

export type RankingWindowProps = {
  t: TranslateFn;
  /** Currently active tab key (host owns the request state). */
  activeTab?: RankingTabKey;
  /** Loaded page for the active tab, if any. */
  page: RankingPage | null;
  /** Viewer name, used to highlight the player's own row. */
  playerName?: string | null;
  /**
   * Optional override of which tabs to show (and in which order). Defaults to
   * the Crystal class boards + online filter. Pass e.g. ["overall","pk","guild"]
   * to surface value-based boards.
   */
  tabs?: RankingTabKey[];
  /** Header label for the value column on value-based boards. */
  valueColumnLabel?: string;
  /** Fired when a tab is chosen — host should dispatch a getRanking request. */
  onSelectTab?: (tab: RankingTabKey) => void;
  onRefresh?: (tab: RankingTabKey) => void;
  /** Toggle the online-only filter (Crystal `OnlineOnly` checkbox). */
  onToggleOnlineOnly?: (onlineOnly: boolean) => void;
  /** Inspect a ranked character (Crystal row click → Inspect packet). */
  onInspect?: (playerId: number, name: string) => void;
  /** Scroll/jump to the viewer's own rank (Crystal "My Rank" click). */
  onGoToMyRank?: () => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.gameShop;

const RANKING_TAB_META: Record<RankingTabKey, { labelKey: string; fallback: string }> = {
  overall: { labelKey: "ui.rankOverall", fallback: "Overall" },
  warrior: { labelKey: "ui.classWarrior", fallback: "Warrior" },
  wizard: { labelKey: "ui.classWizard", fallback: "Wizard" },
  taoist: { labelKey: "ui.classTaoist", fallback: "Taoist" },
  assassin: { labelKey: "ui.classAssassin", fallback: "Assassin" },
  archer: { labelKey: "ui.classArcher", fallback: "Archer" },
  online: { labelKey: "ui.rankOnline", fallback: "Online" },
  pk: { labelKey: "ui.rankPk", fallback: "PK" },
  guild: { labelKey: "ui.rankGuild", fallback: "Guild" },
};

const DEFAULT_RANKING_TABS: RankingTabKey[] = [
  "overall",
  "warrior",
  "wizard",
  "taoist",
  "assassin",
  "archer",
  "online",
];

/** Boards whose ranked metric is a value (PK points / guild score) not level. */
const VALUE_TABS: ReadonlySet<RankingTabKey> = new Set<RankingTabKey>(["pk", "guild"]);

export function RankingWindow({
  t,
  activeTab,
  page,
  playerName,
  tabs,
  valueColumnLabel,
  onSelectTab,
  onRefresh,
  onToggleOnlineOnly,
  onInspect,
  onGoToMyRank,
  onClose,
}: RankingWindowProps) {
  const [internalTab, setInternalTab] = useState<RankingTabKey>(activeTab ?? "overall");

  useEffect(() => {
    if (activeTab) {
      setInternalTab(activeTab);
    }
  }, [activeTab]);

  const tab = activeTab ?? internalTab;
  const tabKeys = tabs && tabs.length > 0 ? tabs : DEFAULT_RANKING_TABS;
  const entries = useMemo(
    () => [...(page?.entries ?? [])].sort((a, b) => a.rank - b.rank),
    [page?.entries],
  );
  // A board is value-based when its tab is a value board OR any row carries a value.
  const valueBoard =
    VALUE_TABS.has(tab) || entries.some((entry) => entry.value !== undefined || entry.valueLabel !== undefined);
  const onlineOnly = page?.onlineOnly === true;

  const selectTab = (next: RankingTabKey) => {
    setInternalTab(next);
    onSelectTab?.(next);
  };

  return (
    <section
      aria-label={t("ui.ranking", [], "Ranking")}
      data-ranking-tab={tab}
      data-ranking-count={entries.length}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.ranking", [], "Ranking")}</div>
      <button
        type="button"
        disabled={!onGoToMyRank || !page}
        data-ranking-myrank={page?.myRank ?? ""}
        onClick={() => onGoToMyRank?.()}
        style={{ ...style.subtitle, ...(!onGoToMyRank || !page ? style.subtitleStatic : null) }}
        title={onGoToMyRank ? t("ui.rankGoToMyRank", [], "Go to my rank") : undefined}
      >
        {page
          ? page.myRank > 0
            ? t("ui.rankMyRank", [page.myRank], `Your rank: ${page.myRank}`)
            : t("ui.rankNotListed", [], "Not listed")
          : t("ui.rankLoadHint", [], "Select a board to load rankings.")}
      </button>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.ranking", [], "Ranking")}>
        {tabKeys.map((key) => {
          const active = key === tab;
          const meta = RANKING_TAB_META[key];
          return (
            <button
              key={key}
              type="button"
              role="tab"
              aria-selected={active}
              data-ranking-tab={key}
              onClick={() => selectTab(key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(meta.labelKey, [], meta.fallback)}
            </button>
          );
        })}
        <label
          style={{ ...style.onlineToggle, ...(!onToggleOnlineOnly ? style.onlineToggleDisabled : null) }}
          title={t("ui.rankOnlineOnly", [], "Online only")}
        >
          <input
            type="checkbox"
            checked={onlineOnly}
            disabled={!onToggleOnlineOnly}
            onChange={(event) => onToggleOnlineOnly?.(event.target.checked)}
          />
          {t("ui.rankOnlineOnly", [], "Online only")}
        </label>
      </div>

      <div style={style.board}>
        <div style={style.head}>
          <span style={style.colRank}>{t("ui.rankColRank", [], "#")}</span>
          <span style={style.colName}>{t("ui.guildName", [], "Name")}</span>
          <span style={style.colClass}>{t("ui.rankColClass", [], "Class")}</span>
          {valueBoard ? (
            <span style={style.colValue}>{valueColumnLabel ?? t("ui.rankColValue", [], "Value")}</span>
          ) : null}
          <span style={style.colLevel}>{t("ui.guildLevel", [], "Level")}</span>
        </div>
        <div style={style.rows}>
          {entries.length === 0 ? (
            <div style={style.empty}>{t("ui.rankEmpty", [], "No ranking data loaded.")}</div>
          ) : (
            entries.map((entry) => {
              const isSelf = playerName != null && entry.name === playerName;
              const rowStyle = {
                ...style.row,
                ...(onInspect ? style.rowClickable : null),
                ...(isSelf ? style.rowSelf : null),
              };
              const cells = (
                <>
                  <span style={{ ...style.colRank, ...rankColor(entry.rank) }}>{entry.rank}</span>
                  <span style={style.colName}>
                    {entry.online ? <span style={style.onlineDot} aria-hidden="true" /> : null}
                    {entry.name}
                  </span>
                  <span style={style.colClass}>{classLabel(t, entry.classKey)}</span>
                  {valueBoard ? (
                    <span style={style.colValue}>{entry.valueLabel ?? formatValue(entry.value)}</span>
                  ) : null}
                  <span style={style.colLevel}>{entry.level}</span>
                </>
              );
              const rowKey = `${entry.rank}-${entry.playerId}-${entry.name}`;
              return onInspect ? (
                <button
                  key={rowKey}
                  type="button"
                  data-ranking-name={entry.name}
                  onClick={() => onInspect(entry.playerId, entry.name)}
                  style={rowStyle}
                >
                  {cells}
                </button>
              ) : (
                <div key={rowKey} data-ranking-name={entry.name} style={rowStyle}>
                  {cells}
                </div>
              );
            })
          )}
        </div>
      </div>

      <div style={style.footer}>
        <div style={style.footerInfo}>
          {page ? t("ui.rankTotal", [page.count], `${page.count} ranked`) : ""}
        </div>
        <button
          type="button"
          disabled={!onRefresh}
          style={{ ...style.actionButton, ...(!onRefresh ? style.actionButtonDisabled : null) }}
          onClick={() => onRefresh?.(tab)}
        >
          {t("ui.refresh", [], "Refresh")}
        </button>
      </div>
    </section>
  );
}

function formatValue(value?: number): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "-";
  return Math.trunc(value).toLocaleString("en-US");
}

function classLabel(t: TranslateFn, classKey: RankingClassKey) {
  switch (classKey) {
    case "wizard":
      return t("ui.classWizard", [], "Wizard");
    case "taoist":
      return t("ui.classTaoist", [], "Taoist");
    case "assassin":
      return t("ui.classAssassin", [], "Assassin");
    case "archer":
      return t("ui.classArcher", [], "Archer");
    case "warrior":
    default:
      return t("ui.classWarrior", [], "Warrior");
  }
}

function rankColor(rank: number): CSSProperties {
  if (rank === 1) return { color: "#f5d76e", fontWeight: 700 };
  if (rank === 2) return { color: "#d4d4d4", fontWeight: 700 };
  if (rank === 3) return { color: "#cd9b62", fontWeight: 700 };
  return {};
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
  subtitle: {
    position: "absolute",
    left: 22,
    top: 28,
    fontSize: 11,
    color: "#cbb38a",
    border: "none",
    background: "none",
    padding: 0,
    textAlign: "left",
    fontFamily: "inherit",
    textShadow: "1px 1px 0 #000",
    cursor: "pointer",
  },
  subtitleStatic: { cursor: "default" },
  close: { position: "absolute", left: 666, top: 6 },
  tabs: { position: "absolute", left: 22, top: 50, display: "flex", gap: 4, alignItems: "center" },
  onlineToggle: {
    display: "flex",
    alignItems: "center",
    gap: 4,
    marginLeft: 8,
    fontSize: 11,
    color: "#cbb38a",
    cursor: "pointer",
  },
  onlineToggleDisabled: { opacity: 0.45, cursor: "default" },
  tab: {
    minWidth: 84,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "4px 8px",
    fontSize: 12,
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  board: {
    position: "absolute",
    left: 22,
    top: 80,
    width: 652,
    height: 332,
    display: "flex",
    flexDirection: "column",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "rgba(11, 8, 5, 0.45)",
  },
  head: {
    display: "flex",
    padding: "5px 12px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.32)",
    fontSize: 10,
    color: "#a89568",
    textTransform: "uppercase",
    letterSpacing: 0.5,
  },
  rows: { flex: 1, overflowY: "auto", display: "flex", flexDirection: "column" },
  empty: { color: "#cbb38a", padding: "12px", fontSize: 12 },
  row: {
    display: "flex",
    alignItems: "center",
    padding: "4px 12px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.12)",
    color: "#e3d3af",
    fontSize: 12,
    width: "100%",
    textAlign: "left",
    fontFamily: "inherit",
    textShadow: "1px 1px 0 #000",
    background: "none",
    borderLeft: "none",
    borderRight: "none",
    borderTop: "none",
  },
  rowClickable: { cursor: "pointer" },
  rowSelf: {
    background: "rgba(95, 53, 24, 0.45)",
    color: "#f8e6bb",
    fontWeight: 700,
  },
  onlineDot: {
    display: "inline-block",
    width: 7,
    height: 7,
    borderRadius: "50%",
    background: "#8be07a",
    marginRight: 5,
    verticalAlign: "middle",
  },
  colRank: { flex: "0 0 56px" },
  colName: { flex: "1 1 auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  colValue: { flex: "0 0 110px", textAlign: "right", color: "#f0d69b" },
  colClass: { flex: "0 0 140px" },
  colLevel: { flex: "0 0 80px", textAlign: "right" },
  footer: {
    position: "absolute",
    left: 22,
    top: 420,
    width: 652,
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: 8,
  },
  footerInfo: { fontSize: 11, color: "#cbb38a" },
  actionButton: {
    border: "1px solid rgba(190, 157, 99, 0.56)",
    background: "linear-gradient(180deg, rgba(95, 53, 24, 0.95), rgba(45, 23, 12, 0.95))",
    color: "#f4dcaf",
    padding: "5px 18px",
    fontSize: 12,
    cursor: "pointer",
  },
  actionButtonDisabled: { opacity: 0.45, cursor: "default" },
};
