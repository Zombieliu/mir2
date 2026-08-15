import assert from "node:assert/strict";
import test from "node:test";

import {
  buildProgressionEquipmentCandidates,
  chooseQuestReward,
  chooseGrindingGoal,
  chooseQuestRewardIndex,
  completedQuestCombatCertifications,
  planNextAuthoritativeQuest,
} from "./autonomous-policy.mjs";
import {
  buildAuthoritativeClassQuestRoute,
  buildGrindingCatalog,
  buildMapTravelGraph,
  findMapTravelRoute,
  loadCrystalQuestRouteSources,
  minimumStartingGoldForMapTravelEdges,
} from "./route-manifest.mjs";

const sources = await loadCrystalQuestRouteSources();
const route = await buildAuthoritativeClassQuestRoute({ className: "Warrior", maxLevel: 50 });
const grindingCatalog = buildGrindingCatalog(sources);
const graph = buildMapTravelGraph(sources);

const q = (questId, stage, objectives = []) => ({ questId, stage, objectives });
const snapshot = (questLog, extra = {}) => ({
  playerLevel: 6,
  mapFileName: "0",
  questLog,
  ...extra,
});

test("maps zero-index q22 endpoints to visible Quest Diary actions", () => {
  const accept = planNextAuthoritativeQuest(snapshot([q(22, "available")]), route, {
    minQuestId: 22,
    maxQuestId: 22,
  });
  assert.deepEqual(accept, {
    kind: "quest-diary",
    action: "accept",
    questId: 22,
    questName: "Forest Yeti's Threat",
    selectedItemIndex: undefined,
  });

  const finish = planNextAuthoritativeQuest(snapshot([q(22, "readyToTurnIn")]), route, {
    minQuestId: 22,
    maxQuestId: 22,
  });
  assert.equal(finish.kind, "quest-diary");
  assert.equal(finish.action, "finish");
});

test("plans q22 hunting from a real current-map respawn", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(22, "inProgress", [{ label: "Kill Forest Yeti", current: 2, required: 5 }]),
  ]), route, { minQuestId: 22, maxQuestId: 22 });
  assert.equal(goal.kind, "hunt");
  assert.equal(goal.monsterName, "ForestYeti");
  assert.equal(goal.targetMapFileName, "0");
  assert.equal(goal.remaining, 3);
  assert.ok(goal.fields.every((field) => field.mapFileName === "0"));
});

test("plans CannibalPlant items through the Crystal corpse-harvest lifecycle", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 10, required: 10 },
      { label: "Cannibal Leaf", current: 3, required: 10 },
    ]),
  ], { playerLevel: 7, player: { x: 288, y: 616 } }), route, { minQuestId: 25, maxQuestId: 25 });
  assert.equal(goal.kind, "hunt");
  assert.equal(goal.monsterName, "CannibalPlant");
  assert.equal(goal.itemName, "CannibalLeaf");
  assert.equal(goal.harvest, true);
  assert.equal(goal.targetMapFileName, "0");
  assert.deepEqual([goal.fields[0].x, goal.fields[0].y], [130, 510]);
});

test("plans q26 PoisonSack through SpittingSpider corpse harvesting", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(26, "inProgress", [
      { label: "Poison Sack", current: 0, required: 10 },
    ]),
  ], { playerLevel: 13, player: { x: 40, y: 280 } }), route, {
    minQuestId: 26,
    maxQuestId: 26,
  });
  assert.equal(goal.kind, "hunt");
  assert.equal(goal.monsterName, "SpittingSpider");
  assert.equal(goal.itemName, "PoisonSack");
  assert.equal(goal.harvest, true);
  assert.equal(goal.targetMapFileName, "0");
});

test("active quest preparation grinds when its real source is far above the player", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 10, required: 10 },
      { label: "Cannibal Leaf", current: 6, required: 10 },
    ]),
  ], {
    playerLevel: 10,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 25,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(goal.kind, "grind");
  assert.equal(goal.preparationForQuestId, 25);
  assert.equal(goal.preparationForMonsterName, "CannibalPlant");
  assert.equal(goal.preparationForMonsterLevel, 20);
  assert.equal(goal.targetLevel, 13);
  assert.ok(goal.monsterLevel <= 12);
});

