import assert from "node:assert/strict";
import { combatAction, combatApproachRange, prepareLoadout, startEmergencyHpRecovery, useClassRecovery, useSupplies } from "./protocol-loadout.mjs";

const tests = [];
const test = (name, body) => tests.push([name, body]);

function info({ stats = [], requiredClass = 1, requiredAmount = 0, requiredType = 0, itemType = 0, requiredGender = 3 } = {}) {
  return { stats, requiredClass, requiredAmount, requiredType, itemType, requiredGender };
}
function wireInfo({ stats = [], requiredClass = 1, requiredAmount = 0, requiredType = 0, itemType = 0, requiredGender = 3 } = {}) {
  return { stats, required_class: requiredClass, required_amount: requiredAmount, required_type: requiredType, item_type: itemType, required_gender: requiredGender };
}
function item(name, uniqueId, equipSlot, template, extra = {}) {
  return { name, uniqueId, equipSlot, quantity: 1, container: "bag1", tooltipSource: { info: template, userItem: { addedStats: [] } }, ...extra };
}
function worn(name, uniqueId, slot, template) {
  return { name, uniqueId, slot, tooltipSource: { info: template, userItem: { addedStats: [] } } };
}
function snapshot(className = "Warrior", level = 7) {
  return {
    playerObjectId: 10, playerHp: 100, playerMaxHp: 100, playerMp: 30, playerMaxMp: 30,
    playerCrystalStats: [], inventoryItems: [], beltItems: [], equipmentItems: [], knownSkills: [],
    entities: [{ objectId: 10, name: "Hero", class: className, gender: "Male", level, x: 10, y: 10, direction: "Down", dead: false }],
  };
}
function mockClient(initial, mutate = () => {}) {
  return {
    snapshot: initial, sent: [],
    send(command) { this.sent.push(command); },
    async request(command, packet) {
      this.sent.push(command); mutate(this, command);
      return { packet, payload: { success: true } };
    },
    async wait(predicate, label) {
      if (!predicate()) throw new Error(`Mock did not settle ${label}`);
      return true;
    },
  };
}

test("equips a strictly dominating eligible class upgrade", async () => {
  const state = snapshot("Warrior", 4);
  state.equipmentItems.push(worn("WoodenSword", 1, "weapon", info({ stats: [{ stat: 4, value: 2 }, { stat: 5, value: 4 }] })));
  state.inventoryItems.push(item("IronSword", 2, "weapon", info({ stats: [{ stat: 4, value: 2 }, { stat: 5, value: 6 }], requiredAmount: 4 })));
  const client = mockClient(state, (current, command) => {
    if (command.type === "equipItem") current.snapshot.equipmentItems = [worn("IronSword", 2, "weapon", state.inventoryItems[0].tooltipSource.info)];
  });
  const result = await prepareLoadout(client);
  assert.deepEqual(client.sent[0], { type: "equipItem", grid: "inventory", uniqueId: 2, to: 0 });
  assert.deepEqual(result.equipped, ["IronSword"]);
});

test("equips BronzeShortSword over SharpDagger by expected damage at zero Luck", async () => {
  const state = snapshot("Warrior", 9);
  state.playerCrystalStats = [
    { stat: 4, value: 5 }, { stat: 5, value: 9 }, { stat: 10, value: 6 },
    { stat: 11, value: 16 },
  ];
  state.equipmentItems.push(worn("SharpDagger", 7, "weapon", info({
    stats: [{ stat: 4, value: 4 }, { stat: 5, value: 6 }], requiredClass: 7, requiredAmount: 2,
  })));
  state.inventoryItems.push(item("BronzeShortSword", 2, "weapon", info({
    stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], requiredClass: 7, requiredAmount: 7,
  })));
  const client = mockClient(state, current => {
    current.snapshot.equipmentItems = [worn("BronzeShortSword", 2, "weapon", state.inventoryItems[0].tooltipSource.info)];
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 4).value = 3;
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 5).value = 14;
  });

  const result = await prepareLoadout(client);

  assert.deepEqual(client.sent, [{ type: "equipItem", grid: "inventory", uniqueId: 2, to: 0 }]);
  assert.deepEqual(result.equipped, ["BronzeShortSword"]);
});

