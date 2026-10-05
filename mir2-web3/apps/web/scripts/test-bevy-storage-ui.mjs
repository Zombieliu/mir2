import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

// Compile only the explicit pure TypeScript dependency graph, never a renderer/Core module.
const cache = new Map();
const pureModules = new Set(["world-model/item-identity", "bevy-bag-model", "bevy-storage-model", "bevy-bag-ui", "bevy-storage-ui"]);
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
    expandedStorageExpiryTimeBinaryDatetime: 639028224000000000 };
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
  assert.equal(p.storage.expiry, 639028224000000000);
  assert.equal(storage.validStorageModel(p.inventory, p.storage), true);
  const missing = world(); delete missing.storageItems[0].authoritativeUniqueId;
  missing.storageItems[0].uniqueId = 300; missing.storageItems[0].tooltipSource = { info: {}, userItem: { unique_id: 7 } };
  assert.equal(projection.projectBevyStorageModel(missing).storage.items[0].uniqueId, null);
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
  const names = ["scenePointFromMouseEvent", "beginCombatUiHold", "endCombatUiHold", "cancelSharedStoragePointer", "handleSharedStoragePointer"];
  const declarations = [];
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.push(node.getText(ast));
    ts.forEachChild(node, visit);
  }
  visit(ast); assert.equal(declarations.length, names.length);
  const f = fixture(); f.activate(); let hook = null, stops = 0;
  class Element { constructor() { this.id = "canvas"; } setPointerCapture() {} }
  const holds = new Map(), router = new storage.StoragePointerRouter();
  const scope = { storagePointerRouterRef: { current: router }, combatUiHoldRef: { current: new Map() },
    onCombatUiHeld(channel, token, held) { if (held) holds.set(channel, token); else if (holds.get(channel) === token) holds.delete(channel); },
    storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => f.host.pointerContext(),
      onBevyStoragePointer(edge) { const accepted = f.host.pointer(edge); hook?.(edge); return accepted; } } },
    stageFrameRef: { current: { focus() {}, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1024, height: 768 }) } },
    stagePresentation: { virtualWidth: 1024, virtualHeight: 768 }, heldScenePointerRef: { current: null },
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