test("high-level quest preparation first completes an available non-combat delivery", () => {
  const availableDelivery = planNextAuthoritativeQuest(snapshot([
    q(8, "completed"),
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 0, required: 10 },
      { label: "Cannibal Leaf", current: 0, required: 10 },
    ]),
    q(29, "available"),
  ], {
    playerLevel: 10,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 29,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(availableDelivery.kind, "talk");
  assert.equal(availableDelivery.action, "accept");
  assert.equal(availableDelivery.questId, 29);

  const activeDelivery = planNextAuthoritativeQuest(snapshot([
    q(8, "completed"),
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 0, required: 10 },
      { label: "Cannibal Leaf", current: 0, required: 10 },
    ]),
    q(29, "inProgress"),
  ], {
    playerLevel: 10,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 29,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(activeDelivery.kind, "talk");
  assert.equal(activeDelivery.action, "finish");
  assert.equal(activeDelivery.questId, 29);
});

test("high-level quest preparation accepts offered combat inside the bounded melee gap", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 0, required: 10 },
      { label: "Cannibal Leaf", current: 0, required: 10 },
    ]),
    q(28, "available"),
    q(29, "completed"),
    q(30, "available"),
  ], {
    playerLevel: 10,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 30,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(goal.kind, "talk");
  assert.equal(goal.action, "accept");
  assert.equal(goal.questId, 28);
});

test("active quest preparation uses the live-proven seven-level melee gap", () => {
  const goal = planNextAuthoritativeQuest(snapshot([
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 10, required: 10 },
      { label: "Cannibal Leaf", current: 6, required: 10 },
    ]),
  ], {
    playerLevel: 12,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 25,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(goal.kind, "grind");
  assert.equal(goal.targetLevel, 13);

  const prepared = planNextAuthoritativeQuest(snapshot([
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 10, required: 10 },
      { label: "Cannibal Leaf", current: 6, required: 10 },
    ]),
  ], {
    playerLevel: 13,
    player: { x: 288, y: 616 },
  }), route, {
    minQuestId: 25,
    maxQuestId: 25,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(prepared.kind, "hunt");
  assert.equal(prepared.monsterName, "CannibalPlant");
  assert.equal(prepared.monsterLevel, 20);
});

test("completed real quest combat unlocks a bounded efficient grind source", () => {
  const state = snapshot([
    q(8, "completed"),
    q(25, "inProgress", [
      { label: "Cannibal Stem", current: 0, required: 10 },
      { label: "Cannibal Leaf", current: 0, required: 10 },
    ]),
  ], {
    playerLevel: 9,
    player: { x: 288, y: 616 },
  });
  const certifications = completedQuestCombatCertifications(state, route);
  assert.ok(certifications.includes("RakingCat"));

  const goal = planNextAuthoritativeQuest(state, route, {
    minQuestId: 25,
    maxQuestId: 25,
    targetLevel: 50,
    grindingCatalog,
  });
  assert.equal(goal.kind, "grind");
  assert.equal(goal.monsterName, "RakingCat");
  assert.equal(goal.monsterLevel, 13);
  assert.ok(goal.monsterLevel <= state.playerLevel + 4);
});

test("selects a reward compatible with the classic class bit", () => {
  const q6 = route.quests.find((quest) => quest.questId === 6);
  assert.equal(chooseQuestRewardIndex(q6, "Warrior"), 0);
  assert.ok((q6.rewards.selectableItems[0].requiredClass & 1) !== 0);
});

test("derives class-compatible progression equipment from authoritative quest rewards", () => {
  const q23 = route.quests.find((quest) => quest.questId === 23);
  assert.equal(chooseQuestReward(q23, "Warrior")?.itemName, "BronzeShortSword");
  const candidates = buildProgressionEquipmentCandidates(route);
  assert.ok(candidates.find((entry) => entry.questId === 23 && entry.name === "BronzeShortSword"));
  assert.ok(candidates.find((entry) => entry.questId === 25 && entry.name === "SteelBangle"));
  assert.ok(!candidates.some((entry) => entry.name === "BronzeHoaSword"));
  assert.ok(!candidates.some((entry) => entry.name === "BrokenSword"));
});

