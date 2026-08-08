"use client";

import { useEffect, useMemo, useState, type CSSProperties } from "react";

import { originalAssetPath } from "../../lib/asset-url";
import { ORIGINAL_UI } from "../../lib/original-ui";
import { classAwareObjectiveLine } from "../../lib/onboarding-guidance";
import { SpriteButton } from "./original-client-overlays";

type TranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

type QuestStage = "available" | "inProgress" | "readyToTurnIn" | "completed";

/** A single quest objective with optional progress counts. */
export type QuestObjective = {
  label: string;
  current?: number;
  required?: number;
  /** Explicitly mark an objective complete (otherwise derived from counts). */
  done?: boolean;
};

/** A single item reward shown in the reward preview grid. */
export type QuestRewardItem = {
  name: string;
  /** Icon index into `/original-ui/Items/<icon>.png`. */
  icon?: number;
  count?: number;
  /** Crystal ItemInfo.Index, retained for tooltips and protocol actions. */
  itemIndex?: number;
  /** Zero-based index expected by Crystal FinishQuest.SelectedItemIndex. */
  selectionIndex?: number;
  /** True for player-selected rewards (Crystal RewardsSelectItem). */
  selectable?: boolean;
};

/** Structured reward breakdown (Crystal QuestRewards). */
export type QuestRewardSummary = {
  gold?: number;
  experience?: number;
  credit?: number;
  items?: QuestRewardItem[];
  /** Rewards the player must choose one of. */
  selectItems?: QuestRewardItem[];
};

/**
 * Mirrors the relevant fields of {@link DisplayQuest} from
 * `original-client-types.ts` without importing the whole world type, so the
 * window stays self-contained and can be driven by any compatible payload.
 *
 * The structured `descriptionLines`, `objectives`, `rewards`, `timeLimit` and
 * `npc` fields are optional enrichment mirroring the Crystal quest detail
 * dialog; when absent the window falls back to the flat string fields.
 */
export type QuestLogEntry = {
  questId: number;
  title: string;
  summary: string;
  objective: string;
  progressLabel: string;
  tracker?: string;
  stage: QuestStage;
  current: number;
  required: number;
  rewardPreview: string;
  /** Full description paragraphs (Crystal Description / CompletionDescription). */
  descriptionLines?: string[];
  /** Per-objective progress list (Crystal TaskList). */
  objectives?: QuestObjective[];
  /** Structured reward breakdown. */
  rewards?: QuestRewardSummary;
  /** Remaining time-limit label (Crystal TimeLimit). */
  timeLimit?: string;
  /** Quest-giver / return NPC name (Crystal ReturnDescription). */
  npc?: string;
};

export type QuestLogWindowProps = {
  t: TranslateFn;
  quests: QuestLogEntry[];
  /** Optional: fired when the player tracks / shares / abandons a quest. */
  onTrackQuest?: (questId: number) => void;
  onShareQuest?: (questId: number) => void;
  onAbandonQuest?: (questId: number) => void;
  onClose: () => void;
  /**
   * Lowercase class key of the local player. Rewrites class-blind onboarding copy
   * (e.g. the guide quest's "Stay in melee range" objective) so ranged classes
   * aren't told to melee. Optional + defensive: absent → objective shown verbatim.
   */
  playerClass?: string | null;
};

type QuestStageFilter = "all" | QuestStage;

const QUEST_STAGE_FILTERS: { key: QuestStageFilter; labelKey: string; fallback: string }[] = [
  { key: "all", labelKey: "ui.questFilterAll", fallback: "All" },
  { key: "inProgress", labelKey: "ui.questFilterActive", fallback: "Active" },
  { key: "readyToTurnIn", labelKey: "ui.questFilterReady", fallback: "Ready" },
  { key: "available", labelKey: "ui.questFilterNew", fallback: "New" },
  { key: "completed", labelKey: "ui.questFilterDone", fallback: "Done" },
];

const QUEST_LOG_ROWS_PER_PAGE = 8;

