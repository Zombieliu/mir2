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
  objectives?: Array<{
    label: string;
    current?: number;
    required?: number;
    done?: boolean;
  }>;
};

const CRYSTAL_QUEST_KEY_BY_ID: Readonly<Record<number, string>> = {
  1: "assistantRequest",
  2: "craftLadyRequest",
  5: "smithFirstTest",
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
  const params = [quest.current, quest.required];
  const localized = {
    ...quest,
    title: t(`${prefix}.title`, [], quest.title),
    summary: t(`${prefix}.summary`, params, quest.summary),
    objective: t(`${stagePrefix}.objective`, params, quest.objective),
    progressLabel: t(`${stagePrefix}.progressLabel`, params, quest.progressLabel),
    tracker: t(`${stagePrefix}.tracker`, params, quest.tracker ?? ""),
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

  return localized;
}

export function localizeQuestLog<T extends LocalizableQuestEntry>(
  quests: readonly T[],
  t: QuestTranslateFn,
): T[] {
  return quests.map((quest) => localizeQuestEntry(quest, t));
}