test("Crystal map graph resolves ordinary adjacent routes", () => {
  const path = findMapTravelRoute(graph, "0", "1");
  assert.ok(path);
  assert.deepEqual(path.map((edge) => [edge.fromMapFileName, edge.toMapFileName]), [["0", "1"]]);
  const errandsPath = findMapTravelRoute(graph, "0", "0100");
  assert.ok(errandsPath);
  assert.deepEqual(
    errandsPath.map((edge) => [edge.fromMapFileName, edge.toMapFileName]),
    [["0", "0101"], ["0101", "0100"]],
  );
  assert.equal(findMapTravelRoute(graph, "0", "0").length, 0);
  assert.equal(findMapTravelRoute(graph, "missing", "1"), null);
});

test("Crystal map graph derives q34 round-trip boats from visible whitelisted NPC scripts", () => {
  const outbound = findMapTravelRoute(graph, "0", "5");
  assert.ok(outbound);
  assert.equal(outbound.length, 1);
  assert.deepEqual(
    {
      kind: outbound[0].kind,
      scriptKey: outbound[0].scriptKey,
      targetSequence: outbound[0].targetSequence,
      goldCost: outbound[0].goldCost,
      minimumGoldExclusive: outbound[0].minimumGoldExclusive,
      npc: outbound[0].npc?.name,
      destination: outbound[0].destination,
    },
    {
      kind: "npc-script",
      scriptKey: "BichonProvince/Sailor",
      targetSequence: ["@brdmove"],
      goldCost: 2_000,
      minimumGoldExclusive: 2_000,
      npc: "Sailor_Rupert",
      destination: { x: 124, y: 353 },
    },
  );

  const returning = findMapTravelRoute(graph, "5", "0");
  assert.ok(returning);
  assert.equal(returning.length, 1);
  assert.equal(returning[0].kind, "npc-script");
  assert.equal(returning[0].scriptKey, "PrajnaIsland/Sailor");
  assert.deepEqual(returning[0].targetSequence, ["@brdmove"]);
  assert.equal(returning[0].goldCost, 2_000);
  assert.equal(
    minimumStartingGoldForMapTravelEdges([...outbound, ...returning]),
    4_001,
    "the agent must fund both strict >2000 checks before boarding the island boat",
  );
});

test("White Valley travel uses the paid visible boat and physical return passage", () => {
  const outbound = findMapTravelRoute(graph, "0", "WhiteVillage");
  assert.ok(outbound);
  assert.equal(outbound.length, 1);
  assert.equal(outbound[0].kind, "npc-script");
  assert.equal(outbound[0].scriptKey, "BichonProvince/Sailor");
  assert.deepEqual(outbound[0].targetSequence, ["@brdmove1"]);
  assert.equal(outbound[0].goldCost, 10_000);
  assert.equal(outbound[0].minimumGoldExclusive, 10_000);
  assert.deepEqual(outbound[0].destination, { x: 67, y: 93 });

  const returning = findMapTravelRoute(graph, "WhiteVillage", "1");
  assert.ok(returning);
  assert.deepEqual(
    returning.map((edge) => [edge.fromMapFileName, edge.toMapFileName]),
    [["WhiteVillage", "bonguk1"], ["bonguk1", "4"], ["4", "1"]],
  );
  assert.ok(returning.every((edge) => edge.kind === "map-movement"));
});

test("q27 Errands has an enabled real-client route to Brian", () => {
  const q27 = route.quests.find((quest) => quest.questId === 27);
  assert.ok(q27);
  assert.equal(q27.finishNpc?.name, "MongchonScout_Brian");
  assert.equal(q27.finishNpc?.mapFileName, "0100");
  assert.deepEqual(q27.blockers, []);
  assert.ok(sources.contentProfile.npcScriptWhitelist.includes(q27.finishNpc.scriptKey));
});

