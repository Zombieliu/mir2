import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

// Compile only the explicit pure TypeScript dependency graph, never a renderer/Core module.
const cache = new Map();
const pureModules = new Set(["world-model/item-identity", "bevy-bag-model", "bevy-storage-model", "bevy-bag-ui", "bevy-storage-ui", "bag-belt-gesture", "npc-purchase-client"]);
function load(name, fresh = false) {
  assert.ok(pureModules.has(name), "unexpected non-pure import: " + name);
  if (!fresh && cache.has(name)) return cache.get(name);
  const source = readFileSync(new URL("../lib/" + name + ".ts", import.meta.url), "utf8");
  const javascript = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("exports", "module", "require", javascript)(module.exports, module,
    request => load(request.replace(/^\.\//, "")));
  if (!fresh) cache.set(name, module.exports);
  return module.exports;
}
const storage = load("bevy-storage-ui");
const projection = load("bevy-storage-model");
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false };
const capabilities = { schemaVersion: 1, storageUiAbiVersion: 1, storageIntentAbiVersion: 1, compiled: true, startup: true };
const worldItem = (container, slot, uniqueId) => ({ container, slot, authoritativeUniqueId: uniqueId,
  key: "sword", name: "WoodenSword", quantity: 1, icon: 2, description: "Exact item" });
function world() {
  return { inventoryCapacity: 54, maxBagSlots: 48, gold: 200,
    inventoryItems: [worldItem("bag2", 4, 0)], beltItems: [], equipmentItems: [],
    storageItems: [worldItem("storage", 3, 7)], storageSize: 160,
    hasStoragePassword: false, storageSessionUnlocked: true, hasExpandedStorage: false,
    expandedStorageExpiryTimeBinaryDatetime: 1000 };
}
function fixture(overrides = {}) {
  let now = 100, status = null, sink = null, state = null, host, behavior = () => false;
  const projected = projection.projectBevyStorageModel(world());
  assert.equal(projected.ok, true);
  let input = { connectionGeneration: 3, sessionGeneration: 5, ownerRevision: 0,
    eligible: true, open: true, serviceRevision: 11, presentation,
    inventory: projected.inventory, storage: projected.storage, player: { level: 1 },
    blockedUniqueIds: [], pendingCells: [], language: "en" };
  const snapshots = [], owners = [], intents = [], edges = [], withdrawals = [];
  const runtime = {
    getMir2StorageUiCapabilities: () => JSON.stringify(capabilities),
    setMir2StorageUiSnapshot(json) { snapshots.push(JSON.parse(json)); return true; },
    getMir2StorageUiStatus: () => JSON.stringify(status),
    setMir2StorageUiIntentSink(callback) { sink = callback; },
    clearMir2StorageUiIntentSink() { sink = null; },
    setMir2StorageUiPointerEdge(json) { edges.push(JSON.parse(json)); return true; },
    withdrawMir2StorageUiSnapshot(json) { withdrawals.push(JSON.parse(json)); return true; },
    ...overrides,
  };
  function replace(lib = storage) {
    return host = new lib.BevyStorageHost({ runtime, read: () => input, now: () => now,
      onOwner(owner, runGeneration, ownerRevision) {
        owners.push({ owner, runGeneration, ownerRevision }); input.ownerRevision = ownerRevision;
      },
      onState(next) { state = next; },
      onIntent(intent) { intents.push(intent); return behavior(intent); },
    });
  }
  replace();
  function echo(extra = {}) {
    const s = snapshots.at(-1);
    assert.ok(s, "a memory renderer may only apply an actually published snapshot");
    status = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision,
      frame: (status?.frame ?? 0) + 1, ready: true, inputEnabled: s.inputEnabled,
      appliedRevision: s.revision, appliedModelRevision: s.modelRevision,
      appliedPresentationRevision: s.presentationRevision, appliedServiceRevision: s.serviceRevision,
      inputRegions: [{ left: 100, top: 100, width: 700, height: 400 }], error: null, ...extra };
  }
  function activate() {
    host.tick(); echo(); host.tick(); echo(); host.tick();
    assert.equal(state.active, true); assert.equal(state.transitioning, false);
  }
  function intent(extra = {}) {
    const s = snapshots.at(-1);
    const value = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision,
      modelRevision: s.modelRevision, presentationRevision: s.presentationRevision,
      serviceRevision: s.serviceRevision, intentSequence: 1, type: "storeItem",
      source: { container: 0, slot: 44, uniqueId: 0 }, target: { container: 4, slot: 10 }, ...extra };
    if (["close", "password", "rent"].includes(value.type)) { delete value.source; delete value.target; }
    return value;
  }
  return { runtime, snapshots, owners, intents, edges, withdrawals, echo, activate, intent, replace,
    get host() { return host; }, get input() { return input; }, set input(value) { input = value; },
    get state() { return state; }, get sink() { return sink; },
    set behavior(callback) { behavior = callback; }, time(ms) { now += ms; },
    emit(value) { assert.equal(typeof sink, "function"); return sink(JSON.stringify(value)); },
  };
}

test("storage source projection preserves Bag2 physical slots, UID0 and binary expiry without deriving identity", () => {
  const p = projection.projectBevyStorageModel(world());
  assert.equal(p.ok, true);
  assert.equal(p.inventory.items[0].container, 0); assert.equal(p.inventory.items[0].slot, 44);
  assert.equal(p.inventory.items[0].uniqueId, 0); assert.equal(p.storage.items[0].container, 4);
  assert.equal(p.storage.expiry, 1000);
  assert.equal(storage.validStorageModel(p.inventory, p.storage), true);
  const missing = world(); delete missing.storageItems[0].authoritativeUniqueId;
  missing.storageItems[0].uniqueId = 300; missing.storageItems[0].tooltipSource = { info: {}, userItem: { unique_id: 7 } };
  assert.equal(projection.projectBevyStorageModel(missing).storage.items[0].uniqueId, null);
});

test("Source35 storage controller safe expiry preserves custody and unsafe numeric dates fail closed", () => {
  const projected = projection.projectBevyStorageModel(world());
  assert.equal(projected.ok, true);
  assert.equal(projected.storage.expiry, 1000);
  assert.equal(storage.validStorageModel(projected.inventory, projected.storage), true);
  const current = fixture(); current.activate();
  assert.equal(current.state.active, true);
  assert.equal(current.snapshots.at(-1).storage.expiry, 1000);
  // Numbers whose original i64 source may already be rounded stay unavailable.
  for (const expiry of [639028224000000000, Number("639028224000000001")]) {
    const input = { ...world(), expandedStorageExpiryTimeBinaryDatetime: expiry };
    const rejected = projection.projectBevyStorageModel(input);
    assert.equal(rejected.ok, false, String(expiry));
    assert.deepEqual(rejected.error, { code: "invalidField", field: "storage" });
    assert.equal(input.expandedStorageExpiryTimeBinaryDatetime, expiry);
  }
  assert.equal(current.state.active, true);
  assert.equal(current.snapshots.at(-1).storage.expiry, 1000);
});


test("Source35 exact storage expiry text survives projection publication and applied owner handoff", () => {
  for (const expiry of ["0", "1000", "-1000", "9007199254740992", "639028224000000001",
    "639028224000000002", "-8584900000000000001", "-9223372036854775808", "9223372036854775807"]) {
    const input = { ...world(), expandedStorageExpiryTimeBinaryDatetime: expiry };
    const projected = projection.projectBevyStorageModel(input);
    assert.equal(projected.ok, true, expiry);
    assert.equal(projected.storage.expiry, expiry);
    assert.equal(storage.validStorageModel(projected.inventory, projected.storage), true);
    const f = fixture();
    f.input = { ...f.input, inventory: projected.inventory, storage: projected.storage,
      blockedUniqueIds: [0], pendingCells: [{ container: 0, slot: 44 }, { container: 4, slot: 10 }] };
    f.activate();
    assert.equal(f.snapshots.at(-1).storage.expiry, expiry);
    assert.equal(input.expandedStorageExpiryTimeBinaryDatetime, expiry);
    assert.deepEqual(f.snapshots.at(-1).blockedUniqueIds, [0]);
    assert.deepEqual(f.snapshots.at(-1).pendingCells, f.input.pendingCells);
    assert.equal(f.snapshots.at(-1).inventory.items[0].uniqueId, 0);
  }
});

