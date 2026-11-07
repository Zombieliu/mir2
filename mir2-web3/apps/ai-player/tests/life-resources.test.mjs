import assert from "node:assert/strict";
import test from "node:test";
import { projectLifeResources } from "../src/life/resources.mjs";

const potion = (uniqueId = 1n, changes = {}) => ({ uniqueId, name: "(HP)DrugSmall", container: "bag1", quantity: 2, ...changes });
const world = (changes = {}) => ({ ready: true, inventoryStale: false, playerHp: 18, playerMaxHp: 18, gold: 85,
  freeBagSlots: 40, currentWeight: 5, maxWeight: 100, inventoryItems: [potion()], beltItems: [potion(2n, { container: "belt", quantity: 1 })], ...changes });

test("resource view counts actual owned HP potions and preserves authoritative scalar values", () => {
  assert.deepEqual(projectLifeResources(world(), { observedAtMs: 1000 }), { observedAtMs: 1000, known: true, hp: 18, maxHp: 18, gold: 85,
    hpPotionCount: 3, usableHpPotionCount: 3, freeBagSlots: 40, currentWeight: 5, maxWeight: 100, inventoryCurrent: true, medicineUnresolved: false });
});

test("unknown or stale inventory is never reported as zero potions or empty capacity", () => {
  const value = projectLifeResources(world({ inventoryStale: true }), { observedAtMs: 1 });
  assert.equal(value.known, false); assert.equal(value.inventoryCurrent, false);
  for (const field of ["hpPotionCount", "usableHpPotionCount", "freeBagSlots", "currentWeight", "maxWeight"]) assert.equal(value[field], null);
  assert.equal(value.hp, 18); assert.equal(value.gold, 85);
  const absent = projectLifeResources({}, { observedAtMs: 1 });
  assert.equal(absent.hpPotionCount, null); assert.equal(absent.gold, null); assert.equal(absent.observedAtMs, null);
});

test("medicine quarantine distinguishes owned quantity from currently usable quantity", () => {
  const filtered = projectLifeResources(world(), { observedAtMs: 1, usableItems: [] });
  assert.equal(filtered.hpPotionCount, 3); assert.equal(filtered.usableHpPotionCount, 0);
  const pending = projectLifeResources(world(), { observedAtMs: 1, medicineUnresolved: true });
  assert.equal(pending.hpPotionCount, 3); assert.equal(pending.usableHpPotionCount, null); assert.equal(pending.known, false);
});

test("invalid, duplicate and unsafe potion quantities remain unknown instead of inflating usable resources", () => {
  for (const inventoryItems of [[potion(1n, { quantity: -1 })], [potion(1n), potion(1n)], [potion(1n, { uniqueId: "1" })], [potion(1n, { quantity: Number.MAX_SAFE_INTEGER })]]) {
    const value = projectLifeResources(world({ inventoryItems, beltItems: [] }), { observedAtMs: 1 });
    assert.equal(value.hpPotionCount, null); assert.equal(value.usableHpPotionCount, null); assert.equal(value.known, false);
  }
});

test("a current empty inventory is known zero while malformed health, balance and capacity are unknown", () => {
  const empty = projectLifeResources(world({ inventoryItems: [], beltItems: [] }), { observedAtMs: 0 });
  assert.equal(empty.hpPotionCount, 0); assert.equal(empty.known, true);
  const invalid = projectLifeResources(world({ playerHp: 19, gold: "85", freeBagSlots: -1, maxWeight: 0 }), { observedAtMs: 1 });
  assert.equal(invalid.hp, null); assert.equal(invalid.gold, null); assert.equal(invalid.freeBagSlots, null); assert.equal(invalid.maxWeight, null);
});