test("q30 JadeRing has an enabled Currish harvest route", () => {
  const q30 = route.quests.find((quest) => quest.questId === 30);
  assert.ok(q30);
  assert.deepEqual(q30.blockers, []);
  const jadeRing = q30.objectives.item.find((objective) => objective.itemName === "JadeRing");
  assert.ok(jadeRing);
  assert.ok(jadeRing.sources.some((source) =>
    source.requiresHarvest &&
    source.monsterName === "Currish" &&
    source.spawnCandidates.some((spawn) => spawn.mapFileName === "2")
  ));
});

test("q29-q34 band has no hidden content or runtime skip", () => {
  for (const questId of [29, 30, 31, 32, 33, 34]) {
    const quest = route.quests.find((entry) => entry.questId === questId);
    assert.ok(quest, `q${questId} should exist in the Warrior route`);
    assert.deepEqual(quest.blockers, [], `q${questId} must remain normally executable`);
  }
  const q34 = route.quests.find((entry) => entry.questId === 34);
  assert.equal(q34.startNpc?.name, "CraftsLady_Kimberly");
  assert.equal(q34.startNpc?.mapFileName, "5");
  assert.equal(q34.finishNpc?.name, "CraftsLady_Alice");
  assert.equal(q34.finishNpc?.mapFileName, "0");
});

test("q35-q47 band has audited sources and no runtime skip", () => {
  const band = route.quests.filter((quest) => quest.questId >= 35 && quest.questId <= 47);

  assert.deepEqual(
    band.map((quest) => quest.questId),
    [35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47],
  );
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q38 = band.find((quest) => quest.questId === 38);
  assert.equal(q38.objectives.item[0].itemName, "Ebony(Fruit)");
  assert.equal(q38.objectives.item[0].sources[0].monsterName, "EbonyTree");

  const q42 = band.find((quest) => quest.questId === 42);
  assert.deepEqual(
    q42.objectives.kill.map((task) => task.monsterName),
    ["RedViper", "TigerViper"],
  );

  const q47 = band.find((quest) => quest.questId === 47);
  assert.deepEqual(
    q47.objectives.item[0].sources.map((source) => ({
      tableKey: source.tableKey,
      monsterName: source.monsterName,
      mapFileName: source.mapFileName,
      questRequired: source.questRequired,
      spawnMaps: [...new Set(source.spawnCandidates.map((spawn) => spawn.mapFileName))],
    })),
    [{
      tableKey: "profile:platinum_176",
      monsterName: "Skeleton",
      mapFileName: "D001",
      questRequired: true,
      spawnMaps: ["D001"],
    }],
  );
});

test("q48-q60 band repairs only the disabled q57 prerequisite and has real sources", () => {
  const band = route.quests.filter((quest) => quest.questId >= 48 && quest.questId <= 60);
  assert.deepEqual(
    band.map((quest) => quest.questId),
    [48, 49, 50, 51, 52, 53, 54, 55, 56, 58, 59, 60],
    "the disabled level-255 q57 template must not be presented as playable content",
  );
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q58 = band.find((quest) => quest.questId === 58);
  assert.equal(q58.eligibility.importedRequiredQuestId, 57);
  assert.equal(q58.eligibility.requiredQuestId, 0);
  assert.equal(q58.eligibility.prerequisiteOverride.requiredQuestId, 0);
  assert.match(q58.eligibility.prerequisiteOverride.sourceNote, /disabled level-255 Template/);

  const q60 = band.find((quest) => quest.questId === 60);
  assert.deepEqual(q60.objectives.kill.map((task) => task.monsterName), ["SpiderFrog"]);
  assert.ok(q60.objectives.kill[0].spawnCandidates.some(
    (spawn) => spawn.mapFileName === "D2041",
  ));
});

