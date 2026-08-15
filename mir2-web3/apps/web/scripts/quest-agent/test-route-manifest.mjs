import assert from "node:assert/strict";
import test from "node:test";

import {
  QUEST_CLASS_MASKS,
  buildAuthoritativeClassQuestRoute,
  buildProgressionSkillBookCatalog,
  buildSafeSupplyLootCatalog,
  decodeCrystalQuestHeader,
  loadCrystalQuestRouteSources,
} from "./route-manifest.mjs";

const sources = await loadCrystalQuestRouteSources();
const route = await buildAuthoritativeClassQuestRoute({ className: "Warrior", maxLevel: 50 });
const quest = (questId) => {
  const value = route.quests.find((candidate) => candidate.questId === questId);
  assert.ok(value, `expected q${questId} in Warrior route`);
  return value;
};

test("decodes authoritative Crystal ClientQuestInfo headers", () => {
  const q1 = sources.questManifest.quests.find((candidate) => candidate.index === 1);
  const q7 = sources.questManifest.quests.find((candidate) => candidate.index === 7);
  assert.ok(q1);
  assert.ok(q7);
  assert.deepEqual(
    pick(decodeCrystalQuestHeader(q1.payload_hex), ["index", "npcIndex", "name", "minLevelNeeded"]),
    { index: 1, npcIndex: 3, name: "Assistant's Request", minLevelNeeded: 1 },
  );
  assert.equal(decodeCrystalQuestHeader(q7.payload_hex).classNeeded, QUEST_CLASS_MASKS.Warrior);
});

test("resolves quest NPC object IDs to real map positions", () => {
  assert.deepEqual(
    pick(quest(4).finishNpc, ["objectId", "name", "mapFileName", "position"]),
    { objectId: 6, name: "Merchant_John", mapFileName: "0", position: { x: 292, y: 603 } },
  );
  assert.deepEqual(
    pick(quest(7).finishNpc, ["objectId", "name", "mapFileName", "position"]),
    { objectId: 10, name: "Master_Wa", mapFileName: "0", position: { x: 110, y: 317 } },
  );
});

test("binds kill tasks to real respawns", () => {
  const q5 = quest(5);
  assert.deepEqual(q5.objectives.kill.map((objective) => objective.monsterName), ["Deer", "Scarecrow"]);
  for (const objective of q5.objectives.kill) {
    assert.ok(objective.spawnCandidates.length > 0, `${objective.monsterName} needs a respawn`);
    assert.ok(objective.spawnCandidates.every((spawn) => Number.isFinite(spawn.position.x)));
  }
});

test("distinguishes ordinary Q-drops from harvest Q-drops", () => {
  const gingerTea = route.quests
    .flatMap((entry) => entry.objectives.item)
    .find((objective) => objective.itemName === "GingerTea");
  const deerMeat = quest(4).objectives.item.find((objective) => objective.itemName === "DeerMeat");
  assert.ok(gingerTea?.sources.length > 0);
  assert.ok(deerMeat?.sources.length > 0);
  assert.ok(gingerTea.sources.some((source) => !source.requiresHarvest));
  assert.ok(deerMeat.sources.some((source) => source.requiresHarvest));
});

test("marks Crystal HarvestMonster subclasses as corpse-harvest item sources", () => {
  for (const [questId, itemName, monsterName, monsterAi] of [
    [25, "CannibalStem", "CannibalPlant", 5],
    [25, "CannibalLeaf", "CannibalPlant", 5],
    [26, "PoisonSack", "SpittingSpider", 4],
  ]) {
    const objective = quest(questId).objectives.item.find(
      (candidate) => candidate.itemName === itemName,
    );
    assert.ok(objective?.sources.length > 0, `${itemName} needs an item source`);
    assert.ok(objective.sources.some((source) =>
      source.monsterName === monsterName &&
      source.requiresHarvest === true &&
      source.spawnCandidates.some((spawn) => spawn.monsterAi === monsterAi)
    ));
  }
});

test("models zero-index quest endpoints as visible Quest Diary actions", () => {
  const q22 = quest(22);
  assert.equal(q22.startNpc, null);
  assert.equal(q22.finishNpc, null);
  assert.ok(q22.specialHandlers.includes("quest-diary-accept"));
  assert.ok(q22.specialHandlers.includes("quest-diary-finish"));
  assert.equal(q22.blockers.length, 0);
});

test("harvest monster drops can satisfy Crystal item tasks", () => {
  const cannibalLeaf = quest(25).objectives.item.find(
    (objective) => objective.itemName === "CannibalLeaf",
  );
  assert.ok(cannibalLeaf?.sources.length > 0);
  assert.ok(cannibalLeaf.sources.some((source) => source.questRequired === false));
  assert.ok(cannibalLeaf.sources.every((source) => source.requiresHarvest));
  assert.ok(!quest(25).blockers.some((blocker) => blocker.includes("CannibalLeaf")));
});

test("binds q140 GoldChestnut to the imported chestnut-tree fields", () => {
  const goldChestnut = quest(140).objectives.item.find(
    (objective) => objective.itemName === "GoldChestnut",
  );
  assert.ok(goldChestnut?.sources.length > 0);
  assert.ok(goldChestnut.sources.some((source) =>
    ["ChestnutTree", "ChestnutTree1", "ChestnutTree2"].includes(source.monsterName) &&
    source.spawnCandidates.some((spawn) => ["0", "1", "11"].includes(spawn.mapFileName))
  ));
  assert.ok(!quest(140).blockers.some((blocker) => blocker.includes("GoldChestnut")));
});