// The window is rendered on top of the Crystal "Title/670" mail frame, a tall
// 312x444 list panel. Coordinates below are expressed in that frame's space.
const FRAME = ORIGINAL_UI.mail;

export function QuestLogWindow({
  t,
  quests,
  onTrackQuest,
  onShareQuest,
  onAbandonQuest,
  onClose,
  playerClass,
}: QuestLogWindowProps) {
  const [stageFilter, setStageFilter] = useState<QuestStageFilter>("all");
  const [page, setPage] = useState(0);
  const [selectedId, setSelectedId] = useState<number | null>(null);

  const filtered = useMemo(
    () => quests.filter((quest) => stageFilter === "all" || quest.stage === stageFilter),
    [quests, stageFilter],
  );
  const pageCount = Math.max(1, Math.ceil(filtered.length / QUEST_LOG_ROWS_PER_PAGE));
  const currentPage = Math.min(page, pageCount - 1);
  const visible = filtered.slice(
    currentPage * QUEST_LOG_ROWS_PER_PAGE,
    currentPage * QUEST_LOG_ROWS_PER_PAGE + QUEST_LOG_ROWS_PER_PAGE,
  );

  const selected = useMemo(() => {
    if (selectedId !== null) {
      const match = quests.find((quest) => quest.questId === selectedId);
      if (match && (stageFilter === "all" || match.stage === stageFilter)) {
        return match;
      }
    }
    return visible[0] ?? filtered[0] ?? null;
  }, [filtered, quests, selectedId, stageFilter, visible]);

  useEffect(() => {
    setPage(0);
  }, [stageFilter]);

  useEffect(() => {
    if (page > pageCount - 1) {
      setPage(pageCount - 1);
    }
  }, [page, pageCount]);

  return (
    <section
      aria-label={t("ui.quest", [], "Quest Log")}
      data-quest-stage-filter={stageFilter}
      data-quest-selected={selected?.questId ?? ""}
      style={style.window}
    >
      <img style={style.frame} src={FRAME.frame} alt="" draggable={false} />
      <img style={style.title} src={FRAME.title} alt="" draggable={false} />
      <div style={style.titleText}>{t("ui.quest", [], "Quest Log")}</div>
      <div style={style.close}>
        <SpriteButton sprite={FRAME.closeButton} label={t("ui.close", [], "Close")} onClick={onClose} />
      </div>
      <div style={style.help}>
        <SpriteButton sprite={FRAME.helpButton} label={t("ui.help", [], "Help")} onClick={() => undefined} />
      </div>

      <div style={style.tabs} role="tablist" aria-label={t("ui.quest", [], "Quest Log")}>
        {QUEST_STAGE_FILTERS.map((filter) => {
          const active = filter.key === stageFilter;
          const count =
            filter.key === "all"
              ? quests.length
              : quests.filter((quest) => quest.stage === filter.key).length;
          return (
            <button
              key={filter.key}
              type="button"
              role="tab"
              aria-selected={active}
              data-quest-tab={filter.key}
              onClick={() => setStageFilter(filter.key)}
              style={{ ...style.tab, ...(active ? style.tabActive : null) }}
            >
              {t(filter.labelKey, [], filter.fallback)}
              <span style={style.tabCount}>{count}</span>
            </button>
          );
        })}
      </div>

      <div style={style.list} aria-label={t("ui.quest", [], "Quest Log")}>
        {visible.length === 0 ? (
          <div style={style.empty}>{t("ui.questEmpty", [], "No quests in this category.")}</div>
        ) : (
          visible.map((quest) => {
            const isSelected = selected?.questId === quest.questId;
            return (
              <button
                key={quest.questId}
                type="button"
                data-quest-id={quest.questId}
                data-quest-stage={quest.stage}
                aria-pressed={isSelected}
                onClick={() => setSelectedId(quest.questId)}
                style={{ ...style.row, ...(isSelected ? style.rowSelected : null) }}
              >
                <span style={{ ...style.stageDot, background: stageColor(quest.stage) }} aria-hidden="true" />
                <span style={style.rowTitle}>{quest.title}</span>
                <span style={style.rowStage}>{stageLabel(t, quest.stage)}</span>
              </button>
            );
          })
        )}
      </div>

      <div style={style.pagePrev}>
        <SpriteButton
          sprite={FRAME.previousButton}
          label={t("ui.previous", [], "Previous")}
          onClick={() => setPage((current) => Math.max(0, current - 1))}
        />
      </div>
      <div style={style.pageLabel}>{`${currentPage + 1} / ${pageCount}`}</div>
      <div style={style.pageNext}>
        <SpriteButton
          sprite={FRAME.nextButton}
          label={t("ui.next", [], "Next")}
          onClick={() => setPage((current) => Math.min(pageCount - 1, current + 1))}
        />
      </div>

      <div style={style.detail} data-quest-detail={selected?.questId ?? ""}>
        {selected ? (
          <>
            <div style={style.detailTitle}>{selected.title}</div>
            <div style={style.detailStageRow}>
              <span style={{ ...style.detailStageTag, color: stageColor(selected.stage) }}>
                {stageLabel(t, selected.stage)}
              </span>
              {selected.tracker ? <span style={style.detailTracker}>{selected.tracker}</span> : null}
              {selected.timeLimit ? (
                <span style={style.detailTimeLimit}>{t("ui.questTimeLimit", [selected.timeLimit], `Time: ${selected.timeLimit}`)}</span>
              ) : null}
            </div>

            {selected.descriptionLines && selected.descriptionLines.length > 0 ? (
              selected.descriptionLines.map((line, index) => (
                <p key={index} style={style.detailSummary}>
                  {line}
                </p>
              ))
            ) : (
              <p style={style.detailSummary}>{selected.summary}</p>
            )}

            <div style={style.detailObjectiveLabel}>{t("ui.questObjective", [], "Objective")}</div>
            {selected.objectives && selected.objectives.length > 0 ? (
              <ul style={style.objectiveList}>
                {selected.objectives.map((objective, index) => {
                  const done = isObjectiveDone(objective);
                  return (
                    <li key={index} style={style.objectiveItem}>
                      <span style={{ ...style.objectiveCheck, color: done ? "#8be07a" : "#9c8d6f" }}>
                        {done ? "✔" : "•"}
                      </span>
                      <span style={{ ...style.objectiveText, ...(done ? style.objectiveTextDone : null) }}>
                        {classAwareObjectiveLine(objective.label, playerClass)}
                      </span>
                      {objective.required !== undefined ? (
                        <span style={style.objectiveCount}>
                          {`${objective.current ?? 0}/${objective.required}`}
                        </span>
                      ) : null}
                    </li>
                  );
                })}
              </ul>
            ) : (
              <>
                <p style={style.detailObjective}>{classAwareObjectiveLine(selected.objective, playerClass)}</p>
                <div style={style.progressRow}>
                  <span>{selected.progressLabel}</span>
                  <span>{progressPercentLabel(selected.current, selected.required)}</span>
                </div>
                <div style={style.progressTrack} role="progressbar" aria-valuenow={selected.current} aria-valuemax={selected.required}>
                  <span
                    style={{
                      ...style.progressFill,
                      width: `${progressPercent(selected.current, selected.required)}%`,
                      background: stageColor(selected.stage),
                    }}
                  />
                </div>
              </>
            )}

            {selected.npc ? (
              <div style={style.detailReturn}>{t("ui.questReturnTo", [selected.npc], `Return to: ${selected.npc}`)}</div>
            ) : null}

            <div style={style.rewardLabel}>{t("ui.questReward", [], "Reward")}</div>
            {selected.rewards ? (
              <QuestRewardView t={t} rewards={selected.rewards} fallback={selected.rewardPreview} />
            ) : (
              <p style={style.reward}>{selected.rewardPreview || t("ui.questNoReward", [], "No reward listed.")}</p>
            )}
          </>
        ) : (
          <div style={style.empty}>{t("ui.questSelectHint", [], "Select a quest to view details.")}</div>
        )}
      </div>

      <div style={style.actions}>
        <button
          type="button"
          disabled={!selected || !onTrackQuest}
          style={{ ...style.actionButton, ...(!selected || !onTrackQuest ? style.actionButtonDisabled : null) }}
          onClick={() => selected && onTrackQuest?.(selected.questId)}
        >
          {t("ui.questTrack", [], "Track")}
        </button>
        <button
          type="button"
          disabled={!selected || !onShareQuest}
          style={{ ...style.actionButton, ...(!selected || !onShareQuest ? style.actionButtonDisabled : null) }}
          onClick={() => selected && onShareQuest?.(selected.questId)}
        >
          {t("ui.questShare", [], "Share")}
        </button>
        <button
          type="button"
          disabled={!selected || !onAbandonQuest || selected?.stage === "completed"}
          style={{
            ...style.actionButton,
            ...(!selected || !onAbandonQuest || selected?.stage === "completed" ? style.actionButtonDisabled : null),
          }}
          onClick={() => selected && onAbandonQuest?.(selected.questId)}
        >
          {t("ui.questAbandon", [], "Abandon")}
        </button>
      </div>
    </section>
  );
}