test("q61-q73 band uses the real D2042 insect-cave route and imported drops", () => {
  const band = route.quests.filter((quest) => quest.questId >= 61 && quest.questId <= 73);
  assert.deepEqual(
    band.map((quest) => quest.questId),
    [61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73],
  );
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q62 = band.find((quest) => quest.questId === 62);
  assert.deepEqual(
    q62.objectives.kill.map((task) => task.monsterName),
    ["KekTal", "VioletKekTal"],
  );
  assert.ok(q62.objectives.kill.every((task) => task.spawnCandidates.some(
    (spawn) => spawn.mapFileName === "D2042",
  )));

  const expectedQuestDrops = new Map([
    [67, ["BugBlood"]],
    [70, ["Antidote"]],
    [71, ["GatheringGlove", "GatheringTool", "GreenHerb"]],
  ]);
  for (const [questId, itemNames] of expectedQuestDrops) {
    const quest = band.find((entry) => entry.questId === questId);
    assert.deepEqual(quest.objectives.item.map((objective) => objective.itemName), itemNames);
    assert.ok(quest.objectives.item.every((objective) => objective.sources.some((source) =>
      source.questRequired &&
      source.spawnCandidates.some((spawn) => spawn.mapFileName === "D2042")
    )));
  }

  for (const questId of [69, 70, 71, 72]) {
    const quest = band.find((entry) => entry.questId === questId);
    assert.ok(
      quest.startNpc?.mapFileName === "D2042" || quest.finishNpc?.mapFileName === "D2042",
      `q${questId} should keep its imported D2042 NPC route`,
    );
  }
});

test("q75-q86 band audits missing Serpent Mine content and keeps normal travel", () => {
  const band = route.quests.filter((quest) => quest.questId >= 75 && quest.questId <= 86);
  assert.deepEqual(
    band.map((quest) => quest.questId),
    [75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86],
  );
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q75 = band.find((quest) => quest.questId === 75);
  assert.deepEqual(q75.objectives.kill.map((task) => task.monsterName), ["ChainGhoul"]);
  assert.ok(q75.objectives.kill[0].spawnCandidates.some((spawn) =>
    spawn.mapFileName === "D421" &&
    spawn.profileRespawn === true &&
    spawn.sourceQuestId === 75 &&
    /no ChainGhoul respawn/.test(spawn.sourceNote)
  ));

  const q78 = band.find((quest) => quest.questId === 78);
  const stolenGold = q78.objectives.item.find((objective) => objective.itemName === "StolenGold");
  assert.ok(stolenGold.sources.some((source) =>
    source.tableKey === "profile:platinum_176" &&
    source.monsterName === "RotNdZombie" &&
    source.mapFileName === "D422" &&
    source.questRequired === true &&
    source.spawnCandidates.some((spawn) =>
      spawn.profileRespawn === true && spawn.sourceQuestId === 78
    )
  ));

  const q82 = band.find((quest) => quest.questId === 82);
  assert.equal(q82.startNpc?.mapFileName, "WhiteVillage");
  assert.ok(q82.objectives.kill[0].spawnCandidates.some(
    (spawn) => spawn.monsterName === "WhiteSerpent" && spawn.mapFileName === "D422",
  ));

  const q85 = band.find((quest) => quest.questId === 85);
  assert.ok(q85.objectives.item[0].sources.some((source) =>
    source.monsterName === "CursedPriest" &&
    source.spawnCandidates.some((spawn) => spawn.mapFileName === "D2031")
  ));
  assert.ok(sources.contentProfile.itemWhitelist.includes("RedThread"));
  assert.ok(sources.contentProfile.itemWhitelist.includes("BlackThread"));
});

test("q87-q100 band keeps imported undead and Wooma hunts with one audited axe repair", () => {
  const band = route.quests.filter((quest) => quest.questId >= 87 && quest.questId <= 100);
  assert.deepEqual(
    band.map((quest) => quest.questId),
    [87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100],
  );
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q89 = band.find((quest) => quest.questId === 89);
  assert.deepEqual(
    q89.objectives.kill.map((task) => task.monsterName),
    ["CursedPriest", "ShiZombie", "CursedZombie"],
  );
  assert.ok(q89.objectives.kill.every((task) => task.spawnCandidates.some(
    (spawn) => spawn.mapFileName === "D2031",
  )));

  const q91 = band.find((quest) => quest.questId === 91);
  const wornAxe = q91.objectives.item.find((objective) => objective.itemName === "WornAxe");
  assert.ok(wornAxe.sources.some((source) =>
    source.tableKey === "profile:platinum_176" &&
    source.monsterName === "BloodyLureSpider" &&
    source.mapFileName === "12" &&
    source.questRequired === true &&
    /q91/.test(source.sourceNote) &&
    source.spawnCandidates.some((spawn) => spawn.mapFileName === "12")
  ));

  const q98 = band.find((quest) => quest.questId === 98);
  assert.ok(q98.objectives.kill.find((task) => task.monsterName === "Dung")
    .spawnCandidates.some((spawn) => spawn.mapFileName === "D022"));
});

