import assert from "node:assert/strict";
import test from "node:test";

import {
  BinaryReader,
  dotNetDateTimeTicks,
  redactExtractedNativeState,
  readBuff,
  readQuestProgress,
} from "./extract-crystal-account-state.mjs";
import {
  buffStateFromNative,
  candidateBagLocationFromCrystalSlot,
  itemStateFromNative,
  questStatesFromNative,
  skillStateFromNative,
  stage5SystemsFromExisting,
  strictUiStateBlockers,
} from "./upsert-web-account-from-crystal-state.mjs";

class BinaryFixture {
  constructor() {
    this.parts = [];
  }

  u8(value) {
    const buffer = Buffer.alloc(1);
    buffer.writeUInt8(value);
    this.parts.push(buffer);
    return this;
  }

  u32(value) {
    const buffer = Buffer.alloc(4);
    buffer.writeUInt32LE(value);
    this.parts.push(buffer);
    return this;
  }

  i32(value) {
    const buffer = Buffer.alloc(4);
    buffer.writeInt32LE(value);
    this.parts.push(buffer);
    return this;
  }

  i64(value) {
    const buffer = Buffer.alloc(8);
    buffer.writeBigInt64LE(BigInt(value));
    this.parts.push(buffer);
    return this;
  }

  build() {
    return Buffer.concat(this.parts);
  }
}

test("Crystal DateTime completion ignores the kind bits", () => {
  const ticks = 638_600_000_000_000_000n;
  const utcBinary = ticks | (1n << 62n);
  assert.equal(dotNetDateTimeTicks(utcBinary), ticks);

  const fixture = new BinaryFixture()
    .i32(1)
    .i64(utcBinary)
    .i64(utcBinary)
    .i32(0)
    .i32(0)
    .i32(0)
    .build();
  const progress = readQuestProgress(new BinaryReader(fixture), 117);
  assert.equal(progress.taken, true);
  assert.equal(progress.completed, true);
});

test("Crystal v117 buff preserves type, remaining time and stats", () => {
  const fixture = new BinaryFixture()
    .u8(112)
    .u32(79_322)
    .i64(10_091_680)
    .i32(1)
    .u8(100)
    .i32(5)
    .i32(0)
    .i32(0)
    .build();
  const buff = readBuff(new BinaryReader(fixture), 117, 0);
  assert.deepEqual(buff, {
    type: 112,
    objectId: 79_322,
    expireTimeMs: "10091680",
    stats: [{ stat: 100, value: 5 }],
    data: [],
    values: [],
  });
  assert.deepEqual(buffStateFromNative(buff), {
    key: "rested",
    name: "Rested",
    description: "Crystal rested buff.",
    expires_at_tick: 10_092,
    attack_bonus: 0,
    defence_bonus: 0,
    stats: [{ stat: 100, value: 5 }],
  });
});

test("Crystal raw inventory slots map to Candidate pages without drift", () => {
  assert.deepEqual(candidateBagLocationFromCrystalSlot(6), { container: "bag1", slot: 0 });
  assert.deepEqual(candidateBagLocationFromCrystalSlot(45), { container: "bag1", slot: 39 });
  assert.deepEqual(candidateBagLocationFromCrystalSlot(46), { container: "bag2", slot: 0 });
  assert.deepEqual(candidateBagLocationFromCrystalSlot(85), { container: "bag2", slot: 39 });
  assert.throws(() => candidateBagLocationFromCrystalSlot(5), /outside raw inventory slots/);
  assert.throws(() => candidateBagLocationFromCrystalSlot(86), /outside raw inventory slots/);
});

test("UserItem projection preserves awake values and socket holes", () => {
  const socket = nativeItem({ slot: 2, uniqueId: "22", itemIndex: 2 });
  const root = nativeItem({
    slot: 6,
    uniqueId: "21",
    itemIndex: 1,
    awake: { type: 3, count: 3, values: [0, 2, 255] },
    socketItems: [socket],
    socketSlots: [null, null, socket],
  });
  const itemByIndex = new Map([
    [1, { item_index: 1, name: "Root", image: 7, item_type: 1, weight: 2, durability: 10, slots: 3, stats: [] }],
    [2, { item_index: 2, name: "Gem", image: 8, item_type: 18, weight: 0, durability: 0, slots: 0, stats: [] }],
  ]);
  const projected = itemStateFromNative(root, itemByIndex, candidateBagLocationFromCrystalSlot(6));
  assert.deepEqual(projected.user_item_metadata.awake_values, [0, 2, 255]);
  assert.equal(projected.socketed.length, 1);
  assert.equal(projected.socketed[0].slot, 2);
  assert.equal(projected.socketed[0].user_item_metadata.captured_socket_position, 2);
  assert.deepEqual(projected.user_item_metadata.captured_socket_positions, [
    null,
    null,
    { unique_id: 22, item_index: 2 },
  ]);
});

test("ready-to-turn-in and delivered quests remain distinct", () => {
  const questByIndex = new Map([
    [1, { index: 1, name: "Assistant's Request", group: "BichonProvince", kill_tasks: [], item_tasks: [], flag_tasks: [] }],
    [2, { index: 2, name: "Delivered", group: "BichonProvince", kill_tasks: [], item_tasks: [], flag_tasks: [] }],
  ]);
  const states = questStatesFromNative({
    currentQuests: [{ index: 1, completed: true, killTasks: [], itemTasks: [], flagTasks: [] }],
    completedQuests: [2],
  }, questByIndex);
  assert.equal(states[0].stage, "readyToTurnIn");
  assert.equal(states[1].stage, "completed");
});

