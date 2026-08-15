import { QUEST_CLASS_MASKS } from "./route-manifest.mjs";
import { normalizedQuestStage, questState } from "./policy.mjs";

/**
 * Data-driven policy for the post-tutorial Crystal route. It only returns
 * semantic goals; the browser runner must realize every goal through visible
 * mouse/keyboard input and may use snapshots only as observations.
 */
export function planNextAuthoritativeQuest(
  snapshot,
  route,
  {
    minQuestId = 10,
    maxQuestId = Number.POSITIVE_INFINITY,
    targetLevel = 50,
    grindingCatalog = [],
    handledBlockers = [],
    deferredQuestIds = [],
  } = {},
) {
  const playerLevel = finiteNumber(snapshot?.playerLevel, 1);
  const handled = new Set(handledBlockers.map(String));
  const deferred = new Set(deferredQuestIds.map(Number));
  const certifiedGrindingMonsterNames = completedQuestCombatCertifications(snapshot, route);
  const eligible = (route?.quests ?? [])
    .filter((quest) =>
      quest.questId >= minQuestId && quest.questId <= maxQuestId &&
      !deferred.has(quest.questId) && quest.eligibility.minLevel <= playerLevel
    );

  const actionable = eligible.filter((quest) =>
    (quest.blockers ?? []).every((blocker) => handled.has(String(blocker)))
  );
  const byStage = (stage) => actionable
    .filter((quest) => normalizedQuestStage(questState(snapshot, quest.questId)?.stage) === stage)
    .sort((left, right) => questPriority(snapshot, left) - questPriority(snapshot, right));

  const ready = byStage("readytoturnin")[0];
  if (ready) return finishGoal(ready, route.className);

  let preparationFallback = null;
  for (const quest of byStage("inprogress")) {
    const objective = nextObjectiveGoal(snapshot, quest);
    if (objective) {
      const sourceLevel = finiteNumber(objective.monsterLevel, playerLevel);
      const levelGap = sourceLevel - playerLevel;
      // Quest min-level is permission to accept the task, not proof that a
      // fresh melee character can sustainably farm its source. Crystal's q25
      // is available at 7 while CannibalPlant is level 20. If the source is
      // far above the player, earn normal XP on a real safe spawn first; the
      // active quest and its container progress remain untouched.
      if (levelGap > 7) {
        const preparationLevel = Math.min(
          finiteNumber(targetLevel, sourceLevel - 7),
          Math.max(playerLevel + 1, sourceLevel - 7),
        );
        const grind = chooseGrindingGoal(snapshot, grindingCatalog, preparationLevel, {
          certifiedMonsterNames: certifiedGrindingMonsterNames,
        });
        if (grind) {
          preparationFallback ??= {
            ...grind,
            preparationForQuestId: quest.questId,
            preparationForMonsterName: objective.monsterName,
            preparationForMonsterLevel: sourceLevel,
          };
          // Another active quest may be an immediately actionable delivery or
          // dialogue. Finish that ordinary work before committing to a long
          // grind, while keeping this high-level objective untouched.
          continue;
        }
      }
      return objective;
    }
    if ((quest.objectives?.flag ?? []).length > 0) {
      return {
        kind: "special-script",
        questId: quest.questId,
        questName: quest.name,
        flags: quest.objectives.flag,
        reason: "quest flag requires a visible scripted-world interaction",
      };
    }
    // Dialogue/carry quests can remain in-progress until their visible return
    // NPC is spoken to. Attempt that hand-off instead of synthesizing state.
    if (quest.finishNpc) return npcQuestGoal("finish", quest, quest.finishNpc, route.className);
    return {
      kind: "wait",
      questId: quest.questId,
      reason: "authoritative quest is active with no unfinished observable objective",
    };
  }

  const availableQuests = byStage("available");
  if (preparationFallback) {
    const safeSideQuest = availableQuests.find(questHasOnlyNonCombatObjectives);
    if (safeSideQuest) {
      return safeSideQuest.startNpc
        ? npcQuestGoal("accept", safeSideQuest, safeSideQuest.startNpc, route.className)
        : diaryQuestGoal("accept", safeSideQuest, route.className);
    }
    const safeCombatSideQuest = availableQuests.find((quest) => {
      const objective = nextObjectiveGoal(snapshot, quest);
      const sourceLevel = objective?.monsterLevel;
      return (
        objective != null &&
        Number.isFinite(sourceLevel) &&
        sourceLevel > 0 &&
        sourceLevel - playerLevel <= 7
      );
    });
    if (safeCombatSideQuest) {
      // An already offered quest against a source inside the same bounded
      // melee gap is useful real-player progression, not filler grinding.
      // Accept it before spending hours on a previously certified low-level
      // spawn; the high-level active quest remains safely deferred.
      return safeCombatSideQuest.startNpc
        ? npcQuestGoal("accept", safeCombatSideQuest, safeCombatSideQuest.startNpc, route.className)
        : diaryQuestGoal("accept", safeCombatSideQuest, route.className);
    }
    return preparationFallback;
  }

  const available = availableQuests[0];
  if (available) {
    return available.startNpc
      ? npcQuestGoal("accept", available, available.startNpc, route.className)
      : diaryQuestGoal("accept", available, route.className);
  }

  const unfinishedBlocked = eligible.find((quest) => {
    const stage = normalizedQuestStage(questState(snapshot, quest.questId)?.stage);
    return stage !== "completed" && (quest.blockers ?? []).some((blocker) => !handled.has(String(blocker)));
  });

  if (playerLevel < targetLevel) {
    const grind = chooseGrindingGoal(snapshot, grindingCatalog, targetLevel, {
      certifiedMonsterNames: certifiedGrindingMonsterNames,
    });
    if (grind) return grind;
  }

  const remaining = eligible.filter(
    (quest) => normalizedQuestStage(questState(snapshot, quest.questId)?.stage) !== "completed",
  );
  if (remaining.length === 0 && playerLevel >= targetLevel) {
    return {
      kind: "done",
      className: route.className,
      targetLevel,
      completedQuestIds: eligible.map((quest) => quest.questId),
    };
  }
  if (unfinishedBlocked) {
    return {
      kind: "blocked",
      questId: unfinishedBlocked.questId,
      questName: unfinishedBlocked.name,
      blockers: unfinishedBlocked.blockers.filter((blocker) => !handled.has(String(blocker))),
    };
  }
  return {
    kind: "wait",
    questId: null,
    reason: playerLevel < targetLevel
      ? `no safe real-spawn grind candidate for level ${playerLevel}`
      : `${remaining.length} eligible quests have not reached an actionable state`,
  };
}