test("q108-q112 uses the physical Sabuk secret gate, imported books, and visible pillars", () => {
  const band = route.quests.filter((quest) => quest.questId >= 108 && quest.questId <= 112);
  assert.deepEqual(band.map((quest) => quest.questId), [108, 109, 110, 111, 112]);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  for (const questId of [108, 109]) {
    assert.ok(band.find((quest) => quest.questId === questId).objectives.item.every(
      (objective) => objective.sources.some((source) =>
        source.monsterName === "Zombie51" &&
        source.questRequired === true &&
        source.spawnCandidates.some((spawn) => spawn.mapFileName === "D701")
      ),
    ));
  }

  const q111 = band.find((quest) => quest.questId === 111);
  assert.deepEqual(q111.objectives.flag.map((objective) => objective.number), [521, 522, 523]);
  assert.ok(q111.objectives.flag.every((objective) =>
    objective.setters.some((setter) => setter.npc.mapFileName === "D701")
  ));

  const q112 = band.find((quest) => quest.questId === 112);
  assert.ok(q112.objectives.kill[0].spawnCandidates.some(
    (spawn) => spawn.monsterName === "CrawlerZombie" && spawn.mapFileName === "D701",
  ));
});

test("q118-q124 uses imported Mineral Mine and Prajna Stone Cave progression", () => {
  const band = route.quests.filter((quest) => quest.questId >= 118 && quest.questId <= 124);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q118 = band.find((quest) => quest.questId === 118);
  assert.ok(q118.objectives.kill.find((task) => task.monsterName === "HungryZombie")
    .spawnCandidates.some((spawn) => spawn.mapFileName === "D2031"));

  const q121 = band.find((quest) => quest.questId === 121);
  assert.deepEqual(q121.objectives.kill.map((task) => task.monsterName), ["ToxicGhoul", "RoninGhoul"]);
  assert.ok(q121.objectives.kill.every((task) =>
    task.spawnCandidates.some((spawn) => spawn.mapFileName === "5")
  ));

  for (const questId of [122, 123]) {
    assert.ok(band.find((quest) => quest.questId === questId).objectives.kill.every(
      (task) => task.spawnCandidates.some((spawn) => spawn.mapFileName === "D2062")
    ));
  }
  assert.ok(band.find((quest) => quest.questId === 124).objectives.item[0].sources.some(
    (source) => source.spawnCandidates.some((spawn) => spawn.mapFileName === "D2062")
  ));
});

test("q125-q133 keeps visible village interiors and physical Prajna Temple floors", () => {
  const band = route.quests.filter((quest) => quest.questId >= 125 && quest.questId <= 133);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );
  for (const questId of [125, 126, 128, 132, 133]) {
    const quest = band.find((candidate) => candidate.questId === questId);
    assert.ok(
      quest.startNpc?.mapFileName === "B354" || quest.finishNpc?.mapFileName === "B354",
      `q${questId} should use the physical VillageChiefHouse`,
    );
  }
  const q129 = band.find((quest) => quest.questId === 129);
  assert.equal(q129.startNpc.mapFileName, "1006");
  assert.ok(q129.objectives.item[0].sources.some((source) =>
    source.monsterName === "SpiderBat" &&
    source.spawnCandidates.some((spawn) => spawn.mapFileName === "5")
  ));
  const q131 = band.find((quest) => quest.questId === 131);
  assert.ok(q131.objectives.kill[0].spawnCandidates.some(
    (spawn) => spawn.monsterName === "Minotaur" && spawn.mapFileName === "D2073",
  ));
  const q132 = band.find((quest) => quest.questId === 132);
  assert.ok(q132.objectives.kill.every((task) =>
    task.spawnCandidates.some((spawn) => spawn.mapFileName === "D2074")
  ));
  const q133 = band.find((quest) => quest.questId === 133);
  assert.ok(q133.objectives.kill.every((task) =>
    task.spawnCandidates.some((spawn) => spawn.mapFileName === "D2075")
  ));
});