test("Taoist without a learned offensive skill equips a held DC upgrade for melee", async () => {
  const state = snapshot("Taoist", 22);
  state.knownSkills.push({ name: "Healing", spell: "Healing", offensive: false, cooldownRemainingTicks: 0, mpCost: 3 });
  state.playerCrystalStats = [
    { stat: 4, value: 5 }, { stat: 5, value: 7 }, { stat: 8, value: 3 },
    { stat: 9, value: 3 }, { stat: 15, value: 0 },
  ];
  state.equipmentItems.push(worn("WoodenSword", 1, "weapon", info({
    stats: [{ stat: 4, value: 2 }, { stat: 5, value: 4 }], requiredClass: 7, requiredAmount: 1,
  })));
  state.inventoryItems.push(item("SolidBronzeAxe", 42, "weapon", info({
    stats: [{ stat: 5, value: 15 }], requiredClass: 7, requiredAmount: 13,
  })));
  const client = mockClient(state, current => {
    current.snapshot.equipmentItems = [worn("SolidBronzeAxe", 42, "weapon", state.inventoryItems[0].tooltipSource.info)];
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 4).value = 3;
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 5).value = 18;
  });

  const result = await prepareLoadout(client);

  assert.deepEqual(client.sent, [{ type: "equipItem", grid: "inventory", uniqueId: 42, to: 0 }]);
  assert.deepEqual(result.equipped, ["SolidBronzeAxe"]);
});

test("Taoist with a learned offensive skill keeps an SC-neutral weapon tradeoff", async () => {
  const state = snapshot("Taoist", 22);
  state.knownSkills.push({ name: "Soul Fire Ball", spell: "SoulFireBall", offensive: true, cooldownRemainingTicks: 0, mpCost: 4 });
  state.playerCrystalStats = [
    { stat: 4, value: 5 }, { stat: 5, value: 7 }, { stat: 8, value: 3 },
    { stat: 9, value: 3 }, { stat: 15, value: 0 },
  ];
  state.equipmentItems.push(worn("WoodenSword", 1, "weapon", info({
    stats: [{ stat: 4, value: 2 }, { stat: 5, value: 4 }], requiredClass: 7, requiredAmount: 1,
  })));
  state.inventoryItems.push(item("SolidBronzeAxe", 42, "weapon", info({
    stats: [{ stat: 5, value: 15 }], requiredClass: 7, requiredAmount: 13,
  })));

  const client = mockClient(state);
  await prepareLoadout(client);

  assert.equal(client.sent.length, 0);
});

test("Wizard may improve held melee damage only while preserving MC", async () => {
  const preserving = snapshot("Wizard", 22);
  preserving.playerCrystalStats = [
    { stat: 4, value: 5 }, { stat: 5, value: 7 }, { stat: 6, value: 3 },
    { stat: 7, value: 3 }, { stat: 15, value: 0 },
  ];
  preserving.equipmentItems.push(worn("WoodenSword", 1, "weapon", info({
    stats: [{ stat: 4, value: 2 }, { stat: 5, value: 4 }], requiredClass: 7, requiredAmount: 1,
  })));
  preserving.inventoryItems.push(item("SolidBronzeAxe", 42, "weapon", info({
    stats: [{ stat: 5, value: 15 }], requiredClass: 7, requiredAmount: 13,
  })));
  const preservingClient = mockClient(preserving, current => {
    current.snapshot.equipmentItems = [worn("SolidBronzeAxe", 42, "weapon", preserving.inventoryItems[0].tooltipSource.info)];
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 4).value = 3;
    current.snapshot.playerCrystalStats.find(entry => entry.stat === 5).value = 18;
  });
  assert.deepEqual((await prepareLoadout(preservingClient)).equipped, ["SolidBronzeAxe"]);

  const losingMc = snapshot("Wizard", 22);
  losingMc.playerCrystalStats = [
    { stat: 4, value: 6 }, { stat: 5, value: 13 }, { stat: 6, value: 4 },
    { stat: 7, value: 5 }, { stat: 15, value: 0 },
  ];
  losingMc.equipmentItems.push(worn("Trident", 1, "weapon", info({
    stats: [{ stat: 4, value: 3 }, { stat: 5, value: 10 }, { stat: 6, value: 1 }, { stat: 7, value: 2 }],
    requiredClass: 7, requiredAmount: 15,
  })));
  losingMc.inventoryItems.push(item("SolidBronzeAxe", 42, "weapon", info({
    stats: [{ stat: 5, value: 15 }], requiredClass: 7, requiredAmount: 13,
  })));

  const losingMcClient = mockClient(losingMc);
  await prepareLoadout(losingMcClient);

  assert.equal(losingMcClient.sent.length, 0);
});

