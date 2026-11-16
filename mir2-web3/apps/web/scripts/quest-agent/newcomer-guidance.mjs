import fs from "node:fs/promises";

export const guidanceUrl = new URL("../../../../config/quest-guidance/newcomer-v1.json", import.meta.url);
export const journeyUrl = new URL("../../../../config/quest-guidance/newcomer-journey-v1.json", import.meta.url);
export const loadNewcomerGuidance = async () => ({
  ...JSON.parse(await fs.readFile(guidanceUrl, "utf8")),
  journey: JSON.parse(await fs.readFile(journeyUrl, "utf8")),
});

/** Validate against all three current class routes, never infer eligibility from IDs. */
export function validateNewcomerGuidance(config, routes) {
  if (config.schema !== 1 || config.profile !== "newcomer-v1") throw new Error("Unsupported guidance profile");
  if (!Number.isInteger(config.maxLevel) || config.maxLevel < 15 || routes.some((r) => r.maxLevel !== config.maxLevel)) {
    throw new Error("Guidance validation requires full profile level coverage");
  }
  const byId = new Map();
  const categories = ["recommended", "optional", "challenge", "deferred"];
  for (const entry of config.quests) {
    if (!Number.isInteger(entry.id) || byId.has(entry.id)) throw new Error(`Duplicate or invalid quest ${entry.id}`);
    if (!categories.includes(entry.category) || !Number.isInteger(entry.order) || entry.order < 0 || !entry.hint?.trim()) {
      throw new Error(`Invalid guidance for quest ${entry.id}`);
    }
    byId.set(entry.id, entry);
  }
  const allIds = new Set(routes.flatMap((route) => route.quests.map((q) => q.questId)));
  if (allIds.size !== byId.size || [...allIds].some((id) => !byId.has(id))) throw new Error("Guidance does not match current quest coverage");
  for (const route of routes) {
    const routeIds = new Set(route.quests.map((q) => q.questId));
    for (const quest of route.quests) {
      const override = config.journey?.questOverrides.find(row => row.questId === quest.questId);
      const previous = override?.requiredQuestId ?? quest.eligibility.requiredQuestId;
      if (!previous) continue;
      const entry = byId.get(quest.questId);
      const predecessor = byId.get(previous);
      if (!routeIds.has(previous) || !predecessor || predecessor.order >= entry.order) {
        throw new Error(`Broken prerequisite order for ${route.className} quest ${quest.questId}`);
      }
      if (entry.category === "recommended" && predecessor.category !== "recommended") {
        throw new Error(`Recommended quest ${quest.questId} depends on a non-recommended quest`);
      }
    }
  }
  return byId;
}

/** Preserve the input; optionally project the server's explicit newcomer overrides. */
export function annotateNewcomerRoute(route, config, journey) {
  const byId = new Map(config.quests.map((entry) => [entry.id, entry]));
  const overrides = new Map((journey?.questOverrides ?? []).map(row => [row.questId, row]));
  return {
    ...route,
    guidanceProfile: config.profile,
    ...(journey ? { journeyProfile: journey.profile, journeyChapters: journey.chapters } : {}),
    quests: route.quests.map((sourceQuest) => {
      const quest = structuredClone(sourceQuest);
      const override = overrides.get(quest.questId);
      if (override) {
        quest.rewards.experience = override.rewardExperience;
        quest.eligibility.requiredQuestId = override.requiredQuestId ?? quest.eligibility.requiredQuestId;
        quest.objectives.kill.forEach(task => {task.count = Math.min(task.count, override.killCountCap ?? task.count);});
        quest.objectives.item.forEach(task => {task.count = Math.min(task.count, override.itemCountCap ?? task.count);});
        if (override.taskDescription) quest.descriptions.task = override.taskDescription;
        quest.newcomerOverride = override;
      }
      return {...quest, guidance: byId.get(quest.questId) ?? null};
    }),
  };
}
