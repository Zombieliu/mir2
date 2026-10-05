import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function load(path) {
  const compiled = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const module = { exports: {} };
  new Function("require", "exports", "module", compiled.outputText)(
    (name) => {
      if (name === "./world-model/item-identity") return load("../lib/world-model/item-identity.ts");
      throw new Error(`Unexpected dependency ${name}`);
    }, module.exports, module,
  );
  return module.exports;
}
const adapter = load("../lib/equipment-gateway-adapter.ts");
const snapshot = () => ({ playerObjectId: 1000, inventoryCapacity: 54, storageSize: 80,
  inventoryItems: [{ uniqueId: 4, container: "bag2", slot: 0 }, { uniqueId: null, container: "quest", slot: 0 }],
  beltItems: [{ uniqueId: 2, container: "belt", slot: 1 }],
  equipmentItems: [{ uniqueId: 0, slot: "weapon" }],
  storageItems: [{ uniqueId: 9, container: "storage", slot: 79 }],
});

test("full raw snapshot projects UID zero, expanded bag, quest, belt and storage distinctly", () => {
  assert.deepEqual(adapter.projectEquipmentGatewaySnapshot(snapshot()), {
    capacity: 54, storageCapacity: 80,
    placements: [
      { uniqueId: 4, container: 0, slot: 40 }, { uniqueId: null, container: 3, slot: 0 },
      { uniqueId: 2, container: 1, slot: 1 }, { uniqueId: 0, container: 2, slot: 0 },
      { uniqueId: 9, container: 4, slot: 79 },
    ],
  });
});

test("bootstrap, partial arrays, unsafe identity and implicit capacity cannot establish authority", () => {
  for (const patch of [{ playerObjectId: null }, { inventoryCapacity: undefined }, { inventoryCapacity: 47 },
    { inventoryItems: undefined }, { beltItems: undefined }, { equipmentItems: undefined },
    { storageSize: undefined }, { storageSize: 0 }]) {
    assert.equal(adapter.projectEquipmentGatewaySnapshot({ ...snapshot(), ...patch }), null);
  }
  for (const uniqueId of [NaN, -1, 0.5, Number.MAX_SAFE_INTEGER + 1, "4"]) {
    const raw = snapshot(); raw.inventoryItems[0].uniqueId = uniqueId;
    assert.equal(adapter.projectEquipmentGatewaySnapshot(raw), null);
  }
});

test("duplicate rows and out-of-capacity slots are preserved for the shared Rust validator", () => {
  const raw = snapshot(); raw.inventoryCapacity = 46;
  raw.inventoryItems.push({ uniqueId: 4, container: "bag2", slot: 0 });
  const result = adapter.projectEquipmentGatewaySnapshot(raw);
  assert.equal(result.placements.filter((item) => item.uniqueId === 4).length, 2);
  assert.equal(result.placements[0].slot, 40);
});

test("misgrouped items and unknown equipment cells fail rather than changing containers", () => {
  for (const mutate of [
    (raw) => { raw.inventoryItems[0].container = "storage"; },
    (raw) => { raw.beltItems[0].container = "bag1"; },
    (raw) => { raw.inventoryItems[0].slot = 40; },
    (raw) => { raw.equipmentItems[0].slot = "unknown"; },
  ]) {
    const raw = snapshot(); mutate(raw);
    assert.equal(adapter.projectEquipmentGatewaySnapshot(raw), null);
  }
});

test("Use classification follows original type metadata, never display names or nested identity", () => {
  assert.deepEqual(adapter.classifyEquipmentUse({ name: "Sword", icon: 30 }), { kind: "unknown" });
  assert.deepEqual(adapter.classifyEquipmentUse({ equipSlot: "braceletRight" }), { kind: "equipment", to: 6 });
  assert.deepEqual(adapter.classifyEquipmentUse({ tooltipSource: { info: { item_type: 1 }, userItem: { unique_id: 0 } } }),
    { kind: "equipment", to: 0 });
  assert.deepEqual(adapter.classifyEquipmentUse({ tooltipSource: { info: { item_type: 3 } } }), { kind: "ordinary" });
  assert.deepEqual(adapter.classifyEquipmentUse({ tooltipSource: { info: { item_type: "1" } } }), { kind: "unknown" });
});

test("current instance checks reject display-only, same-name replacement and duplicate cells", () => {
  const item = { name: "Sword", key: "sword", container: "bag1", slot: 4, uniqueId: 4, authoritativeUniqueId: 0 };
  const world = { inventoryItems: [item], beltItems: [], equipmentItems: [], storageItems: [] };
  assert.equal(adapter.currentEquipmentCommandItem(world, 0, "inventory"), item);
  assert.equal(adapter.currentEquipmentCommandItem(world, 4, "inventory"), null);
  assert.equal(adapter.currentEquipmentCommandItem({ ...world, inventoryItems: [{ ...item, authoritativeUniqueId: 8 }] }, 0, "inventory"), null);
  assert.equal(adapter.currentEquipmentCommandItem({ ...world, inventoryItems: [item, { ...item, authoritativeUniqueId: 9 }] }, 0, "inventory"), null);
  assert.equal(adapter.currentEquipmentCommandItem({ ...world, storageItems: [{ ...item, container: "storage" }] }, 0, "inventory"), null);
});

test("equipment command tuples are exact and do not derive UID from destination", () => {
  assert.deepEqual(adapter.equipmentGatewayOperation({ type: "removeItem", uniqueId: 0, grid: "Inventory", to: 40 }),
    { kind: "remove", uniqueId: 0, grid: "inventory", to: 40 });
  for (const bad of [{ uniqueId: undefined }, { uniqueId: "0" }, { grid: "unknown" }, { to: -1 }, { to: 14 }]) {
    assert.equal(adapter.equipmentGatewayOperation({ type: "equipItem", uniqueId: 0, grid: "inventory", to: 0, ...bad }), null);
  }
});