test("expected-damage weapon selection requires zero authoritative Luck and no other regression", async () => {
  for (const { name, stats, luck } of [
    { name: "Lucky uncertainty", stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], luck: 1 },
    { name: "Malformed Luck", stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], luck: 0.5 },
    { name: "String Luck", stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], luck: "0" },
    { name: "Accuracy tradeoff", stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], luck: 0 },
    { name: "Speed tradeoff", stats: [{ stat: 4, value: 2 }, { stat: 5, value: 11 }], luck: 0 },
  ]) {
    const state = snapshot("Warrior", 9);
    state.playerCrystalStats = [{ stat: 4, value: 5 }, { stat: 5, value: 9 }, { stat: 15, value: luck }];
    const wornStats = [{ stat: 4, value: 4 }, { stat: 5, value: 6 }];
    if (name === "Accuracy tradeoff") wornStats.push({ stat: 10, value: 1 });
    if (name === "Speed tradeoff") wornStats.push({ stat: 14, value: 1 });
    state.equipmentItems.push(worn("Current", 1, "weapon", info({ stats: wornStats })));
    state.inventoryItems.push(item(name, 2, "weapon", info({ stats, requiredAmount: 7 })));

    const client = mockClient(state);
    await prepareLoadout(client);
    assert.equal(client.sent.length, 0, name);
  }
});

test("keeps GoldenPendant because PrecisionPendant is an attack-versus-agility tradeoff", async () => {
  const state = snapshot("Warrior", 9);
  state.playerCrystalStats = [{ stat: 4, value: 5 }, { stat: 5, value: 9 }, { stat: 11, value: 16 }, { stat: 15, value: 0 }];
  state.equipmentItems.push(worn("GoldenPendant", 5, "necklace", info({ stats: [{ stat: 11, value: 1 }], requiredClass: 31, requiredAmount: 2 })));
  state.inventoryItems.push(item("PrecisionPendant", 11, "necklace", info({ stats: [{ stat: 5, value: 1 }], requiredClass: 31, requiredAmount: 3 })));

  const client = mockClient(state);
  await prepareLoadout(client);
  assert.equal(client.sent.length, 0);
});

test("equips a left-labelled bracelet into the empty opposite wrist", async () => {
  const state = snapshot("Warrior", 10);
  state.equipmentItems.push(worn("WornIronBracelet", 8, "braceletLeft", info({
    stats: [{ stat: 5, value: 1 }, { stat: 10, value: 1 }], requiredClass: 31, requiredAmount: 3,
  })));
  state.inventoryItems.push(item("SteelBangle", 4, "braceletLeft", info({
    stats: [{ stat: 1, value: 2 }, { stat: 3, value: 1 }], requiredClass: 31, requiredAmount: 8,
  })));
  const client = mockClient(state, (current, command) => {
    if (command.type === "equipItem") {
      current.snapshot.equipmentItems.push(worn(
        "SteelBangle", 4, "braceletRight", state.inventoryItems[0].tooltipSource.info,
      ));
    }
  });

  const result = await prepareLoadout(client);

  assert.deepEqual(client.sent, [{ type: "equipItem", grid: "inventory", uniqueId: 4, to: 6 }]);
  assert.deepEqual(result.equipped, ["SteelBangle"]);
  assert.equal(client.snapshot.equipmentItems.some(entry => entry.uniqueId === 8 && entry.slot === "braceletLeft"), true);
});

test("does not replace either occupied wrist with an incomparable bracelet", async () => {
  const state = snapshot("Warrior", 10);
  state.equipmentItems.push(
    worn("WornIronBracelet", 8, "braceletLeft", info({ stats: [{ stat: 5, value: 1 }, { stat: 10, value: 1 }] })),
    worn("SecondIronBracelet", 9, "braceletRight", info({ stats: [{ stat: 5, value: 1 }, { stat: 10, value: 1 }] })),
  );
  state.inventoryItems.push(item("SteelBangle", 4, "braceletLeft", info({
    stats: [{ stat: 1, value: 2 }, { stat: 3, value: 1 }], requiredClass: 31, requiredAmount: 8,
  })));

  const client = mockClient(state);
  await prepareLoadout(client);

  assert.equal(client.sent.length, 0);
});

test("does not replace gear for a tradeoff, wrong class, excess level, or missing metadata", async () => {
  for (const candidate of [
    item("Tradeoff", 2, "weapon", info({ stats: [{ stat: 4, value: 1 }, { stat: 5, value: 9 }] })),
    item("WizardBlade", 3, "weapon", info({ stats: [{ stat: 5, value: 20 }], requiredClass: 2 })),
    item("Overlevel", 4, "weapon", info({ stats: [{ stat: 5, value: 20 }], requiredAmount: 8 })),
    { name: "Opaque", uniqueId: 5, equipSlot: "weapon" },
  ]) {
    const state = snapshot("Warrior", 7);
    state.equipmentItems.push(worn("Balanced", 1, "weapon", info({ stats: [{ stat: 4, value: 2 }, { stat: 5, value: 4 }] })));
    state.inventoryItems.push(candidate);
    const client = mockClient(state);
    await prepareLoadout(client);
    assert.equal(client.sent.length, 0, candidate.name);
  }
});

