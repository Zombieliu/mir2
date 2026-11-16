import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// Strictly pure source dependencies; no WASM/runtime loader or filesystem fixture writes.
const sources = { identity: "../lib/world-model/item-identity.ts",
  equipment: "../lib/equipment-gateway-adapter.ts", parcel: "../lib/mail-parcel-gateway-adapter.ts",
  storage: "../lib/storage-gateway-adapter.ts" };
const allow = { identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment", "./world-model/item-identity": "identity",
    "./mail-parcel-gateway-adapter": "parcel" } };
const modules = new Map();
function loadPure(name) {
  assert(Object.hasOwn(sources, name), "module outside pure allowlist");
  if (modules.has(name)) return modules.get(name);
  const url = new URL(sources[name], import.meta.url);
  const code = ts.transpileModule(readFileSync(url, "utf8"), { fileName: fileURLToPath(url),
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("require", "exports", "module", code)((id) => {
    assert(Object.hasOwn(allow[name], id), "unexpected " + name + " dependency: " + id);
    return loadPure(allow[name][id]);
  }, module.exports, module);
  modules.set(name, module.exports);
  return module.exports;
}
const adapter = loadPure("storage");
const item = (id, container, slot) => ({ uniqueId: id, authoritativeUniqueId: id, container, slot,
  name: "same display name", quantity: 1, tooltipSource: { info: { item_type: 3 } } });
function world() {
  return { inventoryCapacity: 54, maxBagSlots: 48, storageSize: 160, hasExpandedStorage: true,
    hasStoragePassword: false, requireStoragePassword: false, storageSessionUnlocked: true,
    inventoryItems: [item(11, "bag1", 3), item(22, "bag2", 3), item(33, "bag1", 7)],
    storageItems: [item(44, "storage", 83)], beltItems: [], equipmentItems: [] };
}
const selection = row => ({ container: row.container, slot: row.slot, uniqueId: row.uniqueId,
  authoritativeUniqueId: row.authoritativeUniqueId });
const requestId = seq => "st-" + String(seq).padStart(16, "0");

const deposit = (w, source = w.inventoryItems[1], target = 4) =>
  adapter.prepareStorageTransfer(w, "deposit", requestId(1), selection(source), { container: "storage", slot: target });
const withdraw = (w, target = { container: "bag2", slot: 7 }) =>
  adapter.prepareStorageTransfer(w, "withdraw", requestId(1), selection(w.storageItems[0]), target);

test("storage adapter maps both Bag2 endpoints to physical cells and exact wire fields", () => {
  const w = world(), d = deposit(w);
  assert.deepEqual(d.command, { type: "storeItemV2", requestId: requestId(1), from: 43, to: 4 });
  assert.deepEqual(withdraw(w).command, { type: "takeBackItemV2", requestId: requestId(1), from: 83, to: 47 });
  assert.equal(d.proof.source.uniqueId, 22); assert.equal(d.proof.target.uniqueId, null);
  assert(Object.isFrozen(d.proof) && Object.isFrozen(d.proof.source) && Object.isFrozen(d.proof.target));
});

test("storage adapter accepts authoritative UID zero and rejects display fallback", () => {
  const w = world(); w.inventoryItems[1] = item(0, "bag2", 3);
  assert.equal(deposit(w).proof.source.uniqueId, 0);
  assert.equal(adapter.prepareStorageTransfer(w, "deposit", requestId(1),
    { ...selection(w.inventoryItems[1]), authoritativeUniqueId: undefined, uniqueId: 43 },
    { container: "storage", slot: 4 }), null);
  delete w.inventoryItems[1].authoritativeUniqueId; assert.equal(deposit(w), null);
});

test("storage adapter rejects source replacement and duplicate cell or global UID", () => {
  const w = world(), selected = selection(w.inventoryItems[1]);
  w.inventoryItems[1] = item(99, "bag2", 3);
  assert.equal(adapter.prepareStorageTransfer(w, "deposit", requestId(1), selected,
    { container: "storage", slot: 4 }), null);
  for (const row of [item(22, "bag2", 3), item(22, "storage", 9)]) {
    const s = world();
    if (row.container === "storage") s.storageItems.push(row); else s.inventoryItems.push(row);
    assert.equal(deposit(s), null);
  }
});

test("storage adapter rejects unsafe identity, malformed request and wrong grids", () => {
  const w = world();
  for (const id of [undefined, NaN, -1, 0.5, "22", Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(adapter.prepareStorageTransfer(w, "deposit", requestId(1),
      { ...selection(w.inventoryItems[1]), authoritativeUniqueId: id }, { container: "storage", slot: 4 }), null);
  }
  for (const bad of [{ container: "quest", slot: 3 }, { container: "belt", slot: 3 },
    { container: "bag2", slot: 40 }, { container: "bag2", slot: -1 }]) {
    assert.equal(adapter.prepareStorageTransfer(w, "deposit", requestId(1),
      { ...selection(w.inventoryItems[1]), ...bad }, { container: "storage", slot: 4 }), null);
  }
  for (const id of ["st-1", "st-000000000000000x", "", " st-0000000000000001"]) {
    assert.equal(adapter.prepareStorageTransfer(w, "deposit", id, selection(w.inventoryItems[1]),
      { container: "storage", slot: 4 }), null);
  }
  assert.equal(withdraw(w, { container: "quest", slot: 7 }), null);
});

test("storage adapter applies bag minus six belt cells and authoritative storage revocation", () => {
  const w = world(); assert(withdraw(w, { container: "bag2", slot: 7 }));
  assert.equal(withdraw(w, { container: "bag2", slot: 8 }), null);
  w.maxBagSlots = 40; assert.equal(deposit(w), null, "capacity fields disagree");
  w.inventoryCapacity = 46; assert.equal(deposit(w), null, "Bag2 not addressable");
  w.inventoryCapacity = 54; w.maxBagSlots = 48; w.hasExpandedStorage = false;
  assert.equal(withdraw(w), null); assert.equal(deposit(w, w.inventoryItems[0], 80), null);
  assert(deposit(w, w.inventoryItems[0], 79));
  w.hasExpandedStorage = true; w.storageSize = 79;
  assert.equal(deposit(w, w.inventoryItems[0], 79), null); assert(deposit(w, w.inventoryItems[0], 78));
});

test("storage adapter requires protection setup and rejects locked password state", () => {
  for (const patch of [{ requireStoragePassword: true, hasStoragePassword: false },
    { hasStoragePassword: true, storageSessionUnlocked: false }]) {
    const w = { ...world(), ...patch }; assert.equal(deposit(w), null); assert.equal(withdraw(w), null);
  }
  assert(deposit({ ...world(), hasStoragePassword: true, storageSessionUnlocked: true }));
});

test("storage final proof rejects live source target capacity lock and wire mutation", () => {
  const w = world(), p = deposit(w); assert(adapter.storageTransferStillCurrent(w, p.command, p.proof));
  for (const mutate of [
    s => { s.inventoryItems[1] = item(99, "bag2", 3); },
    s => { s.storageItems.push(item(55, "storage", 4)); },
    s => { s.storageItems.push({ ...item(55, "storage", 4), authoritativeUniqueId: undefined }); },
    s => { s.storageItems.push(item(55, "storage", 4), item(66, "storage", 4)); },
    s => { s.hasStoragePassword = true; s.storageSessionUnlocked = false; },
    s => { s.maxBagSlots = 40; s.inventoryCapacity = 46; },
  ]) { const s = world(); mutate(s); assert.equal(adapter.storageTransferStillCurrent(s, p.command, p.proof), false); }
  for (const patch of [{ to: 5 }, { from: 3 }, { requestId: requestId(2) }, { uniqueId: 22 }]) {
    assert.equal(adapter.storageTransferStillCurrent(w, { ...p.command, ...patch }, p.proof), false);
  }
  w.storageItems.push(item(55, "storage", 4)); const occupied = deposit(w);
  assert.equal(occupied.proof.target.uniqueId, 55); w.storageItems.at(-1).authoritativeUniqueId = 66;
  assert.equal(adapter.storageTransferStillCurrent(w, occupied.command, occupied.proof), false);
});

test("storage reservation protects source and empty destination across other item commands", () => {
  const w = world(), p = deposit(w), pending = [{ proof: p.proof, enteredSocket: false }];
  for (const command of [
    { type: "dropItem", uniqueId: 22 }, { type: "useItem", uniqueId: 22, grid: "inventory" },
    { type: "splitItem", uniqueId: 22, grid: "inventory", count: 1 },
    { type: "mergeItem", idFrom: 33, idTo: 22, gridFrom: "inventory", gridTo: "inventory" },
    { type: "moveItem", grid: "inventory", from: 7, to: 43 },
    { type: "storeItemV2", requestId: requestId(2), from: 7, to: 4 },
    { type: "takeBackItemV2", requestId: requestId(2), from: 83, to: 43 },
  ]) assert.equal(adapter.storageMutationAllowed(w, command, pending), false, JSON.stringify(command));
  assert.equal(adapter.storageMutationAllowed(w, { type: "dropItem", uniqueId: 33 }, pending), true);
  assert.equal(adapter.storageMutationAllowed(w, p.command, pending, p.proof), true);
  assert.equal(adapter.storageMutationAllowed(w, p.command, pending, { ...p.proof }), false);
});


test("storage reservations block Mail attach and quote/send items while permitting lock cleanup", () => {
  const w = world(), p = deposit(w), pending = [{ proof: p.proof, enteredSocket: true }];
  assert.equal(adapter.storageMutationAllowed(w, { type: "collectParcel", mailId: 7 }, pending), false,
    "automatic collection cannot prove its chosen bag cells avoid a storage reservation");
  assert.equal(adapter.storageMutationAllowed(w, { type: "collectParcel", mailId: 7 }, []), true,
    "idle storage preserves existing collection flow");
  for (const command of [
    { type: "mailLockedItem", uniqueId: 22, locked: true },
    { type: "mailCost", itemsIdx: [0, 22, 0, 0, 0] },
    { type: "sendMail", itemsIdx: [22, 0, 0, 0, 0] },
  ]) assert.equal(adapter.storageMutationAllowed(w, command, pending), false, JSON.stringify(command));
  for (const command of [
    { type: "mailLockedItem", uniqueId: 22, locked: false },
    { type: "mailLockedItem", uniqueId: 33, locked: true },
    { type: "mailCost", itemsIdx: [0, 33, 0, 0, 0] },
    { type: "sendMail", itemsIdx: [0, 0, 0, 0, 0] },
  ]) assert.equal(adapter.storageMutationAllowed(w, command, pending), true, JSON.stringify(command));
  for (const command of [{ type: "mailCost" }, { type: "sendMail", itemsIdx: null },
    { type: "mailLockedItem", uniqueId: 22, locked: 1 }]) {
    assert.equal(adapter.storageMutationAllowed(w, command, pending), false);
  }
  // Reserved physical cells survive authoritative replacement, not just the
  // UID captured by the original proof.
  w.storageItems.push(item(55, "storage", 4));
  w.inventoryItems[1] = item(99, "bag2", 3);
  for (const id of [55, 99]) {
    assert.equal(adapter.storageMutationAllowed(w, { type: "mailLockedItem", uniqueId: id, locked: true }, pending), false);
    assert.equal(adapter.storageMutationAllowed(w, { type: "sendMail", itemsIdx: [id] }, pending), false);
  }
});
