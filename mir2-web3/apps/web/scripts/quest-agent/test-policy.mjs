import assert from "node:assert/strict";
import fs from "node:fs/promises";
import test from "node:test";

import {
  CdpClient,
  classifyBrowserDiagnostics,
  readAgentState,
  targetCombatEvidenceSince,
  wsEventFramesSince,
} from "./browser-driver.mjs";

import {
  BICHON_Q1_Q5_ROUTE,
  BICHON_Q1_Q9_ROUTE,
  QUEST_AGENT_CONTRACT,
  assessGrindingSourceStall,
  assessQuestCombatResourceStrain,
  auditOutgoingBrowserCommand,
  chooseImmediateMeleeTarget,
  collisionPathHasImmediateDynamicBlock,
  collisionPathNeedsPerpendicularFrontier,
  collisionPathNeedsStickyDetour,
  continuousCollisionRunAvoidsTransfers,
  combatMemoryRequiresSupplyRecall,
  dangerousHostileAvoidanceCells,
  denseAdjacentHostileCount,
  duplicateEquippedItemsForSale,
  entityIsLiveActor,
  entityAttackIsRecent,
  equipmentRepairCandidates,
  expandRespawnPatrolFields,
  findCollisionGridPath,
  incidentalTravelThreatIsTrivial,
  missingStarterEquipment,
  nearestActiveHostile,
  nearestGroundDropByName,
  nearestHealthPotionGroundDrop,
  nearestBlockingHostile,
  nearestRespawnApproachPoint,
  objectiveProgress,
  offensiveCombatSkillHotkey,
  ordinarySupplyLootForSale,
  planHealthPotionPurchase,
  planNextQ1Q5,
  planNextQ1Q9,
  protectedTransfersForNavigation,
  rankCombatTargetsByIsolation,
  rankRespawnFieldsForTravel,
  reconcileConfirmedDeadMonsterObjects,
  respawnCorridorAvoidanceWaypoint,
  respawnCorridorExposure,
  respawnTerminalExposure,
  respawnTravelAttemptBudget,
  retreatPointFromHostile,
  restorativeSelfSkillHotkey,
  safeRecoveryPaceTargets,
  selectBestAvailableEquipmentUpgrade,
  selectProgressingCollisionDetour,
  shouldCaptureGoalFrame,
  shouldEnforceShelterEscapeResourceBudget,
  shouldFundHealthPotions,
  surplusQuestMaterialsForSale,
  supersededProgressionGearForSale,
  unresolvedCombatResourceStrains,
} from "./policy.mjs";
import {
  isTransientQuestAgentFatal,
  isTransientQuestAgentExit,
  replaceCliOption,
  restartDelayMs,
  sanitizeAttemptSummary,
  signalExitCode,
  stripSupervisorOptions,
} from "./supervisor-policy.mjs";

const q = (questId, stage, objectives = []) => ({ questId, stage, objectives });
const snapshot = (...questLog) => ({ questLog });

test("nearby gold selection stays within the visible pickup radius", () => {
  const state = {
    player: { x: 100, y: 100 },
    groundDrops: [
      { objectId: "far", name: "Gold", x: 102, y: 100 },
      { objectId: "item", name: "CannibalLeaf", x: 100, y: 100 },
      { objectId: "near-b", name: "Gold", x: 101, y: 100 },
      { objectId: "near-a", name: "Gold", x: 100, y: 100 },
    ],
  };
  assert.equal(nearestGroundDropByName(state, "Gold", 1)?.objectId, "near-a");
  assert.equal(nearestGroundDropByName(state, "Gold", 0)?.objectId, "near-a");
  assert.equal(
    nearestGroundDropByName(state, "Gold", 1, ["near-a"])?.objectId,
    "near-b",
  );
  assert.equal(nearestGroundDropByName({ groundDrops: state.groundDrops }, "Gold", 1), null);
  assert.equal(nearestGroundDropByName({
    player: { x: 100, y: 100 },
    groundDrops: [
      { objectId: "quantity-label", name: "51 Gold", x: 101, y: 100 },
      { objectId: "not-gold", name: "GoldNecklace", x: 100, y: 100 },
    ],
  }, "Gold", 1)?.objectId, "quantity-label");
});

test("dense adjacent occupancy counts only live hostile monsters in the movement ring", () => {
  const state = {
    player: { x: 10, y: 10 },
    entities: [
      { kind: "monster", disposition: "hostile", dead: false, x: 10, y: 9 },
      { kind: "monster", disposition: "hostile", dead: false, x: 11, y: 10 },
      { kind: "monster", disposition: "hostile", dead: false, x: 9, y: 11 },
      { kind: "monster", disposition: "hostile", dead: true, x: 10, y: 11 },
      { kind: "monster", disposition: "hostile", dead: false, hp: 0, x: 11, y: 9 },
      { kind: "monster", disposition: "friendly", dead: false, x: 9, y: 10 },
      { kind: "npc", disposition: "hostile", dead: false, x: 10, y: 10 },
      { kind: "monster", disposition: "hostile", dead: false, x: 10, y: 10 },
      { kind: "monster", disposition: "hostile", dead: false, x: 12, y: 10 },
    ],
  };
  assert.equal(denseAdjacentHostileCount(state), 3);
  assert.equal(denseAdjacentHostileCount(state, 2), 4);
  assert.equal(denseAdjacentHostileCount({ entities: state.entities }), 0);
});

test("authoritative zero HP overrides a lagging rendered death flag", () => {
  assert.equal(entityIsLiveActor({ dead: false, hp: 0 }), false);
  assert.equal(entityIsLiveActor({ dead: false, hp: "0" }), false);
  assert.equal(entityIsLiveActor({ dead: false, hp: 20 }), true);
  assert.equal(entityIsLiveActor({ dead: false }), true);
});

test("confirmed monster death remains excluded while a stale rendered actor persists", () => {
  const confirmedAt = 1_000;
  const records = new Map([
    ["202215", { confirmedAt, absenceObserved: false }],
  ]);

  const reconciled = reconcileConfirmedDeadMonsterObjects(
    records,
    [{ objectId: 202215, kind: "monster", dead: false, hp: null }],
    confirmedAt + 2_000,
    10 * 60_000,
  );

  assert.deepEqual(reconciled.get("202215"), {
    confirmedAt,
    absenceObserved: false,
  });
});

test("confirmed monster object is released only after absence and a positive-HP respawn", () => {
  const confirmedAt = 1_000;
  const records = new Map([
    ["202215", { confirmedAt, absenceObserved: false }],
  ]);

  const absent = reconcileConfirmedDeadMonsterObjects(
    records,
    [],
    confirmedAt + 2_000,
    10 * 60_000,
  );
  assert.deepEqual(absent.get("202215"), {
    confirmedAt,
    absenceObserved: true,
  });

  const staleCorpse = reconcileConfirmedDeadMonsterObjects(
    absent,
    [{ objectId: "202215", kind: "monster", dead: false, hp: null }],
    confirmedAt + 3_000,
    10 * 60_000,
  );
  assert.equal(staleCorpse.has("202215"), true);

  const respawned = reconcileConfirmedDeadMonsterObjects(
    absent,
    [{ objectId: "202215", kind: "monster", dead: false, hp: 80 }],
    confirmedAt + 4_000,
    10 * 60_000,
  );
  assert.equal(respawned.has("202215"), false);
});

test("confirmed monster object hold expires defensively", () => {
  const confirmedAt = 1_000;
  const records = new Map([
    ["202215", { confirmedAt, absenceObserved: false }],
  ]);

  const reconciled = reconcileConfirmedDeadMonsterObjects(
    records,
    [{ objectId: "202215", kind: "monster", dead: false, hp: null }],
    confirmedAt + 10 * 60_000,
    10 * 60_000,
  );

  assert.equal(reconciled.has("202215"), false);
});

test("safe recovery pacing stays in a bounded cardinal loop around its anchor", () => {
  assert.deepEqual(safeRecoveryPaceTargets({ x: 2, y: 11 }, 2), [
    { x: 4, y: 11 },
    { x: 2, y: 13 },
    { x: 0, y: 11 },
    { x: 2, y: 9 },
  ]);
  assert.deepEqual(safeRecoveryPaceTargets({ x: 2, y: 11 }, 0), [
    { x: 3, y: 11 },
    { x: 2, y: 12 },
    { x: 1, y: 11 },
    { x: 2, y: 10 },
  ]);
  assert.deepEqual(safeRecoveryPaceTargets(null, 2), []);
});

test("safe recovery rotates a portal only after bounded net-progress stall", async () => {
  const policy = await import("./policy.mjs");
  let progress = policy.assessRecoveryTransferProgress?.({
    transferKey: "near",
    distance: 38,
    now: 1_000,
  });
  assert.deepEqual(progress, {
    transferKey: "near",
    bestDistance: 38,
    lastProgressAt: 1_000,
    stalled: false,
  });

  progress = policy.assessRecoveryTransferProgress?.({
    transferKey: "near",
    distance: 29,
    now: 2_000,
    previous: progress,
    stalledAfterMs: 45_000,
  });
  assert.equal(progress.stalled, false);
  assert.equal(progress.bestDistance, 29);

  progress = policy.assessRecoveryTransferProgress?.({
    transferKey: "near",
    distance: 38,
    now: 47_000,
    previous: progress,
    stalledAfterMs: 45_000,
  });
  assert.equal(progress.stalled, true);
  assert.equal(progress.bestDistance, 29);

  const alternate = policy.assessRecoveryTransferProgress?.({
    transferKey: "alternate",
    distance: 47,
    now: 47_000,
    previous: progress,
    stalledAfterMs: 45_000,
  });
  assert.deepEqual(alternate, {
    transferKey: "alternate",
    bestDistance: 47,
    lastProgressAt: 47_000,
    stalled: false,
  });
});

test("visible HP-drug selection ignores unrelated and distant drops", () => {
  const state = {
    player: { x: 100, y: 100 },
    groundDrops: [
      { objectId: "far", name: "(HP)DrugSmall", x: 109, y: 100 },
      { objectId: "gold", name: "Gold", x: 100, y: 100 },
      { objectId: "near", name: "(HP)DrugSmall", x: 103, y: 101 },
    ],
  };
  assert.equal(nearestHealthPotionGroundDrop(state, 8)?.objectId, "near");
  assert.equal(nearestHealthPotionGroundDrop(state, 2), null);
});

test("potion funding is limited to an underfunded character in the supply area", () => {
  const state = {
    mapFileName: "0",
    player: { x: 288, y: 616 },
    gold: 31,
    beltItems: [],
    inventoryItems: [],
  };
  const options = { merchant: { x: 288, y: 608 }, minimumGold: 40 };
  assert.equal(shouldFundHealthPotions(state, options), true);
  assert.equal(shouldFundHealthPotions({ ...state, gold: 40 }, options), false);
  assert.equal(shouldFundHealthPotions({
    ...state,
    gold: 9,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 1 }],
  }, { ...options, minimumGold: 160, minimumPotions: 5 }), true);
  assert.equal(shouldFundHealthPotions({
    ...state,
    gold: 160,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 1 }],
  }, { ...options, minimumGold: 160, minimumPotions: 5 }), false);
  assert.equal(shouldFundHealthPotions({
    ...state,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 1 }],
  }, options), false);
  assert.equal(shouldFundHealthPotions({
    ...state,
    player: { x: 80, y: 80 },
  }, options), false);
  assert.equal(shouldFundHealthPotions({ ...state, mapFileName: "1" }, options), false);
});

test("potion purchases preserve working capital until a meaningful batch is affordable", () => {
  const plan = (currentQuantity, gold, unitPrice = 40) =>
    planHealthPotionPurchase({
      currentQuantity,
      gold,
      unitPrice,
      departureStock: 10,
      workingStock: 5,
    });

  assert.equal(plan(0, 199), 0);
  assert.equal(plan(0, 200), 5);
  assert.equal(plan(0, 399), 5);
  assert.equal(plan(0, 400), 10);
  assert.equal(plan(4, 39), 0);
  assert.equal(plan(4, 40), 1);
  assert.equal(plan(5, 199), 0);
  assert.equal(plan(5, 200), 5);
  assert.equal(plan(9, 40), 1);
  assert.equal(plan(10, 400), 0);
  assert.equal(plan(0, 400, 0), 0);
});

test("q1-q5 policy follows the original beginner hand-offs", () => {
  assert.deepEqual(planNextQ1Q5(snapshot(q(1, "available"))), {
    kind: "talk", action: "accept", questId: 1, npcKey: "assistant", target: "@quest:accept:1",
  });
  assert.equal(planNextQ1Q5(snapshot(q(1, "readyToTurnIn"))).npcKey, "craftLady");
  assert.equal(planNextQ1Q5(snapshot(q(1, "completed"), q(2, "available"))).questId, 2);
  assert.equal(
    planNextQ1Q5(snapshot(q(1, "completed"), q(2, "inProgress"), q(5, "available"))).questId,
    5,
  );
  assert.deepEqual(
    planNextQ1Q5(snapshot(q(1, "completed"), q(2, "inProgress"), q(5, "inProgress"))),
    { kind: "hunt", questId: 2, monsterName: "Scarecrow", harvest: false },
  );
  assert.equal(
    planNextQ1Q5(snapshot(q(1, "completed"), q(2, "completed"), q(3, "readyToTurnIn"))).rewardChoiceTarget,
    "@quest:finish:3:0",
  );
  assert.deepEqual(
    planNextQ1Q5(snapshot(q(1, "completed"), q(2, "completed"), q(3, "completed"), q(4, "inProgress"))),
    { kind: "hunt", questId: 4, monsterName: "Deer", harvest: true },
  );
});

test("collision navigation keeps a detour whose first step must regress", () => {
  const player = { x: 278, y: 609 };
  const target = { x: 110, y: 440 };
  assert.equal(collisionPathNeedsStickyDetour(player, target, [
    player,
    { x: 279, y: 609 },
    { x: 279, y: 600 },
    { x: 270, y: 590 },
  ]), true);
  assert.equal(collisionPathNeedsStickyDetour(player, target, [
    player,
    { x: 277, y: 608 },
    { x: 270, y: 600 },
  ]), false);
});

test("sticky dynamic detours freeze only after net destination progress", () => {
  const player = { x: 0, y: 0 };
  const target = { x: 10, y: 0 };
  assert.deepEqual(
    selectProgressingCollisionDetour([
      player,
      { x: -1, y: 0 },
      { x: -2, y: 0 },
      { x: 0, y: 1 },
      { x: 1, y: 1 },
      { x: 2, y: 1 },
    ], player, target, { preferredSteps: 2 }),
    { x: 1, y: 1 },
  );
  assert.equal(
    selectProgressingCollisionDetour([
      player,
      { x: -1, y: 0 },
      { x: -2, y: 0 },
    ], player, target, { preferredSteps: 2 }),
    null,
  );
});

test("collision navigation crosses a perpendicular frontier around a long wall", () => {
  const bounds = { minX: 308, maxX: 412, minY: 512, maxY: 613 };
  const player = { x: 352, y: 550 };
  const target = { x: 610, y: 405 };
  assert.equal(
    collisionPathNeedsPerpendicularFrontier(player, target, bounds, { x: 359, y: 574 }),
    true,
  );
  assert.equal(
    collisionPathNeedsPerpendicularFrontier(player, target, bounds, { x: 411, y: 520 }),
    false,
  );
  assert.equal(
    collisionPathNeedsPerpendicularFrontier(player, { x: 380, y: 540 }, bounds, { x: 359, y: 574 }),
    false,
  );
});

test("global collision path goes around a wall instead of oscillating at it", () => {
  const blocked = [];
  for (let y = 0; y <= 25; y += 1) blocked.push({ x: 10, y });
  const path = findCollisionGridPath({
    start: { x: 5, y: 10 },
    target: { x: 20, y: 10 },
    bounds: { minX: 0, maxX: 30, minY: 0, maxY: 30 },
    blocked,
  });
  assert.ok(path);
  assert.deepEqual(path[0], { x: 5, y: 10 });
  assert.deepEqual(path.at(-1), { x: 20, y: 10 });
  assert.ok(path.some((point) => point.y >= 26));
  assert.ok(!path.some((point) => point.x === 10 && point.y <= 25));
});

test("global collision search expands beyond a bounded false-unreachable corridor", async () => {
  const policy = await import("./policy.mjs");
  const margins = policy.collisionAtlasSearchMargins?.(192, {
    mapWidth: 700,
    mapHeight: 700,
  });
  assert.deepEqual(margins, [72, 240, 700]);
  assert.deepEqual(
    policy.collisionAtlasSearchMargins?.(192, {
      mapWidth: 3_000,
      mapHeight: 3_000,
    }),
    [72, 240, 384],
  );

  const blocked = [];
  for (let y = 300; y < 700; y += 1) blocked.push({ x: 400, y });
  const start = { x: 503, y: 635 };
  const target = { x: 311, y: 631 };
  const route = (margin) => findCollisionGridPath({
    start,
    target,
    bounds: {
      minX: Math.max(0, Math.min(start.x, target.x) - margin),
      maxX: Math.min(699, Math.max(start.x, target.x) + margin),
      minY: Math.max(0, Math.min(start.y, target.y) - margin),
      maxY: Math.min(699, Math.max(start.y, target.y) + margin),
    },
    blocked,
  });
  assert.equal(route(margins[1]), null);
  assert.ok(route(margins.at(-1)));
});

test("global collision path stops at pickup range and does not cut blocked corners", () => {
  const path = findCollisionGridPath({
    start: { x: 1, y: 1 },
    target: { x: 7, y: 7 },
    desiredDistance: 4,
    bounds: { minX: 0, maxX: 10, minY: 0, maxY: 10 },
    blocked: [{ x: 2, y: 1 }],
  });
  assert.ok(path);
  assert.ok(Math.max(Math.abs(path.at(-1).x - 7), Math.abs(path.at(-1).y - 7)) <= 4);
  assert.notDeepEqual(path[1], { x: 2, y: 2 });
});