test("honours a stat-based item requirement", async () => {
  const state = snapshot("Wizard", 7);
  state.playerCrystalStats = [{ stat: 7, value: 5 }];
  state.inventoryItems.push(item("MC Robe", 7, "armour", info({ stats: [{ stat: 3, value: 2 }], requiredClass: 2, requiredType: 4, requiredAmount: 5 })));
  const client = mockClient(state, current => {
    current.snapshot.equipmentItems.push(worn("MC Robe", 7, "armour", state.inventoryItems[0].tooltipSource.info));
  });
  await prepareLoadout(client);
  assert.equal(client.sent[0].type, "equipItem");
});

test("reads real Gateway snake_case requirements and instance added_stats", async () => {
  const state = snapshot("Wizard", 7);
  state.playerCrystalStats = [{ stat: 7, value: 5 }];
  state.equipmentItems.push({
    name: "Old Robe", uniqueId: 9, slot: "armour",
    tooltipSource: { info: wireInfo({ stats: [{ stat: 3, value: 1 }], requiredClass: 2 }) },
  });
  state.inventoryItems.push({
    name: "Quest Robe", uniqueId: 0, equipSlot: "armour", quantity: 1, container: "bag1",
    tooltipSource: {
      info: wireInfo({ stats: [{ stat: 3, value: 1 }], requiredClass: 2, requiredType: 4, requiredAmount: 5 }),
      userItem: { unique_id: 0, added_stats: [{ stat: 3, value: 2 }] },
    },
  });
  const client = mockClient(state, current => {
    current.snapshot.equipmentItems = [{ ...state.inventoryItems[0], slot: "armour" }];
  });
  await prepareLoadout(client);
  assert.deepEqual(client.sent[0], { type: "equipItem", grid: "inventory", uniqueId: 0, to: 1 });

  const wrongClass = snapshot("Warrior", 7);
  wrongClass.inventoryItems.push({ ...state.inventoryItems[0], uniqueId: 1 });
  const rejected = mockClient(wrongClass);
  await prepareLoadout(rejected);
  assert.equal(rejected.sent.length, 0);
});

test("learns only the class beginner book at its authoritative level", async () => {
  const state = snapshot("Wizard", 7);
  state.inventoryItems.push(item("FireBall", 21, null, wireInfo({ itemType: 20, requiredClass: 2, requiredAmount: 7 })));
  const client = mockClient(state, (current, command) => {
    if (command.type === "useItem") current.snapshot.knownSkills.push({ name: "FireBall", spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 });
  });
  const result = await prepareLoadout(client);
  assert.deepEqual(client.sent[0], { type: "useItem", uniqueId: 21, grid: "inventory" });
  assert.deepEqual(result.learned, ["FireBall"]);

  const tooYoung = snapshot("Wizard", 6);
  tooYoung.inventoryItems = state.inventoryItems;
  const youngClient = mockClient(tooYoung);
  await prepareLoadout(youngClient);
  assert.equal(youngClient.sent.length, 0);
});

test("learns every held eligible approved progression book without inventing levels", async () => {
  const state = snapshot("Wizard", 16);
  state.inventoryItems.push(
    item("FireBall", 21, null, wireInfo({ itemType: 20, requiredClass: 2, requiredAmount: 7 })),
    item("GreatFireBall", 22, null, wireInfo({ itemType: 20, requiredClass: 2, requiredAmount: 16 })),
    item("SoulFireBall", 23, null, wireInfo({ itemType: 20, requiredClass: 4, requiredAmount: 12 })),
  );
  const client = mockClient(state, (current, command) => {
    const held = current.snapshot.inventoryItems.find(entry => Number(entry.uniqueId) === Number(command.uniqueId));
    if (command.type === "useItem" && held) current.snapshot.knownSkills.push({ spell: held.name, level: 0, cooldownRemainingTicks: 0, mpCost: 4 });
  });
  const result = await prepareLoadout(client);
  assert.deepEqual(result.learned, ["FireBall", "GreatFireBall"]);
  assert.deepEqual(client.sent.map(command => command.uniqueId), [21, 22]);
  assert.deepEqual(state.knownSkills.map(skill => [skill.spell, skill.level]), [["FireBall", 0], ["GreatFireBall", 0]]);
});