function QuestRewardView({
  t,
  rewards,
  fallback,
}: {
  t: TranslateFn;
  rewards: QuestRewardSummary;
  fallback: string;
}) {
  const hasChips =
    (rewards.experience ?? 0) > 0 || (rewards.gold ?? 0) > 0 || (rewards.credit ?? 0) > 0;
  const items = rewards.items ?? [];
  const selectItems = rewards.selectItems ?? [];
  const hasAnything = hasChips || items.length > 0 || selectItems.length > 0;

  if (!hasAnything) {
    return <p style={style.reward}>{fallback || t("ui.questNoReward", [], "No reward listed.")}</p>;
  }

  return (
    <div style={style.rewardBox}>
      {hasChips ? (
        <div style={style.rewardChips}>
          {(rewards.experience ?? 0) > 0 ? (
            <span style={style.rewardChip}>{t("ui.questRewardExp", [formatNumber(rewards.experience!)], `EXP ${formatNumber(rewards.experience!)}`)}</span>
          ) : null}
          {(rewards.gold ?? 0) > 0 ? (
            <span style={style.rewardChip}>{t("ui.questRewardGold", [formatNumber(rewards.gold!)], `Gold ${formatNumber(rewards.gold!)}`)}</span>
          ) : null}
          {(rewards.credit ?? 0) > 0 ? (
            <span style={style.rewardChip}>{t("ui.questRewardCredit", [formatNumber(rewards.credit!)], `Credit ${formatNumber(rewards.credit!)}`)}</span>
          ) : null}
        </div>
      ) : null}
      {items.length > 0 ? <RewardItems items={items} /> : null}
      {selectItems.length > 0 ? (
        <>
          <div style={style.rewardSelectLabel}>{t("ui.questRewardSelect", [], "Choose one:")}</div>
          <RewardItems items={selectItems} />
        </>
      ) : null}
    </div>
  );
}