test("long routing reacts only to dynamic occupancy on the immediate physical step", () => {
  const diagonalPath = [{ x: 10, y: 10 }, { x: 11, y: 9 }, { x: 12, y: 8 }];
  assert.equal(collisionPathHasImmediateDynamicBlock(diagonalPath, [{ x: 12, y: 8 }]), false);
  assert.equal(collisionPathHasImmediateDynamicBlock(diagonalPath, [{ x: 12, y: 8 }], 2), true);
  assert.equal(collisionPathHasImmediateDynamicBlock(diagonalPath, [{ x: 11, y: 9 }]), true);
  assert.equal(collisionPathHasImmediateDynamicBlock(diagonalPath, [{ x: 11, y: 10 }]), true);
  assert.equal(collisionPathHasImmediateDynamicBlock(diagonalPath, [{ x: 10, y: 9 }]), true);
});

test("global routing relaxes stale correction memory only after remembered routes close", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function collisionAtlasPathToward"),
    runner.indexOf("async function collisionAtlasCorridor"),
  );
  assert.match(
    body,
    /const staticPath = findCollisionGridPath\([\s\S]{0,500}blocked,[\s\S]{0,120}occupied: \[\]/,
  );
  assert.match(
    body,
    /if \(staticPath\) return staticPath;[\s\S]{0,900}if \(rejectedCells\.length > 0\)[\s\S]{0,350}const relaxedStaticPath = findCollisionGridPath\([\s\S]{0,350}blocked: staticBlocked[\s\S]{0,100}occupied: \[\]/,
  );
  assert.match(body, /collision atlas relaxed .*expired-candidate/);
});

test("travel policy identifies only an adjacent non-target hostile", () => {
  const state = {
    player: { x: 10, y: 10 },
    entities: [
      { objectId: "target", kind: "monster", disposition: "hostile", name: "CannibalPlant", x: 10, y: 11 },
      { objectId: "far", kind: "monster", disposition: "hostile", name: "ForestYeti", x: 13, y: 10 },
      { objectId: "npc", kind: "npc", disposition: "hostile", name: "Guard", x: 9, y: 10 },
      { objectId: "zero-hp", kind: "monster", disposition: "hostile", name: "ForestYeti", dead: false, hp: 0, x: 10, y: 9, attackUntil: 300 },
      { objectId: "threat", kind: "monster", disposition: "hostile", name: "ForestYeti", x: 9, y: 10, attackUntil: 200 },
    ],
  };
  assert.equal(nearestBlockingHostile(state, "CannibalPlant", new Map(), 100)?.objectId, "threat");
  assert.equal(
    nearestBlockingHostile(state, "CannibalPlant", new Map([["threat", 500]]), 100),
    null,
  );
  assert.equal(
    nearestBlockingHostile(
      state,
      "CannibalPlant",
      new Map(),
      100,
      (entity) => entity.objectId === "target",
    ),
    null,
  );
  assert.equal(
    nearestBlockingHostile(
      state,
      "CannibalPlant",
      new Map(),
      100,
      (entity) => entity.objectId === "threat",
    )?.objectId,
    "threat",
  );

  state.entities.push({
    objectId: "selected-clickable",
    kind: "monster",
    disposition: "hostile",
    name: "ForestYeti",
    x: 11,
    y: 10,
    attackUntil: 150,
  });
  assert.equal(
    nearestBlockingHostile(
      state,
      "CannibalPlant",
      new Map(),
      100,
      (entity) => ["threat", "selected-clickable"].includes(entity.objectId),
      "selected-clickable",
    )?.objectId,
    "selected-clickable",
    "an already selected physical hit target should clear before a newer invisible attacker",
  );
});

test("travel combat interruption requires a recent rendered monster attack", () => {
  assert.equal(entityAttackIsRecent({ attackStartedAt: 8_000 }, 10_000, 3_500), true);
  assert.equal(entityAttackIsRecent({ attackUntil: 7_000 }, 10_000, 3_500), true);
  assert.equal(entityAttackIsRecent({ attackUntil: 6_499 }, 10_000, 3_500), false);
  assert.equal(entityAttackIsRecent({}, 10_000, 3_500), false);
});

test("active harvest and recovery threats require rendered attack evidence", () => {
  const state = {
    player: { x: 10, y: 10 },
    entities: [
      { objectId: "idle", kind: "monster", disposition: "hostile", x: 9, y: 10 },
      { objectId: "corpse", kind: "monster", disposition: "hostile", dead: true, x: 10, y: 11, attackUntil: 10_000 },
      { objectId: "zero-hp", kind: "monster", disposition: "hostile", dead: false, hp: 0, x: 11, y: 10, attackUntil: 10_000 },
      { objectId: "attacker", kind: "monster", disposition: "hostile", x: 12, y: 10, attackUntil: 9_000 },
      { objectId: "far", kind: "monster", disposition: "hostile", x: 30, y: 10, attackUntil: 10_000 },
    ],
  };
  assert.equal(nearestActiveHostile(state, {
    excludeObjectId: "corpse",
    maxDistance: 8,
    now: 10_000,
    withinMs: 3_500,
  })?.objectId, "attacker");
  assert.deepEqual(retreatPointFromHostile(state, state.entities[3], 8), { x: 2, y: 10 });
});

test("incidental travel combat is limited to monsters with a real level disadvantage", () => {
  assert.equal(incidentalTravelThreatIsTrivial(10, 13), true);
  assert.equal(incidentalTravelThreatIsTrivial(11, 13), false);
  assert.equal(incidentalTravelThreatIsTrivial(13, 13), false);
  assert.equal(incidentalTravelThreatIsTrivial(15, 13), false);
  assert.equal(incidentalTravelThreatIsTrivial(undefined, 13), false);
});

test("a currently attacking requested monster overrides stale approach cooldown", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /Number\(monsterCooldownUntil\.get\(String\(entry\.objectId\)\) \?\? 0\) <= now \|\|\s+entityAttackIsRecent\(entry, now, ACTIVE_TRAVEL_THREAT_WINDOW_MS\)/,
  );
  assert.match(
    runner,
    /Number\(quarantinedMonsterUntil\.get\(String\(entry\.objectId\)\) \?\? 0\) <= now &&/,
  );
  assert.match(
    runner,
    /quarantinedMonsterUntil\.set\(objectId, quarantineUntil\)[\s\S]{0,120}monsterCooldownUntil\.set\(objectId, quarantineUntil\)/,
  );
});

test("melee combat selects an isolated pack-edge target before the closest cluster", () => {
  const state = {
    player: { x: 160, y: 526 },
    entities: [
      { objectId: "edge", kind: "monster", disposition: "hostile", x: 142, y: 527 },
      { objectId: "near-a", kind: "monster", disposition: "hostile", x: 147, y: 527 },
      { objectId: "near-b", kind: "monster", disposition: "hostile", x: 148, y: 526 },
      { objectId: "corpse", kind: "monster", disposition: "hostile", dead: true, x: 142, y: 526 },
      { objectId: "npc", kind: "npc", disposition: "hostile", x: 142, y: 528 },
    ],
  };
  assert.deepEqual(
    rankCombatTargetsByIsolation(state, state.entities.slice(0, 3))
      .map((entry) => entry.objectId),
    ["edge", "near-b", "near-a"],
  );
});

test("melee acquisition approaches a safer edge unless an adjacent target is attacking", () => {
  const state = {
    player: { x: 10, y: 10 },
    entities: [
      { objectId: "adjacent-a", kind: "monster", disposition: "hostile", x: 11, y: 10 },
      { objectId: "adjacent-b", kind: "monster", disposition: "hostile", x: 11, y: 11 },
      { objectId: "edge", kind: "monster", disposition: "hostile", x: 18, y: 10 },
    ],
  };
  assert.equal(chooseImmediateMeleeTarget(state, state.entities, { now: 10_000 }), null);
  state.entities[0].attackUntil = 9_000;
  assert.equal(
    chooseImmediateMeleeTarget(state, state.entities, { now: 10_000 })?.objectId,
    "adjacent-a",
  );
});

test("a near-death potion-exhausting win triggers adaptive combat preparation", () => {
  const before = {
    playerHp: 111,
    playerMaxHp: 111,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 6 }],
  };
  const after = {
    playerHp: 8,
    playerMaxHp: 111,
    beltItems: [],
  };
  assert.deepEqual(assessQuestCombatResourceStrain(before, after), {
    severe: true,
    depleted: true,
    criticalHealth: true,
    excessivePotionUse: true,
    potionsBefore: 6,
    potionsAfter: 0,
    potionsUsed: 6,
    hp: 8,
    maxHp: 111,
    healthRatio: 8 / 111,
  });
  assert.equal(assessQuestCombatResourceStrain(before, {
    ...after,
    playerHp: 90,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 5 }],
  }).severe, false);
});

test("a grind source cools down only after repeated failed goals without authoritative EXP", () => {
  const goal = { kind: "grind", monsterName: "RakingCat" };
  const before = { playerLevel: 14, playerExperience: 861 };
  assert.deepEqual(
    assessGrindingSourceStall(goal, before, {
      playerLevel: 14,
      playerExperience: 953,
    }, {
      failed: true,
      previousStalls: 2,
      now: 1_000,
      cooldownMs: 5_000,
    }),
    {
      tracked: true,
      progressed: true,
      stallCount: 0,
      cooldownUntil: null,
    },
  );
  assert.deepEqual(
    assessGrindingSourceStall(goal, before, before, {
      failed: true,
      previousStalls: 2,
      now: 1_000,
      cooldownMs: 5_000,
    }),
    {
      tracked: true,
      progressed: false,
      stallCount: 3,
      cooldownUntil: 6_000,
    },
  );
  assert.equal(
    assessGrindingSourceStall(goal, before, before, {
      failed: false,
      previousStalls: 2,
    }).cooldownUntil,
    null,
  );
});

test("a critically depleted shelter escape keeps moving until arrival or normal death recovery", () => {
  const base = {
    playerHp: 90,
    playerMaxHp: 135,
    beltItems: [{ name: "(HP)DrugSmall", quantity: 5 }],
    inventoryItems: [],
  };
  assert.equal(shouldEnforceShelterEscapeResourceBudget(base), true);
  assert.equal(shouldEnforceShelterEscapeResourceBudget({
    ...base,
    playerHp: 9,
    beltItems: [],
  }), false);
  assert.equal(shouldEnforceShelterEscapeResourceBudget({
    ...base,
    playerHp: 90,
    beltItems: [],
  }), false);
  assert.equal(shouldEnforceShelterEscapeResourceBudget({
    ...base,
    playerHp: 26,
  }), false);
});

test("a later confirmed kill resolves only older combat resource strain", () => {
  const strains = [
    { monsterName: "CannibalPlant", at: 100, consecutiveStrains: 1 },
    { monsterName: "CannibalPlant", at: 300, consecutiveStrains: 2 },
    { monsterName: "RakingCat", at: 100, consecutiveStrains: 1 },
  ];
  assert.deepEqual(
    unresolvedCombatResourceStrains(strains, [
      { monsterName: "CannibalPlant", at: 200 },
    ]),
    [strains[1], strains[2]],
  );
  assert.deepEqual(
    unresolvedCombatResourceStrains(strains, [
      { monsterName: "CannibalPlant", at: 400 },
      { monsterName: "RakingCat", at: 150 },
    ]),
    [],
  );
});

test("an unresolved severe combat strain restores the one-time supply recall", () => {
  const strains = [
    { monsterName: "SpittingSpider", severe: true, at: 300 },
  ];
  assert.equal(combatMemoryRequiresSupplyRecall(strains, []), true);
  assert.equal(
    combatMemoryRequiresSupplyRecall(strains, [], {
      currentPotionQuantity: 10,
      requiredPotionQuantity: 10,
    }),
    false,
  );
  assert.equal(
    combatMemoryRequiresSupplyRecall(strains, [], {
      currentPotionQuantity: 9,
      requiredPotionQuantity: 10,
    }),
    true,
  );
  assert.equal(
    combatMemoryRequiresSupplyRecall(strains, [
      { monsterName: "SpittingSpider", at: 400 },
    ]),
    false,
  );
  assert.equal(
    combatMemoryRequiresSupplyRecall([
      { monsterName: "SpittingSpider", severe: false, at: 500 },
    ], []),
    false,
  );
  // Legacy strain rows predate the explicit severe flag but were emitted only
  // after the severe-strain predicate had already passed.
  assert.equal(
    combatMemoryRequiresSupplyRecall([
      { monsterName: "CannibalPlant", at: 600 },
    ], []),
    true,
  );
});

test("offensive combat skills follow the visible F1-F8 bar without casting passive or ground skills", () => {
  const skills = [
    { key: "fencing", name: "Fencing", castKind: "passive", offensive: false, hotkey: 1 },
    { key: "fire-wall", name: "FireWall", spell: "FireWall", castKind: "ground", offensive: true, hotkey: 2 },
    { key: "fire-ball", name: "FireBall", spell: "FireBall", castKind: "target", offensive: true, hotkey: 3, cooldownRemainingTicks: 0 },
  ];
  assert.deepEqual(offensiveCombatSkillHotkey(skills), { slot: 3, skill: skills[2] });
  skills[2].cooldownRemainingTicks = 1;
  assert.equal(offensiveCombatSkillHotkey(skills), null);
  const unbound = { key: "spirit-sword", name: "SpiritSword", offensive: true };
  assert.deepEqual(offensiveCombatSkillHotkey([unbound]), { slot: 1, skill: unbound });
});

test("restorative self skills select only ready Crystal Healing from the visible F1-F8 bar", () => {
  const skills = [
    { key: "fire-ball", name: "FireBall", spell: "FireBall", castKind: "target", offensive: true, hotkey: 1 },
    { key: "mass-healing", name: "MassHealing", spell: "MassHealing", castKind: "ground", offensive: false, hotkey: 2 },
    { key: "healing", name: "Healing", spell: "Healing", castKind: "target", offensive: false, hotkey: 3, cooldownRemainingTicks: 0 },
  ];
  assert.deepEqual(restorativeSelfSkillHotkey(skills), { slot: 3, skill: skills[2] });
  skills[2].cooldownRemainingTicks = 1;
  assert.equal(restorativeSelfSkillHotkey(skills), null);
});