test("approved later books still obey their authoritative required level", async () => {
  const state = snapshot("Warrior", 14);
  state.knownSkills.push({ spell: "Fencing", level: 0 });
  state.inventoryItems.push(item("Sl Sand", 30, null, wireInfo({ itemType: 20, requiredClass: 1, requiredAmount: 0 })));
  state.inventoryItems.push(item("Slaying", 31, null, wireInfo({ itemType: 20, requiredClass: 1, requiredAmount: 15 })));
  const client = mockClient(state);
  const result = await prepareLoadout(client);
  assert.deepEqual(result.learned, []);
  assert.deepEqual(client.sent, []);
});

test("approved book sets cover the three newcomer class progressions", async () => {
  for (const [className, mask, spells] of [
    ["Warrior", 1, ["Fencing", "Slaying"]],
    ["Wizard", 2, ["FireBall", "GreatFireBall"]],
    ["Taoist", 4, ["Healing", "SpiritSword", "SoulFireBall"]],
  ]) {
    const state = snapshot(className, 30);
    state.inventoryItems = spells.map((spell, index) =>
      item(spell, 100 + index, null, wireInfo({ itemType: 20, requiredClass: mask, requiredAmount: 1 })));
    const client = mockClient(state, (current, command) => {
      const held = current.snapshot.inventoryItems.find(entry => Number(entry.uniqueId) === Number(command.uniqueId));
      current.snapshot.knownSkills.push({ spell: held.name, level: 0, cooldownRemainingTicks: 0, mpCost: 0 });
    });
    const result = await prepareLoadout(client);
    assert.deepEqual(result.learned, spells, className);
    assert.deepEqual(client.sent.map(command => command.type), spells.map(() => "useItem"), className);
  }
});

test("uses held HP and MP drugs only at conservative deficits", async () => {
  const state = snapshot("Warrior", 7);
  state.playerHp = 40; state.playerMp = 5;
  state.beltItems.push({ name: "(HP)DrugSmall", uniqueId: 30, quantity: 2, container: "belt" });
  state.inventoryItems.push({ name: "(MP)DrugSmall", uniqueId: 31, quantity: 1, container: "bag1" });
  const client = mockClient(state, (current, command) => {
    if (command.uniqueId === 30) current.snapshot.playerHp = 70;
    if (command.uniqueId === 31) current.snapshot.playerMp = 25;
  });
  const result = await prepareLoadout(client);
  assert.deepEqual(client.sent, [
    { type: "useItem", uniqueId: 30, grid: "belt" },
    { type: "useItem", uniqueId: 31, grid: "inventory" },
  ]);
  assert.deepEqual(result.consumed, ["(HP)DrugSmall", "(MP)DrugSmall"]);
});

test("useSupplies accepts the real first-item uniqueId zero", async () => {
  const state = snapshot("Warrior", 7);
  state.playerHp = 40;
  state.inventoryItems.push({ name: "(HP)DrugSmall", uniqueId: 0, quantity: 1, container: "bag1" });
  const client = mockClient(state, current => { current.snapshot.playerHp = 70; });
  assert.deepEqual(await useSupplies(client), ["(HP)DrugSmall"]);
  assert.deepEqual(client.sent, [{ type: "useItem", uniqueId: 0, grid: "inventory" }]);
});

test("useSupplies accepts a higher explicit HP recovery threshold", async () => {
  const state = snapshot("Warrior", 14);
  state.playerHp = 80;
  state.playerMaxHp = 100;
  state.inventoryItems.push({ name: "(HP)DrugSmall", uniqueId: 41, quantity: 2, container: "bag1" });
  const client = mockClient(state, current => { current.snapshot.playerHp = 95; });
  assert.deepEqual(await useSupplies(client, { hpThreshold: 0.9, mpThreshold: 0 }), ["(HP)DrugSmall"]);
  assert.deepEqual(client.sent, [{ type: "useItem", uniqueId: 41, grid: "inventory" }]);
});

test("emergency HP recovery sends immediately without awaiting an item acknowledgement", async () => {
  const state = snapshot("Taoist", 12);
  state.playerHp = 39;
  state.playerMaxHp = 68;
  state.beltItems.push({ name: "(HP)DrugSmall", uniqueId: 0, quantity: 2, container: "belt" });
  let requests = 0;
  const client = mockClient(state);
  client.request = async () => { requests += 1; throw new Error("must not wait during retreat"); };

  const result = startEmergencyHpRecovery(client, { hpThreshold: 0.85 });

  assert.equal(requests, 0);
  assert.equal(result[0].pending, true);
  assert.deepEqual(client.sent, [{ type: "useItem", uniqueId: 0, grid: "belt" }]);
});