test("strict projection accepts the audited base fixture and rejects precision loss", () => {
  const item = nativeItem({ slot: 0, uniqueId: "1", itemIndex: 1 });
  const nativeState = {
    schemaVersion: "mir2-crystal-account-state-v2",
    source: { fullyConsumed: true },
    header: { version: 117, customVersion: 0 },
  };
  const account = {
    storageCapacity: 80,
    storageItems: [],
    hasStoragePassword: false,
    expandedStorageExpiryDateBinary: "0",
  };
  const character = {
    inventoryCapacity: 46,
    questInventoryCapacity: 40,
    beltItems: [item],
    bagItems: [],
    questInventoryItems: [],
    equipmentItems: [],
    currentQuests: [],
    completedQuests: [],
    magics: [],
    buffs: [],
    flags: [],
    heroes: [0],
  };
  const items = new Map([[1, { item_index: 1 }]]);
  assert.deepEqual(strictUiStateBlockers(nativeState, account, character, items, new Map(), new Map()), []);
  character.inventoryCapacity = 54;
  character.bagItems = [nativeItem({ slot: 53, uniqueId: "2", itemIndex: 1 })];
  assert.deepEqual(
    strictUiStateBlockers(nativeState, account, character, items, new Map(), new Map()),
    [],
    "first Crystal expansion and its final unlocked Bag2 cell must be durable",
  );
  character.beltItems[0].uniqueId = "9007199254740993";
  assert.match(
    strictUiStateBlockers(nativeState, account, character, items, new Map(), new Map()).join("\n"),
    /precision loss/,
  );
});

test("Stage5 projection carries Crystal hair and modes", () => {
  const systems = stage5SystemsFromExisting(null, {
    hair: 8,
    attackMode: 5,
    petMode: 3,
    allowGroup: false,
  });
  assert.equal(systems.appearance.hair, 8);
  assert.equal(systems.attackMode, 5);
  assert.equal(systems.petMode, 3);
  assert.equal(systems.group.allowGroup, false);
});

test("Crystal skill names resolve to Candidate canonical starter keys", () => {
  const magicBySpell = new Map([
    ["Healing", { spell: "Healing", name: "Healing", delayBase: 1200, delayReduction: 100 }],
    ["Fury", { spell: "Fury", name: "Fury", delayBase: 2200, delayReduction: 100 }],
  ]);
  const starterSkillBySpell = new Map([
    ["Healing", { crystal_spell: "Healing", key: "minor-heal", name: "Minor Heal", cooldown_ticks: 3 }],
    ["Fury", { crystal_spell: "Fury", key: "battle-focus", name: "Battle Focus", cooldown_ticks: 7 }],
  ]);
  const healing = skillStateFromNative(
    { spell: 61, level: 1, experience: 2, key: 3 },
    magicBySpell,
    starterSkillBySpell,
  );
  const fury = skillStateFromNative(
    { spell: 16, level: 2, experience: 4, key: 5 },
    magicBySpell,
    starterSkillBySpell,
  );
  assert.equal(healing.key, "minor-heal");
  assert.equal(healing.cooldown_ticks, 3);
  assert.equal(fury.key, "battle-focus");
  assert.equal(fury.cooldown_ticks, 7);
});

test("evidence-state redaction removes account and rental-owner identity", () => {
  const raw = {
    schemaVersion: "mir2-crystal-account-state-v2",
    source: {
      dbPath: "C:\\private\\Server.MirADB",
      itemManifestPath: "E:\\repo\\crystal_item_manifest.json",
    },
    filters: { account: "private-account", character: "Hero" },
    account: {
      accountID: "private-account",
      userName: "Private User",
      email: "private@example.test",
      gold: 0,
      credit: 0,
      storageCapacity: 80,
      storageItems: [nativeItem({ rentalInformation: { ownerName: "RentalOwner" } })],
      characters: [{
        name: "Hero",
        beltItems: [],
        bagItems: [],
        equipmentItems: [],
        questInventoryItems: [],
      }],
    },
  };
  const redacted = redactExtractedNativeState(raw);
  const encoded = JSON.stringify(redacted);
  assert.equal(redacted.identity.redacted, true);
  assert.match(redacted.account.accountID, /^\[redacted:[0-9a-f]{12}\]$/);
  assert.equal(redacted.source.dbPath, "Server.MirADB");
  assert.equal(redacted.account.userName, null);
  assert.equal(redacted.account.email, null);
  assert.doesNotMatch(encoded, /private-account|Private User|private@example|RentalOwner|C:\\\\private/);
});

function nativeItem(overrides = {}) {
  return {
    slot: 0,
    uniqueId: "1",
    itemIndex: 1,
    currentDura: 0,
    maxDura: 0,
    count: 1,
    soulBoundId: -1,
    identified: false,
    cursed: false,
    socketItems: [],
    socketSlots: [],
    gemCount: 0,
    addedStats: [],
    awake: { type: 0, count: 0, values: [] },
    refinedValue: 0,
    refineAdded: 0,
    refineSuccessChance: 0,
    weddingRing: -1,
    expireInfo: null,
    rentalInformation: null,
    isShopItem: false,
    sealedInfo: null,
    gmMade: false,
    ...overrides,
  };
}
