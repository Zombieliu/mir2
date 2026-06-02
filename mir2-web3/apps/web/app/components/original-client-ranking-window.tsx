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
  | "online";

export type RankingWindowProps = {
  t: TranslateFn;
  /** Currently active tab key (host owns the request state). */
  activeTab?: RankingTabKey;
  /** Loaded page for the active tab, if any. */
  page: RankingPage | null;
  /** Viewer name, used to highlight the player's own row. */
  playerName?: string | null;
  /** Fired when a tab is chosen — host should dispatch a getRanking request. */
  onSelectTab?: (tab: RankingTabKey) => void;
  onRefresh?: (tab: RankingTabKey) => void;
  onClose: () => void;
};

const FRAME = ORIGINAL_UI.gameShop;

const RANKING_TABS: { key: RankingTabKey; labelKey: string; fallback: string }[] = [
  { key: "overall", labelKey: "ui.rankOverall", fallback: "Overall" },
  { key: "warrior", labelKey: "ui.classWarrior", fallback: "Warrior" },
  { key: "wizard", labelKey: "ui.classWizard", fallback: "Wizard" },
  { key: "taoist", labelKey: "ui.classTaoist", fallback: "Taoist" },
  { key: "assassin", labelKey: "ui.classAssassin", fallback: "Assassin" },
  { key: "archer", labelKey: "ui.classArcher", fallback: "Archer" },
  { key: "online", labelKey: "ui.rankOnline", fallback: "Online" },
];

export function RankingWindow({
  t,
  activeTab,
  page,
  playerName,
  onSelectTab,
  onRefresh,
  onClose,
}: RankingWindowProps) {
  const [internalTab, setInternalTab] = useState<RankingTabKey>(activeTab ?? "overall");

  useEffect(() => {
    if (activeTab) {
      setInternalTab(activeTab);
    }
  }, [activeTab]);

  const tab = activeTab ?? internalTab;
  const entries = useMemo(
    () => [...(page?.entries ?? [])].sort((a, b) => a.rank - b.rank),
    [page?.entries],
  );

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
      <div style={style.subtitle}>
        {page
          ? t("ui.rankMyRank", [page.myRank > 0 ? page.myRank : "-"], `Your rank: ${page.myRank > 0 ? page.myRank : "-"}`)
          : t("ui.rankLoadHint", [], "Select a board to load rankings.")}
      </div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.ranking", [], "Ranking")}>
        {RANKING_TABS.map((entry) => {
          const active = entry.key === tab;
          return (
            <button
              key={entry.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-ranking-tab={entry.key}
              onClick={() => selectTab(entry.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(entry.labelKey, [], entry.fallback)}
            </button>
          );
        })}
      </div>

      <div style={style.board}>
        <div style={style.head}>
          <span style={style.colRank}>{t("ui.rankColRank", [], "#")}</span>
          <span style={style.colName}>{t("ui.guildName", [], "Name")}</span>
          <span style={style.colClass}>{t("ui.rankColClass", [], "Class")}</span>
          <span style={style.colLevel}>{t("ui.guildLevel", [], "Level")}</span>
        </div>
        <div style={style.rows}>
          {entries.length === 0 ? (
            <div style={style.empty}>{t("ui.rankEmpty", [], "No ranking data loaded.")}</div>
          ) : (
            entries.map((entry) => {
              const isSelf = playerName != null && entry.name === playerName;
              return (
                <div
                  key={`${entry.rank}-${entry.playerId}-${entry.name}`}
                  data-ranking-name={entry.name}
                  style={{ ...style.row, ...(isSelf ? style.rowSelf : null) }}
                >
                  <span style={{ ...style.colRank, ...rankColor(entry.rank) }}>{entry.rank}</span>
                  <span style={style.colName}>{entry.name}</span>
                  <span style={style.colClass}>{classLabel(t, entry.classKey)}</span>
                  <span style={style.colLevel}>{entry.level}</span>
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
  subtitle: { position: "absolute", left: 22, top: 30, fontSize: 11, color: "#cbb38a" },
  close: { position: "absolute", left: 666, top: 6 },
  tabs: { position: "absolute", left: 22, top: 50, display: "flex", gap: 4 },
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
    padding: "4px 12px",
    borderBottom: "1px solid rgba(190, 157, 99, 0.12)",
    color: "#e3d3af",
    fontSize: 12,
  },
  rowSelf: {
    background: "rgba(95, 53, 24, 0.45)",
    color: "#f8e6bb",
    fontWeight: 700,
  },
  colRank: { flex: "0 0 56px" },
  colName: { flex: "1 1 auto", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
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