test("emergency HP recovery waits for the prior gradual potion dose before using another", () => {
  const state = snapshot("Taoist", 12);
  state.playerHp = 39;
  state.playerMaxHp = 68;
  state.beltItems.push({ name: "(HP)DrugSmall", uniqueId: 0, quantity: 2, container: "belt" });
  const client = mockClient(state);
  let now = 1_000;

  assert.equal(startEmergencyHpRecovery(client, { now: () => now }).length, 1);
  now += 3_000;
  assert.deepEqual(startEmergencyHpRecovery(client, { now: () => now }), []);
  now += 3_000;
  assert.equal(startEmergencyHpRecovery(client, { now: () => now }).length, 1);
  assert.deepEqual(client.sent, [
    { type: "useItem", uniqueId: 0, grid: "belt" },
    { type: "useItem", uniqueId: 0, grid: "belt" },
  ]);
});

test("dead players emit no restorative item or Healing commands", async () => {
  const state = snapshot("Taoist", 18);
  state.playerHp = 0;
  state.entities[0].hp = 0;
  state.entities[0].dead = true;
  state.beltItems.push({ name: "(HP)DrugSmall", uniqueId: 77, quantity: 20, container: "belt" });
  state.knownSkills.push({ name: "Healing", spell: "Healing", cooldownRemainingTicks: 0, mpCost: 4 });
  const client = mockClient(state);

  assert.deepEqual(await useSupplies(client), []);
  assert.deepEqual(startEmergencyHpRecovery(client), []);
  assert.equal(await useClassRecovery(client), null);
  assert.deepEqual(client.sent, []);
});

test("a rejected restorative refreshes authoritative state without aborting the journey", async () => {
  const state = snapshot("Taoist", 12);
  state.playerHp = 20;
  state.inventoryItems.push({ name: "(HP)DrugSmall", uniqueId: 44, quantity: 1, container: "bag1" });
  const events = [];
  const client = {
    snapshot: state,
    sequence: 5,
    events,
    sent: [],
    diagnostics: [],
    record(_direction, payload) { this.diagnostics.push(payload); },
    send(command) {
      this.sent.push(command);
      if (command.type === "clientVersion") {
        this.sequence += 1;
        events.push({ sequence: this.sequence, direction: "received", type: "worldSnapshot" });
      }
    },
    async request(command) {
      this.sent.push(command);
      return { payload: { success: false } };
    },
    async wait(predicate, label) {
      if (!predicate()) throw new Error(`Mock did not settle ${label}`);
      return true;
    },
  };

  assert.deepEqual(await useSupplies(client), []);
  assert.deepEqual(client.sent, [
    { type: "useItem", uniqueId: 44, grid: "inventory" },
    { type: "clientVersion" },
  ]);
  assert.equal(client.diagnostics[0].type, "restorativeRejected");
});

test("Wizard casts exact known FireBall at a target in range", async () => {
  const state = snapshot("Wizard", 7);
  state.knownSkills.push({ name: "Fire Ball", spell: "FireBall", offensive: true, castKind: "target", cooldownRemainingTicks: 0, mpCost: 3 });
  const client = mockClient(state);
  const result = await combatAction(client, { objectId: 99, x: 14, y: 8, dead: false });
  assert.equal(result.kind, "magic");
  assert.deepEqual(client.sent[0], { type: "magic", objectId: 10, spell: "FireBall", direction: "UpRight", targetId: 99, x: 14, y: 8, spellTargetLock: true });
});

test("Wizard prefers ready GreatFireBall and falls back while it cools down", async () => {
  const state = snapshot("Wizard", 16);
  state.playerMp = 20;
  state.knownSkills.push(
    { spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 },
    { spell: "GreatFireBall", cooldownRemainingTicks: 0, mpCost: 7 },
  );
  const target = { objectId: 99, x: 14, y: 8, dead: false };
  const ready = mockClient(state);
  const result = await combatAction(ready, target);
  assert.equal(result.spell, "GreatFireBall");
  assert.equal(ready.sent[0].spell, "GreatFireBall");
  assert.equal(combatApproachRange(ready, target), 6);

  state.knownSkills[1].cooldownRemainingTicks = 2;
  const cooling = mockClient(state);
  const fallback = await combatAction(cooling, target);
  assert.equal(fallback.kind, "magic");
  assert.equal(fallback.spell, "FireBall");
  assert.equal(cooling.sent[0].spell, "FireBall");

  state.knownSkills[0].cooldownRemainingTicks = 1;
  const allCooling = mockClient(state);
  assert.deepEqual(await combatAction(allCooling, target), {
    kind: "wait", spell: "GreatFireBall", targetId: 99, delayMs: 650,
  });
  assert.deepEqual(allCooling.sent, []);
});