test("Source35 malformed dates and unsafe direct host numbers withdraw an active storage owner", () => {
  for (const expiry of ["+1", "-0", "01", "-01", " 1", "1 ", "", "1e3", "1.0",
    "9223372036854775808", "-9223372036854775809", Number.MIN_SAFE_INTEGER - 1,
    Number.MAX_SAFE_INTEGER + 1, Number("639028224000000001"), NaN, Infinity, 1.5, null, {}, []]) {
    const input = { ...world(), expandedStorageExpiryTimeBinaryDatetime: expiry };
    assert.equal(projection.projectBevyStorageModel(input).ok, false, String(expiry));
    const f = fixture(); f.activate();
    const i = f.intent(); f.emit(i); assert.equal(f.host.allows(i), true);
    f.input.storage = { ...f.input.storage, expiry };
    assert.equal(storage.validStorageModel(f.input.inventory, f.input.storage), false);
    assert.equal(f.host.claim(i), false);
    const count = f.snapshots.length; f.host.tick();
    assert.equal(f.snapshots.length, count, "invalid carrier never reaches the snapshot setter");
    assert.equal(f.state.active, false); assert.equal(f.host.pointerContext(), null);
    assert.equal(f.withdrawals.length, 1);
  }
});

test("storage source projection rejects duplicate cells/UIDs, cross-pane UID collisions and invalid accessible inputs", () => {
  for (const change of [
    w => { w.storageItems.push(worldItem("storage", 4, 7)); },
    w => { w.storageItems.push(worldItem("storage", 3, 8)); },
    w => { w.storageItems[0].authoritativeUniqueId = 0; },
    w => { w.storageItems[0].slot = 160; },
    w => { w.storageItems[0].container = "bag1"; },
    w => { w.inventoryItems[0].slot = 8; },
    w => { w.maxBagSlots = 47; },
    w => { w.storageSize = 161; },
    w => { w.hasStoragePassword = "false"; },
    w => { w.expandedStorageExpiryTimeBinaryDatetime = Number.NaN; },
  ]) { const w = world(); change(w); assert.equal(projection.projectBevyStorageModel(w).ok, false); }
  const defaultSize = world(); defaultSize.storageSize = 0;
  assert.equal(projection.projectBevyStorageModel(defaultSize).storage.size, 80);
});

test("independent ABI1 requires compiled/startup and every storage function; missing capability keeps fallback", () => {
  for (const caps of [{ ...capabilities, compiled: false }, { ...capabilities, startup: false },
    { ...capabilities, schemaVersion: 0 }, { ...capabilities, storageUiAbiVersion: 0 },
    { ...capabilities, storageIntentAbiVersion: 0 }, null]) {
    const f = fixture({ getMir2StorageUiCapabilities: () => JSON.stringify(caps) }); f.host.tick();
    assert.equal(storage.supportsStorage(f.runtime), false); assert.equal(f.state.active, false);
    assert.equal(f.state.transitioning, false); assert.equal(f.sink, null); assert.equal(f.snapshots.length, 0);
  }
  for (const name of ["setMir2StorageUiSnapshot", "getMir2StorageUiStatus", "setMir2StorageUiIntentSink",
    "clearMir2StorageUiIntentSink", "setMir2StorageUiPointerEdge", "withdrawMir2StorageUiSnapshot"]) {
    const f = fixture({ [name]: undefined }); f.host.tick();
    assert.equal(storage.supportsStorage(f.runtime), false); assert.equal(f.state.active, false);
    assert.equal(f.snapshots.length, 0);
  }
  const malformed = fixture({ getMir2StorageUiCapabilities() { throw Error("unavailable"); } });
  assert.doesNotThrow(() => malformed.host.tick()); assert.equal(malformed.state.active, false);
});

test("prepare then arming never accepts input; exact enabled echo establishes one owner revision", () => {
  const f = fixture(); f.host.tick();
  assert.equal(f.snapshots.at(-1).inputEnabled, false); assert.equal(f.host.pointerContext(), null);
  assert.equal(f.emit(f.intent()), false); assert.equal(f.intents.length, 0);
  assert.equal(Object.hasOwn(f.snapshots.at(-1), "eligible"), false);
  f.echo(); f.host.tick();
  assert.equal(f.state.transitioning, true); assert.equal(f.state.active, false);
  const transition = f.owners.at(-1); assert.equal(transition.owner, "transition");
  assert.equal(f.host.pointerContext(), null); assert.equal(f.emit(f.intent()), false);
  for (let i = 0; i < 3; i++) { f.time(100); f.host.tick(); assert.deepEqual(f.owners.at(-1), transition); }
  f.echo(); f.host.tick();
  assert.equal(f.state.active, true); assert.equal(f.owners.at(-1).owner, "bevy");
  assert.equal(f.owners.at(-1).ownerRevision, transition.ownerRevision);
  assert.ok(f.host.pointerContext());
});

test("every applied revision and identity must match before an intent can reach Page", () => {
  for (const field of ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision",
    "appliedRevision", "appliedModelRevision", "appliedPresentationRevision", "appliedServiceRevision"]) {
    const f = fixture(); f.activate(); const i = f.intent();
    f.echo({ [field]: i[field] === undefined ? 0 : i[field] + 1 });
    assert.equal(f.host.pointerContext(), null, field); assert.equal(f.emit(i), false, field);
    assert.equal(f.intents.length, 0, field);
  }
  for (const patch of [{ ready: false }, { inputEnabled: false }, { error: "paint failed" },
    { inputRegions: [{ left: 0, top: 0, width: 0, height: 30 }] }]) {
    const f = fixture(); f.activate(); f.echo(patch);
    assert.equal(f.host.pointerContext(), null); assert.equal(f.emit(f.intent()), false);
  }
});

test("unapplied arming timeout releases transition despite advancing renderer frames", () => {
  for (const patch of [{ ready: false }, { appliedModelRevision: 0 }, { appliedServiceRevision: 0 }, { inputEnabled: false }]) {
    const f = fixture(); f.host.tick(); f.echo(); f.host.tick();
    assert.equal(f.state.transitioning, true);
    for (let i = 0; i < 5; i++) { f.time(450); f.echo(patch); f.host.tick(); }
    assert.equal(f.state.active, false); assert.equal(f.state.transitioning, false);
    assert.equal(f.owners.at(-1).owner, "react"); assert.equal(f.snapshots.at(-1).inputEnabled, false);
  }
});

test("false or throwing Page sink retains the admitted proof but never replays its sequence", () => {
  for (const response of [() => false, () => { throw Error("definitely unavailable"); }]) {
    const f = fixture(); f.activate(); f.behavior = response; const i = f.intent();
    assert.equal(f.emit(i), false); assert.equal(f.host.allows(i), true);
    assert.equal(f.emit(i), false); assert.equal(f.intents.length, 1); assert.equal(f.host.allows(i), true);
    assert.equal(f.host.claim(i), true); assert.equal(f.host.claim(i), false); assert.equal(f.host.allows(i), false);
    assert.equal(f.emit(i), false); assert.equal(f.intents.length, 1);
  }
});

test("successful dispatch claims exactly once and replacement intent cannot authorize an older DTO", () => {
  const f = fixture(); f.activate();
  f.behavior = i => f.host.claim(i);
  const first = f.intent(); assert.equal(f.emit(first), true); assert.equal(f.host.claim(first), false);
  assert.equal(f.emit(first), false); assert.equal(f.intents.length, 1);
  f.behavior = () => false;
  const second = f.intent({ intentSequence: 2 }); assert.equal(f.emit(second), false);
  const third = f.intent({ intentSequence: 3, target: { container: 4, slot: 11 } }); assert.equal(f.emit(third), false);
  assert.equal(f.host.allows(second), false); assert.equal(f.host.claim(second), false);
  assert.equal(f.host.allows({ ...third, target: { container: 4, slot: 12 } }), false);
  assert.equal(f.host.claim(third), true); assert.equal(f.host.claim(third), false);
});

test("live model, service, session, owner and layout changes invalidate admitted proof before polling", () => {
  for (const change of [
    f => { f.input.inventory = { ...f.input.inventory, gold: 201 }; },
    f => { f.input.storage = { ...f.input.storage, unlocked: false }; },
    f => { f.input.storage = { ...f.input.storage, items: [] }; },
    f => { f.input.storage = { ...f.input.storage, expiry: "639028224000000001" }; },
    f => { f.input.serviceRevision++; }, f => { f.input.sessionGeneration++; },
    f => { f.input.connectionGeneration++; }, f => { f.input.ownerRevision++; },
    f => { f.input.presentation = { ...presentation, stageCssScale: 0.8 }; },
    f => { f.input.language = "zh-CN"; }, f => { f.input.open = false; }, f => { f.input.eligible = false; },
  ]) {
    const f = fixture(); f.activate(); const i = f.intent(); f.emit(i); assert.equal(f.host.allows(i), true);
    change(f); assert.equal(f.host.allows(i), false); assert.equal(f.host.claim(i), false);
    assert.equal(f.emit({ ...i, intentSequence: 2 }), false); assert.equal(f.intents.length, 1);
  }
});