export function chooseQuestRewardIndex(quest, className) {
  const reward = chooseQuestReward(quest, className);
  return reward ? Number(reward.selectionIndex ?? 0) : undefined;
}

export function chooseQuestReward(quest, className, { gender = "male" } = {}) {
  const rewards = quest?.rewards?.selectableItems ?? [];
  if (!rewards.length) return null;
  const classMask = QUEST_CLASS_MASKS[className] ?? 0;
  const genderMask = String(gender).toLowerCase() === "female" ? 2 : 1;
  const classCompatible = (reward) =>
    Number(reward.requiredClass ?? 0) === 0 ||
    (Number(reward.requiredClass) & classMask) !== 0;
  const genderCompatible = (reward) =>
    Number(reward.requiredGender ?? 0) === 0 ||
    (Number(reward.requiredGender) & genderMask) !== 0;
  return rewards.find((reward) => classCompatible(reward) && genderCompatible(reward))
    ?? rewards.find(classCompatible)
    ?? rewards[0];
}

/**
 * Build strongest-first equipment candidates from the exact rewards the
 * visible quest flow can award. Item type is intentionally resolved later
 * from the live inventory's authoritative equipSlot metadata, so consumables
 * and skill books naturally fall out without a hand-maintained item list.
 */
export function buildProgressionEquipmentCandidates(route, { gender = "male" } = {}) {
  const className = String(route?.className ?? "Warrior");
  const classMask = QUEST_CLASS_MASKS[className] ?? 0;
  const genderMask = String(gender).toLowerCase() === "female" ? 2 : 1;
  const compatible = (reward) => (
    (Number(reward?.count ?? 0) > 0) &&
    (
      Number(reward?.requiredClass ?? 0) === 0 ||
      (Number(reward?.requiredClass) & classMask) !== 0
    ) &&
    (
      Number(reward?.requiredGender ?? 0) === 0 ||
      (Number(reward?.requiredGender) & genderMask) !== 0
    )
  );
  const candidates = (route?.quests ?? []).flatMap((quest) => {
    const fixed = (quest?.rewards?.fixedItems ?? []).filter(compatible);
    const selected = chooseQuestReward(quest, className, { gender });
    return [...fixed, ...(selected && compatible(selected) ? [selected] : [])].map((reward) => ({
      questId: Number(quest.questId),
      minLevel: Number(quest?.eligibility?.minLevel ?? 1),
      name: String(reward.itemName),
    }));
  });
  candidates.sort((left, right) =>
    right.questId - left.questId || right.minLevel - left.minLevel || left.name.localeCompare(right.name)
  );
  return candidates.filter((candidate, index) =>
    candidate.name && candidates.findIndex((entry) => entry.name === candidate.name) === index
  );
}