test("Wizard falls back to affordable FireBall when GreatFireBall lacks MP", async () => {
  const state = snapshot("Wizard", 16);
  state.playerMp = 5;
  state.knownSkills.push(
    { spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 },
    { spell: "GreatFireBall", cooldownRemainingTicks: 0, mpCost: 7 },
  );
  const client = mockClient(state);
  const result = await combatAction(client, { objectId: 99, x: 14, y: 8, dead: false });
  assert.equal(result.spell, "FireBall");
  assert.equal(client.sent[0].spell, "FireBall");
});

test("combat approach keeps a ready Wizard FireBall at a safe ranged distance", () => {
  const state = snapshot("Wizard", 7);
  state.playerMp = 3;
  state.knownSkills.push({ spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 });

  assert.equal(
    combatApproachRange(mockClient(state), { objectId: 99, x: 30, y: 30, dead: false }),
    6,
  );
});

test("Wizard holds spell range and waits without a command while FireBall cools down", async () => {
  const state = snapshot("Wizard", 7);
  state.knownSkills.push({ spell: "FireBall", cooldownRemainingTicks: 1, mpCost: 3 });
  const client = mockClient(state);
  const target = { objectId: 99, x: 14, y: 8, dead: false };

  assert.equal(combatApproachRange(client, target), 6);
  assert.deepEqual(await combatAction(client, target), {
    kind: "wait", spell: "FireBall", targetId: 99, delayMs: 650,
  });
  assert.deepEqual(client.sent, []);
});

test("combat approach closes to melee without enough authoritative MP", () => {
  const state = snapshot("Wizard", 7);
  state.playerMp = 2;
  state.knownSkills.push({ spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 });

  assert.equal(combatApproachRange(mockClient(state), { objectId: 99 }), 1);
});

test("combat approach remains melee for other classes", () => {
  for (const className of ["Warrior", "Taoist"]) {
    const state = snapshot(className, 20);
    state.knownSkills.push({ spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 0 });
    assert.equal(combatApproachRange(mockClient(state), { objectId: 99 }), 1, className);
  }
});

test("Taoist Healing targets self when badly hurt", async () => {
  const state = snapshot("Taoist", 7); state.playerHp = 50;
  state.knownSkills.push({ name: "Healing", spell: "Healing", offensive: false, castKind: "target", cooldownRemainingTicks: 0, mpCost: 3 });
  const client = mockClient(state);
  await combatAction(client, { objectId: 99, x: 11, y: 10, dead: false });
  assert.deepEqual(client.sent[0], { type: "magic", objectId: 10, spell: "Healing", direction: "Down", targetId: 10, x: 10, y: 10, spellTargetLock: true });
});

test("Taoist class recovery can heal during a non-combat interaction", async () => {
  const state = snapshot("Taoist", 12); state.playerHp = 80; state.playerMaxHp = 100;
  state.knownSkills.push({ name: "Healing", spell: "Healing", offensive: false, castKind: "target", cooldownRemainingTicks: 0, mpCost: 3 });
  const client = mockClient(state);

  const result = await useClassRecovery(client, { hpThreshold: 0.85 });

  assert.equal(result.kind, "magic");
  assert.deepEqual(client.sent[0], { type: "magic", objectId: 10, spell: "Healing", direction: "Down", targetId: 10, x: 10, y: 10, spellTargetLock: true });
});

test("class recovery emits no action for Warrior or a Taoist without affordable Healing", async () => {
  const warrior = mockClient(snapshot("Warrior", 12));
  assert.equal(await useClassRecovery(warrior, { hpThreshold: 1 }), null);
  assert.deepEqual(warrior.sent, []);

  const taoistState = snapshot("Taoist", 12); taoistState.playerHp = 50; taoistState.playerMp = 0;
  taoistState.knownSkills.push({ spell: "Healing", cooldownRemainingTicks: 0, mpCost: 3 });
  const taoist = mockClient(taoistState);
  assert.equal(await useClassRecovery(taoist, { hpThreshold: 0.85 }), null);
  assert.deepEqual(taoist.sent, []);
});