test("own transport reservations do not pretend to replace the model; handoff preserves pending data", () => {
  const f = fixture(); f.activate(); const i = f.intent(); f.emit(i);
  f.input.blockedUniqueIds = [0]; f.input.pendingCells = [{ container: 0, slot: 44 }, { container: 4, slot: 10 }];
  assert.equal(f.host.allows(i), true);
  f.host.withdraw(); assert.equal(f.host.allows(i), false); assert.equal(f.state.active, false);
  assert.deepEqual(f.input.blockedUniqueIds, [0]);
  assert.deepEqual(f.input.pendingCells, [{ container: 0, slot: 44 }, { container: 4, slot: 10 }]);
  assert.equal(f.withdrawals.length, 1);
  f.activate(); assert.equal(f.host.allows(i), false); assert.equal(f.emit(i), false);
  const next = f.intent({ intentSequence: 2 }); f.emit(next); assert.equal(f.host.allows(next), true);
});

test("stopped renderer frame expires synchronously at final allows/claim", () => {
  const f = fixture(); f.activate(); const i = f.intent(); f.emit(i); f.time(2001);
  assert.equal(f.host.allows(i), false); assert.equal(f.host.claim(i), false);
  assert.equal(f.emit({ ...i, intentSequence: 2 }), false); assert.equal(f.intents.length, 1);
  f.host.tick(); assert.equal(f.state.active, false); assert.equal(f.snapshots.at(-1).inputEnabled, false);
});

test("invalid layout/model and rejected snapshot setters keep React rather than exposing an input owner", () => {
  for (const patch of [{ presentation: null }, { presentation: { ...presentation, touch: true, stageCssScale: 0.1 } },
    { inventory: null }, { storage: null }, { eligible: false }, { serviceRevision: 0 }]) {
    const f = fixture(); Object.assign(f.input, patch); f.host.tick();
    assert.equal(f.state.active, false); assert.equal(f.state.transitioning, false); assert.equal(f.host.pointerContext(), null);
  }
  for (const setter of [() => false, () => { throw Error("snapshot unavailable"); }]) {
    const f = fixture({ setMir2StorageUiSnapshot: setter });
    assert.doesNotThrow(() => f.host.tick()); assert.equal(f.state.active, false);
    assert.equal(f.state.transitioning, false); assert.equal(f.host.pointerContext(), null);
  }
});

test("storage pointer origin stays fixed across both panes and world crossings", () => {
  const f = fixture(); f.activate(); const c = f.host.pointerContext(), router = new storage.StoragePointerRouter();
  const down = router.down(c, 1, 0, 150, 150); assert.equal(down.origin, "storage");
  assert.equal(f.host.pointer(down), true);
  assert.equal(router.down(c, 2, 0, 250, 150), null, "second pointer cannot replace the lease");
  assert.equal(router.edge("move", 1, 900, 700).origin, "storage");
  assert.equal(router.edge("up", 1, 900, 700).origin, "storage");
  const worldDown = router.down(c, 1, 2, 900, 700); assert.equal(worldDown.origin, "world");
  assert.equal(router.edge("move", 1, 150, 150).origin, "world");
  const up = router.edge("up", 1, 150, 150); assert.equal(up.origin, "world");
  assert.ok(up.sequence > worldDown.sequence); assert.equal(router.held, null);
});

test("quarantined old terminal retains its identity and cannot clear a distinct new pointer lease", () => {
  const f = fixture(); f.activate(); const router = new storage.StoragePointerRouter();
  const old = router.down(f.host.pointerContext(), 1, 0, 150, 150);
  const cancel = router.cancel("blur"); assert.equal(cancel.phase, "blur");
  f.host.withdraw(); f.activate();
  const fresh = router.down(f.host.pointerContext(), 2, 0, 150, 150); assert.notEqual(fresh.ownerRevision, old.ownerRevision);
  const late = router.edge("up", 1, 150, 150); assert.equal(late.ownerRevision, old.ownerRevision);
  assert.equal(router.held, fresh); assert.equal(router.matches(f.host.pointerContext()), true);
  assert.equal(f.host.pointer(late), true); assert.equal(f.edges.at(-1).phase, "cancel");
  assert.equal(router.held, fresh);
});

test("stale cancel/blur clean only the old renderer epoch and preserve a newly admitted proof", () => {
  for (const phase of ["cancel", "blur"]) {
    const f = fixture(); f.activate(); const router = new storage.StoragePointerRouter();
    const old = router.down(f.host.pointerContext(), 1, 0, 150, 150); f.host.withdraw(); f.activate();
    const fresh = f.intent({ intentSequence: 2 }); f.emit(fresh); assert.equal(f.host.allows(fresh), true);
    assert.equal(f.host.pointer({ ...old, phase }), true);
    assert.equal(f.edges.at(-1).ownerRevision, old.ownerRevision);
    assert.equal(f.host.allows(fresh), true, "retired gesture must not withdraw the new final-send proof");
    assert.equal(f.host.claim(fresh), true);
  }
  const f = fixture(); f.activate(); const router = new storage.StoragePointerRouter();
  const current = router.down(f.host.pointerContext(), 1, 0, 150, 150), proof = f.intent(); f.emit(proof);
  assert.equal(f.host.pointer(current), true);
  f.emit({ ...proof, intentSequence: 2 });
  const liveProof = { ...proof, intentSequence: 2 };
  assert.equal(f.host.allows(liveProof), true);
  f.host.pointer({ ...current, phase: "cancel" }); assert.equal(f.host.allows(liveProof), false);
});

test("same-runtime HMR replacement owns sink and snapshot; old stop/withdraw cannot retire it", () => {
  const f = fixture(); f.activate(); const oldHost = f.host, oldSink = f.sink, old = f.intent();
  const replacement = f.replace(load("bevy-storage-ui", true)); f.activate();
  assert.notEqual(f.intent().runGeneration, old.runGeneration);
  const newSink = f.sink, counts = [f.snapshots.length, f.withdrawals.length, f.owners.length];
  oldHost.withdraw(); oldHost.stop(); oldHost.tick();
  assert.equal(f.sink, newSink); assert.deepEqual([f.snapshots.length, f.withdrawals.length, f.owners.length], counts);
  assert.equal(oldSink(JSON.stringify(old)), false); assert.equal(replacement, f.host);
  const fresh = f.intent(); f.emit(fresh); assert.equal(f.host.allows(fresh), true);
  f.host.stop(); assert.equal(f.sink, null); assert.equal(f.host.allows(fresh), false);
});

test("intent parser preserves physical UID0 while rejecting malformed identities and grid directions", () => {
  const f = fixture(); f.activate(); const i = f.intent();
  assert.equal(storage.parseStorageIntent(JSON.stringify(i)).source.uniqueId, 0);
  for (const type of ["close", "password", "rent"]) assert.equal(storage.parseStorageIntent(JSON.stringify(f.intent({ type }))).type, type);
  for (const patch of [{ intentSequence: 0 }, { serviceRevision: 0 }, { modelRevision: -1 }, { sessionGeneration: 0 },
    { source: { container: 1, slot: 0, uniqueId: 0 } }, { source: { container: 0, slot: 80, uniqueId: 0 } },
    { source: { container: 0, slot: 44 } }, { target: { container: 0, slot: 0 } },
    { target: { container: 4, slot: 160 } }, { type: "equipItem" }]) {
    assert.equal(storage.parseStorageIntent(JSON.stringify({ ...i, ...patch })), null);
  }
});

test("late startup attaches the independent sink and capability withdrawal retires the accepted surface", () => {
  let caps = { ...capabilities, startup: false };
  const f = fixture({ getMir2StorageUiCapabilities: () => JSON.stringify(caps) });
  f.host.tick(); assert.equal(f.sink, null); assert.equal(f.state.active, false);
  caps = { ...capabilities }; f.activate(); assert.equal(typeof f.sink, "function");
  const i = f.intent(); f.emit(i); assert.equal(f.host.allows(i), true);
  caps = { ...capabilities, compiled: false }; f.host.tick();
  assert.equal(f.state.active, false); assert.equal(f.state.transitioning, false);
  assert.equal(f.host.allows(i), false); assert.equal(f.withdrawals.length, 1);
});

