import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScript(url, requireModule = () => {
  throw new Error(`unexpected module import from ${url}`);
}) {
  const compiled = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url),
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, requireModule);
  return module.exports;
}

const identity = loadTypeScript(new URL("../lib/world-model/item-identity.ts", import.meta.url));
const { projectBevyBagModel } = loadTypeScript(
  new URL("../lib/bevy-bag-model.ts", import.meta.url),
  (specifier) => {
    assert.equal(specifier, "./world-model/item-identity");
    return identity;
  },
);

// The Web WorldState currently produced by page.tsx has these fields. In
// particular, an equipped instance really carries key and quantity at login;
// display uniqueId and authoritativeUniqueId are separate properties.
function normalWebState() {
  return {
    inventoryCapacity: 46,
    maxBagSlots: 40,
    gold: 200,
    inventoryItems: [{
      key: "crystal-item-221", name: "WoodenSword", icon: 30,
      uniqueId: 4, authoritativeUniqueId: 4,
      slot: 4, container: "bag1", quantity: 1, description: "",
      durabilityCurrent: 4000, durabilityMax: 4000, equipSlot: "weapon",
    }],
    beltItems: [{
      key: "crystal-item-658", name: "(HP)DrugSmall", icon: 398,
      uniqueId: 2, authoritativeUniqueId: 2,
      slot: 0, container: "belt", quantity: 11, description: "",
    }],
    equipmentItems: [{
      slot: "weapon", key: "crystal-item-221", uniqueId: 0,
      authoritativeUniqueId: 0, quantity: 1, name: "WoodenSword", icon: 30,
      shape: 0, description: "", durabilityCurrent: 4000,
      durabilityMax: 4000, attack: 0, defence: 0, grade: "common",
      addedAttack: 0, addedDefence: 0, addedLuck: 0, socketSlots: 0,
    }],
  };
}

function project(state) {
  return projectBevyBagModel(state);
}

test("normal Web login state becomes the Rust inventory JSON without losing UID zero", () => {
  const result = project(normalWebState());
  assert.equal(result.ok, true);
  assert.deepEqual(result.model, {
    capacity: 46, gold: 200,
    items: [
      {
        uniqueId: 4, key: "crystal-item-221", name: "WoodenSword", quantity: 1,
        slot: 4, container: 0, icon: 30, description: "",
        durabilityCurrent: 4000, durabilityMax: 4000, equipSlot: "weapon",
      },
      {
        uniqueId: 2, key: "crystal-item-658", name: "(HP)DrugSmall", quantity: 11,
        slot: 0, container: 1, icon: 398, description: "",
      },
      {
        uniqueId: 0, key: "crystal-item-221", name: "WoodenSword", quantity: 1,
        slot: 0, container: 2, icon: 30, description: "",
        durabilityCurrent: 4000, durabilityMax: 4000, shape: 0, socketSlots: 0,
        attack: 0, defence: 0, addedAttack: 0, addedDefence: 0,
        addedLuck: 0, grade: "common",
      },
    ],
  });
});

test("pre-StartGame bootstrap rows are displayable without inventing equipment UID", () => {
  // Selected rows from cp04-origin-evidence/native-wire.jsonl, id 2's FIRST
  // worldSnapshot, before the real character inventory replaced this
  // bootstrap content. The observer omitted capacity/gold; use a valid
  // expanded capacity here. Equipment has key and quantity but no uniqueId.
  const state = {
    inventoryCapacity: 54, maxBagSlots: 48, gold: 200,
    inventoryItems: [
      { key: "red-potion", name: "Red Potion", icon: 398, uniqueId: 0,
        authoritativeUniqueId: 0, slot: 0, container: "bag1", quantity: 5,
        description: "" },
      { key: "town-teleport", name: "Town Teleport", icon: 402, uniqueId: 40,
        authoritativeUniqueId: 40, slot: 0, container: "bag2", quantity: 1,
        description: "" },
    ],
    beltItems: [],
    equipmentItems: [
      { slot: "weapon", key: "crystal-item-221", name: "WoodenSword", icon: 30,
        quantity: 1, description: "", durabilityCurrent: 4000,
        durabilityMax: 4000, attack: 0, defence: 0 },
      { slot: "armour", key: "crystal-item-317", name: "BaseDress(M)", icon: 60,
        quantity: 1, description: "", durabilityCurrent: 5000,
        durabilityMax: 5000, attack: 0, defence: 0 },
    ],
  };
  const result = project(state);
  assert.equal(result.ok, true);
  assert.deepEqual(result.model.items.map((item) => [item.container, item.slot, item.uniqueId]), [
    [0, 0, 0], [0, 40, 40], [2, 0, null], [2, 1, null],
  ]);
});