function RewardItems({ items }: { items: QuestRewardItem[] }) {
  return (
    <div style={style.rewardItems} role="list">
      {items.map((item, index) => (
        <div key={index} role="listitem" title={item.name} style={style.rewardItemCell}>
          {typeof item.icon === "number" ? (
            <img style={style.rewardItemIcon} src={originalAssetPath(`/original-ui/Items/${item.icon}.png`)} alt="" draggable={false} />
          ) : (
            <span style={style.rewardItemText}>{item.name.slice(0, 3)}</span>
          )}
          {item.count && item.count > 1 ? <span style={style.rewardItemCount}>{item.count}</span> : null}
        </div>
      ))}
    </div>
  );
}

function isObjectiveDone(objective: QuestObjective): boolean {
  if (typeof objective.done === "boolean") return objective.done;
  if (objective.required !== undefined && objective.required > 0) {
    return (objective.current ?? 0) >= objective.required;
  }
  return false;
}

function formatNumber(value: number): string {
  return Math.max(0, Math.trunc(value)).toLocaleString("en-US");
}

function stageLabel(t: TranslateFn, stage: QuestStage) {
  switch (stage) {
    case "available":
      return t("ui.questStageAvailable", [], "Available");
    case "inProgress":
      return t("ui.questStageInProgress", [], "In Progress");
    case "readyToTurnIn":
      return t("ui.questStageReady", [], "Ready");
    case "completed":
    default:
      return t("ui.questStageCompleted", [], "Completed");
  }
}