test("admitted renderer tuple is frozen before callback and cannot be rewritten for a final claim", () => {
  const f = fixture(); f.activate();
  f.behavior = i => {
    assert.equal(Object.isFrozen(i), true); assert.equal(Object.isFrozen(i.source), true);
    assert.equal(Object.isFrozen(i.target), true);
    assert.throws(() => { i.source.slot = 0; }, TypeError);
    assert.throws(() => { i.target.slot = 11; }, TypeError);
    assert.throws(() => { i.intentSequence = 30; }, TypeError);
    return false;
  };
  const i = f.intent(); assert.equal(f.emit(i), false); assert.equal(f.host.allows(i), true);
  assert.equal(f.host.claim(i), true);
});

test("the common bag pane and storage pane both accept same-container moves with exact source identities", () => {
  const f = fixture(); f.activate();
  for (const [sequence, source, target] of [
    [1, { container: 0, slot: 44, uniqueId: 0 }, { container: 0, slot: 45 }],
    [2, { container: 4, slot: 3, uniqueId: 7 }, { container: 4, slot: 4 }],
  ]) {
    const i = f.intent({ type: "moveItem", intentSequence: sequence, source, target });
    assert.deepEqual(storage.parseStorageIntent(JSON.stringify(i)).source, source);
    assert.equal(f.emit(i), false); assert.equal(f.host.allows(i), true); assert.equal(f.host.claim(i), true);
  }
  const crossing = f.intent({ type: "moveItem", source: { container: 0, slot: 44, uniqueId: 0 }, target: { container: 4, slot: 10 } });
  assert.equal(storage.parseStorageIntent(JSON.stringify(crossing)), null);
});

test("desktop and touch admission use their declared geometry thresholds without claiming actual device layout", () => {
  for (const p of [{ ...presentation, logicalWidth: 760, logicalHeight: 430 },
    { ...presentation, touch: true, stageCssScale: 0.3125 }]) assert.equal(storage.fitsStorage(p), true);
  for (const p of [{ ...presentation, logicalWidth: 759 }, { ...presentation, logicalHeight: 429 },
    { ...presentation, touch: true, stageCssScale: 0.3 },
    { ...presentation, stageCssScale: Number.NaN }, { ...presentation, logicalWidth: 760.5 }]) {
    assert.equal(storage.fitsStorage(p), false); const f = fixture(); f.input.presentation = p; f.host.tick();
    assert.equal(f.state.active, false); assert.equal(f.host.pointerContext(), null);
  }
});

function shellStorageFixture() {
  const text = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("shell.tsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const names = ["scenePointFromMouseEvent", "beginCombatUiHold", "endCombatUiHold", "cancelSharedStoragePointer", "handleSharedStoragePointer",
    "cancelWorldFishingHeldPointer", "retireWorldFishingPhysical"];
  const declarations = [];
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.push(node.getText(ast));
    ts.forEachChild(node, visit);
  }
  visit(ast); assert.equal(declarations.length, names.length);
  const f = fixture(); f.activate(); let hook = null, stops = 0;
  class Element { constructor() { this.id = "canvas"; } setPointerCapture() {} }
  const holds = new Map(), router = new storage.StoragePointerRouter();
  const scope = { parityUiBlocksGameplay:undefined, onHeroShortcut:undefined, storagePointerRouterRef: { current: router }, combatUiHoldRef: { current: new Map() },
    onCombatUiHeld(channel, token, held) { if (held) holds.set(channel, token); else if (holds.get(channel) === token) holds.delete(channel); },
    storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => f.host.pointerContext(),
      onBevyStoragePointer(edge) { const accepted = f.host.pointer(edge); hook?.(edge); return accepted; } } },
    stageFrameRef: { current: { focus() {}, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1024, height: 768 }) } },
    stagePresentation: { virtualWidth: 1024, virtualHeight: 768 }, heldScenePointerRef: { current: null },
    worldFishingPhysicalRef: { current: null }, worldFishingTerminalRef: { current: null },
    onViewportDirectionStop() { stops++; }, stopHeldScenePointer() { scope.heldScenePointerRef.current = null; stops++; },
    sceneInteractionReady: true, questLocalModalOpen: false, mobileMoreOpen: false, bevyQuestUiCapturesPointer: false,
    bevyStorageUiActive: true, bevyStorageUiTransitioning: false,
    bagPointerRouterRef: { current: { held: null } }, characterPointerRouterRef: { current: { held: null } },
    hudPointerRouterRef: { current: { held: null } }, spellsPointerRouterRef: { current: { held: null } },
    HTMLElement: Element, sharedUiCanvasId: () => "canvas", webGl2SharedCanvasPrototype: false,
  };
  const js = ts.transpileModule(declarations.join("\n"), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const api = new Function(...Object.keys(scope), js + ";return {" + names.join(",") + "};")(...Object.values(scope));
  return { f, scope, api, router, holds, get stops() { return stops; }, set hook(callback) { hook = callback; },
    event(x = 150, y = 150, pointerId = 1) { return { target: new Element(), clientX: x, clientY: y,
      pointerId, pointerType: "mouse", button: 0, preventDefault() {} }; } };
}

test("actual Shell storage routing holds combat only for UI origin and preserves a crossing world gesture", () => {
  const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
  assert.equal(s.router.held.origin, "storage"); assert.equal(s.holds.size, 1);
  assert.equal(s.scope.heldScenePointerRef.current, null);
  s.api.handleSharedStoragePointer(s.event(900, 700), "up"); assert.equal(s.holds.size, 0);
  s.api.handleSharedStoragePointer(s.event(900, 700, 2), "down");
  assert.equal(s.router.held.origin, "world"); assert.equal(s.holds.size, 0);
  assert.ok(s.scope.heldScenePointerRef.current);
  s.api.handleSharedStoragePointer(s.event(150, 150, 2), "move");
  assert.equal(s.router.held.origin, "world"); assert.equal(s.scope.heldScenePointerRef.current.sceneX, 150);
  s.api.handleSharedStoragePointer(s.event(150, 150, 2), "up");
  assert.equal(s.router.held, null); assert.equal(s.scope.heldScenePointerRef.current, null);
});

test("actual Shell old cancellation cannot release a synchronously established replacement UI lease/token", () => {
  const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
  let freshLease, freshHold;
  s.hook = edge => {
    if (edge.phase !== "cancel") return; s.hook = null; s.f.host.withdraw(); s.f.activate();
    s.api.handleSharedStoragePointer(s.event(160, 160, 2), "down");
    freshLease = s.router.held; freshHold = s.scope.combatUiHoldRef.current.get("storage");
  };
  s.api.cancelSharedStoragePointer();
  assert.equal(s.router.held, freshLease); assert.equal(s.scope.combatUiHoldRef.current.get("storage"), freshHold);
  assert.equal(s.holds.get("storage"), freshHold.token);
});

test("actual Shell old cancel or up cannot clear a synchronously established replacement world hold", () => {
  for (const terminal of ["cancel", "up"]) {
    const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
    let freshLease, freshWorld, stopsAfterFresh;
    s.hook = edge => {
      if (edge.phase !== terminal) return; s.hook = null; s.f.host.withdraw(); s.f.activate();
      s.api.handleSharedStoragePointer(s.event(900, 700, 2), "down");
      freshLease = s.router.held; freshWorld = s.scope.heldScenePointerRef.current; stopsAfterFresh = s.stops;
      assert.equal(freshLease.origin, "world"); assert.ok(freshWorld);
    };
    if (terminal === "cancel") s.api.cancelSharedStoragePointer(); else s.api.handleSharedStoragePointer(s.event(), "up");
    assert.equal(s.router.held, freshLease);
    assert.equal(s.scope.heldScenePointerRef.current, freshWorld, "old terminal cannot clear the new world-origin hold");
    assert.equal(s.stops, stopsAfterFresh, "old terminal cannot stop the replacement gesture");
  }
});


