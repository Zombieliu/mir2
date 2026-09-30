const COFARM_FOLLOWERS = new Map([
  // CleanSkull is guaranteed while q124 is active and the player kills the
  // same bone monsters required by q122/q123. Accept q124 first, then let the
  // ordinary follower quests advance both logs before turning it in.
  [124, [122, 123]],
]);

function normalizedStage(value) {
  return String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
}

export function cofarmFollowers(questId, remainingQuestIds = []) {
  const remaining = new Set(remainingQuestIds.map(Number));
  return (COFARM_FOLLOWERS.get(Number(questId)) ?? []).filter(id => remaining.has(id));
}

export function shouldDeferCofarmQuest(questId, stage, remainingQuestIds = []) {
  return normalizedStage(stage) === 'inprogress' &&
    cofarmFollowers(questId, remainingQuestIds).length > 0;
}

/** Finish already accepted one-step tails before opening a longer expedition. */
export function prioritizeNearCompleteActiveQuests(questIds, questLog, maximumRemaining = 1) {
  const ordered = [...questIds].map(Number);
  const routeOrder = new Map(ordered.map((id, index) => [id, index]));
  const activeById = new Map((questLog ?? []).map(quest => [Number(quest?.questId), quest]));
  const priority = ordered.filter(id => {
    const quest = activeById.get(id);
    return normalizedStage(quest?.stage) === 'inprogress' &&
      remainingQuestWork(quest) <= Math.max(0, Number(maximumRemaining));
  }).sort((left, right) =>
    remainingQuestWork(activeById.get(left)) - remainingQuestWork(activeById.get(right)) ||
    routeOrder.get(left) - routeOrder.get(right));
  const promoted = new Set(priority);
  return [...priority, ...ordered.filter(id => !promoted.has(id))];
}

function remainingQuestWork(quest) {
  const objectives = Array.isArray(quest?.objectives) ? quest.objectives : [];
  if (objectives.length) {
    return objectives.reduce((sum, objective) => sum + Math.max(
      0,
      Number(objective?.required ?? 0) - Number(objective?.current ?? 0),
    ), 0);
  }
  return Math.max(0, Number(quest?.required ?? 0) - Number(quest?.current ?? 0));
}