export function chooseGrindingGoal(
  snapshot,
  catalog,
  targetLevel = 50,
  { certifiedMonsterNames = [] } = {},
) {
  const level = finiteNumber(snapshot?.playerLevel, 1);
  if (level >= targetLevel) return null;
  const currentMap = String(snapshot?.mapFileName ?? "");
  const certified = new Set(
    (Array.isArray(certifiedMonsterNames) ? certifiedMonsterNames : [])
      .map(normalizeName)
      .filter(Boolean),
  );
  const candidates = (catalog ?? [])
    .filter((monster) => {
      const combatCertified = certified.has(normalizeName(monster.monsterName));
      const maximumLevel = level + (combatCertified ? 4 : 1);
      return monster.level >= Math.max(1, level - 4) && monster.level <= maximumLevel;
    })
    .flatMap((monster) => (monster.spawns ?? []).map((spawn) => ({
      monster,
      spawn,
      combatCertified: certified.has(normalizeName(monster.monsterName)),
    })))
    .sort((left, right) =>
      Number(String(right.spawn.mapFileName) === currentMap) - Number(String(left.spawn.mapFileName) === currentMap) ||
      grindCandidateScore(snapshot, left.monster, left.spawn, level, left.combatCertified) -
        grindCandidateScore(snapshot, right.monster, right.spawn, level, right.combatCertified) ||
      right.spawn.count - left.spawn.count ||
      right.monster.experience - left.monster.experience
    );
  const best = candidates[0];
  if (!best) return null;
  return {
    kind: "grind",
    questId: null,
    monsterName: best.monster.monsterName,
    targetLevel,
    targetMapFileName: best.spawn.mapFileName,
    fields: candidateFields(best.monster.spawns, best.spawn.mapFileName, best.spawn, snapshot),
    monsterLevel: best.monster.level,
    experience: best.monster.experience,
    hp: best.monster.hp,
  };
}

export function completedQuestCombatCertifications(snapshot, route) {
  const names = new Set();
  for (const quest of route?.quests ?? []) {
    if (normalizedQuestStage(questState(snapshot, quest.questId)?.stage) !== "completed") continue;
    for (const objective of quest.objectives?.kill ?? []) {
      if (objective?.monsterName) names.add(String(objective.monsterName));
    }
    for (const objective of quest.objectives?.item ?? []) {
      for (const source of objective?.sources ?? []) {
        if (source?.monsterName) names.add(String(source.monsterName));
      }
    }
  }
  return [...names];
}

function finishGoal(quest, className) {
  return quest.finishNpc
    ? npcQuestGoal("finish", quest, quest.finishNpc, className)
    : diaryQuestGoal("finish", quest, className);
}

function questHasOnlyNonCombatObjectives(quest) {
  return (
    (quest?.objectives?.kill?.length ?? 0) === 0 &&
    (quest?.objectives?.item?.length ?? 0) === 0 &&
    (quest?.objectives?.flag?.length ?? 0) === 0
  );
}

function diaryQuestGoal(action, quest, className) {
  return {
    kind: "quest-diary",
    action,
    questId: quest.questId,
    questName: quest.name,
    selectedItemIndex: action === "finish" ? chooseQuestRewardIndex(quest, className) : undefined,
  };
}

function npcQuestGoal(action, quest, binding, className) {
  return {
    kind: "talk",
    action,
    questId: quest.questId,
    questName: quest.name,
    target: `@quest:${action}:${quest.questId}`,
    selectedItemIndex: action === "finish" ? chooseQuestRewardIndex(quest, className) : undefined,
    npc: {
      npcIndex: binding.objectId,
      label: binding.name,
      mapFileName: binding.mapFileName,
      x: binding.position.x,
      y: binding.position.y,
    },
  };
}