// Source67: extract actual Page ownership consumers and their existing storage
// request/open/onOwner wiring. The runtime below is the original data-only host
// fixture; no React renderer, Core, browser, socket or gateway is initialized.
const storagePageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const storagePageAst = ts.createSourceFile("storage-ownership-page.tsx", storagePageSource,
  ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const storagePageNames = ["retainReactBagInteraction", "requestBagCompatibility", "endStorageService", "openInventory"];
const storagePageDeclarations = new Map(), storagePageCases = [], storagePageOptions = [];
(function visit(node) {
  if (ts.isFunctionDeclaration(node) && storagePageNames.includes(node.name?.text)) {
    assert(!storagePageDeclarations.has(node.name.text), "sole actual storage ownership consumer");
    storagePageDeclarations.set(node.name.text, node.getText(storagePageAst));
  }
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && node.expression.text === "NPCStorage")
    storagePageCases.push(node.getText(storagePageAst));
  if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "useBevyStorageUi")
    storagePageOptions.push(node.arguments[0]);
  ts.forEachChild(node, visit);
})(storagePageAst);
assert.deepEqual([...storagePageDeclarations.keys()].sort(), [...storagePageNames].sort());
assert.equal(storagePageCases.length, 1); assert.equal(storagePageOptions.length, 1);
function storageOption(name) {
  const found = storagePageOptions[0].properties.filter(node => ts.isPropertyAssignment(node)
    && node.name.getText(storagePageAst) === name);
  assert.equal(found.length, 1); return found[0].initializer;
}
const storageOpenExpressions = [];
(function visit(node) {
  if (ts.isPropertyAssignment(node) && node.name.getText(storagePageAst) === "open")
    storageOpenExpressions.push(node.initializer.getText(storagePageAst));
  ts.forEachChild(node, visit);
})(storageOption("read"));
assert.equal(storageOpenExpressions.length, 1);
const actualStorageRequested = new Function("bevyQuestUiRequested", "storageServiceActive", "storageCompatibility",
  "return (" + storageOption("requested").getText(storagePageAst) + ");");
const storageOwnershipJavaScript = ts.transpileModule(storagePageNames.map(name => storagePageDeclarations.get(name)).join("\n")
  + '\nfunction receiveActualNpcStorage(){switch("NPCStorage"){' + storagePageCases[0] + '}}'
  + '\nfunction actualStorageOpen(){return (' + storageOpenExpressions[0] + ');}'
  + '\nconst actualStorageOwner=' + storageOption("onOwner").getText(storagePageAst) + ';',
  { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 } }).outputText;
function pageStorageOwnershipFixture({ serviceActive = true, bagRequested = true, bagOpen = true,
  bagMode = null, ingress = true, sharedRequested = true } = {}) {
  const port = fixture(), trace = [], ownerRevisions = [];
  port.host.stop(); let host = null, hostState = null, service = serviceActive, compatibility = false, openVersion = 1;
  const reservations = new Map(), reserved = { proof: { source: { slot: 0 }, target: { slot: 7 } }, enteredSocket: true };
  reservations.set("existing", reserved);
  const scope = {
    bevyBagUiRequested: bagRequested, bagOpenRef: { current: bagOpen },
    bagCompatibilityModeRef: { current: bagMode }, bagYieldRef: { current: () => trace.push("bagYield") },
    storageServiceActiveRef: { current: serviceActive }, storageCompatibilityRef: { current: false },
    storageServiceRevisionRef: { current: 11 }, storageUiIngressRef: { current: null },
    equipmentBagOwnerRef: { current: { owner: "react", ownerRevision: 0 } },
    equipmentConnectionGenerationRef: { current: 3 }, equipmentSessionGenerationRef: { current: 5 },
    equipmentControllerRef: { current: { setOwnerRevision: revision => ownerRevisions.push(revision) } },
    renderBagOwner: revision => trace.push("owner:" + scope.equipmentBagOwnerRef.current.owner),
    pendingStorageRequestsRef: { current: reservations },
    storageRentalRef: { current: { retire: () => trace.push("retireRental") } },
    setStorageRentalPrompt: () => {}, setStoragePasswordOpenVersion: () => {},
    setBagCompatibilityMode: value => trace.push("bagMode:" + value),
    setStorageCompatibility: value => { compatibility = value; trace.push("storageCompatibility:" + value); },
    setStorageServiceActive: value => { service = value; },
    setStorageServiceOpenVersion: value => { openVersion = typeof value === "function" ? value(openVersion) : value; },
    retireNpcShopService: () => {}, updateWorld: () => {},
    npcRepairServiceRef: { current: null }, closeNpcRepairService: () => trace.push("closeRepair"),
    sharedHudNavigationRef: { current: { ready: true } },
    navigateSharedHud: action => { trace.push("hud:" + action.type); return true; },
    setActiveInventoryTab: tab => trace.push("tab:" + tab),
    setShowInventory: value => { scope.bagOpenRef.current = value; trace.push("bagOpen:" + value); },
    closeTouchSecondaryWindows: () => trace.push("touchClose"),
  };
  const api = new Function(...Object.keys(scope), storageOwnershipJavaScript + "\nreturn {"
    + storagePageNames.join(",") + ",receiveActualNpcStorage,actualStorageOpen,actualStorageOwner};")(...Object.values(scope));
  function attach() {
    host?.stop();
    host = new storage.BevyStorageHost({ runtime: port.runtime, now: () => 100,
      read: () => ({ ...port.input, serviceRevision: scope.storageServiceRevisionRef.current,
        ownerRevision: scope.equipmentBagOwnerRef.current.ownerRevision, open: api.actualStorageOpen() }),
      onOwner: api.actualStorageOwner, onState: state => { hostState = state; }, onIntent: () => false,
    });
    scope.storageUiIngressRef.current = { withdraw() { trace.push("withdraw"); host.withdraw(); } };
  }
  if (ingress) attach();
  return { scope, api, trace, ownerRevisions, port, reserved, reservations, attach,
    get host() { return host; }, get hostState() { return hostState; }, get openVersion() { return openVersion; },
    requested: () => actualStorageRequested(sharedRequested, service, compatibility),
    activate() { host.tick(); port.echo(); host.tick(); port.echo(); host.tick(); assert.equal(hostState.active, true); },
  };
}

test("Source67 actual compatibility interaction withdraws shared storage before retaining its React owner", () => {
  const f = pageStorageOwnershipFixture(); f.activate();
  const intent = f.port.intent(); assert.equal(f.port.emit(intent), false); assert.equal(f.host.allows(intent), true);
  f.trace.length = 0; f.api.retainReactBagInteraction();
  assert.deepEqual(f.trace, ["withdraw", "owner:react", "storageCompatibility:true", "bagYield", "bagMode:fullInventory"]);
  assert.equal(f.scope.equipmentBagOwnerRef.current.owner, "react");
  assert.equal(f.scope.storageCompatibilityRef.current, true); assert.equal(f.requested(), false);
  assert.equal(f.api.actualStorageOpen(), false); assert.equal(f.hostState.active, false);
  assert.equal(f.host.allows(intent), false); assert.equal(f.host.claim(intent), false);
  for (let i = 0; i < 3; i++) { f.host.tick(); f.port.echo(); f.host.tick(); }
  assert.equal(f.hostState.active, false); assert.equal(f.hostState.transitioning, false);
  assert.equal(f.port.snapshots.at(-1).open, false); assert.equal(f.port.snapshots.at(-1).inputEnabled, false);
  assert.equal(f.reservations.get("existing"), f.reserved); assert.equal(f.reserved.enteredSocket, true);
});

test("Source67 repeated actual storage compatibility interaction is idempotent with or without a requested host", () => {
  for (const requested of [true, false]) {
    const f = pageStorageOwnershipFixture({ ingress: requested, sharedRequested: requested });
    f.trace.length = 0; f.api.retainReactBagInteraction(); const first = [...f.trace];
    f.api.retainReactBagInteraction(); f.api.retainReactBagInteraction();
    assert.deepEqual(f.trace, first); assert.equal(f.scope.storageCompatibilityRef.current, true);
    assert.equal(f.scope.bagCompatibilityModeRef.current, "fullInventory");
    assert.equal(f.requested(), false); assert.equal(f.api.actualStorageOpen(), false);
    assert.equal(f.trace.filter(value => value === "withdraw").length, requested ? 1 : 0);
    assert.equal(f.reservations.get("existing"), f.reserved);
  }
});