test("q135-q137 unlocks Ancient Stone Tomb through the visible StoneHeart gate", () => {
  const band = route.quests.filter((quest) => quest.questId >= 135 && quest.questId <= 137);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );
  const q135 = band.find((quest) => quest.questId === 135);
  assert.ok(q135.rewards.fixedItems.some((reward) =>
    reward.itemName === "StoneHeart" &&
    reward.count === 1 &&
    reward.profileOverride === true &&
    /MysteriousStone/.test(reward.sourceNote)
  ));
  const ancientGate = graph.edges.find((edge) =>
    edge.kind === "npc-script" &&
    edge.fromMapFileName === "D715" &&
    edge.toMapFileName === "D710A"
  );
  assert.ok(ancientGate);
  assert.deepEqual(ancientGate.targetSequence, ["@stonetomba"]);
  assert.deepEqual(ancientGate.requiredItems, [{ item: "StoneHeart", count: 1 }]);
  assert.deepEqual(ancientGate.itemCosts, [{ item: "StoneHeart", count: 1 }]);
  for (const questId of [136, 137]) {
    assert.ok(band.find((quest) => quest.questId === questId).objectives.item[0].sources.some(
      (source) => source.spawnCandidates.some((spawn) =>
        ["D711A", "D712A", "D713A"].includes(spawn.mapFileName)
      )
    ));
  }
});

test("q138-q139 reaches Red Cavern through the complete physical dungeon chain", () => {
  const band = route.quests.filter((quest) => quest.questId >= 138 && quest.questId <= 139);
  assert.deepEqual(band.map((quest) => quest.questId), [138, 139]);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  const q138 = band.find((quest) => quest.questId === 138);
  assert.deepEqual(
    q138.objectives.kill.map((task) => task.monsterName),
    ["GhastlyLeecher", "CyanoGhast", "MutatedManworm", "CrazyManworm"],
  );
  assert.ok(q138.objectives.kill.every((task) =>
    task.spawnCandidates.some((spawn) => ["R01", "R02"].includes(spawn.mapFileName))
  ));

  const q139 = band.find((quest) => quest.questId === 139);
  assert.ok(q139.objectives.kill[0].spawnCandidates.some((spawn) =>
    spawn.monsterName === "DreamDevourer" && spawn.mapFileName === "RCK"
  ));
  const bossRoute = findMapTravelRoute(graph, "HELL00", "RCK");
  assert.ok(bossRoute);
  assert.ok(bossRoute.every((edge) => edge.kind === "map-movement"));
  assert.deepEqual(
    bossRoute.map((edge) => edge.toMapFileName),
    ["R01", "R02", "R03", "R04", "R05", "R06", "R07", "R08", "R09", "R10", "R11", "R12", "RCK"],
  );
});

test("q143-q151 keeps the Holy Sword chain on visible scripts, real fields, and physical portals", () => {
  const band = route.quests.filter((quest) => quest.questId >= 143 && quest.questId <= 151);
  assert.deepEqual(
    band.flatMap((quest) => quest.blockers.map((blocker) => `q${quest.questId}: ${blocker}`)),
    [],
  );

  for (const questId of [146, 147]) {
    assert.ok(band.find((quest) => quest.questId === questId).objectives.flag.every((objective) =>
      objective.setters.some((setter) =>
        setter.npc.mapFileName === "D10061" &&
        setter.npc.scriptKey === "WoomyonWoods/TaoistVillage/BigTaoist"
      )
    ));
  }

  const q148 = band.find((quest) => quest.questId === 148);
  const chipSource = q148.objectives.item[0].sources.find((source) =>
    source.profileOverride === true &&
    source.monsterName === "RedEvilApe" &&
    source.mapFileName === "D10053"
  );
  assert.ok(chipSource);
  assert.equal(chipSource.questRequired, true);
  assert.match(chipSource.sourceNote, /q148.*RedEvilApe.*RedMoonEvil1/);
  assert.ok(chipSource.spawnCandidates.some((spawn) => spawn.mapFileName === "D10053"));

  const q149 = band.find((quest) => quest.questId === 149);
  assert.ok(q149.objectives.item[0].sources.some((source) =>
    source.monsterName === "RedEvilApe" &&
    source.questRequired === true &&
    source.spawnCandidates.some((spawn) => spawn.mapFileName === "D10053")
  ));

  const q151 = band.find((quest) => quest.questId === 151);
  assert.deepEqual(
    q151.objectives.carry.map((task) => [task.itemName, task.count]),
    [["EvilApeOil", 2], ["EvilApeHeart", 2]],
  );
  assert.ok(q151.objectives.kill[0].spawnCandidates.some((spawn) =>
    spawn.monsterName === "RedMoonEvil" && spawn.mapFileName === "D10062"
  ));
  for (const [from, to] of [["D10051", "D10061"], ["D10052", "D10062"]]) {
    const physicalRoute = findMapTravelRoute(graph, from, to);
    assert.ok(physicalRoute);
    assert.ok(physicalRoute.every((edge) => edge.kind === "map-movement"));
  }
});

