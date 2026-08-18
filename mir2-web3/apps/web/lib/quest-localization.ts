import {
  localizeCrystalEntityName,
  localizeCrystalItemName,
} from "./crystal-content-localization";

export type QuestTranslateFn = (
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string;

export type LocalizableQuestEntry = {
  questId: number;
  title: string;
  summary: string;
  objective: string;
  progressLabel: string;
  tracker?: string;
  stage: "available" | "inProgress" | "readyToTurnIn" | "completed";
  current: number;
  required: number;
  rewardPreview: string;
  descriptionLines?: string[];
  npc?: string;
  objectives?: Array<{
    label: string;
    current?: number;
    required?: number;
    done?: boolean;
  }>;
  rewards?: {
    items?: Array<{ name: string; [key: string]: unknown }>;
    selectItems?: Array<{ name: string; [key: string]: unknown }>;
    [key: string]: unknown;
  };
};

const CRYSTAL_QUEST_KEY_BY_ID: Readonly<Record<number, string>> = {
  1: "assistantRequest",
  2: "craftLadyRequest",
  3: "talkWithButcher",
  4: "huntForButcher",
  5: "smithFirstTest",
  6: "smithSecondTest",
  7: "meetWarriorInstructor",
  8: "fencingSkillTest",
  9: "toBichon",
  154: "emperorsProblem",
};

/**
 * Localizes the original Crystal quest payload only at the presentation edge.
 * The gateway keeps the canonical packet text untouched, while switching the
 * web-client language immediately refreshes every visible quest string.
 */
export function localizeQuestEntry<T extends LocalizableQuestEntry>(
  quest: T,
  t: QuestTranslateFn,
): T {
  const questKey = CRYSTAL_QUEST_KEY_BY_ID[quest.questId];
  if (!questKey) return quest;

  const prefix = `content.quest.${questKey}`;
  const stagePrefix = `${prefix}.stage.${quest.stage}`;
  const genericStagePrefix = `content.quest.generic.stage.${quest.stage}`;
  const params = [quest.current, quest.required];
  const genericObjective = t(`${genericStagePrefix}.objective`, params, quest.objective);
  const questObjective = t(`${prefix}.objective`, params, quest.objective);
  const stageObjectiveFallback =
    quest.stage === "available" || quest.stage === "inProgress"
      ? questObjective
      : genericObjective;
  const genericProgressLabel = t(
    `${genericStagePrefix}.progressLabel`,
    params,
    quest.progressLabel,
  );
  const genericTracker = t(`${genericStagePrefix}.tracker`, params, quest.tracker ?? "");
  const localized = {
    ...quest,
    title: t(`${prefix}.title`, [], quest.title),
    summary: t(`${prefix}.summary`, params, quest.summary),
    objective: t(`${stagePrefix}.objective`, params, stageObjectiveFallback),
    progressLabel: t(`${stagePrefix}.progressLabel`, params, genericProgressLabel),
    tracker: t(`${stagePrefix}.tracker`, params, genericTracker),
    rewardPreview: t(`${prefix}.rewardPreview`, [], quest.rewardPreview),
  } as T;

  if (quest.descriptionLines?.length) {
    localized.descriptionLines = [
      t(`${prefix}.description`, params, quest.descriptionLines.join("\n")),
    ];
  }

  if (quest.objectives?.length) {
    localized.objectives = quest.objectives.map((objective, index) => ({
      ...objective,
      label: t(`${prefix}.objective.${index}`, params, objective.label),
    }));
  }

  if (quest.npc) {
    localized.npc = localizeCrystalEntityName(quest.npc, t);
  }

  if (quest.rewards) {
    localized.rewards = {
      ...quest.rewards,
      ...(quest.rewards.items
        ? {
            items: quest.rewards.items.map((item) => ({
              ...item,
              name: localizeCrystalItemName(item.name, t),
            })),
          }
        : {}),
      ...(quest.rewards.selectItems
        ? {
            selectItems: quest.rewards.selectItems.map((item) => ({
              ...item,
              name: localizeCrystalItemName(item.name, t),
            })),
          }
        : {}),
    };
  }

  return localized;
}

export function localizeQuestLog<T extends LocalizableQuestEntry>(
  quests: readonly T[],
  t: QuestTranslateFn,
): T[] {
  return quests.map((quest) => localizeQuestEntry(quest, t));
}