function nextObjectiveGoal(snapshot, quest) {
  const state = questState(snapshot, quest.questId);
  const killTasks = quest.objectives?.kill ?? [];
  const itemTasks = quest.objectives?.item ?? [];
  const totalCountedTasks = killTasks.length + itemTasks.length;
  const goals = [];

  for (const task of killTasks) {
    const progress = taskProgress(state, task.monsterName, task.count, totalCountedTasks);
    if (progress.current >= progress.required) continue;
    const spawn = preferredSpawn(snapshot, task.spawnCandidates ?? []);
    if (!spawn) continue;
    goals.push({
      kind: "hunt",
      questId: quest.questId,
      questName: quest.name,
      monsterName: task.monsterName,
      monsterLevel: finiteNumber(spawn.monsterLevel, null),
      monsterHp: finiteNumber(spawn.monsterHp, null),
      harvest: false,
      targetMapFileName: spawn.mapFileName,
      fields: candidateFields(task.spawnCandidates, spawn.mapFileName, spawn, snapshot),
      objective: { ...progress, type: "kill", label: task.monsterName },
      remaining: progress.required - progress.current,
    });
  }

  for (const task of itemTasks) {
    const progress = taskProgress(state, task.itemName, task.count, totalCountedTasks);
    if (progress.current >= progress.required) continue;
    const sourceRows = (task.sources ?? []).flatMap((source) =>
      (source.spawnCandidates ?? []).map((spawn) => ({ source, spawn }))
    );
    const preferred = sourceRows.sort((left, right) =>
      Number(String(right.spawn.mapFileName) === String(snapshot?.mapFileName)) -
        Number(String(left.spawn.mapFileName) === String(snapshot?.mapFileName)) ||
      dropExpectedCost(left.source) - dropExpectedCost(right.source) ||
      right.spawn.count - left.spawn.count ||
      spawnDistance(snapshot, left.spawn) - spawnDistance(snapshot, right.spawn)
    )[0];
    if (!preferred) continue;
    goals.push({
      kind: "hunt",
      questId: quest.questId,
      questName: quest.name,
      monsterName: preferred.source.monsterName,
      monsterLevel: finiteNumber(preferred.spawn.monsterLevel, null),
      monsterHp: finiteNumber(preferred.spawn.monsterHp, null),
      itemName: task.itemName,
      harvest: preferred.source.requiresHarvest === true,
      targetMapFileName: preferred.spawn.mapFileName,
      fields: candidateFields(
        preferred.source.spawnCandidates,
        preferred.spawn.mapFileName,
        preferred.spawn,
        snapshot,
      ),
      objective: { ...progress, type: "item", label: task.itemName },
      remaining: progress.required - progress.current,
    });
  }

  return goals.sort((left, right) =>
    Number(String(right.targetMapFileName) === String(snapshot?.mapFileName)) -
      Number(String(left.targetMapFileName) === String(snapshot?.mapFileName)) ||
    right.remaining - left.remaining ||
    left.monsterName.localeCompare(right.monsterName)
  )[0] ?? null;
}

function taskProgress(stateQuest, label, required, taskCount) {
  const wanted = normalizeName(label);
  const objective = (stateQuest?.objectives ?? []).find((entry) =>
    normalizeName(entry?.label).includes(wanted)
  );
  return {
    current: finiteNumber(
      objective?.current,
      taskCount === 1 ? finiteNumber(stateQuest?.current, 0) : 0,
    ),
    required: Math.max(1, finiteNumber(
      objective?.required,
      taskCount === 1 ? finiteNumber(stateQuest?.required, required) : required,
    )),
  };
}

function questPriority(snapshot, quest) {
  const currentMap = String(snapshot?.mapFileName ?? "");
  const stage = normalizedQuestStage(questState(snapshot, quest.questId)?.stage);
  const binding = stage === "available" ? quest.startNpc : quest.finishNpc;
  const onCurrentMap = binding == null || String(binding.mapFileName) === currentMap;
  return (onCurrentMap ? 0 : 1_000_000) + quest.eligibility.minLevel * 1_000 + quest.questId;
}

function preferredSpawn(snapshot, spawns) {
  return [...spawns].sort((left, right) =>
    Number(String(right.mapFileName) === String(snapshot?.mapFileName)) -
      Number(String(left.mapFileName) === String(snapshot?.mapFileName)) ||
    right.count - left.count ||
    spawnDistance(snapshot, left) - spawnDistance(snapshot, right) ||
    left.delayMinutes - right.delayMinutes
  )[0] ?? null;
}