test("last post-StartGame snapshot projects the actual player loadout", () => {
  // Selected item rows from id 2's LAST worldSnapshot, 05:50:04 local. The
  // observer omitted capacity/gold; these values match the saved player state.
  // Full tooltipSource objects are excluded from this embedded fixture and
  // covered by the metadata pass-through test below.
  const bag = (slot, uniqueId, key, name, icon, quantity, grade, equipSlot) => ({
    slot, uniqueId, authoritativeUniqueId: uniqueId, key, name, icon, quantity,
    container: "bag1", description: "", grade, addedAttack: 0, addedDefence: 0,
    ...(equipSlot === undefined ? {} : { equipSlot }),
  });
  const state = {
    inventoryCapacity: 46, maxBagSlots: 40, gold: 200,
    inventoryItems: [
      bag(1, 1, "crystal-item-317", "BaseDress(M)", 60, 1, "common", "armour"),
      bag(2, 2, "crystal-item-658", "(HP)DrugSmall", 398, 11, "none"),
      bag(3, 3, "crystal-item-722", "Candle", 130, 1, "none", "torch"),
      bag(0, 4, "crystal-item-221", "WoodenSword", 30, 1, "common", "weapon"),
    ],
    beltItems: [],
    equipmentItems: [{
      slot: "weapon", key: "crystal-item-221", name: "WoodenSword", icon: 30,
      uniqueId: 0, authoritativeUniqueId: 0, quantity: 1, description: "",
      durabilityCurrent: 4000, durabilityMax: 4000, stateImage: 30,
      attack: 0, defence: 0, grade: "common", addedAttack: 0, addedDefence: 0,
    }],
  };
  const result = project(state);
  assert.equal(result.ok, true);
  assert.deepEqual(result.model.items.map((item) => [item.container, item.slot, item.uniqueId]), [
    [0, 1, 1], [0, 2, 2], [0, 3, 3], [0, 0, 4], [2, 0, 0],
  ]);
  assert.equal(result.model.items[3].grade, "common");
  assert.equal(result.model.items[3].addedAttack, 0);
  assert.equal(result.model.items[4].stateImage, 30);
});

test("display aliases and tooltip user ids never become concrete authority", () => {
  const state = normalWebState();
  state.inventoryItems[0].uniqueId = 987;
  delete state.inventoryItems[0].authoritativeUniqueId;
  state.inventoryItems[0].tooltipSource = { info: { item_index: 221 }, userItem: { unique_id: 987 } };
  const missing = project(state);
  assert.equal(missing.ok, true);
  assert.equal(missing.model.items[0].uniqueId, null);
  assert.deepEqual(missing.model.items[0].tooltipSource, state.inventoryItems[0].tooltipSource);

  state.inventoryItems[0].authoritativeUniqueId = Number.NaN;
  assert.equal(project(state).model.items[0].uniqueId, null);
  state.inventoryItems[0].authoritativeUniqueId = Number.MAX_SAFE_INTEGER + 1;
  assert.equal(project(state).model.items[0].uniqueId, null);
  state.inventoryItems[0].authoritativeUniqueId = 0;
  state.equipmentItems = [];
  assert.equal(project(state).model.items[0].uniqueId, 0);
});

test("second bag is local in Web and global in Rust, bounded by expansion authority", () => {
  const state = normalWebState();
  state.inventoryCapacity = 54;
  state.maxBagSlots = 48;
  state.inventoryItems = [{ ...state.inventoryItems[0], container: "bag2", slot: 0 }];
  assert.equal(project(state).model.items[0].slot, 40);
  state.inventoryItems[0].slot = 7;
  assert.equal(project(state).model.items[0].slot, 47);
  state.inventoryItems[0].slot = 8;
  assert.deepEqual(project(state).error.code, "invalidSlot");
  state.inventoryCapacity = 86;
  state.maxBagSlots = 80;
  state.inventoryItems[0].slot = 39;
  assert.equal(project(state).model.items[0].slot, 79);
  state.inventoryItems[0].slot = 40; // Already-global slot accidentally supplied as local.
  assert.equal(project(state).error.code, "invalidSlot");
});

test("quest cells map to the distinct read-only container", () => {
  const state = normalWebState();
  state.inventoryItems = [{ ...state.inventoryItems[0], container: "quest", slot: 39 }];
  const result = project(state);
  assert.equal(result.ok, true);
  assert.equal(result.model.items[0].container, 3);
  assert.equal(result.model.items[0].slot, 39);
  state.inventoryItems[0].slot = 40;
  assert.equal(project(state).error.code, "invalidSlot");
});