test("Taoist uses SoulFireBall only with a real equipped standard Amulet", async () => {
  const state = snapshot("Taoist", 16);
  state.playerMp = 12;
  state.knownSkills.push({ spell: "SoulFireBall", cooldownRemainingTicks: 0, mpCost: 4 });
  state.equipmentItems.push({ name: "Amulet", uniqueId: 0, quantity: 2, slot: "amulet", tooltipSource: { info: { item_index: 712 } } });
  const target = { objectId: 99, x: 14, y: 8, dead: false };
  state.entities.push(target);
  const client = mockClient(state);
  assert.equal(combatApproachRange(client, target), 6);
  const result = await combatAction(client, target);
  assert.equal(result.spell, "SoulFireBall");
  assert.deepEqual(client.sent[0], { type: "magic", objectId: 10, spell: "SoulFireBall", direction: "UpRight", targetId: 99, x: 14, y: 8, spellTargetLock: true });

  state.equipmentItems = [{ name: "GreenPoison", uniqueId: 4, quantity: 9, slot: "amulet", tooltipSource: { info: { item_index: 710 } } }];
  const without = mockClient(state);
  assert.equal(combatApproachRange(without, target), 1);
  assert.equal((await combatAction(without, target)).kind, "attack");
});

test("Taoist reloads a consumed equipped Amulet stack before the next SoulFireBall", async () => {
  const state = snapshot("Taoist", 19);
  state.maxBagSlots = 40;
  state.playerMp = 12;
  state.knownSkills.push({ spell: "SoulFireBall", cooldownRemainingTicks: 0, mpCost: 4 });
  state.beltItems.push({
    name: "Amulet", uniqueId: 7120, quantity: 11, slot: 4, container: "belt",
    tooltipSource: { info: { item_index: 712, requiredClass: 4, requiredType: 0, requiredAmount: 18 } },
  });
  const target = { objectId: 99, kind: "monster", x: 14, y: 8, hp: 185, dead: false };
  state.entities.push(target);
  const client = mockClient(state, (current, command) => {
    if (command.type === "moveItem") {
      const amulet = current.snapshot.beltItems.shift();
      current.snapshot.inventoryItems.push({ ...amulet, container: "bag1", slot: 0 });
    } else if (command.type === "equipItem") {
      const amulet = current.snapshot.inventoryItems.shift();
      current.snapshot.equipmentItems.push({ ...amulet, container: "equipment", slot: "amulet" });
    }
  });

  assert.equal(combatApproachRange(client, target), 6);
  const result = await combatAction(client, target);

  assert.equal(result.spell, "SoulFireBall");
  assert.deepEqual(client.sent, [
    { type: "moveItem", grid: "belt", from: 4, to: 6 },
    { type: "equipItem", grid: "inventory", uniqueId: 7120, to: 9 },
    { type: "magic", objectId: 10, spell: "SoulFireBall", direction: "UpRight", targetId: 99, x: 14, y: 8, spellTargetLock: true },
  ]);
});

test("Warrior passive and unknown skills never produce a magic cast", async () => {
  const state = snapshot("Warrior", 7);
  state.knownSkills.push({ name: "Fencing", spell: "Fencing", offensive: false, cooldownRemainingTicks: 0, mpCost: 0 });
  state.knownSkills.push({ name: "Slaying", spell: "Slaying", offensive: true, cooldownRemainingTicks: 0, mpCost: 0 });
  const client = mockClient(state);
  const target = { objectId: 99, x: 11, y: 10, dead: false };
  const result = await combatAction(client, target);
  assert.equal(result.kind, "attack");
  assert.deepEqual(client.sent[0], { type: "attack", objectId: 99 });
});

test("unavailable or out-of-range FireBall falls back to ordinary attack", async () => {
  for (const change of [
    skill => { skill.mpCost = 40; },
    (_skill, target) => { target.x = 30; },
    skill => { skill.spell = "UnknownSpell"; },
  ]) {
    const state = snapshot("Wizard", 7);
    const skill = { spell: "FireBall", cooldownRemainingTicks: 0, mpCost: 3 };
    const target = { objectId: 99, x: 11, y: 10, dead: false };
    change(skill, target); state.knownSkills.push(skill);
    const client = mockClient(state);
    await combatAction(client, target);
    assert.deepEqual(client.sent[0], { type: "attack", objectId: 99 });
  }
});

let failures = 0;
for (const [name, body] of tests) {
  try { await body(); console.log(`ok - ${name}`); }
  catch (error) { failures += 1; console.error(`not ok - ${name}\n${error.stack}`); }
}
console.log(`${tests.length - failures}/${tests.length} protocol loadout tests passed`);
if (failures) process.exitCode = 1;