function candidateFields(spawns, preferredMap, preferredSpawn = null, snapshot = null) {
  return (spawns ?? [])
    .filter((spawn) => String(spawn.mapFileName) === String(preferredMap))
    .sort((left, right) =>
      Number(right === preferredSpawn) - Number(left === preferredSpawn) ||
      spawnDistance(snapshot, left) - spawnDistance(snapshot, right)
    )
    .map((spawn) => ({
      mapFileName: String(spawn.mapFileName),
      x: Number(spawn.position.x),
      y: Number(spawn.position.y),
      count: Number(spawn.count),
      spread: Number(spawn.spread),
      delayMinutes: Number(spawn.delayMinutes),
    }));
}

function spawnDistance(snapshot, spawn) {
  const player = snapshot?.player;
  const position = spawn?.position;
  if (!player || !position) return Number.POSITIVE_INFINITY;
  return Math.max(
    Math.abs(finiteNumber(player.x, 0) - finiteNumber(position.x, 0)),
    Math.abs(finiteNumber(player.y, 0) - finiteNumber(position.y, 0)),
  );
}

function dropExpectedCost(source) {
  const numerator = Math.max(1, finiteNumber(source?.chanceNumerator, 1));
  const denominator = Math.max(numerator, finiteNumber(source?.chanceDenominator, numerator));
  return denominator / numerator + (source?.requiresHarvest ? 2 : 0);
}

function grindRiskScore(monster, playerLevel, combatCertified = false) {
  const levelDelta = Math.abs(finiteNumber(monster.level, playerLevel) - playerLevel);
  const hpPerExperience = finiteNumber(monster.hp, 1) / Math.max(1, finiteNumber(monster.experience, 1));
  // Crystal AI 1/2 are the Hen/Deer flee families. They are excellent safe
  // supply sources, but a real melee client spends most of its time chasing
  // them and frequently loses the target at the AOI edge. Prefer a similarly
  // safe non-fleeing source for sustained XP; the dedicated supply loop still
  // selects Deer explicitly for guaranteed Venison.
  const evasiveMeleePenalty = [1, 2].includes(finiteNumber(monster.ai, 0)) ? 25 : 0;
  if (combatCertified) {
    // A completed real kill/harvest quest proves that this character has
    // already sustained the normal-client fight. Within the separately
    // bounded four-level window, optimize for HP spent per XP instead of
    // repeatedly farming a congested low-value beginner spawn forever.
    return hpPerExperience + levelDelta * 0.02 + evasiveMeleePenalty;
  }
  return levelDelta * 10 + hpPerExperience + evasiveMeleePenalty;
}

function grindCandidateScore(snapshot, monster, spawn, playerLevel, combatCertified) {
  const distance = spawnDistance(snapshot, spawn);
  // Within one real map, one hundred physical tiles are worth one combat-risk
  // point for a one-off fight. A level-preparation run, however, pays that
  // walk once and then replans beside the same spawn for many ordinary kills.
  // Amortize the trip over a bounded estimate of the kills still required for
  // the current level; otherwise every goal charges the full walk again and a
  // multi-hour run remains trapped on low-XP village-edge monsters. Keep the
  // bound deliberately small so a merely efficient source cannot justify an
  // unsafe journey across an entire map. Cross-map locality remains the
  // stronger first sort key above.
  const currentExperience = finiteNumber(snapshot?.playerExperience, Number.NaN);
  const maximumExperience = finiteNumber(snapshot?.playerMaxExperience, Number.NaN);
  const monsterExperience = finiteNumber(monster?.experience, Number.NaN);
  const remainingExperience = maximumExperience - currentExperience;
  const expectedKills =
    Number.isFinite(remainingExperience) && remainingExperience > 0 &&
    Number.isFinite(monsterExperience) && monsterExperience > 0
      ? Math.ceil(remainingExperience / monsterExperience)
      : 1;
  const travelAmortization = Math.max(1, Math.min(20, expectedKills));
  const travelPenalty = Number.isFinite(distance)
    ? distance / (100 * travelAmortization)
    : 0;
  return grindRiskScore(monster, playerLevel, combatCertified) + travelPenalty;
}

function normalizeName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function finiteNumber(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}