test("navigation and grinding stay inside the active runtime profile", () => {
  const allowedMaps = new Set(
    sources.contentProfile.mapWhitelist.map((entry) => String(entry.fileName)),
  );
  const allowedMonsters = new Set(
    sources.contentProfile.monsterWhitelist.map((name) => String(name).toLowerCase()),
  );
  assert.ok(graph.nodes.length > 0);
  assert.ok(graph.nodes.every((node) => allowedMaps.has(node.mapFileName)));
  assert.ok(graph.edges.every((edge) =>
    allowedMaps.has(edge.fromMapFileName) && allowedMaps.has(edge.toMapFileName)
  ));
  assert.ok(grindingCatalog.every((entry) =>
    allowedMonsters.has(entry.monsterName.toLowerCase()) &&
    entry.spawns.every((spawn) => allowedMaps.has(spawn.mapFileName))
  ));

  const reachable = new Set(["0"]);
  for (let changed = true; changed;) {
    changed = false;
    for (const edge of graph.edges) {
      if (!reachable.has(edge.fromMapFileName) || reachable.has(edge.toMapFileName)) continue;
      reachable.add(edge.toMapFileName);
      changed = true;
    }
  }
  assert.equal(reachable.size, graph.nodes.length, "every enabled map must be reachable from Bichon");
});

test("level gaps produce a real-spawn grind goal without privileged spawning", () => {
  const goal = chooseGrindingGoal(snapshot([], { playerLevel: 12 }), grindingCatalog, 50);
  assert.equal(goal.kind, "grind");
  assert.equal(goal.monsterName, "RakingCat");
  assert.ok(goal.monsterLevel >= 8 && goal.monsterLevel <= 13);
  assert.ok(goal.fields.length > 0);
  assert.ok(goal.fields.every((field) => field.mapFileName === goal.targetMapFileName));
});

test("a nearby sustainable grind source beats a marginally more efficient remote field", () => {
  const state = snapshot([], {
    playerLevel: 13,
    player: { x: 305, y: 609 },
  });
  const goal = chooseGrindingGoal(state, grindingCatalog, 14, {
    certifiedMonsterNames: ["RakingCat", "SpittingSpider"],
  });
  assert.equal(goal.monsterName, "RakingCat");
  assert.ok(
    Math.max(
      Math.abs(goal.fields[0].x - state.player.x),
      Math.abs(goal.fields[0].y - state.player.y),
    ) < 100,
  );
});

test("unhandled flag tasks stay explicit instead of being skipped or mutated", () => {
  const flagged = route.quests.find((quest) => quest.objectives.flag.length > 0);
  assert.ok(flagged);
  const goal = planNextAuthoritativeQuest(snapshot([
    q(flagged.questId, "inProgress"),
  ], { playerLevel: flagged.eligibility.minLevel }), route, {
    minQuestId: flagged.questId,
    maxQuestId: flagged.questId,
    handledBlockers: ["flag task requires an explicit visible-script handler"],
  });
  assert.equal(goal.kind, "special-script");
  assert.equal(goal.questId, flagged.questId);
});