test("equipment labels use the existing explicit 14-slot wire mapping", () => {
  const slots = [
    "weapon", "armour", "helmet", "torch", "necklace", "braceletLeft",
    "braceletRight", "ringLeft", "ringRight", "amulet", "belt", "boots", "stone", "mount",
  ];
  const state = normalWebState();
  state.inventoryItems = [];
  state.beltItems = [];
  state.equipmentItems = slots.map((slot, uniqueId) => ({
    ...state.equipmentItems[0], slot, uniqueId, authoritativeUniqueId: uniqueId,
  }));
  const result = project(state);
  assert.equal(result.ok, true);
  assert.deepEqual(result.model.items.map((item) => item.slot), slots.map((_, index) => index));
  state.equipmentItems[0].slot = "unknown";
  assert.equal(project(state).error.code, "invalidSlot");
  state.equipmentItems[0].slot = "toString";
  assert.equal(project(state).error.code, "invalidSlot");
});

test("invalid capacity, currency and mandatory row values deny ownership", () => {
  const state = normalWebState();
  for (const capacity of [0, 40, 47, 53, 55, 87, Number.NaN, 54.5]) {
    state.inventoryCapacity = capacity;
    assert.equal(project(state).error.code, "invalidCapacity");
  }
  state.inventoryCapacity = 54;
  assert.equal(project(state).error.code, "invalidCapacity"); // maxBagSlots still 40.
  state.inventoryCapacity = 46;
  for (const gold of [-1, 4_294_967_296, Number.NaN, 1.5]) {
    state.gold = gold;
    assert.equal(project(state).error.code, "invalidField");
  }
  state.gold = 200;
  delete state.equipmentItems[0].key;
  assert.deepEqual(project(state).error, {
    code: "invalidField", source: "equipmentItems", index: 0, field: "key",
  });
  state.equipmentItems[0].key = "crystal-item-221";
  delete state.equipmentItems[0].quantity;
  assert.equal(project(state).error.field, "quantity");
  state.equipmentItems[0].quantity = 1;
  state.inventoryItems[0].quantity = 4_294_967_296;
  assert.equal(project(state).error.field, "quantity");
});

test("unknown source containers and duplicate slots or concrete identities fail closed", () => {
  const state = normalWebState();
  state.inventoryItems.push({ ...state.inventoryItems[0], uniqueId: 5, authoritativeUniqueId: 5 });
  assert.equal(project(state).error.code, "duplicateGrid");
  state.inventoryItems.pop();
  state.inventoryItems.push({ ...state.inventoryItems[0], slot: 5 });
  assert.equal(project(state).error.code, "duplicateUniqueId");
  state.inventoryItems.pop();
  state.inventoryItems[0].container = "storage";
  assert.equal(project(state).error.code, "unexpectedContainer");
  state.inventoryItems[0].container = "bag1";
  state.beltItems[0].container = "bag1";
  assert.equal(project(state).error.code, "unexpectedContainer");
  state.beltItems[0].container = "belt";
  state.inventoryItems[0] = null;
  assert.deepEqual(project(state).error, {
    code: "invalidField", source: "inventoryItems", index: 0, field: "item",
  });
  delete state.inventoryItems[0];
  assert.equal(project(state).error.code, "invalidField");
});

test("explicit source metadata survives, absent metadata is not invented", () => {
  const state = normalWebState();
  const source = { info: { item_index: 221, image: 0, stack_size: 1 }, userItem: { unique_id: 7 } };
  state.equipmentItems[0].tooltipSource = source;
  state.equipmentItems[0].stateImage = 25;
  state.inventoryItems[0].grade = "common";
  state.inventoryItems[0].addedAttack = 2;
  state.inventoryItems[0].addedDefence = 1;
  const result = project(state);
  assert.equal(result.ok, true);
  assert.deepEqual(result.model.items[2].tooltipSource, source);
  assert.equal(result.model.items[2].stateImage, 25);
  assert.equal(result.model.items[0].grade, "common");
  assert.equal(result.model.items[0].addedAttack, 2);
  assert.equal(result.model.items[0].addedDefence, 1);
  assert.equal(result.model.items[0].tooltipSource, undefined);
  assert.equal(result.model.items[0].sellValue, undefined);
  assert.equal(result.model.items[0].attack, undefined);
  state.equipmentItems[0].tooltipSource = { userItem: { unique_id: 7 } };
  assert.deepEqual(project(state).error, {
    code: "invalidField", source: "equipmentItems", index: 0, field: "tooltipSource",
  });
});
