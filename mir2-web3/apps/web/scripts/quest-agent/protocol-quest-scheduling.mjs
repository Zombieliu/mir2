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
