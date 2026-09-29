import assert from "node:assert/strict";
import test from "node:test";
import { buildClassQuestRoute, loadCrystalQuestRouteSources } from "./route-manifest.mjs";
import { annotateNewcomerRoute, loadNewcomerGuidance, validateNewcomerGuidance } from "./newcomer-guidance.mjs";

const sources = await loadCrystalQuestRouteSources();
const config = await loadNewcomerGuidance();
const routes = ["Warrior", "Wizard", "Taoist"].map((className) => buildClassQuestRoute(sources, { className, maxLevel: config.maxLevel }));

test("guidance covers the real three-class quest union and keeps recommended prerequisites", () => {
  const entries = validateNewcomerGuidance(config, routes);
  assert.equal(entries.size, 144);
  assert.equal(entries.get(154).category, "deferred");
  assert.equal(entries.get(152).category, "challenge");
  assert.equal(entries.get(153).category, "challenge");
  for (const id of [117, 118, 119, 125, 126, 127, 128, 134]) {
    assert.equal(entries.get(id).category, "recommended");
  }
});

test("annotation preserves every class, reward, objective, eligibility and original order", () => {
  for (const route of routes) {
    const before = JSON.stringify(route);
    const annotated = annotateNewcomerRoute(route, config);
    assert.equal(annotated.quests.length, 138);
    assert.deepEqual(annotated.quests.map(({ guidance, ...quest }) => quest), route.quests);
    assert.equal(JSON.stringify(route), before);
    assert.ok(annotated.quests.every((quest) => quest.guidance));
  }
});

test("level-15 guidance stays complete and later reverse-ID prerequisites are ordered", () => {
  for (const route of routes) {
    const early = buildClassQuestRoute(sources, {className: route.className, maxLevel: 15});
    assert.equal(annotateNewcomerRoute(early, config).quests.filter((q) => q.guidance).length, 42);
  }
  const order = (id) => config.quests.find((q) => q.id === id).order;
  assert.ok(order(110) < order(111) && order(111) < order(108) && order(108) < order(109));
  for (const chain of [
    [114, 115, 116],
    [117, 118, 119],
    [121, 122, 123],
    [125, 126, 127, 128],
    [132, 133],
    [135, 136],
    [143, 144, 145, 146, 147, 148, 149, 150, 151],
  ]) {
    for (let i = 1; i < chain.length; i++) assert.ok(order(chain[i - 1]) < order(chain[i]));
  }
  assert.equal(config.quests.find((q) => q.id === 113).category, "recommended");
  assert.equal(config.quests.find((q) => q.id === 137).category, "optional");
});

test("invalid, stale and prerequisite-breaking configuration is rejected", () => {
  for (const change of [
    (c) => c.quests.push(c.quests[0]),
    (c) => c.quests.pop(),
    (c) => { c.quests[0].category = "unknown"; },
    (c) => { c.quests.find((q) => q.id === 35).order = 1; },
    (c) => { c.quests.find((q) => q.id === 27).category = "optional"; },
  ]) {
    const invalid = structuredClone(config);
    change(invalid);
    assert.throws(() => validateNewcomerGuidance(invalid, routes));
  }
});

test("instructor tips distinguish passive, offensive and healing skills", () => {
  const tip = (id) => config.quests.find((q) => q.id === id).hint;
  assert.match(tip(9), /Fencing.*passive/);
  assert.match(tip(12), /FireBall.*MP potions/);
  assert.match(tip(15), /Healing.*restore health/);
});

test("specific monster and empty-reward tips still match authoritative content", () => {
  for (const [classIndex, questId] of [[0, 8], [1, 11], [2, 14]]) {
    const quest = routes[classIndex].quests.find((q) => q.questId === questId);
    assert.deepEqual(quest.objectives.kill.map((goal) => goal.monsterName).sort(), ["Oma", "RakingCat"]);
  }
  for (const [questId, itemName] of [[37, "BrokenSword"], [41, "StrongLeatherShoes"], [62, "BronzeStrap"], [112, "ExorcistRing"]]) {
    const rewards = routes[0].quests.find((q) => q.questId === questId).rewards;
    assert.ok(rewards.gold > 0 && rewards.experience > 0);
    assert.ok(rewards.fixedItems.some((item) => item.itemName === itemName && item.count === 0));
    assert.ok(rewards.fixedItems.every((item) => item.count === 0));
    assert.equal(rewards.selectableItems.length, 0);
  }
});
