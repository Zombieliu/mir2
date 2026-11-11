import {
  localizeCrystalEntityName,
  localizeCrystalItemName,
} from "./crystal-content-localization";
import {
  localizeNewcomerV2Text,
  newcomerV2ObjectiveTextFor,
  newcomerV2QuestCopyFor,
  stripQuestObjectiveProgress,
} from "./newcomer-v2-journey";

export type QuestTranslateFn = ((
  key: string,
  params?: Array<string | number>,
  fallback?: string,
) => string) & { systemText?: (value: string) => string };

export function formatQuestText(
  t: QuestTranslateFn,
  key: string,
  fallback: string,
  args: Readonly<Record<string, string | number>>,
): string {
  return t(key, [], fallback).replace(/\{([a-z]+)\}/gi, (token, name: string) =>
    Object.hasOwn(args, name) ? String(args[name]) : token,
  );
}

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
  22: "forestYetiThreat",
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
  const newcomerV2Copy = newcomerV2QuestCopyFor(quest.questId);
  if (newcomerV2Copy) {
    return localizeNewcomerV2QuestEntry(quest, newcomerV2Copy, t);
  }

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
    localized.rewards = localizeQuestRewards(quest.rewards, t);
  }

  return localized;
}

function localizeNewcomerV2QuestEntry<T extends LocalizableQuestEntry>(
  quest: T,
  copy: NonNullable<ReturnType<typeof newcomerV2QuestCopyFor>>,
  t: QuestTranslateFn,
): T {
  const targetCopyApplies = quest.stage === "inProgress" || quest.stage === "readyToTurnIn";
  const prefix = `quest.${quest.questId}`;
  const originalDescription = quest.descriptionLines?.join("\n") ?? "";
  const description = t(`${prefix}.description`, [], originalDescription);
  const localized = {
    ...quest,
    title: t(`${prefix}.title`, [], localizeNewcomerV2Text(copy.title, t)),
    ...(description !== originalDescription ? { descriptionLines: [description] } : {}),
    ...(targetCopyApplies && !t.systemText ? { objective: localizeNewcomerV2Text(copy.objective, t) } : {}),
  } as T;

  if (quest.objectives?.length) {
    localized.objectives = quest.objectives.map((objective) => ({
      ...objective,
      label: localizeNewcomerObjective(quest.questId, objective.label, t, objective.required !== undefined),
    }));
  }

  if (t.systemText) {
    localized.objective = quest.stage === "inProgress" && localized.objectives?.length
      ? localized.objectives.map(({ label }) => label).join(" · ")
      : localizeNewcomerObjective(quest.questId, quest.objective, t);
    localized.progressLabel = localizeNewcomerProgressLabel(quest.progressLabel, t);
    if (quest.tracker !== undefined) {
      localized.tracker = quest.tracker === quest.title + " is available." || quest.tracker === quest.title + " completed."
        ? ""
        : localizeNewcomerObjective(quest.questId, quest.tracker, t);
    }
  }
  if (quest.npc) localized.npc = localizeCrystalEntityName(quest.npc, t);
  if (quest.rewards) localized.rewards = localizeQuestRewards(quest.rewards, t);

  return localized;
}

function localizeNewcomerObjective(questId: number, label: string, t: QuestTranslateFn, hasCounter?: boolean): string {
  const copy = newcomerV2ObjectiveTextFor(questId, label);
  const systemText = t.systemText;
  if (!systemText) return copy ? localizeNewcomerV2Text(copy, t) : label;
  const source = stripQuestObjectiveProgress(label);
  const sentence = source.replace(/[.。]$/, "");
  if (!copy) {
    const instruction = localizeNewcomerInstruction(questId, sentence, t);
    if (instruction !== null) return instruction;
    if (hasCounter !== undefined) return label;
    return systemText(label);
  }

  const defeat = /^Defeat (\d+) (.+)$/.exec(sentence);
  const kill = /^Kill (.+)$/.exec(sentence);
  let translated = systemText(copy.en);
  if (defeat) {
    const [, count, name] = defeat;
    translated = formatQuestText(t, "quest.objective.defeat", "Defeat {count} {name}.", { count, name: systemText(name) });
  } else if (kill) {
    translated = formatQuestText(t, "quest.objective.kill", "Kill {name}", { name: systemText(kill[1]) });
  }
  return hasCounter ? translated : translated + label.trim().slice(source.length);
}

function localizeNewcomerInstruction(questId: number, text: string, t: QuestTranslateFn): string | null {
  switch (text) {
    case "Return to Assistant Jane":
      return formatQuestText(t, "quest.action.return", "Return to {name}", {
        name: t("content.npc.3.name", [], "Assistant Jane"),
      });
    case "Return to the Bichon Wall Board":
      return t("quest.journey.copy.17", [], text);
    case "Return to the Quest Diary":
      return t("quest.journey.copy.18", [], text);
    case "Return to the quest endpoint":
    case "Return to the quest NPC and hand in the task":
      return t("content.quest.generic.stage.readyToTurnIn.objective", [], text);
    case "No automatic completion condition":
      return t(`quest.${questId}.description`, [], text);
    case "Main-node reward claimed":
    case "Growth reward claimed":
      return t("content.quest.generic.stage.completed.objective", [], text);
    default:
      return null;
  }
}

function localizeNewcomerProgressLabel(label: string, t: QuestTranslateFn): string {
  const count = /^Progress (\d+\/\d+)$/.exec(label);
  if (count) return t("ui.questProgressHeading", [], "Progress") + " " + count[1];
  const level = /^Level (\d+)\+(?:, up to (\d+))?$/.exec(label);
  if (level) {
    return formatQuestText(t, "quest.ui.levelTitle", "Lv {level} {title}", {
      level: level[2] ? level[1] + "–" + level[2] : level[1] + "+",
      title: "",
    }).trim();
  }
  if (label === "Available.") return t("ui.questStage.available", [], label);
  if (label === "Ready to turn in." || label === "Completed.") {
    const stage = label === "Completed." ? "completed" : "readyToTurnIn";
    return t("content.quest.generic.stage." + stage + ".progressLabel", [], label);
  }
  return t.systemText?.(label) ?? label;
}

function localizeQuestRewards(rewards: NonNullable<LocalizableQuestEntry["rewards"]>, t: QuestTranslateFn) {
  const localizeItems = (items: NonNullable<typeof rewards.items>) =>
    items.map((item) => ({ ...item, name: localizeCrystalItemName(item.name, t) }));
  return {
    ...rewards,
    ...(rewards.items ? { items: localizeItems(rewards.items) } : {}),
    ...(rewards.selectItems ? { selectItems: localizeItems(rewards.selectItems) } : {}),
  };
}

export function localizeQuestLog<T extends LocalizableQuestEntry>(
  quests: readonly T[],
  t: QuestTranslateFn,
): T[] {
  return quests.map((quest) => localizeQuestEntry(quest, t));
}