function stageColor(stage: QuestStage) {
  switch (stage) {
    case "available":
      return "#7fd1ff";
    case "inProgress":
      return "#f0d69b";
    case "readyToTurnIn":
      return "#8be07a";
    case "completed":
    default:
      return "#9c8d6f";
  }
}

function progressPercent(current: number, required: number) {
  return Math.min(100, Math.max(0, (current / Math.max(required, 1)) * 100));
}

function progressPercentLabel(current: number, required: number) {
  if (required <= 0) {
    return current > 0 ? `${current}` : "0";
  }
  return `${current}/${required}`;
}

const style: Record<string, CSSProperties> = {
  window: {
    position: "absolute",
    left: 562,
    top: 5,
    width: FRAME.width,
    height: FRAME.height,
    zIndex: 29,
    color: "#f0eee8",
    fontSize: 12,
    textShadow: "1px 1px 0 #000",
    fontFamily: "inherit",
  },
  frame: { position: "absolute", inset: 0, width: FRAME.width, height: FRAME.height, pointerEvents: "none" },
  title: { position: "absolute", left: 18, top: 9 },
  titleText: {
    position: "absolute",
    left: 18,
    top: 8,
    height: 16,
    lineHeight: "16px",
    fontSize: 12,
    fontWeight: 700,
    color: "#f4dcaf",
    letterSpacing: 0.5,
  },
  close: { position: "absolute", left: 288, top: 3 },
  help: { position: "absolute", left: 262, top: 3 },
  tabs: {
    position: "absolute",
    left: 10,
    top: 30,
    width: 292,
    display: "flex",
    gap: 2,
  },
  tab: {
    flex: 1,
    minWidth: 0,
    border: "1px solid rgba(190, 157, 99, 0.5)",
    background: "linear-gradient(180deg, rgba(52, 32, 18, 0.92), rgba(28, 17, 9, 0.92))",
    color: "#cbb38a",
    padding: "2px 0",
    fontSize: 10,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    cursor: "pointer",
  },
  tabActive: {
    background: "linear-gradient(180deg, rgba(120, 74, 34, 0.96), rgba(70, 40, 20, 0.96))",
    color: "#f8e6bb",
    borderColor: "rgba(214, 180, 110, 0.85)",
  },
  tabCount: { fontSize: 9, opacity: 0.8 },
  list: {
    position: "absolute",
    left: 10,
    top: 56,
    width: 292,
    height: 196,
    display: "flex",
    flexDirection: "column",
    gap: 2,
    overflow: "hidden",
  },
  empty: { color: "#cbb38a", padding: "8px 4px", fontSize: 11 },
  row: {
    display: "flex",
    alignItems: "center",
    gap: 6,
    width: "100%",
    height: 22,
    padding: "0 6px",
    border: "1px solid transparent",
    background: "rgba(20, 13, 7, 0.4)",
    color: "#e3d3af",
    textAlign: "left",
    cursor: "pointer",
  },
  rowSelected: {
    background: "rgba(95, 53, 24, 0.5)",
    borderColor: "rgba(214, 180, 110, 0.7)",
  },
  stageDot: { width: 7, height: 7, borderRadius: "50%", flex: "0 0 auto" },
  rowTitle: { flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  rowStage: { fontSize: 10, color: "#cbb38a", flex: "0 0 auto" },
  pagePrev: { position: "absolute", left: 132, top: 256 },
  pageLabel: {
    position: "absolute",
    left: 150,
    top: 256,
    width: 64,
    textAlign: "center",
    fontSize: 11,
    color: "#cbb38a",
    lineHeight: "16px",
  },
  pageNext: { position: "absolute", left: 214, top: 256 },
  detail: {
    position: "absolute",
    left: 12,
    top: 278,
    width: 288,
    height: 118,
    overflowY: "auto",
    overflowX: "hidden",
    border: "1px solid rgba(190, 157, 99, 0.32)",
    background: "linear-gradient(180deg, rgba(27, 19, 10, 0.78), rgba(11, 8, 5, 0.7))",
    padding: "6px 8px",
  },
  detailTitle: { color: "#f8e6bb", fontSize: 12, fontWeight: 700, marginBottom: 2 },
  detailStageRow: { display: "flex", gap: 8, alignItems: "baseline", marginBottom: 3, flexWrap: "wrap" },
  detailStageTag: { fontSize: 10, fontWeight: 700 },
  detailTracker: { fontSize: 10, color: "#b7a884" },
  detailTimeLimit: { fontSize: 10, color: "#e0b06a" },
  detailSummary: { margin: "0 0 4px", fontSize: 11, color: "#d6c6a5", lineHeight: 1.3 },
  detailObjectiveLabel: { fontSize: 9, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  detailObjective: { margin: "0 0 4px", fontSize: 11, color: "#e3d3af", lineHeight: 1.3 },
  objectiveList: { listStyle: "none", margin: "2px 0 4px", padding: 0, display: "flex", flexDirection: "column", gap: 2 },
  objectiveItem: { display: "flex", alignItems: "baseline", gap: 5, fontSize: 11 },
  objectiveCheck: { flex: "0 0 auto", fontSize: 10 },
  objectiveText: { flex: 1, minWidth: 0, color: "#e3d3af", lineHeight: 1.3 },
  objectiveTextDone: { color: "#9c8d6f", textDecoration: "line-through" },
  objectiveCount: { flex: "0 0 auto", fontSize: 10, color: "#cbb38a" },
  detailReturn: { fontSize: 10, color: "#b7a884", marginBottom: 4 },
  progressRow: { display: "flex", justifyContent: "space-between", fontSize: 10, color: "#cbb38a", marginBottom: 2 },
  progressTrack: {
    position: "relative",
    height: 6,
    background: "rgba(0, 0, 0, 0.55)",
    border: "1px solid rgba(190, 157, 99, 0.4)",
    overflow: "hidden",
    marginBottom: 4,
  },
  progressFill: { position: "absolute", left: 0, top: 0, bottom: 0, display: "block" },
  rewardLabel: { fontSize: 9, color: "#a89568", textTransform: "uppercase", letterSpacing: 0.5 },
  reward: { margin: 0, fontSize: 11, color: "#f0d69b", lineHeight: 1.3 },
  rewardBox: { display: "flex", flexDirection: "column", gap: 3, marginTop: 2 },
  rewardChips: { display: "flex", flexWrap: "wrap", gap: 4 },
  rewardChip: {
    fontSize: 10,
    color: "#f0d69b",
    border: "1px solid rgba(214, 180, 110, 0.45)",
    borderRadius: 2,
    padding: "1px 6px",
    background: "rgba(0, 0, 0, 0.3)",
  },
  rewardSelectLabel: { fontSize: 9, color: "#a89568" },
  rewardItems: { display: "flex", flexWrap: "wrap", gap: 4 },
  rewardItemCell: {
    position: "relative",
    width: 28,
    height: 28,
    border: "1px solid rgba(190, 157, 99, 0.3)",
    background: "rgba(0, 0, 0, 0.4)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    overflow: "hidden",
  },
  rewardItemIcon: { width: 24, height: 24, imageRendering: "pixelated" },
  rewardItemText: { fontSize: 9, color: "#cbb38a" },
  rewardItemCount: {
    position: "absolute",
    right: 1,
    bottom: 0,
    fontSize: 8,
    lineHeight: "9px",
    padding: "0 1px",
    color: "#f8e6bb",
    background: "rgba(0, 0, 0, 0.6)",
  },
  actions: {
    position: "absolute",
    left: 12,
    top: 402,
    width: 288,
    display: "flex",
    gap: 6,
  },
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