test("Source67 actual pure Bag interaction keeps the original requested/open/compatibility gates", () => {
  for (const [bagRequested, bagOpen, bagMode, yields] of [
    [true, true, null, 1], [false, true, null, 0], [true, false, null, 0], [true, true, "delete", 0],
  ]) {
    const f = pageStorageOwnershipFixture({ serviceActive: false, ingress: false, bagRequested, bagOpen, bagMode });
    f.api.retainReactBagInteraction();
    assert.equal(f.trace.filter(value => value === "bagYield").length, yields);
    assert.equal(f.scope.bagCompatibilityModeRef.current, yields ? "fullInventory" : bagMode);
    assert.equal(f.scope.storageCompatibilityRef.current, false); assert.equal(f.trace.includes("withdraw"), false);
    assert.equal(f.requested(), false);
  }
});

test("Source67 actual service close and NPCStorage reopen restore a fresh shared handoff without reusing proof", () => {
  const f = pageStorageOwnershipFixture(); f.activate();
  const oldIntent = f.port.intent(); f.port.emit(oldIntent);
  f.api.retainReactBagInteraction(); f.api.endStorageService();
  assert.equal(f.scope.storageServiceActiveRef.current, false); assert.equal(f.scope.storageCompatibilityRef.current, false);
  assert.equal(f.openVersion, 0); assert.equal(f.requested(), false);
  f.api.receiveActualNpcStorage();
  assert.equal(f.scope.storageServiceActiveRef.current, true); assert.equal(f.scope.storageCompatibilityRef.current, false);
  assert.equal(f.scope.storageServiceRevisionRef.current, 12); assert.equal(f.openVersion, 1);
  assert.equal(f.requested(), true); assert.equal(f.api.actualStorageOpen(), true);
  f.attach(); f.activate(); assert.equal(f.scope.equipmentBagOwnerRef.current.owner, "bevy");
  assert.equal(f.host.allows(oldIntent), false); assert.equal(f.host.claim(oldIntent), false);
  const freshIntent = f.port.intent(); assert.equal(f.port.emit(freshIntent), false);
  assert.equal(f.host.allows(freshIntent), true); assert.equal(f.host.claim(freshIntent), true);
  assert.equal(f.reservations.get("existing"), f.reserved, "UI close/reopen never retires an entered transport reservation");
});

test("Source67 actual leased storage page changes preserve ownership and avoid another shared openBag", () => {
  const f = pageStorageOwnershipFixture(); f.api.retainReactBagInteraction(); f.trace.length = 0;
  for (const page of ["bag1", "quest", "bag2"]) f.api.openInventory(page);
  assert.deepEqual(f.trace, ["tab:bag1", "tab:quest", "tab:bag2"]);
  assert.equal(f.scope.storageCompatibilityRef.current, true); assert.equal(f.openVersion, 1);
  assert.equal(f.scope.bagOpenRef.current, true); assert.equal(f.scope.bagCompatibilityModeRef.current, "fullInventory");
  const pureBag = pageStorageOwnershipFixture({ serviceActive: false, ingress: false });
  pureBag.api.openInventory("bag2"); assert.deepEqual(pureBag.trace, ["hud:openBag", "tab:bag2"]);
  const sharedStorage = pageStorageOwnershipFixture({ ingress: false });
  sharedStorage.api.openInventory("bag1"); assert.deepEqual(sharedStorage.trace, ["hud:openBag", "tab:bag1"]);
});


