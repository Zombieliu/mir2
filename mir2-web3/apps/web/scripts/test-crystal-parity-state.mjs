import assert from "node:assert/strict";
import test from "node:test";

import {
  buildQaParityExpectation,
  compareQaParityState,
} from "./crystal-parity-state.mjs";

function fixturePayload() {
  return {
    character: { name: "ParityMage", level: 1, class: "Wizard", gender: "Male" },
    mapFileName: "0",
    position: { x: 290, y: 620 },
    direction: "Right",
    hp: 15,
    maxHp: 15,
    mp: 17,
    maxMp: 17,
    experience: 15,
    maxExperience: 100,
    gold: 0,
    inventoryCapacity: 54,
    inventoryItemsJson: [
      JSON.stringify({
        key: "bag-two-item",
        unique_id: 125,
        slot: 7,
        container: "bag2",
        quantity: 1,
        icon: 8,
        durability_current: 3,
        durability_max: 4,
      }),
      JSON.stringify({
        key: "quest-leaf",
        unique_id: 124,
        slot: 0,
        container: "quest",
        quantity: 5,
        icon: 7,
        durability_current: null,
        durability_max: null,
      }),
    ],
    beltItemsJson: [
      JSON.stringify({
        key: "small-hp-drug",
        unique_id: 119,
        slot: 0,
        container: "belt",
        quantity: 1,
        icon: 1,
        durability_current: null,
        durability_max: null,
      }),
    ],
    storageItemsJson: [],
    equipmentItemsJson: [
      JSON.stringify({
        key: "wooden-sword",
        user_item_unique_id: 120,
        slot: "weapon",
        quantity: 1,
        icon: 2,
        shape: 3,
        durability_current: 10,
        durability_max: 10,
      }),
    ],
    questStatesJson: [
      JSON.stringify({
        quest_id: 1,
        title: "Assistant's Request",
        stage: "readyToTurnIn",
        current: 1,
        required: 1,
      }),
    ],
    skillStatesJson: [
      JSON.stringify({
        key: "healing",
        name: "Healing",
        level: 1,
        experience: 7,
        hotkey: 2,
        delay_ms: 900,
        cast_time_ms: 0,
      }),
    ],
    buffStatesJson: [
      JSON.stringify({
        key: "rested",
        name: "Rested",
        attack_bonus: 0,
        defence_bonus: 0,
        stats: [{ stat: 100, value: 5 }],
      }),
    ],
    hair: 8,
    attackMode: 5,
    petMode: 0,
    allowGroup: true,
  };
}

function matchingActual() {
  return {
    screen: "game",
    mapFileName: "0",
    player: { x: 290, y: 620 },
    authoritativePlayer: { x: 290, y: 620, direction: "right" },
    selfPlayer: {
      name: "ParityMage",
      level: 1,
      classKey: "wizard",
      genderKey: "male",
      direction: "right",
    },
    playerHp: 15,
    playerMaxHp: 15,
    playerMp: 17,
    playerMaxMp: 17,
    playerExperience: 15,
    playerMaxExperience: 100,
    gold: 0,
    inventoryCapacity: 54,
    inventoryItems: [
      {
        key: "quest-leaf",
        uniqueId: 124,
        slot: 0,
        container: "quest",
        quantity: 5,
        icon: 7,
        durabilityCurrent: null,
        durabilityMax: null,
      },
      {
        key: "bag-two-item",
        uniqueId: 125,
        slot: 7,
        container: "bag2",
        quantity: 1,
        icon: 8,
        durabilityCurrent: 3,
        durabilityMax: 4,
      },
    ],
    beltItems: [
      {
        key: "small-hp-drug",
        uniqueId: 119,
        slot: 0,
        container: "belt",
        quantity: 1,
        icon: 1,
        durabilityCurrent: null,
        durabilityMax: null,
      },
    ],
    storageItems: [],
    equipmentItems: [
      {
        key: "wooden-sword",
        uniqueId: 120,
        slot: "weapon",
        quantity: 1,
        icon: 2,
        shape: 3,
        durabilityCurrent: 10,
        durabilityMax: 10,
      },
    ],
    quests: [
      {
        questId: 1,
        title: "Assistant's Request",
        stage: "readyToTurnIn",
        current: 1,
        required: 1,
      },
    ],
    skills: [
      {
        key: "healing",
        name: "Healing",
        level: 1,
        experience: 7,
        hotkey: 2,
        delayMs: 900,
        castTimeMs: 0,
      },
    ],
    buffs: [
      {
        key: "rested",
        name: "Rested",
        attackBonus: 0,
        defenceBonus: 0,
        stats: [{ stat: 100, label: "Rested", value: 5 }],
      },
    ],
    stage5: { hair: 8, attackMode: 5, petMode: 0, allowGroup: true },
  };
}

test("exact Crystal parity comparison checks identities, slots, state, and appearance", () => {
  const expected = buildQaParityExpectation(fixturePayload());
  const comparison = compareQaParityState(expected, matchingActual());
  assert.equal(comparison.ok, true, JSON.stringify(comparison.mismatches));
  assert.deepEqual(comparison.mismatches, []);
});

test("same item counts cannot hide a wrong Crystal slot", () => {
  const expected = buildQaParityExpectation(fixturePayload());
  const actual = matchingActual();
  actual.inventoryItems[0].slot = 9;
  const comparison = compareQaParityState(expected, actual);
  assert.equal(comparison.ok, false);
  assert.ok(comparison.mismatches.some((mismatch) => mismatch.path === "inventoryItems"));
});

test("malformed encoded state fails closed before capture", () => {
  const payload = fixturePayload();
  payload.questStatesJson = ["{not-json"];
  assert.throws(() => buildQaParityExpectation(payload), /questStatesJson\[0\] is malformed/);
});