test("combat skill execution uses a physical F-key and never emits a direct magic command", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const helper = runner.slice(
    runner.indexOf("async function useOffensiveCombatSkillIfReady("),
    runner.indexOf("async function killMonster("),
  );
  const kill = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  assert.match(
    helper,
    /offensiveCombatSkillHotkey\(state\.knownSkills\)[\s\S]{0,700}client\.pressKey\([\s\S]{0,200}`F\$\{selected\.slot\}`/,
  );
  assert.match(helper, /action: "cast-offensive-combat-skill"/);
  assert.doesNotMatch(helper, /client\.send|WebSocket|\.send\(/);
  assert.match(kill, /if \(live\) await useOffensiveCombatSkillIfReady\(state, live\)/);
});

test("Taoist self-healing uses a physical F-key and the normal client targets self", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  const helper = runner.slice(
    runner.indexOf("async function useRestorativeSelfSkillIfNeeded("),
    runner.indexOf("async function killMonster("),
  );
  const castSkill = page.slice(
    page.indexOf("function castSkill(skillKey: string)"),
    page.indexOf("function transferMap(key: string)"),
  );
  assert.match(
    helper,
    /restorativeSelfSkillHotkey\(state\.knownSkills\)[\s\S]{0,900}client\.pressKey\([\s\S]{0,200}`F\$\{selected\.slot\}`/,
  );
  assert.match(helper, /action: "cast-restorative-self-skill"/);
  assert.doesNotMatch(helper, /client\.send|WebSocket|\.send\(/);
  assert.match(helper, /entry\.type\) === "magic"[\s\S]{0,220}entry\.targetId/);
  assert.match(
    castSkill,
    /skill\.offensive[\s\S]{0,180}selectedTarget\?\.kind === "player"[\s\S]{0,180}: self/,
  );
});

test("progression skill books are picked up and learned only through visible inventory input", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const pickup = runner.slice(
    runner.indexOf("async function collectVisibleProgressionSkillBookIfNeeded("),
    runner.indexOf("async function collectVisibleSafeSupplyLootIfNeeded("),
  );
  const learning = runner.slice(
    runner.indexOf("async function learnProgressionSkillIfReady("),
    runner.indexOf("async function equipOnboardingGearIfReady("),
  );
  assert.match(
    pickup,
    /progressionSkillBookCatalog\.filter[\s\S]{0,400}playerLevel[\s\S]{0,700}nearestGroundDropByName[\s\S]{0,220}\b8\b/,
  );
  assert.match(pickup, /client\.clickSelector\([\s\S]{0,180}pick-up-progression-skill-book/);
  assert.match(pickup, /client\.pressKey\(" ", "Space", 32/);
  assert.doesNotMatch(pickup, /client\.send|WebSocket|\.send\(/);
  assert.match(
    learning,
    /progressionSkillBookCatalog[\s\S]{0,900}button\.inventory-item-card[\s\S]{0,180}learn-progression-skill/,
  );
});

test("long grind evidence keeps level changes and bounded visual checkpoints", () => {
  const grind = { kind: "grind" };
  const level13 = { playerLevel: 13 };
  assert.equal(shouldCaptureGoalFrame({ kind: "hunt" }, level13, level13, 37), true);
  assert.equal(shouldCaptureGoalFrame(grind, level13, level13, 1), true);
  assert.equal(shouldCaptureGoalFrame(grind, level13, level13, 99), false);
  assert.equal(shouldCaptureGoalFrame(grind, level13, level13, 100), true);
  assert.equal(shouldCaptureGoalFrame(grind, level13, { playerLevel: 14 }, 37), true);
});

test("repetitive grind goal records use compact state while semantic goals retain full evidence", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(runner, /before: compactGoalState\(before, goal\)/);
  assert.match(runner, /goalRecord\.after = compactGoalState\(after, goal\)/);
  assert.match(
    runner,
    /shouldCaptureGoalFrame\([\s\S]{0,240}GRIND_SCREENSHOT_SAMPLE_INTERVAL[\s\S]{0,260}captureEvidenceFrame/,
  );
  const compact = runner.slice(
    runner.indexOf("function compactGoalState("),
    runner.indexOf("function recordMilestone("),
  );
  assert.match(compact, /goal\?\.kind !== "grind"[\s\S]{0,80}compactState\(state\)/);
  assert.doesNotMatch(compact, /questLog|nearbyEntities|groundDrops|logs:/);
});

test("read-only agent snapshots preserve rendered attack evidence", async () => {
  let evaluation = "";
  await readAgentState({
    evaluate: async (expression) => {
      evaluation = expression;
      return {};
    },
  });
  assert.match(evaluation, /attackStartedAt: entry\?\.attackStartedAt \?\? null/);
  assert.match(evaluation, /attackUntil: entry\?\.attackUntil \?\? null/);
  assert.match(evaluation, /const visiblePlayer = validPosition\(self\)[\s\S]{0,120}\? self/);
  assert.match(evaluation, /const authoritativePlayer = validPosition\(state\.authoritativePlayer\)/);
  assert.match(
    evaluation,
    /player: authoritativePlayer[\s\S]{0,180}x: authoritativePlayer\.x, y: authoritativePlayer\.y/,
  );
  assert.match(evaluation, /renderedPlayer: visiblePlayer/);
  assert.match(evaluation, /castKind: skill\?\.castKind \?\? null/);
  assert.match(evaluation, /offensive: skill\?\.offensive === true/);
  assert.match(evaluation, /cooldownRemainingTicks: skill\?\.cooldownRemainingTicks \?\? 0/);
});

test("websocket event evidence preserves ordering for authoritative bootstrap", () => {
  const client = {
    wsReceived: [
      {
        at: 99,
        url: "ws://127.0.0.1:7310/ws",
        payloadData: JSON.stringify({ type: "worldSnapshot", payload: { stale: true } }),
      },
      {
        at: 101,
        url: "ws://127.0.0.1:7310/ws",
        payloadData: JSON.stringify({ type: "packet", packet: "UserInformation", payload: {} }),
      },
      {
        at: 102,
        url: "ws://127.0.0.1:7310/ws",
        payloadData: JSON.stringify({ type: "worldSnapshot", payload: { authoritative: true } }),
      },
      {
        at: 103,
        url: "ws://127.0.0.1:7310/metrics",
        payloadData: JSON.stringify({ type: "worldSnapshot", payload: { unrelated: true } }),
      },
    ],
  };
  assert.deepEqual(wsEventFramesSince(client, 100, "worldSnapshot"), [
    {
      at: 102,
      event: { type: "worldSnapshot", payload: { authoritative: true } },
    },
  ]);
});

test("autonomous planning waits for the post-UserInformation personal snapshot", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const waitIndex = runner.indexOf("await waitForAuthoritativePersonalBootstrap()");
  const policyIndex = runner.indexOf("async function runQuestPolicy()");
  assert.ok(waitIndex >= 0 && waitIndex < policyIndex);
  assert.match(
    runner,
    /latestUserInformation\.at,[\s\S]{0,80}"worldSnapshot"[\s\S]{0,500}Array\.isArray\(snapshot\?\.beltItems\)/,
  );
  assert.match(runner, /StartGame did not deliver an authoritative world snapshot after UserInformation/);
});

test("read-only agent snapshots reject a transient post-revive 0,0 sentinel", async () => {
  let calls = 0;
  const state = await readAgentState({
    evaluate: async () => {
      calls += 1;
      return calls === 1
        ? {
            screen: "game",
            sceneInteractionReady: true,
            wsState: "open",
            player: null,
            playerDead: false,
          }
        : {
            screen: "game",
            sceneInteractionReady: true,
            wsState: "open",
            player: { x: 288, y: 616 },
            playerDead: false,
          };
    },
  });
  assert.equal(calls, 2);
  assert.deepEqual(state.player, { x: 288, y: 616 });
});

test("route halos avoid dangerous monsters without walling off ordinary spawn fields", () => {
  const state = {
    playerLevel: 10,
    entities: [
      { kind: "monster", disposition: "hostile", name: "CannibalPlant", x: 20, y: 20 },
      { kind: "monster", disposition: "hostile", name: "WoomaGuardian", x: 30, y: 30 },
      { kind: "monster", disposition: "hostile", name: "UnknownBoss", x: 40, y: 40 },
      { kind: "npc", disposition: "hostile", name: "Guard", x: 50, y: 50 },
    ],
  };
  const cells = dangerousHostileAvoidanceCells(state, [
    { monsterName: "CannibalPlant", level: 6 },
    { monsterName: "WoomaGuardian", level: 18 },
  ]);
  const keys = new Set(cells.map((cell) => `${cell.x},${cell.y}`));
  assert.equal(keys.has("21,20"), false);
  assert.equal(keys.has("32,30"), true);
  assert.equal(keys.has("41,40"), true);
  assert.equal(keys.has("42,40"), false);
  assert.equal(keys.has("51,50"), false);

  const certifiedCells = dangerousHostileAvoidanceCells(
    state,
    [{ monsterName: "WoomaGuardian", level: 18 }],
    { safeMonsterNames: ["wooma guardian"] },
  );
  const certifiedKeys = new Set(certifiedCells.map((cell) => `${cell.x},${cell.y}`));
  assert.equal(certifiedKeys.has("32,30"), false);
  assert.equal(certifiedKeys.has("41,40"), true);
});

test("dangerous respawn travel balances distance against field density", () => {
  const fields = [
    { x: 130, y: 510, count: 40, spread: 70 },
    { x: 475, y: 500, count: 40, spread: 90 },
    { x: 610, y: 405, count: 20, spread: 60 },
  ];
  assert.deepEqual(
    rankRespawnFieldsForTravel({ x: 288, y: 609 }, fields).map((field) => [field.x, field.y]),
    [[475, 500], [130, 510], [610, 405]],
  );
  assert.equal(rankRespawnFieldsForTravel({ x: 278, y: 606 }, fields)[0], fields[1]);
});

test("large respawn travel approaches the nearest interior edge", () => {
  assert.deepEqual(
    nearestRespawnApproachPoint(
      { x: 288, y: 616 },
      { x: 130, y: 510, count: 40, spread: 70 },
    ),
    { x: 190, y: 550 },
  );
  assert.deepEqual(
    expandRespawnPatrolFields(
      [{ mapFileName: "0", x: 130, y: 510, count: 40, spread: 70 }],
      { player: { x: 288, y: 616 } },
    ).slice(0, 2).map(({ x, y }) => [x, y]),
    [[190, 550], [130, 510]],
  );
});

test("long respawn travel budget permits real building detours", () => {
  assert.equal(respawnTravelAttemptBudget(30), 15);
  assert.equal(respawnTravelAttemptBudget(122), 366);
  assert.equal(respawnTravelAttemptBudget(1_000), 480);
});

test("respawn travel avoids crossing a dense aggressive source field", () => {
  const player = { x: 294, y: 577 };
  const east = { x: 475, y: 500, count: 40, spread: 90 };
  const west = { x: 130, y: 510, count: 40, spread: 70 };
  const catField = { x: 340, y: 550, count: 80, spread: 50 };
  assert.ok(
    respawnCorridorExposure(player, east, [catField]) >
      respawnCorridorExposure(player, west, [catField]),
  );
  assert.equal(rankRespawnFieldsForTravel(player, [east, west])[0], east);
  assert.equal(
    rankRespawnFieldsForTravel(player, [east, west], { hazards: [catField] })[0],
    west,
  );
});

test("respawn travel inserts a safer orthogonal waypoint around a hostile band", () => {
  const player = { x: 288, y: 609 };
  const target = { x: 399, y: 545 };
  const hazards = [
    { x: 340, y: 550, count: 40, spread: 50 },
    { x: 340, y: 550, count: 40, spread: 50 },
  ];
  const waypoint = respawnCorridorAvoidanceWaypoint(player, target, hazards);
  assert.deepEqual(
    { x: waypoint?.x, y: waypoint?.y },
    { x: 399, y: 609 },
  );
  assert.ok(Number(waypoint?.detourExposure) < Number(waypoint?.directExposure));
});

test("long NPC travel can opt into progressive lateral waypoints around a hostile band", () => {
  const player = { x: 290, y: 604 };
  const target = { x: 324, y: 291 };
  const hazards = [
    { x: 305, y: 480, count: 80, spread: 75 },
    { x: 300, y: 540, count: 60, spread: 45 },
  ];
  const waypoint = respawnCorridorAvoidanceWaypoint(player, target, hazards, {
    minimumImprovementRatio: 0.9,
    minimumLegDistance: 24,
    perpendicularOffsets: [24, 40, 64, 96, 128],
    progressRatios: [0.33, 0.5, 0.67],
  });
  assert.ok(waypoint);
  assert.ok(Number(waypoint.detourExposure) < Number(waypoint.directExposure));
  assert.ok(Math.max(
    Math.abs(Number(waypoint.x) - target.x),
    Math.abs(Number(waypoint.y) - target.y),
  ) < Math.max(Math.abs(player.x - target.x), Math.abs(player.y - target.y)));
  assert.notEqual(Number(waypoint.x), target.x);
  assert.notEqual(Number(waypoint.x), player.x);
  const alternate = respawnCorridorAvoidanceWaypoint(player, target, hazards, {
    minimumImprovementRatio: 0.9,
    minimumLegDistance: 24,
    perpendicularOffsets: [24, 40, 64, 96, 128],
    progressRatios: [0.33, 0.5, 0.67],
    candidateIndex: 1,
  });
  assert.ok(alternate);
  assert.notDeepEqual(
    { x: alternate.x, y: alternate.y },
    { x: waypoint.x, y: waypoint.y },
  );
});

test("long NPC travel applies source-backed hostile corridor waypoints before direct walking", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /coordinateDistance >= 80[\s\S]{0,300}aggressiveRespawnTravelHazards\(state\)[\s\S]{0,500}perpendicularOffsets: \[24, 40, 64, 96, 128\]/,
  );
  assert.match(
    runner,
    /NPC hostile-corridor detour:[\s\S]{0,500}navigateNear\(corridorWaypoint, 2,[\s\S]{0,220}respawnTravelAttemptBudget\(detourDistance\)/,
  );
});

test("ordinary map transfers use progressive hostile detours and a cumulative travel budget", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const travelBody = runner.slice(
    runner.indexOf("async function travelToMap("),
    runner.indexOf("async function ensureVisibleScriptTravelFunding"),
  );
  assert.match(
    travelBody,
    /for \(let detour = 0; detour < 3 && distance >= 80; detour \+= 1\)[\s\S]{0,500}aggressiveRespawnTravelHazards[\s\S]{0,1600}respawnCorridorAvoidanceWaypoint/,
  );
  assert.match(travelBody, /map hostile-corridor detour:/);
  assert.match(
    travelBody,
    /candidateIndex < 32[\s\S]{0,1400}navigateNear\(corridorWaypoint, 2,[\s\S]{0,500}resourceBaseline: journeyResourceBaseline,[\s\S]{0,120}resourceAccountingGoal: journeyResourceGoal/,
  );
  assert.match(travelBody, /reject unreachable hostile-corridor waypoint:/);
  assert.match(
    runner,
    /goal\.kind === "travel"[\s\S]{0,900}travel resource risk:[\s\S]{0,300}returning to visible supply/,
  );
});

test("an optional map hazard waypoint cannot reject an otherwise valid transfer", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const travelBody = runner.slice(
    runner.indexOf("async function travelToMap("),
    runner.indexOf("async function ensureVisibleScriptTravelFunding"),
  );
  assert.match(
    travelBody,
    /navigateNear\(corridorWaypoint, 2,[\s\S]{0,900}catch \(error\) \{[\s\S]{0,700}!isRetryableVisibleTransferNavigationError\(error\)[\s\S]{0,350}reject unreachable hostile-corridor waypoint/,
  );
  assert.match(
    travelBody,
    /if \(!reachedSafeWaypoint\)[\s\S]{0,300}retaining direct physical route[\s\S]{0,500}if \(!reachedDuringDetour\)[\s\S]{0,220}navigateNear\(target, 0/,
  );
});

test("an unreachable optional hazard elbow falls back to the direct respawn field", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /navigationError instanceof NavigationUnreachableError &&\s*corridorWaypoint[\s\S]{0,900}corridorWaypoint = null;\s*continue;[\s\S]{0,180}if \(navigationError instanceof NavigationUnreachableError\)/,
  );
});

test("respawn travel avoids a target region overlapped by dense hostiles", () => {
  const player = { x: 0, y: 0 };
  const near = { x: 10, y: 0, count: 10, spread: 10 };
  const safer = { x: 30, y: 0, count: 10, spread: 10 };
  const hostilePack = { x: 10, y: 0, count: 50, spread: 10 };
  assert.ok(
    respawnTerminalExposure(near, [hostilePack]) >
      respawnTerminalExposure(safer, [hostilePack]),
  );
  assert.equal(
    rankRespawnFieldsForTravel(player, [near, safer], {
      hazards: [hostilePack],
      exposureWeight: 0,
    })[0],
    safer,
  );
});

test("terminal overlap cannot force an arbitrarily long respawn detour", () => {
  const player = { x: 0, y: 0 };
  const overlappedNear = { x: 10, y: 0, count: 10, spread: 10 };
  const distant = { x: 500, y: 0, count: 10, spread: 10 };
  const duplicatedSourceTables = Array.from(
    { length: 20 },
    () => ({ x: 10, y: 0, count: 100, spread: 10 }),
  );
  assert.equal(
    rankRespawnFieldsForTravel(player, [overlappedNear, distant], {
      hazards: duplicatedSourceTables,
      exposureWeight: 0,
      terminalExposureWeight: 1_000_000,
      terminalExposureCap: 100,
    })[0],
    overlappedNear,
  );
});

test("repeated hunting prefers lower terminal overlap over a slightly shorter field", () => {
  const player = { x: 288, y: 609 };
  const east = { x: 475, y: 500, count: 40, spread: 90 };
  const west = { x: 130, y: 510, count: 40, spread: 70 };
  const hazards = [
    { x: 190, y: 547, count: 50, spread: 55 },
  ];
  assert.equal(
    rankRespawnFieldsForTravel(player, [west, east], { hazards })[0],
    east,
  );
});

test("supply liquidation selects only gear superseded by a later equipped quest reward", () => {
  const candidates = [
    { questId: 0, name: "WoodenSword" },
    { questId: 3, name: "SharpDagger" },
    { questId: 6, name: "BronzeWarriorSword" },
    { questId: 23, name: "BronzeShortSword" },
    { questId: 22, name: "PrecisionPendant" },
    { questId: 2, name: "GoldenPendant" },
  ];
  const state = {
    equipmentItems: [
      { slot: "weapon", name: "BronzeShortSword" },
      { slot: "necklace", name: "PrecisionPendant" },
    ],
    inventoryItems: [
      { name: "SharpDagger", container: "bag1", equipSlot: "weapon", sellValue: 500 },
      { name: "BronzeWarriorSword", container: "bag1", equipSlot: "weapon", sellValue: 550 },
      { name: "WoodenSword", container: "bag1", equipSlot: "weapon", sellValue: 25 },
      { name: "GoldenPendant", container: "bag1", equipSlot: "necklace", sellValue: 400 },
      { name: "UnknownRelic", container: "bag1", equipSlot: "weapon", sellValue: 9999 },
      { name: "CannibalLeaf", container: "bag1", sellValue: 20 },
    ],
  };
  assert.deepEqual(
    supersededProgressionGearForSale(state, candidates).map((item) => item.name),
    ["BronzeWarriorSword", "SharpDagger", "GoldenPendant", "WoodenSword"],
  );
});

test("duplicate loot is sellable only while an allow-listed same-name copy stays equipped", () => {
  const state = {
    equipmentItems: [{ slot: "ringLeft", name: "CopperRing" }],
    inventoryItems: [
      { uniqueId: "extra-ring", name: "CopperRing", container: "bag1", sellValue: 250 },
      { uniqueId: "extra-necklace", name: "GoldenPendant", container: "bag1", sellValue: 20 },
    ],
  };
  assert.deepEqual(
    duplicateEquippedItemsForSale(state, ["CopperRing"]).map((item) => item.uniqueId),
    ["extra-ring"],
  );
  assert.deepEqual(duplicateEquippedItemsForSale({
    ...state,
    equipmentItems: [],
  }, ["CopperRing"]), []);
});

test("ordinary supply loot retains its authoritative merchant proof", () => {
  const state = {
    inventoryItems: [
      { name: "HexagonalRing", uniqueId: 7, container: "bag1", sellValue: 250 },
      { name: "UnknownRelic", uniqueId: 8, container: "bag1", sellValue: 9999 },
    ],
  };
  assert.deepEqual(
    ordinarySupplyLootForSale(state, [{ name: "HexagonalRing", merchantKey: "ring" }])
      .map(({ name, uniqueId, liquidationMerchantKey }) => ({
        name, uniqueId, liquidationMerchantKey,
      })),
    [{ name: "HexagonalRing", uniqueId: 7, liquidationMerchantKey: "ring" }],
  );
});

test("supply liquidation selects only allow-listed bag material backed by quest-container progress", () => {
  const state = {
    questLog: [{
      questId: 25,
      stage: "inProgress",
      objectives: [{ label: "Collect CannibalLeaf", current: 6, required: 10 }],
    }],
    inventoryItems: [
      { name: "CannibalLeaf", uniqueId: 6, container: "bag1", quantity: 1, sellValue: 50 },
      { name: "CannibalLeaf", uniqueId: 12, container: "quest", quantity: 6, sellValue: 50 },
      { name: "CannibalStem", uniqueId: 7, container: "bag1", quantity: 1, sellValue: 50 },
      { name: "UnknownRelic", uniqueId: 8, container: "bag1", quantity: 1, sellValue: 9999 },
    ],
  };
  assert.deepEqual(
    surplusQuestMaterialsForSale(state, ["CannibalLeaf"]).map((item) => item.uniqueId),
    [6],
  );
  state.inventoryItems = state.inventoryItems.filter((item) => item.container !== "quest");
  assert.deepEqual(surplusQuestMaterialsForSale(state, ["CannibalLeaf"]), []);
});

test("supply liquidation routes each proven slot to a source-compatible visible merchant", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(runner, /equipSlot: "weapon"[\s\S]{0,180}npcs\.blacksmith/);
  assert.match(
    runner,
    /equipSlot: "necklace"[\s\S]{0,180}mapFileName: "0141"[\s\S]{0,320}npcIndex: 449/,
  );
  assert.match(
    runner,
    /state = await travelToMap\(merchantRoute\.mapFileName, \{[\s\S]{0,180}resourceBaseline: liquidationResourceBaseline,[\s\S]{0,180}resourceAccountingGoal: liquidationResourceGoal/,
  );
  assert.match(runner, /await travelToMap\(supplyHomeMapFileName, \{/);
  assert.match(
    runner,
    /const transferCandidates = liveTransfers\.length > 0[\s\S]{0,120}liveTransfers[\s\S]{0,120}sourcePortals\.map/,
  );
  assert.match(runner, /source-map-move:/);
  assert.match(runner, /dedicated grace period/);
  assert.match(
    runner,
    /portalProbe\.keys\.length === 1[\s\S]{0,1800}enter-visible-map-transfer-diagonal-approach[\s\S]{0,1000}if \(componentMoved\)[\s\S]{0,200}continue;/,
  );
  assert.doesNotMatch(
    runner,
    /else \{\s*await client\.pressKeyChord\(portalProbe\.keys, transferInput\)/,
  );
  assert.match(
    runner,
    /resumingInsideLiquidationMerchant[\s\S]{0,620}travelToMap\(supplyHomeMapFileName, \{[\s\S]{0,120}enforceCombatResourceBudget: false/,
  );
  assert.match(
    runner,
    /itemNames: Object\.freeze\(\["CannibalLeaf"\]\)[\s\S]{0,300}materialDealerReece[\s\S]{0,100}dialogTarget: "@Sell"/,
  );
});

test("map changes clear stale visible transfer cells before the destination snapshot", async () => {
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  assert.match(page, /mapTransfers: mapChanged \? \[\] : current\.mapTransfers/);
});

test("death recovery releases an unacknowledged revive request and retries visible input", async () => {
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(page, /setTimeout\(\(\) => setReviveRequested\(false\), 8_000\)/);
  assert.match(runner, /attempt < 3[\s\S]{0,1200}button\.disabled[\s\S]{0,1200}revive-in-town/);
  assert.match(
    runner,
    /reviveLocation = wsPacketsSince\([\s\S]{0,160}reviveRequestedAt,[\s\S]{0,80}"UserLocation"/,
  );
  assert.match(
    runner,
    /const revivedPacket = wsPacketsSince\([\s\S]{0,160}reviveRequestedAt,[\s\S]{0,80}"Revived"[\s\S]{0,160}reviveLocation && revivedPacket/,
  );
  assert.match(runner, /if \(!reviveLocation[\s\S]{0,1200}renderedTownLocationSettled/);
  assert.match(runner, /lacked authoritative ` \+[\s\S]{0,100}UserLocation\/Revived evidence/);
});

test("equipment policy never downgrades an occupied progression slot", () => {
  const state = {
    inventoryItems: [{ name: "WoodenSword" }],
    equipmentItems: [{ slot: "weapon", name: "BronzeWarriorSword" }],
  };
  const candidates = [
    { minLevel: 5, name: "BronzeWarriorSword" },
    { minLevel: 2, name: "WoodenSword" },
  ];
  assert.equal(selectBestAvailableEquipmentUpgrade(state, candidates, 7), null);
  assert.deepEqual(
    missingStarterEquipment(state, [{ name: "WoodenSword", slot: "weapon" }]),
    [],
  );
});

test("equipment policy can advance another slot after the strongest weapon is already equipped", () => {
  const state = {
    inventoryItems: [
      { name: "BronzeShortSword", equipSlot: "weapon" },
      { name: "SteelBangle", equipSlot: "braceletLeft" },
      { name: "WornIronBracelet", equipSlot: "braceletLeft" },
    ],
    equipmentItems: [{ slot: "weapon", name: "BronzeShortSword" }],
  };
  const candidates = [
    { questId: 23, minLevel: 7, name: "BronzeShortSword" },
    { questId: 25, minLevel: 7, name: "SteelBangle" },
    { questId: 5, minLevel: 1, name: "WornIronBracelet" },
  ];
  assert.deepEqual(selectBestAvailableEquipmentUpgrade(state, candidates, 7), {
    questId: 25,
    minLevel: 7,
    name: "SteelBangle",
    slot: "braceletLeft",
  });
});

test("equipment repair policy selects only low-durability equipped slots", () => {
  const state = {
    equipmentItems: [
      { slot: "weapon", name: "Sword", durabilityCurrent: 20, durabilityMax: 100 },
      { slot: "armour", name: "Dress", durabilityCurrent: 0, durabilityMax: 100 },
      { slot: "necklace", name: "Pendant", durabilityCurrent: 30, durabilityMax: 100 },
      { slot: "boots", name: "Loafer", durabilityCurrent: null, durabilityMax: null },
    ],
  };
  assert.deepEqual(
    equipmentRepairCandidates(state).map((item) => item.slot),
    ["armour", "weapon"],
  );
  assert.deepEqual(
    equipmentRepairCandidates(state, { slots: ["weapon"] }).map((item) => item.slot),
    ["weapon"],
  );
});

test("equipment repair walks to a visible merchant and clicks the normal repair UI", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const helper = runner.slice(
    runner.indexOf("async function repairProgressionEquipmentIfNeeded("),
    runner.indexOf("async function usePotionIfNeeded("),
  );
  const driver = await fs.readFile(new URL("browser-driver.mjs", import.meta.url), "utf8");
  assert.match(helper, /equipmentRepairCandidates\([\s\S]{0,100}EQUIPMENT_REPAIR_THRESHOLD_RATIO/);
  assert.match(
    helper,
    /const urgent = candidates\.some\(\(item\) => String\(item\.slot\) === "weapon"\)/,
  );
  assert.match(helper, /travelToMap\([\s\S]{0,350}openNpcDialog\([^)]*, "@Repair"/);
  assert.match(helper, /clickDialogTarget\("@Repair"/);
  assert.match(helper, /client\.clickSelector\(row,[\s\S]{0,350}client\.clickSelector\("\.npc-shop-confirm"/);
  assert.match(helper, /durabilityCurrent[\s\S]*recordMilestone\("equipment-repaired"/);
  assert.doesNotMatch(helper, /client\.send|WebSocket|\.send\(/);
  assert.match(
    driver,
    /equipmentItems:[\s\S]{0,350}durabilityCurrent: item\?\.durabilityCurrent[\s\S]{0,120}durabilityMax: item\?\.durabilityMax/,
  );
});

test("q5 chooses the objective with the most work left", () => {
  const common = [q(1, "completed"), q(2, "completed"), q(3, "completed"), q(4, "completed")];
  const deer = planNextQ1Q5(snapshot(
    ...common,
    q(5, "inProgress", [
      { label: "Kill Deer 3/10", current: 3, required: 10 },
      { label: "Kill Scarecrow 7/10", current: 7, required: 10 },
    ]),
  ));
  assert.equal(deer.monsterName, "Deer");
  const scarecrow = planNextQ1Q5(snapshot(
    ...common,
    q(5, "inProgress", [
      { label: "Kill Deer 9/10", current: 9, required: 10 },
      { label: "Kill Scarecrow 2/10", current: 2, required: 10 },
    ]),
  ));
  assert.equal(scarecrow.monsterName, "Scarecrow");
});

test("q6-q9 policy follows the original Warrior hand-offs and class reward", () => {
  const q1Q5 = [1, 2, 3, 4, 5].map((questId) => q(questId, "completed"));
  assert.deepEqual(planNextQ1Q9(snapshot(...q1Q5, q(6, "available"))), {
    kind: "talk", action: "accept", questId: 6, npcKey: "blacksmith", target: "@quest:accept:6",
  });
  assert.deepEqual(planNextQ1Q9(snapshot(...q1Q5, q(6, "inProgress"))), {
    kind: "hunt", questId: 6, monsterName: "HookingCat", harvest: false,
  });
  assert.equal(
    planNextQ1Q9(snapshot(...q1Q5, q(6, "readyToTurnIn"))).rewardChoiceTarget,
    "@quest:finish:6:0",
  );

  const q1Q6 = [...q1Q5, q(6, "completed")];
  assert.equal(planNextQ1Q9(snapshot(...q1Q6, q(7, "available"))).npcKey, "assistant");
  assert.equal(planNextQ1Q9(snapshot(...q1Q6, q(7, "readyToTurnIn"))).npcKey, "masterWa");

  const q1Q7 = [...q1Q6, q(7, "completed")];
  const q8 = planNextQ1Q9(snapshot(
    ...q1Q7,
    q(8, "inProgress", [
      { label: "Kill Oma", current: 8, required: 10 },
      { label: "Kill RakingCat", current: 2, required: 10 },
    ]),
  ));
  assert.equal(q8.monsterName, "RakingCat");

  const visibleOma = planNextQ1Q9({
    questLog: [
      ...q1Q7,
      q(8, "inProgress", [
        { label: "Kill Oma", current: 8, required: 10 },
        { label: "Kill RakingCat", current: 2, required: 10 },
      ]),
    ],
    entities: [{ kind: "monster", name: "Oma", dead: false }],
  });
  assert.equal(visibleOma.monsterName, "Oma");

  const q1Q8 = [...q1Q7, q(8, "completed")];
  assert.equal(planNextQ1Q9(snapshot(...q1Q8, q(9, "available"))).npcKey, "masterWa");
  assert.equal(planNextQ1Q9(snapshot(...q1Q8, q(9, "readyToTurnIn"))).npcKey, "mirGuide");
});

test("single-objective progress falls back to the authoritative quest counters", () => {
  assert.deepEqual(
    objectiveProgress({
      current: 2,
      required: 5,
      objectives: [{ label: "Collect {Deer Meat/LightSteelBlue} by Hunting {Deer's/Crimson}" }],
    }, "DeerMeat"),
    {
      current: 2,
      required: 5,
      label: "Collect {Deer Meat/LightSteelBlue} by Hunting {Deer's/Crimson}",
    },
  );
});

test("respawn-region patrol covers the centre and AOI-sized interior quadrants", () => {
  const fields = expandRespawnPatrolFields([
    { mapFileName: "0", x: 180, y: 420, spread: 50, count: 20 },
  ]);
  assert.equal(fields.length, 9);
  assert.deepEqual(fields[0], {
    mapFileName: "0", x: 180, y: 420, spread: 50, count: 20,
    patrolCenterX: 180, patrolCenterY: 420,
  });
  assert.ok(fields.some((field) => field.x === 155 && field.y === 395));
  assert.ok(fields.some((field) => field.x === 205 && field.y === 445));
});

test("resource-sensitive patrol keeps the entry edge then prefers safer samples", () => {
  const player = { x: 160, y: 160 };
  const field = { mapFileName: "0", x: 100, y: 100, count: 40, spread: 60 };
  const hazards = [{ x: 100, y: 100, count: 80, spread: 25 }];
  const unranked = expandRespawnPatrolFields([field], { player });
  const safer = expandRespawnPatrolFields([field], { player, hazards });
  assert.deepEqual([safer[0].x, safer[0].y], [unranked[0].x, unranked[0].y]);
  assert.notDeepEqual([safer[1].x, safer[1].y], [100, 100]);
  assert.ok(
    respawnTerminalExposure({ ...safer[1], spread: 9 }, hazards) <=
      respawnTerminalExposure({ ...unranked[1], spread: 9 }, hazards),
  );
});

test("respawn patrol sweeps the selected field before moving to another centre", () => {
  const fields = expandRespawnPatrolFields([
    { mapFileName: "0", x: 475, y: 500, spread: 90, count: 40 },
    { mapFileName: "0", x: 130, y: 510, spread: 70, count: 40 },
  ]);
  assert.deepEqual(fields.slice(0, 2).map(({ x, y }) => [x, y]), [[475, 500], [447, 472]]);
  assert.deepEqual([fields[9].x, fields[9].y], [130, 510]);
});

test("respawn arrival approaches a known AOI target before rotating fields", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /const fieldEncounter = rankMonsterApproachTargets[\s\S]{0,1100}field-approach-/,
  );
  assert.match(
    runner,
    /const orderedSourceFields = rankRespawnFieldsForTravel[\s\S]{0,700}let cursor = 0;/,
  );
  assert.doesNotMatch(runner, /fieldCursor/);
});

test("live repeated quest deaths trigger a one-level ordinary grind before retry", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const adaptiveBody = runner.slice(
    runner.indexOf("function adaptiveCombatPreparationGoal("),
    runner.indexOf("function adaptiveGrindingRiskGoal("),
  );
  assert.match(
    adaptiveBody,
    /const deaths = Number\(questMonsterDeaths[\s\S]*deaths >= 2[\s\S]*preparationLevel = playerLevel \+ 1[\s\S]*chooseGrindingGoal/,
  );
});

test("runtime command audit rejects every privileged shortcut", () => {
  for (const type of QUEST_AGENT_CONTRACT.forbiddenClientCommands) {
    assert.equal(auditOutgoingBrowserCommand({ type }).ok, false, type);
  }
  assert.equal(auditOutgoingBrowserCommand({ type: "chat", message: "@MOB Deer" }).ok, false);
  for (const type of ["walk", "run", "turn", "interact", "attack", "harvest", "selectNpcDialog"] ) {
    assert.equal(auditOutgoingBrowserCommand({ type }).ok, true, type);
  }
});

test("audit accepts privileged-looking packets only when caused by matching visible UI input", () => {
  const recentInputs = [
    { kind: "mouse", action: "enter-visible-map-transfer", transferKey: "0:1:12:20", at: 100 },
    { kind: "mouse", action: "quest-diary-accept", questId: 22, at: 110 },
    { kind: "mouse", action: "quest-diary-finish", questId: 23, selectedItemIndex: 0, at: 120 },
  ];
  assert.equal(
    auditOutgoingBrowserCommand({ type: "transferMap", key: "0:1:12:20" }, { recentInputs }).ok,
    true,
  );
  assert.equal(
    auditOutgoingBrowserCommand({ type: "acceptQuest", questIndex: 22, npcIndex: 0 }, { recentInputs }).ok,
    true,
  );
  assert.equal(
    auditOutgoingBrowserCommand(
      { type: "finishQuest", questIndex: 23, selectedItemIndex: 0 },
      { recentInputs },
    ).ok,
    true,
  );
  assert.equal(
    auditOutgoingBrowserCommand({ type: "transferMap", key: "wrong" }, { recentInputs }).ok,
    false,
  );
  assert.equal(
    auditOutgoingBrowserCommand({ type: "acceptQuest", questIndex: 23 }, { recentInputs }).ok,
    false,
  );
});

test("route fixtures match the generated Crystal q1-q5 manifest", async () => {
  const manifestPath = new URL(
    "../../../../packages/game-data/data/generated/crystal_quest_packet_manifest.json",
    import.meta.url,
  );
  const manifest = JSON.parse(await fs.readFile(manifestPath, "utf8"));
  const byId = new Map(manifest.quests.map((quest) => [Number(quest.index), quest]));
  assert.deepEqual(BICHON_Q1_Q5_ROUTE.quests, [1, 2, 3, 4, 5]);
  assert.deepEqual(
    BICHON_Q1_Q5_ROUTE.quests.map((id) => [byId.get(id)?.npc_index, byId.get(id)?.finish_npc_index]),
    [[3, 4], [4, 3], [3, 6], [6, 6], [5, 5]],
  );
  assert.equal(byId.get(2).item_tasks[0].item_name, "GingerTea");
  assert.equal(byId.get(4).item_tasks[0].item_name, "DeerMeat");
  assert.deepEqual(
    byId.get(5).kill_tasks.map((task) => [task.monster_name, task.count]),
    [["Deer", 10], ["Scarecrow", 10]],
  );
});

test("route fixtures match the generated Crystal q6-q9 Warrior manifest", async () => {
  const manifest = JSON.parse(await fs.readFile(new URL(
    "../../../../packages/game-data/data/generated/crystal_quest_packet_manifest.json",
    import.meta.url,
  ), "utf8"));
  const byId = new Map(manifest.quests.map((quest) => [Number(quest.index), quest]));
  assert.deepEqual(BICHON_Q1_Q9_ROUTE.quests, [1, 2, 3, 4, 5, 6, 7, 8, 9]);
  assert.deepEqual(
    [6, 7, 8, 9].map((id) => [byId.get(id)?.npc_index, byId.get(id)?.finish_npc_index]),
    [[5, 5], [3, 10], [10, 10], [10, 26]],
  );
  assert.deepEqual(byId.get(6).kill_tasks.map((task) => [task.monster_name, task.count]), [["HookingCat", 10]]);
  assert.deepEqual(
    byId.get(8).kill_tasks.map((task) => [task.monster_name, task.count]),
    [["Oma", 10], ["RakingCat", 10]],
  );
  assert.equal(BICHON_Q1_Q9_ROUTE.equipment.q6WarriorChoiceName, "BronzeWarriorSword");
});

test("executable agent sources contain no direct bridge or DOM-mutation shortcuts", async () => {
  const sources = [
    "autonomous-policy.mjs",
    "browser-driver.mjs",
    "run-q1-q5.mjs",
    "run-q1-q9.mjs",
    "run-1-50.mjs",
    "supervisor-policy.mjs",
  ];
  const forbidden = [
    /__mir2Stage5\s*\?*\.\s*send/,
    /sendCommand\s*\(/,
    /HTMLInputElement\.prototype/,
    /\.click\s*\(\s*\)/,
    /type\s*:\s*["'](?:transferMap|moveTo|pickUp|acceptQuest|finishQuest|abandonQuest|shareQuest|stage5Command)["']/,
    /@(?:MOB|SETQUEST|LEVEL|GIVE|MOVE)\b/i,
  ];
  for (const file of sources) {
    const body = await fs.readFile(new URL(file, import.meta.url), "utf8");
    for (const pattern of forbidden) assert.doesNotMatch(body, pattern, `${file}: ${pattern}`);
  }
});

test("the full-route supervisor retries transport loss and the exact StartGame lease handoff", () => {
  assert.equal(isTransientQuestAgentFatal("Error: CDP socket closed; pending evaluate"), true);
  assert.equal(isTransientQuestAgentFatal("browser process exited before target attach"), true);
  assert.equal(
    isTransientQuestAgentFatal("visible Start Game flow failed: character is already online or route lease is unavailable"),
    true,
  );
  assert.equal(isTransientQuestAgentFatal("visible Start Game flow did not enter the world"), false);
  assert.equal(isTransientQuestAgentFatal("q25 source was not found"), false);
  assert.equal(isTransientQuestAgentExit({ exitCode: 13, summary: null }), true);
  assert.equal(isTransientQuestAgentExit({ exitCode: 13, summary: { fatal: "syntax error" } }), false);
  assert.equal(isTransientQuestAgentExit({ exitCode: 1, signal: "SIGTERM", summary: null }), true);
  assert.equal(isTransientQuestAgentExit({ exitCode: 1, summary: null }), false);
  assert.equal(restartDelayMs(1, 20_000), 20_000);
  assert.equal(restartDelayMs(4, 20_000), 60_000);
  assert.equal(signalExitCode("SIGINT"), 130);
  assert.equal(signalExitCode("SIGTERM"), 143);
  assert.equal(signalExitCode("unknown"), 1);

  const raw = ["--output", "/tmp/old", "--maxRestarts", "4", "--headed"];
  assert.deepEqual(stripSupervisorOptions(raw), ["--output", "/tmp/old", "--headed"]);
  assert.deepEqual(
    replaceCliOption(raw, "output", "/tmp/new"),
    ["--maxRestarts", "4", "--headed", "--output", "/tmp/new"],
  );
  assert.deepEqual(
    sanitizeAttemptSummary(2, 1, null, {
      completed: false,
      fatal: "CDP socket closed",
      kills: 6,
      shortcutAudit: { violations: [] },
    }),
    {
      attempt: 2,
      exitCode: 1,
      signal: null,
      reportAvailable: true,
      completed: false,
      fatal: "CDP socket closed",
      runtimeMs: 0,
      goals: 0,
      goalsOk: 0,
      kills: 6,
      deaths: 0,
      revives: 0,
      shortcutViolations: 0,
      criticalConsoleErrorCount: 0,
      criticalNetworkFailureCount: 0,
    },
  );
  assert.deepEqual(
    sanitizeAttemptSummary(3, 13, null, null),
    {
      attempt: 3,
      exitCode: 13,
      signal: null,
      reportAvailable: false,
      completed: false,
      fatal: null,
      runtimeMs: null,
      goals: null,
      goalsOk: null,
      kills: null,
      deaths: null,
      revives: null,
      shortcutViolations: null,
      criticalConsoleErrorCount: null,
      criticalNetworkFailureCount: null,
    },
  );
});

test("the full-route supervisor resumes the latest written attempt", async () => {
  const supervisor = await fs.readFile(new URL("run-1-50.mjs", import.meta.url), "utf8");
  assert.match(supervisor, /const attemptReport = path\.join\(attemptDir, "report\.json"\)/);
  assert.match(
    supervisor,
    /if \(await fileExists\(attemptReport\)\) \{[\s\S]{0,160}replaceCliOption\(childArgs, "resumeReport", attemptReport\)/,
  );
});

test("typed cancellation wakes pending CDP commands for graceful evidence finalization", async () => {
  const client = new CdpClient("ws://unused.invalid");
  client.ws = { send() {} };
  const shutdown = new Error("typed graceful shutdown");
  shutdown.name = "QuestAgentShutdownError";
  const pending = client.send("Runtime.evaluate", {}, 60_000);
  client.cancelPending(shutdown);
  await assert.rejects(pending, (error) => error === shutdown);
});

test("the supervisor isolates its runner process group before forwarding Ctrl-C", async () => {
  const supervisor = await fs.readFile(new URL("run-1-50.mjs", import.meta.url), "utf8");
  assert.match(supervisor, /detached: process\.platform !== "win32"/);
  assert.match(
    supervisor,
    /quest-agent supervisor forwarding graceful shutdown[\s\S]{0,300}activeChild\.kill\(signal\)/,
  );
});

test("multi-day supervisor forwards stop signals and the runner writes evidence before browser close", async () => {
  const supervisor = await fs.readFile(new URL("run-1-50.mjs", import.meta.url), "utf8");
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    supervisor,
    /process\.on\(signal[\s\S]{0,500}activeChild\.kill\(signal\)/,
  );
  assert.match(
    supervisor,
    /requestedSignal != null[\s\S]{0,180}publishLatestAttempt/,
  );
  assert.match(
    runner,
    /class QuestAgentShutdownError[\s\S]{0,1200}shutdownSignal != null/,
  );
  assert.match(
    runner,
    /finalizeEvidence\(fatal, interruptionSignal\)[\s\S]{0,120}stopBrowser\(browser\)/,
  );
});

test("visible StartGame retries one exact new route-lease error through the real button", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(runner, /for \(let attempt = 1; attempt <= 2 && !entered; attempt \+= 1\)/);
  assert.match(
    runner,
    /const startRequestedAt = Date\.now\(\);[\s\S]{0,180}clickSelector\("\.select-action\.start button", \{ action: "start-game", attempt \}\)/,
  );
  assert.match(
    runner,
    /wsEventFramesSince\(client, startRequestedAt, "error"\)[\s\S]{0,180}TRANSIENT_START_GAME_ROUTE_LEASE_MESSAGE/,
  );
  assert.match(runner, /START_GAME_ROUTE_LEASE_RETRY_MS = 30_000/);
  assert.match(runner, /await delay\(START_GAME_ROUTE_LEASE_RETRY_MS\)/);
  assert.doesNotMatch(runner, /start-game-route-lease-retry[\s\S]{0,220}sendCommand/);
});

test("ground-drop pickup walks into range and targets the exact visible object", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  const scene = await fs.readFile(
    new URL("../../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
    "utf8",
  );
  assert.match(runner, /navigateNear\(drop, 1, \{ maxAttempts: 30 \}\)/);
  assert.match(runner, /ground-drop-marker\[data-object-id=/);
  assert.match(
    runner,
    /else if \(!collected\) \{[\s\S]{0,300}neither appeared as a visible[\s\S]{0,120}authoritative Q-drop progress/,
  );
  assert.match(runner, /nearestGroundDropByName\([\s\S]{0,180}groundDropCooldownUntil\.keys\(\)/);
  assert.match(runner, /collectNearbyGoldIfVisible\(before, 8\)/);
  assert.match(
    runner,
    /nearestHealthPotionGroundDrop\(\{[\s\S]{0,300}groundDropCooldownUntil\.has/,
  );
  assert.match(runner, /const supplyDropDeadline = Date\.now\(\) \+ 2_500/);
  assert.match(runner, /const unconfirmedDropDeadline = Date\.now\(\) \+ 2_500/);
  assert.match(runner, /const deferredHarvestDropDeadline = Date\.now\(\) \+ 2_500/);
  assert.match(
    runner,
    /deferredHarvestDropDeadline[\s\S]{0,900}collectVisibleSafeSupplyLootIfNeeded\(deferredState\)[\s\S]{0,1000}recoveredFromVisibleDrop: true/,
  );
  assert.match(
    runner,
    /if \(!result\.success\) \{[\s\S]{0,1400}collectNearbyGoldIfVisible\(afterUnconfirmedHunt\)[\s\S]{0,500}return true;/,
  );
  assert.match(runner, /Number\(s\.gold \?\? 0\) >/);
  assert.match(
    runner,
    /optional visible potion funding deferred:[\s\S]{0,900}return true;[\s\S]{0,900}continue;/,
  );
  assert.match(runner, /failFastWhenCollisionPathUnavailable: true/);
  assert.match(runner, /OPTIONAL_DROP_REJECTED_COOLDOWN_MS = 30_000/);
  assert.match(runner, /HEALTH_POTION_DEPARTURE_STOCK = 10/);
  assert.match(
    runner,
    /localPotionSupplyIncomplete\(before\)[\s\S]{0,500}hold quest departure for HP stock/,
  );
  assert.match(runner, /Number\(state\.gold \?\? 0\) <= lastPotionRestockGold/);
  assert.doesNotMatch(runner, /preservePotionStockForSupply/);
  assert.match(
    runner,
    /state\.playerDead \|\| state\.deathOverlayVisible[\s\S]{0,180}interruptedByDeath: true/,
  );
  assert.match(runner, /collectVisibleSafeSupplyLootIfNeeded\(before\)/);
  assert.match(runner, /pick-up-visible-sellable-supply-loot/);
  assert.match(
    runner,
    /normalizeName\(itemName\) === normalizeName\("Venison"\)[\s\S]{0,100}deerFundingUnavailableUntil = 0/,
  );
  assert.doesNotMatch(runner, /liquidatedItemIds/);
  assert.match(runner, /SAFE_DUPLICATE_EQUIPPED_SUPPLY_LOOT = Object\.freeze\(\["CopperRing"\]\)/);
  assert.match(runner, /npcIndex: 447, label: "Merchant Alice", x: 20, y: 23/);
  assert.match(page, /pointTileDistance\(nextSelf, pendingDrop\) <= 1/);
  assert.match(page, /pointTileDistance\(serverSelf, drop\) > 1/);
  assert.match(scene, /data-object-id=\{drop\.objectId\}/);
});

test("global collision segments use the client's physical continuous-movement gesture", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /if \(steeringDistance > GLOBAL_COLLISION_PATH_THRESHOLD\)/,
  );
  assert.doesNotMatch(
    runner,
    /if \(!forcedDetourTarget && steeringDistance > GLOBAL_COLLISION_PATH_THRESHOLD\)/,
  );
  assert.match(
    runner,
    /if \(usedGlobalCollisionPath\)[\s\S]{0,2400}holdKeyChord\([\s\S]{0,300}key: "Shift"/,
  );
  assert.match(runner, /action: "navigate-visible-collision-run-segment"/);
  assert.doesNotMatch(runner, /maxSafeLevelDelta: -100/);
  assert.match(runner, /STICKY_NAVIGATION_DETOUR_TTL_MS = 90_000/);
  assert.match(runner, /expire sticky collision detour:/);
  assert.match(runner, /continuousCollisionRunAvoidsTransfers\(\{/);
  assert.match(runner, /transfer-guarded collision run:/);
});

test("continuous collision runs stop near protected map transfers", () => {
  const transfers = [{ minX: 286, maxX: 286, minY: 296, maxY: 296 }];
  assert.equal(continuousCollisionRunAvoidsTransfers({
    start: { x: 290, y: 292 },
    direction: { x: -1, y: 1 },
    plannedSteps: 4,
    mapTransfers: transfers,
  }), false);
  assert.equal(continuousCollisionRunAvoidsTransfers({
    start: { x: 310, y: 292 },
    direction: { x: 0, y: 1 },
    plannedSteps: 4,
    mapTransfers: transfers,
  }), true);
  assert.equal(continuousCollisionRunAvoidsTransfers({
    start: { x: 290, y: 292 },
    direction: { x: 0, y: 0 },
    plannedSteps: 4,
    mapTransfers: transfers,
  }), false);
});

test("intentional map travel leaves an entire same-destination doorway cluster open", () => {
  const transfers = [
    { key: "a", toMapFileName: "0", minX: 411, maxX: 411, minY: 569, maxY: 569 },
    { key: "b", toMapFileName: "0", minX: 410, maxX: 410, minY: 569, maxY: 569 },
    { key: "c", toMapFileName: "0", minX: 410, maxX: 410, minY: 568, maxY: 568 },
    { key: "shop", toMapFileName: "0120", minX: 516, maxX: 516, minY: 492, maxY: 492 },
  ];
  assert.deepEqual(
    protectedTransfersForNavigation(transfers, "0").map((entry) => entry.key),
    ["shop"],
  );
  assert.deepEqual(
    protectedTransfersForNavigation(transfers).map((entry) => entry.key),
    ["a", "b", "c", "shop"],
  );
});

test("supply travel visibly recovers from an accidentally entered protected map", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function returnToSupplyAreaForPotionsIfNeeded"),
    runner.indexOf("async function collectVisibleHealthPotionDropIfNeeded"),
  );
  assert.match(body, /error instanceof NavigationEnteredUnexpectedMapError/);
  assert.match(
    body,
    /state = await travelToMap\(homeMapFileName, \{[\s\S]{0,120}enforceCombatResourceBudget: false/,
  );
  assert.match(body, /recordMilestone\("protected-transfer-recovered"/);
  assert.match(body, /assertNoShortcutFrames\(\)/);
  assert.doesNotMatch(body, /transferMap|stage5Command|qa\./i);
});

test("collision escape exhausts the bounded direction set around dynamic occupancy", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const atlasBody = runner.slice(
    runner.indexOf("async function collisionAtlasPathToward"),
    runner.indexOf("async function collisionAtlasCorridor"),
  );
  const probesBody = runner.slice(
    runner.indexOf("async function prioritizedMovementProbes"),
    runner.indexOf("async function travelToMap"),
  );
  assert.match(runner, /const DISCRETE_MOVEMENT_INPUT_GUARD_MS = 1_000/);
  assert.match(
    runner,
    /function rememberAcknowledgedMovement[\s\S]{0,500}async function waitForDiscreteMovementInput/,
  );
  assert.match(
    runner,
    /async function tryCollisionPathStep[\s\S]{0,1800}await waitForDiscreteMovementInput\(\);[\s\S]{0,120}client\.pressKey/,
  );
  assert.match(
    runner,
    /async function dispatchKeyboardEscapeProbes[\s\S]{0,1000}await waitForDiscreteMovementInput\(\);/,
  );
  assert.match(runner, /const REJECTED_COLLISION_CELL_TTL_MS = 30_000/);
  assert.match(
    runner,
    /new Set\([\s\S]{0,120}activeRejectedCollisionCells\(expectedMapFileName\)/,
  );
  assert.match(runner, /function rememberRejectedCollisionCell/);
  assert.match(runner, /function activeRejectedCollisionCells/);
  assert.match(
    runner,
    /const staticPath = findCollisionGridPath\(\{[\s\S]{0,900}blocked,[\s\S]{0,80}occupied: \[\]/,
  );
  assert.doesNotMatch(
    runner,
    /const staticPath = findCollisionGridPath\(\{[\s\S]{0,240}blocked: staticBlocked/,
  );
  assert.match(runner, /for \(const probe of probes\.slice\(0, 8\)\)/);
  assert.match(
    runner,
    /const revisits = ranked\.filter\([\s\S]{0,700}return \[\.\.\.unvisited, \.\.\.revisits\]/,
  );
  assert.match(
    runner,
    /const detour = selectProgressingCollisionDetour\(saferPath, player, target\)[\s\S]{0,160}saferPath\.detourEndpoint/,
  );
  assert.match(
    runner,
    /const detour = selectProgressingCollisionDetour\(dynamicPath, player, target\)[\s\S]{0,160}dynamicPath\.detourEndpoint/,
  );
  assert.match(runner, /positionVisitCount/);
  assert.match(runner, /reject cycling collision edge:/);
  assert.match(atlasBody, /\.filter\(\(entry\) => !entityIsCorpse\(entry\)/);
  assert.match(probesBody, /\.filter\(\(entry\) => !entityIsCorpse\(entry\)\)/);
  assert.doesNotMatch(atlasBody, /!entry\.dead/);
  assert.doesNotMatch(probesBody, /!entry\.dead/);
  assert.match(
    runner,
    /signatureVisits >= 3[\s\S]{0,900}collisionAtlasPathToward/,
  );
});

test("long travel flees while moving and clears a safe attacker only after stalling", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.doesNotMatch(runner, /interruptOnBlockingThreatName: monsterName/);
  assert.match(runner, /blockingThreat && activeGoal && stalledChunks >= 2/);
  assert.match(
    runner,
    /incidentalTravelThreatIsTrivial\(profile\.level, playerLevel\)/,
  );
  assert.match(runner, /clear stalled adjacent travel threat:/);
  assert.match(runner, /incidentalTravelThreat: true/);
  assert.match(
    runner,
    /incidentalTravelOrigin: before\.player[\s\S]{0,240}Number\(before\.player\.y\)/,
  );
  assert.match(
    runner,
    /goal\.incidentalTravelThreat[\s\S]{0,220}chebyshev\(goal\.incidentalTravelOrigin, live\) > 8/,
  );
  assert.match(
    runner,
    /goal\.incidentalTravelThreat && live && chebyshev\(state\.player, live\) > 1[\s\S]{0,320}disengaged from the adjacent travel block/,
  );
  assert.match(runner, /delay\(goal\.incidentalTravelThreat \? 80 : 350\)/);
  assert.match(
    runner,
    /navigationError instanceof NavigationInterruptedByDeathError[\s\S]{0,1000}throw navigationError;/,
  );
  assert.match(
    runner,
    /const revivesBeforeSearch = evidence\.revives[\s\S]{0,500}main policy must replan from town/,
  );
});

test("moving-target melee chase may physically clear one certified adjacent occupant", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const combatBody = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  assert.match(
    combatBody,
    /await navigateNear\(live, 1, \{[\s\S]{0,900}clearTrivialOccupancy: true/,
  );
  assert.match(
    runner,
    /clearTrivialOccupancy && \(stagnant >= 2 \|\| signatureVisits >= 3\)/,
  );
});

test("monster approach clears certified occupancy without enabling it on ordinary field travel", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const searchBody = runner.slice(
    runner.indexOf("async function findMonster("),
    runner.indexOf("async function clearAdjacentTravelThreat("),
  );
  assert.equal(
    [...searchBody.matchAll(/clearTrivialOccupancy: true/g)].length,
    3,
    "known, encountered, and reached-field monster approaches should clear one certified occupant",
  );
  assert.match(
    searchBody,
    /maxAttempts: resourceSensitiveSearch \? 4 : 6,[\s\S]{0,220}clearTrivialOccupancy: true,[\s\S]{0,120}resourceBaseline/,
  );
  assert.match(
    searchBody,
    /const encounterAttempts = resourceSensitiveSearch \? 4 : 6;[\s\S]{0,500}clearTrivialOccupancy: true,[\s\S]{0,120}resourceBaseline/,
  );
  assert.doesNotMatch(
    searchBody,
    /navigateNear\(travelTarget, 8, \{[\s\S]{0,300}clearTrivialOccupancy: true/,
  );
});

test("incomplete-potion extended routes physically return to the supply area", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const returnBody = runner.slice(
    runner.indexOf("async function returnToSupplyAreaForPotionsIfNeeded"),
    runner.indexOf("async function collectVisibleHealthPotionDropIfNeeded"),
  );
  assert.match(
    returnBody,
    /potionQuantity >= HEALTH_POTION_DEPARTURE_STOCK[\s\S]{0,400}!inSupplyArea && potionQuantity >= HEALTH_POTION_FIELD_RESERVE[\s\S]{0,2200}navigateNear\(merchant, 5, \{[\s\S]{0,180}respawnTravelAttemptBudget\(distance\)[\s\S]{0,120}abortOnDeath: true/,
  );
  assert.match(returnBody, /incomplete HP supply/);
  assert.match(returnBody, /potionSupplyRecallRequested/);
  assert.match(
    returnBody,
    /let changedMap = false[\s\S]{0,700}state = await travelToMap\(homeMapFileName, \{[\s\S]{0,120}enforceCombatResourceBudget: false[\s\S]{0,120}changedMap = true[\s\S]{0,500}return changedMap/,
  );
  assert.match(
    runner,
    /rendered player snapshot did not settle before autonomous planning/,
  );
  assert.match(
    runner,
    /travel exceeded the sustainable combat resource budget/,
  );
});

test("funded potion purchases stage safe working stock before full departure stock", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /HEALTH_POTION_FUNDING_WORKING_STOCK = 5/,
  );
  assert.match(runner, /HEALTH_POTION_FIELD_RESERVE = 5/);
  assert.match(
    runner,
    /const plannedQuantity = planHealthPotionPurchase\([\s\S]{0,1000}plannedQuantity <= 0/,
  );
  assert.match(
    runner,
    /const quantity = planHealthPotionPurchase\([\s\S]{0,1500}action: "buy-health-potions"/,
  );
  assert.doesNotMatch(runner, /emergencyPartialRestock/);
  assert.match(
    runner,
    /const estimatedRestockQuantityTarget = estimatedPotionQuantity <[\s\S]{0,1000}Number\(state\.gold \?\? 0\) < estimatedSupplyGoldTarget[\s\S]{0,220}liquidateSupersededGearForPotions/,
  );
  assert.match(runner, /HEALTH_POTION_HEAL_AMOUNT = 30/);
  assert.match(runner, /QUEST_DEPARTURE_HEALTH_RATIO = 0\.62/);
  assert.match(runner, /HEALTH_POTION_RESTOCK_RETRY_MS = 5_000/);
  assert.match(
    runner,
    /usePotionIfNeeded\([\s\S]{0,100}QUEST_DEPARTURE_HEALTH_RATIO/,
  );
  assert.match(runner, /healthRatioThreshold = 0\.62/);
  assert.match(
    runner,
    /estimatedSupplyGoldTarget[\s\S]{0,500}liquidateSupersededGearForPotions\([\s\S]{0,120}estimatedSupplyGoldTarget/,
  );
  assert.match(runner, /hold quest departure for HP recovery:/);
});

test("supply hunting stays inside authoritative village-edge respawn fields", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /const catalogFundingFields = authoritativeFundingFields\([\s\S]{0,500}catalogFundingFields\.length > 0\s+\? catalogFundingFields/,
  );
  assert.match(runner, /HEALTH_POTION_FUNDING_FIELD_RADIUS = 64/);
  const fundingFieldsBody = runner.slice(
    runner.indexOf("function authoritativeFundingFields(monsterName, state)"),
    runner.indexOf("async function restockHealthPotionsIfNeeded"),
  );
  assert.match(
    fundingFieldsBody,
    /\.filter\(\(spawn\) => String\(spawn\.mapFileName\) === currentMapFileName\)[\s\S]{0,350}chebyshev\(supplyAnchor, spawn\.position\) <= HEALTH_POTION_FUNDING_FIELD_RADIUS/,
  );
  assert.match(
    runner,
    /const preferNearestSupplyTarget = activeGoal\?\.supplyFunding === true[\s\S]{0,500}rankMonsterApproachTargets/,
  );
  assert.match(
    runner,
    /function rankMonsterApproachTargets[\s\S]{0,650}chebyshev\(state\.player, left\) - chebyshev\(state\.player, right\)/,
  );
  assert.match(runner, /fundingGoal,[\s\S]{0,100}fundingStateBefore,[\s\S]{0,500}killMonster\([\s\S]{0,300}fundingStateBefore/);
});

test("unsafe potion funding shelters with emergency-only potion survival", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(runner, /SAFE_RECOVERY_MAP_FILE_NAME = "0141"/);
  assert.match(runner, /SAFE_FUNDING_MIN_HEALTH_RATIO = 0\.70/);
  assert.match(runner, /SAFE_FUNDING_READY_HEALTH_RATIO = 0\.90/);
  assert.match(runner, /SUPPLY_FUNDING_THREAT_SHELTER_MS = 120_000/);
  const recoveryBody = runner.slice(
    runner.indexOf("async function recoverHealthInSafeInteriorIfNeeded"),
    runner.indexOf("function localPotionSupplyIncomplete"),
  );
  assert.match(
    recoveryBody,
    /!shelterActive[\s\S]{0,100}healthPotionQuantity\(state\) >= HEALTH_POTION_FIELD_RESERVE[\s\S]{0,100}return false/,
  );
  assert.match(
    recoveryBody,
    /currentMapFileName === SAFE_RECOVERY_MAP_FILE_NAME && shelterActive[\s\S]{0,500}supplyFundingShelterUntil = 0/,
  );
  const recoveryLoopArrivalBody = recoveryBody.slice(
    recoveryBody.indexOf("while (Date.now() < recoveryDeadline)"),
    recoveryBody.indexOf("if (state.playerDead || state.deathOverlayVisible)"),
  );
  assert.match(
    recoveryLoopArrivalBody,
    /String\(state\.mapFileName\) === SAFE_RECOVERY_MAP_FILE_NAME/,
  );
  assert.match(recoveryLoopArrivalBody, /supplyFundingShelterUntil = 0/);
  assert.match(
    recoveryLoopArrivalBody,
    /safeRecoveryThreatSettleUntil = Math\.max/,
  );
  assert.match(recoveryLoopArrivalBody, /shelterActive = false/);
  const activeShelterThreatBody = recoveryBody.slice(
    recoveryBody.indexOf("const activeShelterThreat"),
    recoveryBody.indexOf("// This interior is the zero-potion funding shelter"),
  );
  assert.match(activeShelterThreatBody, /nearestActiveHostile\(state/);
  assert.match(
    activeShelterThreatBody,
    /String\(transfer\.toMapFileName[\s\S]{0,180}SAFE_RECOVERY_MAP_FILE_NAME/,
  );
  assert.match(
    activeShelterThreatBody,
    /nearestPointInTransferBounds\(state\.player, recoveryTransfer\)/,
  );
  assert.match(
    activeShelterThreatBody,
    /assessRecoveryTransferProgress\([\s\S]{0,900}safeRecoveryTransferProgress\.stalled/,
  );
  assert.match(
    activeShelterThreatBody,
    /safeRecoveryTransferCongestionUntil\.set\([\s\S]{0,1200}rotate congested recovery transfer:/,
  );
  assert.match(
    activeShelterThreatBody,
    /allowTransferToMap: recoveryTransfer[\s\S]{0,180}SAFE_RECOVERY_MAP_FILE_NAME[\s\S]{0,250}return true/,
  );
  assert.match(
    activeShelterThreatBody,
    /navigateNear\(retreat, recoveryTransfer \? 0 : 1,[\s\S]{0,180}maxAttempts: 4,[\s\S]{0,500}clearTrivialOccupancy: true/,
  );
  assert.match(
    activeShelterThreatBody,
    /const shelterOccupancyClearGoal = \{[\s\S]{0,180}kind: "travel",[\s\S]{0,180}supplyFunding: false/,
  );
  assert.match(
    activeShelterThreatBody,
    /navigateNear\(retreat, recoveryTransfer \? 0 : 1,[\s\S]{0,650}resourceAccountingGoal: shelterOccupancyClearGoal/,
  );
  assert.match(
    recoveryBody,
    /const shelterEscapeGoal = \{[\s\S]{0,260}kind: "travel"[\s\S]{0,500}shouldEnforceShelterEscapeResourceBudget\(state\)/,
  );
  assert.match(
    recoveryBody,
    /travelToMap\(SAFE_RECOVERY_MAP_FILE_NAME,[\s\S]{0,500}autoUsePotions: true[\s\S]{0,900}resourceBaseline: state,[\s\S]{0,180}resourceAccountingGoal: shelterEscapeGoal,[\s\S]{0,180}enforceCombatResourceBudget: enforceShelterEscapeResourceBudget,[\s\S]{0,180}clearTrivialOccupancy: true/,
  );
  assert.match(
    recoveryBody,
    /navigateNear\(retreat, recoveryTransfer \? 0 : 1[\s\S]{0,500}autoUsePotions: true/,
  );
  assert.match(recoveryBody, /safe-passive-health-recovered/);
  assert.match(recoveryBody, /safeRecoveryPaceTargets\(/);
  assert.match(
    runner,
    /const SAFE_RECOVERY_THREAT_SETTLE_MS = 20_000/,
  );
  assert.match(
    recoveryBody,
    /safeRecoveryThreatSettleUntil = Math\.max\([\s\S]{0,180}settleStartedAt \+ SAFE_RECOVERY_THREAT_SETTLE_MS/,
  );
  assert.match(
    recoveryBody,
    /const interiorSettling = currentMapFileName === SAFE_RECOVERY_MAP_FILE_NAME[\s\S]{0,120}Date\.now\(\) < safeRecoveryThreatSettleUntil/,
  );
  assert.match(
    recoveryBody,
    /liveHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO[\s\S]{0,180}Date\.now\(\) >= safeRecoveryThreatSettleUntil/,
  );
  assert.match(
    recoveryBody,
    /navigateNear\(recoveryPaceTarget, 0,[\s\S]{0,300}autoUsePotions: false/,
  );
  assert.doesNotMatch(
    recoveryBody,
    /type:\s*["']tick["']|stage5Command|WorldCommand|MoveTo/,
  );
  assert.match(runner, /assertSafeSupplyFundingState\(activeGoal, liveState, monsterName\)/);
  assert.match(
    runner,
    /if \(!goal\.supplyFunding\) \{\s+const healing = await useRestorativeSelfSkillIfNeeded\([^)]*\);\s+if \(!healing\) await usePotionIfNeeded/,
  );
});

test("field continuation consumes stock down to a bounded reserve before returning", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const returnBody = runner.slice(
    runner.indexOf("async function returnToSupplyAreaForPotionsIfNeeded"),
    runner.indexOf("async function collectVisibleHealthPotionDropIfNeeded"),
  );
  assert.match(
    returnBody,
    /const inSupplyArea =[\s\S]{0,300}!inSupplyArea && potionQuantity >= HEALTH_POTION_FIELD_RESERVE/,
  );
  assert.match(runner, /if \(await retreatFromUnsafeActiveThreatIfNeeded\(before\)\) \{/);
  assert.match(runner, /Unsafe combat recovery owns the next input[\s\S]{0,500}continue/);
  assert.match(
    runner,
    /async function retreatFromUnsafeActiveThreatIfNeeded[\s\S]{0,900}const lowStock = potionQuantity < HEALTH_POTION_FIELD_RESERVE[\s\S]{0,220}const unsafeHealth = healthRatio < QUEST_DEPARTURE_HEALTH_RATIO[\s\S]{0,900}nearestActiveHostile[\s\S]{0,1800}nearestPointInTransferBounds\(state\.player, recoveryTransfer\)[\s\S]{0,800}autoUsePotions: true/,
  );
  assert.match(
    runner,
    /recoverQuestDepartureHealthIfNeeded\(before\)[\s\S]{0,700}Recovery must precede every optional pickup/,
  );
  assert.match(
    runner,
    /function localPotionSupplyIncomplete[\s\S]{0,500}healthPotionQuantity\(state\) < HEALTH_POTION_DEPARTURE_STOCK/,
  );
  const restockBody = runner.slice(
    runner.indexOf("async function restockHealthPotionsIfNeeded"),
    runner.indexOf("async function liquidateSupersededGearForPotions"),
  );
  assert.match(
    restockBody,
    /!initiallyInSupplyArea[\s\S]{0,160}!resumingInsideLiquidationMerchant[\s\S]{0,160}initialPotionQuantity >= HEALTH_POTION_FIELD_RESERVE[\s\S]{0,100}return false/,
  );
});

test("supply NPC navigation clears only a repeatedly blocking trivial occupant", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const navigationBody = runner.slice(
    runner.indexOf("async function navigateNear("),
    runner.indexOf("async function collisionPathToward("),
  );
  assert.match(
    navigationBody,
    /clearTrivialOccupancy && \(stagnant >= 2 \|\| signatureVisits >= 3\)[\s\S]{0,900}nearestPhysicallyClickableTrivialAdjacentHostile[\s\S]{0,500}supplyFunding: true/,
  );
  assert.match(
    navigationBody,
    /const blocker = await nearestPhysicallyClickableTrivialAdjacentHostile\(state\);/,
    "a same-name target occupying the only adjacent exit must remain clearable",
  );
  assert.match(
    runner,
    /openNpcDialog\(merchant, "@BuySell", \{[\s\S]{0,220}clearTrivialOccupancy: true,[\s\S]{0,180}resourceBaseline: restockResourceBaseline,[\s\S]{0,180}resourceAccountingGoal: restockResourceGoal/,
  );
  assert.match(
    runner,
    /openNpcDialog\(npc, goal\.target, \{[\s\S]{0,120}clearTrivialOccupancy: true,[\s\S]{0,160}resourceBaseline/,
  );
  assert.match(
    runner,
    /incidentalTravelThreatIsTrivial\(profile\.level, playerLevel\)/,
  );
  assert.match(
    runner,
    /function nearestTrivialAdjacentHostile[\s\S]{0,900}completedQuestCertifiesMonster\(state, entity\?\.name\)/,
  );
  assert.match(
    runner,
    /async function nearestPhysicallyClickableTrivialAdjacentHostile[\s\S]{0,1800}physicalEntityHitTargets[\s\S]{0,800}state\?\.selectedObjectId/,
  );
  assert.match(
    navigationBody,
    /denseAdjacentHostileCount\(state\) >= 3[\s\S]{0,900}nearestPhysicallyClickableTrivialAdjacentHostile\([\s\S]{0,120}quarantinedMonsterUntil[\s\S]{0,900}denseOccupancyClears \+= 1/,
  );
});

test("low-stock supply work retreats before optional actions and budgets every NPC trip", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const policyLoop = runner.slice(
    runner.indexOf("async function runQuestPolicy()"),
    runner.indexOf("async function executeTalkGoal"),
  );
  assert.ok(
    policyLoop.indexOf("retreatFromUnsafeActiveThreatIfNeeded(before)") <
      policyLoop.indexOf("collectVisibleHealthPotionDropIfNeeded(before)"),
    "an active unsafe attacker must be handled before optional pickups",
  );
  assert.ok(
    policyLoop.indexOf("recoverQuestDepartureHealthIfNeeded(before)") <
      policyLoop.indexOf("collectVisibleHealthPotionDropIfNeeded(before)"),
    "low-HP recovery must run before optional supply and NPC actions",
  );

  const retreatBody = runner.slice(
    runner.indexOf("async function retreatFromUnsafeActiveThreatIfNeeded"),
    runner.indexOf("async function returnToSupplyAreaForPotionsIfNeeded"),
  );
  assert.doesNotMatch(retreatBody, /if \(inSupplyArea\) return false/);
  assert.match(
    retreatBody,
    /const lowStock = potionQuantity < HEALTH_POTION_FIELD_RESERVE[\s\S]{0,220}const unsafeHealth = healthRatio < QUEST_DEPARTURE_HEALTH_RATIO[\s\S]{0,1000}supplyFundingShelterUntil = Math\.max\([\s\S]{0,1800}unsafe \$\{inSupplyArea \? "supply" : "field"\} disengage/,
  );
  assert.match(
    retreatBody,
    /String\(transfer\.toMapFileName[\s\S]{0,180}SAFE_RECOVERY_MAP_FILE_NAME[\s\S]{0,800}nearestPointInTransferBounds\(state\.player, recoveryTransfer\)[\s\S]{0,900}autoUsePotions: true[\s\S]{0,220}allowTransferToMap: recoveryTransfer/,
  );

  const safetyBody = runner.slice(
    runner.indexOf("function assertSafeSupplyNpcActionState"),
    runner.indexOf("async function fundHealthPotionsWithSafeHuntIfNeeded"),
  );
  assert.match(
    safetyBody,
    /const activeThreat = nearestActiveHostile[\s\S]{0,500}if \(activeThreat\)[\s\S]{0,700}throw new SupplyFundingSafetyError[\s\S]{0,250}potionQuantity >= HEALTH_POTION_FIELD_RESERVE/,
  );

  const restockBody = runner.slice(
    runner.indexOf("async function restockHealthPotionsIfNeeded"),
    runner.indexOf("async function liquidateSupersededGearForPotions"),
  );
  assert.match(restockBody, /assertSafeSupplyNpcActionState\(state, "visible health-potion restock"\)/);
  assert.ok(
    restockBody.indexOf("initialPotionQuantity >= HEALTH_POTION_DEPARTURE_STOCK") <
      restockBody.indexOf('assertSafeSupplyNpcActionState(state, "visible health-potion restock")'),
    "an already-full belt must exit before NPC safety can arm the shelter latch",
  );
  const fundingBody = runner.slice(
    runner.indexOf("async function fundHealthPotionsWithSafeHuntIfNeeded"),
    runner.indexOf("function authoritativeFundingFields"),
  );
  assert.ok(
    fundingBody.indexOf("if (!shouldFundHealthPotions") <
      fundingBody.indexOf("assertSafeSupplyNpcActionState(state, fundingReason)"),
    "a no-op funding check must exit before NPC safety can arm the shelter latch",
  );
  assert.match(
    restockBody,
    /const restockResourceBaseline = state[\s\S]{0,500}openNpcDialog\(merchant, "@BuySell", \{[\s\S]{0,220}resourceBaseline: restockResourceBaseline,[\s\S]{0,180}resourceAccountingGoal: restockResourceGoal/,
  );

  const liquidationBody = runner.slice(
    runner.indexOf("async function liquidateSupersededGearForPotions"),
    runner.indexOf("async function repairProgressionEquipmentIfNeeded"),
  );
  assert.match(liquidationBody, /assertSafeSupplyNpcActionState\(state, "visible supply liquidation"\)/);
  assert.match(
    liquidationBody,
    /const liquidationResourceBaseline = state[\s\S]{0,900}openNpcDialog\(merchant, merchantRoute\.dialogTarget, \{[\s\S]{0,220}resourceBaseline: liquidationResourceBaseline,[\s\S]{0,180}resourceAccountingGoal: liquidationResourceGoal/,
  );

  const navigationBody = runner.slice(
    runner.indexOf("async function navigateNear("),
    runner.indexOf("async function collisionPathToward("),
  );
  assert.match(
    navigationBody,
    /const state = await readAgentState\(client\);[\s\S]{0,900}rememberQuestCombatResourceStrain\([\s\S]{0,260}throw new CombatResourceBudgetError\([\s\S]{0,300}navigation did not reach/,
  );
  assert.match(
    navigationBody,
    /const clearingGoal = resourceAccountingGoal \?\?[\s\S]{0,500}clearAdjacentTravelThreat\([\s\S]{0,180}resourceBaseline/,
  );
});

test("scripted map travel uses visible NPC links and audits the paid map change", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function executeVisibleNpcScriptMapTransfer"),
    runner.indexOf("async function recoverRouteMapIfAdjacent"),
  );
  assert.match(body, /await openNpcDialog\(npc, targetSequence\[0\]/);
  assert.match(body, /await clickDialogTarget\(/);
  assert.match(body, /sceneInteractionReady === true/);
  assert.match(body, /beforeGold - afterGold !== goldCost/);
  assert.match(body, /recordMilestone\("visible-npc-script-transfer"/);
  assert.match(body, /assertNoShortcutFrames\(\)/);
  assert.doesNotMatch(body, /transferMap|stage5Command|qa\./i);
  assert.match(
    runner,
    /function talkGoalScriptTravelGoldRequirement[\s\S]{0,1200}returnRoute[\s\S]{0,500}minimumStartingGoldForMapTravelEdges\(journey\)/,
  );
  assert.match(
    runner,
    /async function ensureVisibleScriptTravelFunding[\s\S]{0,1800}liquidateSupersededGearForPotions\(state, requiredGold\)[\s\S]{0,900}minimumGoldTarget: requiredGold/,
  );
});

test("ordinary map travel rotates same-destination entrances only after physical unreachability", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const travelBody = runner.slice(
    runner.indexOf("async function travelToMap("),
    runner.indexOf("async function ensureVisibleScriptTravelFunding("),
  );
  assert.match(travelBody, /const liveTransfers = \(state\.mapTransfers \?\? \[\]\)/);
  assert.match(travelBody, /for \(const candidate of transferCandidates\)/);
  assert.match(
    travelBody,
    /if \(!isRetryableVisibleTransferNavigationError\(error\)\) throw error/,
  );
  assert.match(travelBody, /rotate unreachable visible transfer/);
  assert.match(
    travelBody,
    /clearTrivialOccupancy = enforceCombatResourceBudget[\s\S]{0,5500}clearTrivialOccupancy,[\s\S]{0,180}resourceBaseline/,
  );
  assert.match(
    runner,
    /function isRetryableVisibleTransferNavigationError[\s\S]{0,240}NavigationUnreachableError[\s\S]{0,160}navigation did not reach/,
  );
});

test("budget-disabled equipment repair travel still clears certified physical occupancy", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const travelBody = runner.slice(
    runner.indexOf("async function travelToMap("),
    runner.indexOf("async function ensureVisibleScriptTravelFunding("),
  );
  assert.match(
    travelBody,
    /let journeyResourceGoal = resourceAccountingGoal \?\? null/,
    "disabling the combat budget must not discard the non-funding occupancy policy",
  );

  const repairBody = runner.slice(
    runner.indexOf("async function repairProgressionEquipmentIfNeeded"),
    runner.indexOf("async function usePotionIfNeeded"),
  );
  assert.match(
    repairBody,
    /const repairTravelGoal = \{[\s\S]{0,300}supplyFunding: false/,
    "repair travel must classify a certified blocker as ordinary travel combat, not supply funding",
  );
  assert.match(
    repairBody,
    /travelToMap\(route\.mapFileName, \{[\s\S]{0,260}enforceCombatResourceBudget: false,[\s\S]{0,180}clearTrivialOccupancy: true,[\s\S]{0,180}resourceAccountingGoal: repairTravelGoal/,
    "an urgent repair route must retain bounded real-client occupancy clearing",
  );
});

test("safe passive recovery accepts authoritative healing completed on the approach", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const recoveryBody = runner.slice(
    runner.indexOf("async function recoverHealthInSafeInteriorIfNeeded"),
    runner.indexOf("async function collectVisibleHealthPotionDropIfNeeded"),
  );
  assert.match(
    recoveryBody,
    /catch \(error\)[\s\S]{0,1500}recoveredHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO/,
  );
  assert.match(recoveryBody, /safe-passive-health-recovered-en-route/);
  assert.match(
    recoveryBody,
    /error instanceof NavigationInterruptedByDeathError \|\|[\s\S]{0,100}error instanceof SupplyFundingSafetyError/,
  );
});

test("safe shelter travel re-evaluates a resource budget crossed en route", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const recoveryBody = runner.slice(
    runner.indexOf("async function recoverHealthInSafeInteriorIfNeeded"),
    runner.indexOf("async function collectVisibleHealthPotionDropIfNeeded"),
  );
  assert.match(
    recoveryBody,
    /catch \(error\)[\s\S]{0,800}error instanceof CombatResourceBudgetError[\s\S]{0,120}return true/,
    "a shelter route that consumes its last potion must resume through the outer recovery loop",
  );
});

test("scripted map travel audits required and consumed access items", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(runner, /const requiredItems = Array\.isArray\(edge\.requiredItems\)/);
  assert.match(runner, /visibleItemQuantity\(state, requirement\.item\)/);
  assert.match(runner, /transfer item audit failed for/);
  assert.match(runner, /itemQuantitiesBefore/);
  assert.match(runner, /itemQuantitiesAfter/);
});

test("resource-heavy grind sources are cooled down and replanned", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /const meaningfulChunkMovement = resourceSensitiveSearch \? 1 : 3;[\s\S]{0,100}chunkMovement >= meaningfulChunkMovement/,
  );
  assert.match(
    runner,
    /const planningState = preferredGrindingPlanningState\(before\)[\s\S]{0,1200}adaptiveGrindingRiskGoal\(planningState, goal\)/,
  );
  assert.match(
    runner,
    /function preferredGrindingPlanningState\(state\)[\s\S]{0,600}mapFileName: String\(BICHON_Q1_Q9_ROUTE\.mapFileName\)/,
  );
  assert.match(
    runner,
    /const riskCooldownUntil = Date\.now\(\) \+ 30 \* 60_000[\s\S]{0,180}grindingMonsterRiskUntil\.set\(monsterKey, riskCooldownUntil\)/,
  );
  assert.match(runner, /adaptive grind source:/);
  assert.match(runner, /goal\.kind === "grind"[\s\S]{0,420}grind risk memory:/);
  assert.match(
    runner,
    /rememberGrindingSourceStall\(goal, goalRecord, before, after\)/,
  );
  const stallBody = runner.slice(
    runner.indexOf("function rememberGrindingSourceStall"),
    runner.indexOf("function restoreAdaptiveCombatMemory"),
  );
  assert.match(stallBody, /assessGrindingSourceStall\(goal, before, after,/);
  assert.match(stallBody, /failed goals without EXP; cooling down source/);
  assert.match(
    runner,
    /inheritedGrindingSourceStalls:[\s\S]{0,500}resumeEvidence\?\.grindingSourceStalls/,
  );
  assert.match(
    runner,
    /const emergencyDeerHarvest =[\s\S]{0,220}fundingHealthRatio >= SAFE_FUNDING_READY_HEALTH_RATIO[\s\S]{0,400}fundingPotionQuantity >= HEALTH_POTION_FUNDING_WORKING_STOCK[\s\S]{0,180}emergencyDeerHarvest[\s\S]{0,180}fundingHealthRatio >= 0\.75[\s\S]{0,6500}supply resource risk: Deer/,
  );
});

test("combat resource strain covers navigation and combat for the whole goal", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /await delay\(700\);\s+const after = await readAgentState\(client\);\s+rememberQuestCombatResourceStrain\(goal, before, after\);/,
  );
  const huntBody = runner.slice(
    runner.indexOf("async function executeHuntGoal(goal, resourceBaseline = null)"),
    runner.indexOf("function rememberQuestCombatResourceStrain"),
  );
  assert.match(runner, /executeHuntGoal\(goal, before\)/);
  assert.match(
    huntBody,
    /findMonster\([\s\S]{0,180}resourceBaseline/,
  );
  assert.match(
    runner,
    /async function findMonster\([\s\S]{0,180}resourceBaseline = null[\s\S]{0,900}assertSearchResourceBudget/,
  );
  assert.match(runner, /const resourceSensitiveSearch = Boolean\([\s\S]{0,120}activeGoal\?\.supplyFunding !== true/);
  assert.match(runner, /const chunkAttempts = resourceSensitiveSearch \? 2 : 8/);
  const killBody = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  assert.match(
    killBody,
    /resourceBaseline = null,[\s\S]{0,100}resourceAccountingGoal = goal[\s\S]{0,2400}rememberQuestCombatResourceStrain\(resourceAccountingGoal, resourceBaseline, state\)/,
  );
  assert.match(runner, /recordedCombatResourceStrainGoals = new WeakSet\(\)/);
  assert.match(runner, /questMonsterResourceStrains = new Map\(\)/);
  assert.match(
    runner,
    /recordedCombatResourceStrainGoals\.has\(goal\)\) return true/,
  );
  assert.match(
    runner,
    /resourceStrainCount < 2[\s\S]{0,900}retry another real spawn before leveling/,
  );
  assert.match(
    runner,
    /restoreAdaptiveCombatMemory\(resumeEvidence\)[\s\S]{0,120}main\(\)\.catch/,
  );
  assert.match(
    runner,
    /inheritedCombatResourceStrains:[\s\S]{0,500}resumeEvidence\?\.combatResourceStrains/,
  );
  assert.match(
    runner,
    /inheritedCombatResourceRecoveries:[\s\S]{0,900}resumeEvidence\?\.kills/,
  );
  assert.match(
    runner,
    /unresolvedCombatResourceStrains\(allStrains, recoveries\)/,
  );
  assert.match(
    runner,
    /const resumedPotionQuantity = healthPotionQuantity\([\s\S]{0,240}report\?\.finalState\?\.belt[\s\S]{0,240}combatMemoryRequiresSupplyRecall\(allStrains, recoveries, \{[\s\S]{0,180}currentPotionQuantity: resumedPotionQuantity[\s\S]{0,180}requiredPotionQuantity: HEALTH_POTION_DEPARTURE_STOCK[\s\S]{0,180}potionSupplyRecallRequested = true/,
  );
  assert.match(
    runner,
    /goalKind: "grind"[\s\S]{0,180}riskCooldownUntil/,
  );
  const restoreBody = runner.slice(
    runner.indexOf("function restoreAdaptiveCombatMemory"),
    runner.indexOf("async function collectQuestItemIfVisible"),
  );
  assert.match(
    restoreBody,
    /legacyGrindRecord[\s\S]{0,500}record\?\.goalKind === "grind"[\s\S]{0,700}recordedAt \+ 30 \* 60_000[\s\S]{0,800}grindingMonsterRiskUntil\.set/,
  );
  assert.match(
    runner,
    /chooseGrindingGoal\(state, grindingCatalog, preparationLevel, \{[\s\S]{0,180}completedQuestCombatCertifications/,
  );
  assert.match(
    runner,
    /resourceStrains >= 1[\s\S]{0,180}plannedGoal\.harvest === true[\s\S]{0,1200}stagedPreparationLevel = Math\.min\(sourceLevel, playerLevel \+ 1\)[\s\S]{0,500}preparationLevel > stagedPreparationLevel/,
  );
  assert.match(
    runner,
    /resourceSensitiveSearch &&[\s\S]{0,180}chebyshev\(encounterPlayerBefore, state\.player\) >= 1[\s\S]{0,300}successful step is not a failed approach/,
  );
  assert.match(
    huntBody,
    /successful normal-client engagement proves[\s\S]{0,320}questMonsterResourceStrains\.delete/,
  );
  assert.match(huntBody, /rememberQuestCombatResourceStrain\(goal, resourceBaseline, stateBefore\)/);
  const travelThreatBody = runner.slice(
    runner.indexOf("async function clearAdjacentTravelThreat("),
    runner.indexOf("async function killMonster("),
  );
  assert.match(
    travelThreatBody,
    /resourceBaseline = null[\s\S]{0,800}resourceBaseline,[\s\S]{0,80}activeGoal/,
  );
  assert.match(
    runner,
    /waitForMovementBurst\([\s\S]{0,220}autoUsePotions,[\s\S]{0,120}resourceBaseline,[\s\S]{0,120}resourceAccountingGoal/,
  );
  assert.match(
    runner,
    /async function waitForMovementBurst\([\s\S]{0,900}CombatResourceBudgetError/,
  );
  assert.match(
    runner,
    /assertSafeSupplyFundingState\([\s\S]{0,160}requestedTarget\.name \?\? resourceAccountingGoal\?\.monsterName/,
  );
});

test("combat completion accepts only target-specific death during deadline settlement", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const killBody = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  assert.match(killBody, /if \(diedPacket \|\| entityIsCorpse\(live\)\)/);
  assert.match(killBody, /const deathSettleDeadline = Math\.min\([\s\S]{0,100}Date\.now\(\) \+ 2_500/);
  assert.match(
    killBody,
    /finalCombatEvidence\.targetDied \|\| entityIsCorpse\(finalTarget\)/,
  );
  assert.doesNotMatch(
    killBody,
    /deathSettleDeadline[\s\S]{0,1600}(objectiveAdvanced|experienceAdvanced)/,
  );
  assert.match(
    killBody,
    /const hardDeadline = Math\.min\([\s\S]{0,180}COMBAT_HARD_DEADLINE_MS[\s\S]{0,260}let progressDeadline = Math\.min/,
  );
  assert.match(
    killBody,
    /targetResponseCount > lastTargetResponseCount[\s\S]{0,500}progressDeadline = Math\.min\([\s\S]{0,120}COMBAT_PROGRESS_WINDOW_MS/,
  );
  assert.match(killBody, /fight reached the 5m hard deadline/);
});

test("confirmed combat deaths cannot be selected again from a stale rendered corpse", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const killBody = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  const matchingBody = runner.slice(
    runner.indexOf("function matchingLiveMonsters"),
    runner.indexOf("function entityIsCorpse"),
  );

  assert.equal(
    (killBody.match(/rememberConfirmedMonsterDeath\(objectId\)/g) ?? []).length,
    2,
  );
  assert.match(
    matchingBody,
    /reconcileConfirmedDeadMonsterObjects\([\s\S]{0,220}state\.entities[\s\S]{0,220}CONFIRMED_DEAD_OBJECT_MAX_HOLD_MS/,
  );
  assert.match(
    matchingBody,
    /!confirmedDeadMonsterObjects\.has\(String\(entry\.objectId\)\)/,
  );
  assert.match(
    runner,
    /restoreConfirmedDeadMonsterMemory\(resumeEvidence\)/,
  );
});

test("late harvest progression settles after corpse removal without another input", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const harvestBody = runner.slice(
    runner.indexOf("async function harvestCorpse("),
    runner.indexOf("function wsPacketNamesSince"),
  );
  assert.match(
    harvestBody,
    /!entityIsCorpse\(liveCorpse\)[\s\S]{0,1800}Boolean\(\$\{progressionExpression\}\) \|\| Boolean\(\$\{corpsePresentExpression\}\)[\s\S]{0,800}completed: true, progressed: true/,
  );
  assert.match(
    harvestBody,
    /harvest corpse reappeared during observation settle:[\s\S]{0,120}continue;/,
  );
  assert.match(
    harvestBody,
    /unacknowledgedPasses >= 3[\s\S]{0,500}const progressed = await waitUntil\([\s\S]{0,180}progressionExpression,[\s\S]{0,100}4_000[\s\S]{0,400}return \{ completed: progressed, progressed \}/,
  );
  const finalSettle = harvestBody.slice(
    harvestBody.indexOf("if (unacknowledgedPasses >= 3)"),
    harvestBody.indexOf("await delay(accepted ? 2_100 : 120)"),
  );
  assert.doesNotMatch(
    finalSettle,
    /pressKey|clickEntity/,
    "the late-progress settle window must remain observation-only",
  );
  const removalSettle = harvestBody.slice(
    harvestBody.indexOf("A successful final pass can remove the corpse"),
    harvestBody.indexOf("harvest stopped: killed"),
  );
  assert.doesNotMatch(
    removalSettle,
    /pressKey|clickEntity/,
    "corpse-removal observation must not send another input",
  );
});

test("active threats preempt corpse harvesting and stationary field recovery", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  const huntBody = runner.slice(
    runner.indexOf("async function executeHuntGoal(goal, resourceBaseline = null)"),
    runner.indexOf("function rememberQuestCombatResourceStrain"),
  );
  const harvestBody = runner.slice(
    runner.indexOf("async function harvestCorpse("),
    runner.indexOf("function wsPacketNamesSince"),
  );
  assert.match(
    harvestBody,
    /nearestActiveHostile\(state,[\s\S]{0,500}interruptedByThreat: true/,
  );
  assert.match(
    harvestBody,
    /corpse .* has no physical hitbox[\s\S]{0,700}sameTileCorpses[\s\S]{0,900}obscuredByCorpse: true/,
  );
  assert.match(
    harvestBody,
    /corpse .* has no physical hitbox[\s\S]{0,1900}collectNearbyGoldIfVisible\(state, 1\)/,
  );
  assert.match(
    huntBody,
    /if \(wantedItem && !goal\.harvest && await collectQuestItemIfVisible[\s\S]{0,300}let target = await findMonster/,
  );
  assert.match(
    huntBody,
    /for \(let engagement = 0; engagement < 4; engagement \+= 1\)[\s\S]{0,6500}clear active harvest threat before next corpse:[\s\S]{0,500}target = liveHarvestThreat/,
  );
  assert.match(huntBody, /defend interrupted harvest from certified threat:/);
  assert.match(
    huntBody,
    /for \(let defenceAttempt = 0; defenceAttempt < 4; defenceAttempt \+= 1\)[\s\S]{0,1800}canDefendHarvestThreat\(state, activeThreat\)[\s\S]{0,900}lureCertifiedHarvestThreatAwayFromCorpse\([\s\S]{0,800}clearAdjacentTravelThreat\(/,
  );
  assert.match(
    huntBody,
    /resumeHarvestAfterCertifiedThreats\(\{[\s\S]{0,300}corpse: killed\.corpse \?\? target[\s\S]{0,650}killRecord\.harvestCompleted = harvest\.completed/,
  );
  assert.match(
    huntBody,
    /if \(remainingThreat\) \{[\s\S]{0,100}threat = remainingThreat;[\s\S]{0,80}continue;/,
  );
  assert.match(huntBody, /certified harvest threat disengaged; resume corpse/);
  assert.match(
    huntBody,
    /const harvest = await harvestCorpse\(corpse, goal, objectiveBefore\)[\s\S]{0,280}harvest\.interruptedByThreat/,
  );
  assert.match(huntBody, /q\$\{goal\.questId\}-harvest-resumed-/);
  assert.match(
    huntBody,
    /authoritativeQuestDropProgressed[\s\S]{0,800}objectiveProgress\(finalQuest, wantedItem\) > wantedItemProgressBeforeLastKill[\s\S]{0,1000}normal kill lifecycle/,
  );
  assert.match(
    runner,
    /unsafe \$\{inSupplyArea \? "supply" : "field"\} disengage:[\s\S]{0,700}navigateNear\(retreat, recoveryTransfer \? 0 : 1,[\s\S]{0,180}maxAttempts: 2/,
  );
  assert.match(
    page,
    /function mergeSnapshotEntityIntoPacketRuntime[\s\S]{0,1800}typeof currentEntity\.attackStartedAt === "number"[\s\S]{0,160}currentEntity\.attackStartedAt/,
  );
  assert.match(
    page,
    /function markPlayerStruck[\s\S]{0,2200}patchEntityInList\(current\.entities, attackerId,[\s\S]{0,180}attackStartedAt: now/,
  );
});

test("completed real quest combat can certify an adjacent harvest defender", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("function completedQuestCertifiesMonster"),
    runner.indexOf("async function disengageFromUnsafeHarvestThreat"),
  );
  assert.match(body, /\[normalizeName\("Scarecrow"\), 2\]/);
  assert.match(body, /\[normalizeName\("Deer"\), 4\]/);
  assert.match(body, /\[normalizeName\("HookingCat"\), 6\]/);
  assert.match(body, /\[normalizeName\("Oma"\), 8\]/);
  assert.match(body, /\[normalizeName\("RakingCat"\), 8\]/);
  assert.match(body, /questIsCompleted\(state, bichonCertificationQuest\)/);
  assert.match(body, /authoritativeRoute\?\.quests/);
  assert.match(body, /quest\.objectives\?\.kill/);
  assert.match(body, /quest\.objectives\?\.item/);
  assert.match(body, /objective\.sources/);
  assert.match(
    body,
    /completedQuestCertifiesMonster\(state, threat\.name\)[\s\S]{0,300}chebyshev\(state\.player, threat\) <= 2[\s\S]{0,100}healthRatio >= 0\.75/,
  );
  assert.match(
    body,
    /async function lureCertifiedHarvestThreatAwayFromCorpse\([\s\S]{0,900}retreatPointFromHostile\(state, liveThreat, 6\)[\s\S]{0,900}navigateNear\(retreat, 1/,
  );
  assert.match(
    body,
    /chebyshev\(liveThreat, corpse\) >= HARVEST_DEFENCE_CORPSE_CLEARANCE[\s\S]{0,1800}separationDeadline = Date\.now\(\) \+ 3_500/,
  );
  assert.doesNotMatch(
    body,
    /WorldCommand|MoveTo|Stage5Command|send\(/,
  );
});

test("a physically visible nearby monster can enter the normal locked-attack chase", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function nearestVisibleMonsterByName"),
    runner.indexOf("function matchingLiveMonsters"),
  );
  assert.match(
    body,
    /clickReachCandidates = candidates\.filter\([\s\S]{0,180}CLIENT_LOCKED_ATTACK_CLICK_RADIUS/,
  );
  assert.match(
    body,
    /physicalEntityHitTargets\(clickReachCandidates\.map\([\s\S]{0,220}clickableSamples/,
  );
  assert.match(
    body,
    /chooseImmediateMeleeTarget\(state, visibleCandidates,[\s\S]{0,220}engagementRadius: CLIENT_LOCKED_ATTACK_CLICK_RADIUS[\s\S]{0,180}searchRadius: CLIENT_LOCKED_ATTACK_CLICK_RADIUS/,
  );
  assert.doesNotMatch(body, /send\(|WorldCommand|MoveTo|Stage5Command/);
});

test("moving combat targets keep the production locked chase before manual navigation", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function killMonster("),
    runner.indexOf("async function harvestCorpse("),
  );
  assert.match(
    body,
    /const relocked = await clickEntity\(objectId,[\s\S]{0,180}relock-visible-moving-monster[\s\S]{0,300}if \(relocked && stalledRelockCount < 2\)[\s\S]{0,300}continue;/,
  );
  assert.match(
    body,
    /combat relock stalled:[\s\S]{0,500}combat chase:[\s\S]{0,300}await navigateNear\(live, 1/,
  );
  assert.match(
    body,
    /relockPlayerSignature === lastRelockPlayerSignature[\s\S]{0,120}stalledRelockCount \+ 1[\s\S]{0,500}stalledRelockCount < 2/,
  );
  assert.match(
    body,
    /await navigateNear\(live, 1, \{[\s\S]{0,160}maxAttempts: 4,[\s\S]{0,500}clearTrivialOccupancy: true,[\s\S]{0,160}resourceBaseline,[\s\S]{0,100}resourceAccountingGoal/,
  );
  const relockBranch = body.slice(
    body.indexOf("const relockPlayerSignature"),
    body.indexOf("if (live && chebyshev(state.player, live) === 0)"),
  );
  assert.doesNotMatch(
    relockBranch,
    /lastProgressAt = Date\.now\(\)/,
    "a successful relock click must not masquerade as authoritative combat progress",
  );
});

test("unsafe harvest threats force a physical retreat and cool the overlapping source field", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const body = runner.slice(
    runner.indexOf("async function disengageFromUnsafeHarvestThreat"),
    runner.indexOf("function rememberQuestCombatResourceStrain"),
  );
  assert.match(
    runner,
    /if \(resumed\.unsafeThreat\) \{[\s\S]{0,240}disengageFromUnsafeHarvestThreat/,
  );
  assert.match(body, /fieldGroupCooldownUntil\.set\(fieldGroupKey, cooldownUntil\)/);
  assert.match(body, /monsterCooldownUntil\.set\(String\(entity\.objectId\), cooldownUntil\)/);
  assert.match(
    body,
    /retreatPointFromHostile\(state, threat, 10\)[\s\S]{0,500}navigateNear\(retreat, 1, \{[\s\S]{0,200}abortOnDeath: true/,
  );
});

test("resource-strained quest combat cools the current real respawn group", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const huntBody = runner.slice(
    runner.indexOf("async function executeHuntGoal"),
    runner.indexOf("function rememberQuestCombatResourceStrain"),
  );
  const helperBody = runner.slice(
    runner.indexOf("function coolDownQuestRespawnFieldsAtPosition"),
    runner.indexOf("function rememberQuestCombatResourceStrain"),
  );
  assert.match(
    huntBody,
    /rememberQuestCombatResourceStrain\(goal, resourceBaseline, stateBefore\)[\s\S]{0,240}coolDownQuestRespawnFieldsAtPosition\([\s\S]{0,120}stateBefore\.player/,
  );
  assert.match(
    helperBody,
    /fieldGroupCooldownUntil\.set\(fieldGroupKey, cooldownUntil\)/,
  );
});

test("recycled inventory ids never suppress newly dropped supply loot", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.doesNotMatch(runner, /liquidatedItemIds/);
  assert.match(runner, /unique IDs are recyclable slots, not permanent item identities/);
});

test("offscreen merchant rows are reached with visible mouse-wheel input", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const shop = await fs.readFile(
    new URL("../../app/components/original-client-game-shop.tsx", import.meta.url),
    "utf8",
  );
  assert.match(shop, /className="npc-shop-list"/);
  assert.match(
    runner,
    /rowBox\.bottom > listBox\.bottom[\s\S]{0,500}wheelSelector\([\s\S]{0,120}"\.npc-shop-list"[\s\S]{0,220}scroll-superseded-gear-into-view/,
  );
});

test("visible NPC interaction retries promptly after normal client approach", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  assert.match(
    runner,
    /if \(!sentInteract\) \{[\s\S]{0,1200}Math\.abs\(Number\(p\.x\)-Number\(n\.x\)\)[\s\S]{0,400}12_000/,
  );
  assert.match(runner, /let arrivedEntity = routeNpcEntity\(state, npc, 5\)/);
  assert.match(
    runner,
    /chebyshev\(state\.player, arrivedEntity\) > 1[\s\S]{0,700}navigateNear\(arrivedEntity, 1, \{[\s\S]{0,120}maxAttempts: 16,[\s\S]{0,120}clearTrivialOccupancy,[\s\S]{0,120}\}\)/,
  );
  assert.match(
    runner,
    /fixed 48-attempt budget cannot reach a distant static NPC[\s\S]{0,700}const coordinateDistance = chebyshev\(state\.player, npc\)[\s\S]{0,2600}const coordinateAttempts = entity[\s\S]{0,160}coordinateDistance \+ 96/,
  );
  assert.match(runner, /sentInteract \? 12_000 : 2_000/);
  assert.match(runner, /maxAttempts: coordinateAttempts/);
});

test("visible map transfers route adjacent before a physical key step into the trigger tile", async () => {
  const runner = await fs.readFile(new URL("run-q1-q5.mjs", import.meta.url), "utf8");
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  assert.match(runner, /if \(allowTransferToMap && distance <= 1\)/);
  assert.doesNotMatch(runner, /if \(allowTransferToMap && distance <= 4\)/);
  assert.match(runner, /enter-visible-map-transfer-diagonal-approach/);
  assert.doesNotMatch(
    runner,
    /client\.pressKeyChord\(portalProbe\.keys, transferInput\)/,
  );
  assert.match(runner, /advanced without changing map/);
  assert.match(
    page,
    /const movementTransferKey = transferKeyForWorldTile\([\s\S]{0,260}pendingTransferRef\.current = movementTransferKey/,
  );
});

test("normal client movement opens indexed Crystal doors before retrying the queued step", async () => {
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  const loader = await fs.readFile(
    new URL("../../lib/crystal-map-loader.ts", import.meta.url),
    "utf8",
  );
  assert.match(
    loader,
    /if \(cell\.doorIndex > 0\)[\s\S]{0,180}outputCell\.doorIndex = cell\.doorIndex[\s\S]{0,120}outputCell\.closedDoor = true/,
  );
  assert.match(
    page,
    /originalMapClosedDoorIndexOnMovementPath\([\s\S]{0,500}type: "openDoor", doorIndex: closedDoorIndex[\s\S]{0,500}scheduleMovementConfirmTick\(\)[\s\S]{0,100}return false/,
  );
  assert.match(
    page,
    /case "Opendoor"[\s\S]{0,500}setOriginalMapDoorClosed\([\s\S]{0,400}if \(!closed\) scheduleMovementConfirmTick\(\)/,
  );
  assert.match(
    page,
    /Indexed Crystal doors stay routeable[\s\S]{0,500}cell\?\.closedDoor && !indexedDoor/,
  );
});

test("inventory activation honors authoritative Crystal equip slots", async () => {
  const utils = await fs.readFile(
    new URL("../../app/components/original-client-inventory-utils.ts", import.meta.url),
    "utf8",
  );
  const inventory = await fs.readFile(
    new URL("../../app/components/original-client-inventory-window.tsx", import.meta.url),
    "utf8",
  );
  assert.match(utils, /if \(item\.equipSlot\) return item\.equipSlot/);
  assert.match(utils, /itemLibraryMeta\.frames\.map/);
  assert.match(utils, /AVAILABLE_ORIGINAL_ITEM_ICONS\.has\(normalizedIcon\)/);
  assert.match(utils, /EMPTY_ORIGINAL_ITEM_ICON/);
  assert.match(inventory, /const equipSlot = equipmentSlotForItem\(item\)/);
  assert.doesNotMatch(inventory, /const equipSlot = equipmentSlotForItemKey\(item\.key\)/);
});

test("quest definitions survive prerequisite-locked snapshot pruning", async () => {
  const page = await fs.readFile(new URL("../../app/page.tsx", import.meta.url), "utf8");
  assert.match(page, /questDefinitionByIdRef\.current\.set\(questId, definition\)/);
  assert.match(page, /const definition = questDefinitionByIdRef\.current\.get\(quest\.questId\)/);
  assert.match(page, /\.\.\.\(definition \?\? \{\}\),\s*\.\.\.\(previousQuestById\.get\(quest\.questId\) \?\? \{\}\)/s);
});

test("browser diagnostics separate optional raster fallbacks from critical failures", () => {
  const consoleErrors = [
    { source: "network", level: "error", text: "404 /original-map/WemadeMir2/Tiles/8.png" },
    { source: "console", level: "warning", text: "[mir2] scene asset missing Object" },
    { source: "exception", level: "error", text: "TypeError: broken" },
  ];
  const networkFailures = [
    { url: "http://127.0.0.1/original-map/WemadeMir2/Tiles/8.png", status: 404 },
    {
      url: "http://127.0.0.1/original-effects/Magic/1394.png",
      status: 0,
      error: "net::ERR_ABORTED",
    },
    {
      url: "http://127.0.0.1/original-ui/Prguse2/361.png",
      status: 0,
      error: "net::ERR_ABORTED",
    },
    {
      url: "http://127.0.0.1/api/scene/crystal?v=v7&map=2",
      status: 0,
      error: "net::ERR_ABORTED",
    },
    { url: "http://127.0.0.1/api/session", status: 500 },
  ];
  const classified = classifyBrowserDiagnostics(consoleErrors, networkFailures);
  assert.equal(classified.knownAssetFallbackConsoleErrors.length, 2);
  assert.equal(classified.knownAssetFallbackNetworkFailures.length, 1);
  assert.equal(classified.abortedOptionalAssetRequests.length, 2);
  assert.deepEqual(classified.abortedSupersededSceneRequests, [networkFailures[3]]);
  assert.deepEqual(classified.criticalConsoleErrors, [consoleErrors[2]]);
  assert.deepEqual(classified.criticalNetworkFailures, [networkFailures[4]]);
});

test("combat evidence is correlated to the selected target object", () => {
  const frame = (packet, payload, at = 200) => ({
    at,
    url: "ws://127.0.0.1:7310/ws",
    payloadData: JSON.stringify({ type: "packet", packet, payload }),
  });
  const client = {
    wsReceived: [
      frame("ObjectAttack", { objectId: 101 }),
      frame("ObjectStruck", { objectId: 700, attackerId: 101 }),
      frame("ObjectHealth", { objectId: 700, percent: 50 }),
      frame("ObjectDied", { objectId: 701 }),
    ],
  };
  assert.deepEqual(targetCombatEvidenceSince(client, 100, 700, 101), {
    ownerAttackCount: 1,
    struckCount: 1,
    healthCount: 1,
    damageCount: 0,
    diedCount: 0,
    targetResponded: true,
    targetDied: false,
  });
  assert.equal(targetCombatEvidenceSince(client, 100, 701, 101).targetDied, true);
  assert.equal(targetCombatEvidenceSince(client, 100, 999, 101).targetResponded, false);
});