// Source71 diagnostics consume the actual Page reader, host, hook and JSX.
// Only the ordinary data ports below are supplied; no renderer/Core/socket runs.
const storageReader71Source = "const actualStorageRead71 = (" + storageOption("read").getText(storagePageAst) + ");";
const storageReader71JavaScript = ts.transpileModule(storageReader71Source,
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
function pageStorageReadFixture71() {
  const trace = [], current = { ...world(), connected: true, playerObjectId: 12,
    entities: [{ objectId: 12, name: "Player", classKey: "warrior", genderKey: "male", level: 1 }],
    playerHp: 10, playerMaxHp: 10, playerMp: 4, playerMaxMp: 4, credit: 0,
    requireStoragePassword: false, activeNpcDialog: null };
  const scope = {
    worldRef: { current }, projectBevyStorageModel: value => { trace.push("project"); return projection.projectBevyStorageModel(value); },
    equipmentControllerRef: { current: { status() { trace.push("ledger"); return { ready: true }; },
      hasPendingInstance() { trace.push("pending"); return { ok: true, reserved: false }; } } },
    mailParcelRef: { current: null }, pendingStorageRequestsRef: { current: new Map() },
    equipmentConnectionGenerationRef: { current: 3 }, equipmentSessionGenerationRef: { current: 5 },
    equipmentBagOwnerRef: { current: { ownerRevision: 0 } }, storageServiceRevisionRef: { current: 11 },
    storageServiceActiveRef: { current: true }, bagOpenRef: { current: true }, storageCompatibilityRef: { current: false },
    bevyQuestUiRequested: true, bevyRuntimeBackend: "webgpu", webGl2SharedCanvasPrototype: false,
    sharedCanvasUsesWebGl2() { trace.push("backend"); return false; }, screenRef: { current: "game" },
    initialSceneAssetsReadyRef: { current: true }, equipmentHostSuspendReasonRef: { current: null },
    bevyQuestReactModalOpen: false, showQuestLog: false, showCharacter: false, heroManagementPage: null,
    cashShopOpenRef: { current: false }, showHeroPet: false, showGuild: false, showGroup: false,
    showFriends: false, showBonds: false, showRanking: false, showMarket: false, showConquest: false,
    showTrade: false, showBuffs: false, showMail: false, showWorldMap: false, showChatSettings: false,
    npcShopService: null, npcRepairService: null, referenceWindowsBlockGameplay() { trace.push("referenceModal"); return false; },
    document: { visibilityState: "visible", hasFocus() { trace.push("focus"); return true; },
      querySelector() { trace.push("query"); return null; } },
    readBevyQuestPresentation() { trace.push("presentation"); return presentation; },
    sharedUiCanvasId: () => "shared-ui", clientProfile: { input: "keyboardMouse" }, language: "en",
  };
  return { scope, current, trace, read() {
    return new Function(...Object.keys(scope), storageReader71JavaScript + "\nreturn actualStorageRead71();")(...Object.values(scope));
  } };
}

test("Source71 actual Page reader records the first eligible gate without evaluating later gates", () => {
  const cases = [
    ["sharedNotRequested", f => { f.scope.bevyQuestUiRequested = false; f.scope.bevyRuntimeBackend = "webgl2"; }, "backend"],
    ["backendUnavailable", f => { f.scope.bevyRuntimeBackend = "webgl2"; }, "screen"],
    ["notGame", f => { f.scope.screenRef.current = "login"; f.current.connected = false; }, "focus"],
    ["disconnected", f => { f.current.connected = false; f.current.entities = []; }, "focus"],
    ["playerUnavailable", f => { f.current.entities = []; f.scope.initialSceneAssetsReadyRef.current = false; }, "focus"],
    ["sceneAssetsPending", f => { f.scope.initialSceneAssetsReadyRef.current = false; }, "focus"],
    ["hostSuspended", f => { f.scope.equipmentHostSuspendReasonRef.current = "suspended"; }, "focus"],
    ["equipmentNotReady", f => { f.scope.equipmentControllerRef.current.status = () => ({ ready: false }); }, "focus"],
    ["pendingReadUnavailable", f => { f.scope.equipmentControllerRef.current.hasPendingInstance = () => ({ ok: false }); }, "focus"],
    ["modalBlocked", f => { f.scope.showMail = true; }, "focus"],
    ["passwordSetupRequired", f => { f.current.requireStoragePassword = true; }, "focus"],
    ["documentHidden", f => { f.scope.document.visibilityState = "hidden"; }, "focus"],
    ["documentUnfocused", f => { f.scope.document.hasFocus = () => { f.trace.push("focus"); return false; }; }, null],
  ];
  for (const [reason, arrange, later] of cases) {
    const f = pageStorageReadFixture71(); arrange(f);
    if (reason === "backendUnavailable") Object.defineProperty(f.scope.screenRef, "current", {
      get() { f.trace.push("screen"); return "game"; } });
    const input = f.read(); assert.equal(input.eligible, false); assert.equal(input.eligibilityReason, reason);
    if (later) assert.equal(f.trace.includes(later), false, "later eligible gate was not evaluated: " + reason);
    assert.equal(f.trace.filter(x => x === "presentation").length, 1, "existing presentation read retained");
  }
  const f = pageStorageReadFixture71(), input = f.read();
  assert.equal(input.eligible, true); assert.equal(input.eligibilityReason, null);
  assert.deepEqual(f.trace, ["project", "ledger", "pending", "pending", "referenceModal", "focus", "query", "query", "presentation"]);
  for (const [arrange, reason] of [
    [g => { g.scope.storageServiceActiveRef.current = false; g.scope.bagOpenRef.current = false; }, "serviceInactive"],
    [g => { g.scope.bagOpenRef.current = false; g.scope.storageCompatibilityRef.current = true; }, "bagClosed"],
    [g => { g.scope.storageCompatibilityRef.current = true; }, "compatibility"],
  ]) { const g = pageStorageReadFixture71(); arrange(g); const v = g.read(); assert.equal(v.open, false); assert.equal(v.openReason, reason); }
});

test("Source71 actual tick ABI refusal precedes its reader and adds no runtime diagnostic reads", () => {
  const calls = []; let denied = false, state;
  const f = fixture({ getMir2StorageUiCapabilities() { calls.push("capabilities"); return JSON.stringify({ ...capabilities, compiled: !denied }); },
    getMir2StorageUiStatus() { calls.push("status"); return "null"; } });
  const host = new storage.BevyStorageHost({ runtime: f.runtime, read() { calls.push("reader"); return f.input; }, now: () => 100,
    onOwner() {}, onState(next) { state = next; }, onIntent: () => false });
  assert.equal(calls.filter(x => x === "reader").length, 1, "original constructor reader retained");
  calls.length = 0; denied = true; host.tick();
  assert.deepEqual(calls, ["capabilities"]);
  assert.deepEqual(state.diagnostic, { owner: "react", phase: "prepare", reason: "abiUnavailable" });
  assert.equal(f.snapshots.length, 0); assert.equal(host.pointerContext(), null);
});

test("Source71 actual host input failures retain the first reason and exclude diagnostics from the wire snapshot", () => {
  for (const [patch, reason] of [
    [{ eligible: false, eligibilityReason: "sceneAssetsPending", connectionGeneration: 0 }, "sceneAssetsPending"],
    [{ open: false, openReason: "compatibility", connectionGeneration: 0 }, "compatibility"],
    [{ connectionGeneration: 0, sessionGeneration: 0 }, "connectionInvalid"],
    [{ sessionGeneration: 0, serviceRevision: 0 }, "sessionInvalid"],
    [{ serviceRevision: 0, presentation: null }, "serviceInvalid"],
    [{ presentation: null }, "presentationInvalid"],
    [{ storage: null }, "modelInvalid"],
  ]) {
    let statusCalls = 0;
    const f = fixture({ getMir2StorageUiStatus() { statusCalls++; return "null"; } });
    f.input = { ...f.input, ...patch }; f.host.tick();
    assert.equal(f.state.diagnostic.reason, reason); assert.equal(f.state.active, false); assert.equal(statusCalls, 0);
    assert.equal(f.host.pointerContext(), null); assert.equal(f.intents.length, 0);
    for (const s of f.snapshots) { assert.equal(s.inputEnabled, false); assert.equal(s.open, false);
      assert.equal("eligibilityReason" in s, false); assert.equal("openReason" in s, false); }
  }
  const f = fixture(); f.input = { ...f.input, eligibilityReason: null, openReason: null }; f.host.tick();
  const s = f.snapshots.at(-1);
  assert.equal("eligibilityReason" in s, false); assert.equal("openReason" in s, false);
  assert.equal(f.state.diagnostic.reason, "statusUnavailable");
});

test("Source71 actual host status failures and enabled handoff preserve their original admission gates", () => {
  for (const [extra, reason] of [
    [{ runGeneration: 0 }, "statusUnavailable"],
    [{ connectionGeneration: 77 }, "identityMismatch"],
    [{ ready: false, error: "private raw error" }, "rendererError"],
    [{ error: "private raw error" }, "rendererError"],
    [{ frame: 0 }, "frameUnavailable"],
    [{ appliedRevision: 0 }, "appliedRevisionMismatch"],
    [{ inputEnabled: true }, "inputStateMismatch"],
    [{ inputRegions: [] }, "inputRegionsUnavailable"],
  ]) {
    const f = fixture(); f.host.tick(); f.echo(extra); f.host.tick();
    assert.equal(f.state.diagnostic.reason, reason); assert.equal(f.state.active, false);
    assert.equal(f.host.pointerContext(), null); assert.equal(f.intents.length, 0);
  }
  const f = fixture(); f.host.tick(); assert.equal(f.snapshots.at(-1).inputEnabled, false);
  f.echo(); f.host.tick(); assert.deepEqual(f.state.diagnostic, { owner: "transition", phase: "arming", reason: null });
  assert.equal(f.host.pointerContext(), null); assert.equal(f.snapshots.at(-1).inputEnabled, true);
  f.echo(); f.host.tick(); assert.deepEqual(f.state.diagnostic, { owner: "bevy", phase: "active", reason: null });
  assert.notEqual(f.host.pointerContext(), null);
  const i = f.intent(); assert.equal(f.emit(i), false); assert.equal(f.host.allows(i), true);
  assert.equal(f.host.claim(i), true); assert.equal(f.host.claim(i), false);
  const stale = fixture(); stale.activate(); stale.time(2101); stale.host.tick();
  assert.equal(stale.state.diagnostic.reason, "frameStale"); assert.equal(stale.host.pointerContext(), null);
  const regressed = fixture(); regressed.activate(); regressed.echo({ frame: 1 }); regressed.host.tick();
  assert.equal(regressed.state.diagnostic.reason, "frameRegressed"); assert.equal(regressed.host.pointerContext(), null);
  const denied = fixture({ setMir2StorageUiSnapshot() { return false; } }); denied.host.tick();
  assert.equal(denied.state.diagnostic.reason, "snapshotRejected"); assert.equal(denied.state.active, false);
});

const storageHook71Source = readFileSync(new URL("../lib/use-bevy-storage-ui.ts", import.meta.url), "utf8");
const storageHook71Ast = ts.createSourceFile("storage-hook71.ts", storageHook71Source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const storageHook71Functions = storageHook71Ast.statements.filter(n => ts.isFunctionDeclaration(n) && n.name?.text === "useBevyStorageUi");
assert.equal(storageHook71Functions.length, 1);
const storageHook71JavaScript = ts.transpileModule(storageHook71Functions[0].getText(storageHook71Ast).replace(/^export /, ""),
  { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
function hookFixture71() {
  const port = fixture(), refs = [], states = [], effects = [], intervals = new Map(), listeners = new Map();
  let refIndex = 0, stateIndex = 0, effectIndex = 0, intervalId = 0, mutations = 0;
  const useRef = value => refs[refIndex++] ?? (refs[refIndex - 1] = { current: value });
  const useState = initial => { const index = stateIndex++; states[index] ??= initial;
    return [states[index], next => { const value = typeof next === "function" ? next(states[index]) : next;
      if (value !== states[index]) { states[index] = value; mutations++; } }]; };
  const useEffect = (body, dependencies) => { const index = effectIndex++, previous = effects[index];
    if (!previous || dependencies.some((v, i) => v !== previous.dependencies[i])) {
      previous?.cleanup?.(); const effect = { body, dependencies, cleanup: null }; effects[index] = effect;
    } };
  function addEventListener(name, fn) { if (!listeners.has(name)) listeners.set(name, new Set()); listeners.get(name).add(fn); }
  function removeEventListener(name, fn) { listeners.get(name)?.delete(fn); }
  const windowPort = { setInterval(fn, ms) { assert.equal(ms, 50); intervals.set(++intervalId, fn); return intervalId; },
    clearInterval(id) { intervals.delete(id); }, addEventListener, removeEventListener };
  const actualHook = new Function("useRef", "useState", "useEffect", "BevyStorageHost", "performance", "window", "document",
    storageHook71JavaScript + "\nreturn useBevyStorageUi;")(useRef, useState, useEffect, storage.BevyStorageHost,
      { now: () => 100 }, windowPort, { addEventListener, removeEventListener });
  const options = { requested: true, inactiveReason: null, runtimeGeneration: 1, runtimeRef: { current: port.runtime },
    read: () => port.input, onOwner(owner, run, revision) { port.input.ownerRevision = revision; }, onIntent: () => false };
  function render() { refIndex = stateIndex = effectIndex = 0; return actualHook(options); }
  function flush() { for (const e of effects) if (e.body) { const body = e.body; e.body = null; e.cleanup = body(); } }
  return { port, refs, states, effects, options, intervals, listeners, render, flush, get mutations() { return mutations; } };
}

test("Source71 actual hook deduplicates diagnostic state and compatibility retires the prepare reason", () => {
  const f = hookFixture71(); f.render(); f.flush();
  assert.equal(f.render().diagnostic.reason, "statusUnavailable");
  const before = f.mutations; [...f.intervals.values()][0]();
  assert.equal(f.mutations, before, "same actual failure does not update React state again");
  f.options.requested = false; f.options.inactiveReason = "compatibility";
  const inactive = f.render(); assert.deepEqual(inactive.diagnostic, { owner: "react", phase: "inactive", reason: "compatibility" }); f.flush();
  assert.equal(f.intervals.size, 0); assert.equal([...f.listeners.values()].every(s => s.size === 0), true);
  f.options.requested = true; f.options.inactiveReason = null; f.options.runtimeGeneration++;
  assert.deepEqual(f.render().diagnostic, { owner: "react", phase: "inactive", reason: null }); f.flush(); f.port.echo(); [...f.intervals.values()][0](); f.port.echo(); [...f.intervals.values()][0]();
  assert.deepEqual(f.render().diagnostic, { owner: "bevy", phase: "active", reason: null });
});

test("Source71 actual hook ignores retired callbacks and cleanup cannot overwrite the replacement state", () => {
  const f = hookFixture71(); f.render(); f.flush();
  const oldCleanup = f.effects[0].cleanup, oldHost = f.refs[1].current, oldCallback = oldHost.options.onState;
  f.options.runtimeGeneration++;
  assert.deepEqual(f.render().diagnostic, { owner: "react", phase: "inactive", reason: null }); f.flush();
  f.port.echo(); [...f.intervals.values()][0](); f.port.echo(); [...f.intervals.values()][0]();
  const active = f.render(), mutations = f.mutations;
  oldCallback({ active: false, transitioning: false, ownerRevision: 999, error: "private old error",
    diagnostic: { owner: "react", phase: "prepare", reason: "rendererError" } });
  assert.equal(f.mutations, mutations); assert.deepEqual(f.render().diagnostic, active.diagnostic);
  // Browser clear/remove are idempotent; a repeated old cleanup must not remove
  // the replacement interval/listeners or notify its state/owner.
  const withdrawals = f.port.withdrawals.length; oldCleanup(); oldHost.tick(); oldHost.stop();
  assert.equal(f.port.withdrawals.length, withdrawals); assert.equal(f.mutations, mutations);
  assert.equal(f.intervals.size, 1); assert.equal([...f.listeners.values()].every(s => s.size === 1), true);
  assert.deepEqual(f.render().diagnostic, active.diagnostic);
  f.options.requested = false; f.options.inactiveReason = "serviceInactive";
  assert.deepEqual(f.render().diagnostic, { owner: "react", phase: "inactive", reason: "serviceInactive" }); f.flush();
});

test("Source71 actual main diagnostic attribute serializes only owner phase and finite reason", () => {
  const shellText = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
  const shellAst = ts.createSourceFile("storage-shell71.tsx", shellText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const attrs = [], pageProps = [];
  (function visit(n) { if (ts.isJsxAttribute(n) && n.name.getText(shellAst) === "data-storage-ui-status") attrs.push(n);
    ts.forEachChild(n, visit); })(shellAst);
  (function visit(n) { if (ts.isJsxAttribute(n) && n.name.getText(storagePageAst) === "storageUiDiagnostics") pageProps.push(n);
    ts.forEachChild(n, visit); })(storagePageAst);
  assert.equal(attrs.length, 1); assert.equal(pageProps.length, 1);
  const propText = pageProps[0].initializer.expression.getText(storagePageAst);
  const attrText = attrs[0].initializer.expression.getText(shellAst);
  const evaluate = new Function("bevyStorageUi", "return (" + propText + ");");
  const consume = new Function("storageUiDiagnostics", "return (" + attrText + ");");
  const f = fixture(); f.host.tick(); f.echo({ ready: true, error: "account token UID raw error should stay private" }); f.host.tick();
  const output = consume(evaluate(f.state));
  assert.deepEqual(JSON.parse(output), { owner: "react", phase: "prepare", reason: "rendererError" });
  assert.deepEqual(Object.keys(JSON.parse(output)).sort(), ["owner", "phase", "reason"]);
  assert.equal(/account|token|UID|raw error|Revision|Generation|uniqueId|session|frame|error/.test(output), false);
  assert.equal(f.host.pointerContext(), null); assert.equal(f.intents.length, 0);
});

// Source72 classifies the already-read renderer error without changing readiness admission.
function rendererReadyFailureFixture72(error) {
  const f = fixture(), calls = [], states = [];
  for (const [key, label] of [
    ["getMir2StorageUiCapabilities", "capabilities"], ["getMir2StorageUiStatus", "status"],
    ["setMir2StorageUiSnapshot", "snapshot"], ["setMir2StorageUiIntentSink", "sink"],
    ["clearMir2StorageUiIntentSink", "clear"], ["setMir2StorageUiPointerEdge", "pointer"],
    ["withdrawMir2StorageUiSnapshot", "withdraw"],
  ]) {
    const original = f.runtime[key];
    f.runtime[key] = function (...args) { calls.push(label); return original.apply(f.runtime, args); };
  }
  const host = new storage.BevyStorageHost({
    runtime: f.runtime,
    read() { calls.push("reader"); return f.input; },
    now() { calls.push("now"); return 100; },
    onOwner(owner, runGeneration, ownerRevision) {
      calls.push("owner"); f.owners.push({ owner, runGeneration, ownerRevision }); f.input.ownerRevision = ownerRevision;
    },
    onState(next) { states.push(next); },
    onIntent(intent) { calls.push("intent"); f.intents.push(intent); return false; },
  });
  calls.length = 0; host.tick();
  f.echo({ ready: false, error });
  calls.length = 0;
  const stateCountBefore = states.length;
  host.tick();
  return { f, host, calls, states, state: states.at(-1), stateUpdates: states.length - stateCountBefore };
}
function assertRendererReadyFailure72(error, reason) {
  const control = rendererReadyFailureFixture72(null), candidate = rendererReadyFailureFixture72(error);
  assert.deepEqual(control.state.diagnostic, { owner: "react", phase: "prepare", reason: "controlsNotReady" });
  assert.deepEqual(candidate.state.diagnostic, { owner: "react", phase: "prepare", reason });
  assert.deepEqual(control.calls, ["capabilities", "reader", "status", "now"]);
  assert.deepEqual(candidate.calls, control.calls, "classification adds no reader, clock or runtime calls");
  assert.equal(candidate.stateUpdates, control.stateUpdates, "the original fallback publication count is unchanged");
  assert.equal(candidate.state.error, control.state.error, "raw renderer errors do not replace the original state message");
  for (const result of [control, candidate]) {
    assert.equal(result.state.active, false); assert.equal(result.state.transitioning, false);
    assert.equal(result.state.ownerRevision, 0); assert.deepEqual(result.f.owners, []);
    assert.equal(result.f.snapshots.length, 1); assert.equal(result.f.snapshots[0].inputEnabled, false);
    assert.equal("diagnostic" in result.f.snapshots[0], false); assert.equal("error" in result.f.snapshots[0], false);
    assert.equal(result.host.pointerContext(), null);
    const intent = result.f.intent();
    assert.equal(result.f.emit(intent), false); assert.equal(result.host.allows(intent), false); assert.equal(result.host.claim(intent), false);
    assert.equal(result.f.intents.length, 0); assert.equal(result.f.edges.length, 0); assert.equal(result.f.withdrawals.length, 0);
    assert.deepEqual(result.calls, ["capabilities", "reader", "status", "now"], "refused input remains on the same short circuit");
  }
  assert.equal(JSON.stringify(candidate.states).includes(error), false, "renderer detail never enters state or diagnostics");
}

test("Source72 ready false classifies the exact asset failure while keeping React ownership and calls", () => {
  assertRendererReadyFailure72("storage assets failed to load", "rendererAssetsFailed");
});

test("Source72 ready false classifies the exact presentation mismatch without changing admission", () => {
  assertRendererReadyFailure72("storage presentation does not match window", "rendererPresentationMismatch");
});

test("Source72 ready false classifies missing icon metadata without an ownership handoff", () => {
  assertRendererReadyFailure72("storage item icon metadata unavailable", "rendererIconMetadataUnavailable");
});

test("Source72 ready false emits only a finite reason for unknown renderer detail", () => {
  assertRendererReadyFailure72("private raw error: Source72 unknown diagnostic fixture", "rendererError");
});