test("safe supply loot is derived from ordinary drops, class stats, and merchant trade lists", () => {
  const merchants = [
    { merchantKey: "blacksmith", scriptKey: "BichonProvince/BorderVillage/Blacksmith" },
    { merchantKey: "necklace", scriptKey: "BichonProvince/BorderVillage/Necklace" },
    { merchantKey: "ring", scriptKey: "BichonProvince/BorderVillage/Ring" },
    {
      merchantKey: "meat",
      scriptKey: "BichonProvince/BorderVillage/Butcher",
      allowStatless: true,
    },
  ];
  const warrior = buildSafeSupplyLootCatalog(sources, {
    className: "Warrior",
    dropTableKeys: ["Provinces/Scarecrow", "Provinces/Deer"],
    merchants,
  });
  const wizard = buildSafeSupplyLootCatalog(sources, {
    className: "Wizard",
    dropTableKeys: ["Provinces/Scarecrow", "Provinces/Deer"],
    merchants,
  });
  assert.ok(warrior.some((entry) =>
    entry.name === "HexagonalRing" && entry.merchantKey === "ring"
  ));
  assert.ok(!warrior.some((entry) => entry.name === "CopperRing"));
  assert.ok(wizard.some((entry) => entry.name === "CopperRing"));
  assert.ok(!wizard.some((entry) => entry.name === "HexagonalRing"));
  assert.ok(warrior.some((entry) =>
    entry.name === "Venison" && entry.merchantKey === "meat"
  ));
  assert.ok(!warrior.some((entry) => entry.name === "GingerTea"));
});

test("progression skill books are class-compatible and level-bounded Crystal items", () => {
  const warrior = buildProgressionSkillBookCatalog(sources, {
    className: "Warrior",
    maxLevel: 20,
  });
  const wizard = buildProgressionSkillBookCatalog(sources, {
    className: "Wizard",
    maxLevel: 20,
  });
  const taoist = buildProgressionSkillBookCatalog(sources, {
    className: "Taoist",
    maxLevel: 20,
  });
  assert.ok(warrior.some((entry) => entry.name === "Fencing" && entry.minLevel === 7));
  assert.ok(!warrior.some((entry) => entry.name === "FireBall"));
  assert.ok(wizard.some((entry) => entry.name === "FireBall" && entry.minLevel === 7));
  assert.ok(!wizard.some((entry) => entry.name === "Healing"));
  assert.ok(taoist.some((entry) => entry.name === "Healing" && entry.minLevel === 7));
  assert.ok(taoist.every((entry) => entry.minLevel <= 20));
});

test("binds flag tasks to visible scripted-world NPC interactions", () => {
  const flagged = route.quests.filter((entry) => entry.objectives.flag.length > 0);
  assert.ok(flagged.length > 0);
  assert.ok(flagged.every((entry) =>
    entry.specialHandlers.includes("flag-script-objective") &&
    entry.objectives.flag.every((objective) => objective.setters.length > 0) &&
    !entry.contentBlockers.some((blocker) => blocker.includes("flag"))
  ));
  assert.ok(flagged.every((entry) =>
    entry.objectives.flag.every((objective) => objective.setters.some((setter) =>
      sources.contentProfile.npcScriptWhitelist.some((scriptKey) =>
        scriptKey.toLowerCase() === setter.scriptKey.toLowerCase()
      )
    )) || entry.runtimeBlockers.some((blocker) => blocker.includes("flag setter"))
  ));
  const ancientOma = quest(153);
  assert.deepEqual(
    ancientOma.objectives.flag.map((objective) => objective.number),
    [533, 534, 535],
  );
  assert.ok(ancientOma.objectives.flag[0].setters.some(
    (setter) => setter.npc.name === "Librarian_Steven" && setter.targetSequence.includes("@information"),
  ));
});

test("summarizes the complete Warrior 1-50 data surface", () => {
  assert.equal(route.schema, "mir2-real-client-quest-route/3");
  assert.equal(route.classMask, QUEST_CLASS_MASKS.Warrior);
  assert.equal(route.segments.length, 4);
  assert.equal(route.segments.reduce((total, segment) => total + segment.questCount, 0), route.routeQuestCount);
  assert.ok(route.capabilityMatrix["kill-objective"] > 0);
  assert.ok(route.capabilityMatrix["cross-map-dialogue"] > 0);
});

test("records current runtime-profile gaps separately from Crystal content gaps", () => {
  assert.equal(route.source.runtimeProfileId, "platinum_176");
  assert.equal(route.source.runtimeProfileVersion, 24);
  assert.deepEqual(quest(22).runtimeBlockers, []);
  assert.deepEqual(quest(27).runtimeBlockers, []);
  assert.deepEqual(quest(27).contentBlockers, []);
  assert.deepEqual(quest(30).runtimeBlockers, []);
  assert.deepEqual(quest(34).runtimeBlockers, []);
  assert.deepEqual(quest(34).contentBlockers, []);
});

test("builds distinct Warrior, Wizard, and Taoist instructor branches", async () => {
  const routes = await Promise.all(
    ["Warrior", "Wizard", "Taoist"].map((className) =>
      buildAuthoritativeClassQuestRoute({ className, maxLevel: 50 })
    ),
  );
  const branchIds = routes.map((classRoute) => classRoute.quests
    .filter((entry) => entry.questId >= 7 && entry.questId <= 15)
    .map((entry) => entry.questId));
  assert.deepEqual(branchIds, [[7, 8, 9], [10, 11, 12], [13, 14, 15]]);
  assert.ok(routes.every((classRoute) => classRoute.routeQuestCount === 140));
});

function pick(value, keys) {
  return Object.fromEntries(keys.map((key) => [key, value[key]]));
}
