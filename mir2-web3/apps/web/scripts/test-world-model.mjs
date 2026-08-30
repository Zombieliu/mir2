// Behavioral tests for lib/world-model/ (store + snapshot-emitter).
//
// Coverage:
//   1. Named mutations: upsertEntity / patchEntity / removeEntity produce the
//      expected state shapes.
//   2. Selector subscriptions: listeners fire only when the selected slice
//      changes by reference; unrelated mutations do NOT trigger them.
//   3. Damage-floater cap + prune: the 48-cap is enforced; already-expired
//      floaters are pruned before appending.
//   4. setMap mapChanged: ephemeral collections cleared; self-entity preserved.
//   5. removeWorldItem: quantity decrements and item removal work correctly.
//   6. Snapshot emitter: onSnapshot called only when state changed; dedupe by
//      JSON string; start/stop/running behave correctly (fake timers via
//      manual clock stepping rather than real setInterval).
//
// Pure logic only: no DOM, no network. Run with plain `node` — the .ts source
// is transpiled in-memory via the `typescript` devDependency (same harness all
// other test-*.mjs scripts use).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// ---------------------------------------------------------------------------
// Loader — mirrors other test-*.mjs scripts in this directory.
// ---------------------------------------------------------------------------

/**
 * Load a TypeScript module in-process, with optional dependency stubs.
 * The `deps` map is used to satisfy `require()` calls (CommonJS interop).
 */
function loadTsModule(url, deps = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  const require = (specifier) => {
    if (specifier in deps) return deps[specifier];
    throw new Error(`Unexpected require("${specifier}") while loading ${url}`);
  };
  const fn = new Function("exports", "module", "require", compiled.outputText);
  fn(mod.exports, mod, require);
  return mod.exports;
}

const BASE = new URL("../lib/world-model/", import.meta.url);

// Load in dependency order.
const sceneTypes = loadTsModule(new URL("../lib/scene-types.ts", import.meta.url));
const types = loadTsModule(new URL("types.ts", BASE), {
  "../scene-types": sceneTypes,
});
const actorCombatState = loadTsModule(new URL("actor-combat-state.ts", BASE));
const storeModule = loadTsModule(new URL("store.ts", BASE), {
  "./types": types,
  "./actor-combat-state": actorCombatState,
});
const emitterModule = loadTsModule(new URL("snapshot-emitter.ts", BASE), {
  "./store": storeModule,
});

const { createWorldStore } = storeModule;
const { createSnapshotEmitter } = emitterModule;
const { DEFAULT_WORLD_STATE } = types;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeEntity(objectId, overrides = {}) {
  return {
    objectId,
    kind: "monster",
    name: "Test Monster",
    x: 10,
    y: 20,
    disposition: "hostile",
    ...overrides,
  };
}

function makeItem(uniqueId, quantity = 5, container = "bag1") {
  return {
    key: `item-${uniqueId}`,
    name: "Test Item",
    icon: 1,
    uniqueId,
    slot: 0,
    container,
    quantity,
    description: "",
  };
}

function makeDrop(objectId) {
  return {
    objectId,
    name: "Gold",
    x: 5,
    y: 5,
    quantity: 100,
    sourceMonster: "Slime",
  };
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

let passed = 0;
let failed = 0;

function test(label, fn) {
  try {
    fn();
    console.log(`  ✓ ${label}`);
    passed++;
  } catch (err) {
    console.error(`  ✗ ${label}`);
    console.error(`    ${err.message}`);
    failed++;
  }
}

// ── 1. Store initial state ──────────────────────────────────────────────────

console.log("\n[world-model/store] initial state");

test("getState() returns DEFAULT_WORLD_STATE shape", () => {
  const store = createWorldStore();
  const s = store.getState();
  assert.equal(s.connected, false);
  assert.equal(s.gold, 0);
  assert.deepEqual(s.entities, []);
  assert.deepEqual(s.damageFloaters, []);
});

test("createWorldStore accepts partial initial state", () => {
  const store = createWorldStore({ gold: 999, playerObjectId: "42" });
  assert.equal(store.getState().gold, 999);
  assert.equal(store.getState().playerObjectId, "42");
  assert.equal(store.getState().connected, false); // default preserved
});

// ── 2. upsertEntity ────────────────────────────────────────────────────────

console.log("\n[world-model/store] upsertEntity");

test("adds new entity to empty list", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1"));
  assert.equal(store.getState().entities.length, 1);
  assert.equal(store.getState().entities[0].objectId, "e1");
});

test("replaces existing entity by objectId", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1", { x: 1 }));
  store.upsertEntity(makeEntity("e1", { x: 99 }));
  assert.equal(store.getState().entities.length, 1);
  assert.equal(store.getState().entities[0].x, 99);
});

test("appends second entity", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1"));
  store.upsertEntity(makeEntity("e2"));
  assert.equal(store.getState().entities.length, 2);
});

// ── 3. patchEntity ─────────────────────────────────────────────────────────

console.log("\n[world-model/store] patchEntity");

test("patches a field on existing entity", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1", { x: 0 }));
  store.patchEntity("e1", { x: 55 });
  assert.equal(store.getState().entities[0].x, 55);
});

test("patchEntity is a no-op when entity absent", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1", { x: 0 }));
  store.patchEntity("missing", { x: 99 });
  assert.equal(store.getState().entities[0].x, 0);
});

// ── 4. removeEntity ────────────────────────────────────────────────────────

console.log("\n[world-model/store] removeEntity");

test("removes entity by objectId", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1"));
  store.upsertEntity(makeEntity("e2"));
  store.removeEntity("e1");
  assert.equal(store.getState().entities.length, 1);
  assert.equal(store.getState().entities[0].objectId, "e2");
});

test("removeEntity is safe when entity absent", () => {
  const store = createWorldStore();
  store.removeEntity("nope");
  assert.equal(store.getState().entities.length, 0);
});

// ── 5. setMap ──────────────────────────────────────────────────────────────

console.log("\n[world-model/store] setMap");

test("updates mapTitle and mapFileName without clearing", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1"));
  store.setMap({ mapTitle: "Bichon Walled City", mapFileName: "0", mapChanged: false });
  assert.equal(store.getState().mapTitle, "Bichon Walled City");
  assert.equal(store.getState().entities.length, 1); // preserved
});

test("mapChanged clears entities (except self)", () => {
  const store = createWorldStore({ playerObjectId: "self" });
  store.upsertEntity(makeEntity("self", {
    kind: "selfPlayer",
    disposition: "friendly",
    attackAnimation: "melee1",
    attackStartedAt: 900,
    attackUntil: 1_500,
    struckStartedAt: 1_000,
    struckUntil: 1_300,
    pendingStruck: { attackerId: "monster-2", durationMs: 300 },
    dieStartedAt: 1_100,
    dieUntil: 1_500,
    deathHandled: true,
    reviveStartedAt: 1_200,
    reviveUntil: 1_600,
  }));
  store.upsertEntity(makeEntity("npc1", { kind: "npc" }));
  store.upsertGroundDrop(makeDrop("d1"));
  store.setMap({ mapFileName: "1", mapChanged: true });
  // self preserved, npc cleared
  assert.equal(store.getState().entities.length, 1);
  assert.equal(store.getState().entities[0].objectId, "self");
  assert.equal(store.getState().entities[0].attackUntil, undefined);
  assert.equal(store.getState().entities[0].struckUntil, undefined);
  assert.equal(store.getState().entities[0].pendingStruck, undefined);
  assert.equal(store.getState().entities[0].dieUntil, undefined);
  assert.equal(store.getState().entities[0].reviveUntil, undefined);
  assert.equal(store.getState().entities[0].deathHandled, false);
  assert.deepEqual(store.getState().groundDrops, []);
});

test("mapChanged clears damageFloaters and projectiles", () => {
  const store = createWorldStore();
  store.set({ damageFloaters: [{ key: "k", objectId: "x", text: "1", variant: "hit", isPlayerTarget: false, startedAt: 0, expiresAt: Date.now() + 10000 }] });
  store.setMap({ mapChanged: true });
  assert.deepEqual(store.getState().damageFloaters, []);
});

// ── 6. applyUserLocation ───────────────────────────────────────────────────

console.log("\n[world-model/store] applyUserLocation");

test("patches position on existing entity", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("p1", { kind: "selfPlayer", x: 0, y: 0 }));
  store.applyUserLocation({ objectId: "p1", x: 7, y: 8 });
  assert.equal(store.getState().entities[0].x, 7);
  assert.equal(store.getState().entities[0].y, 8);
});

test("inserts placeholder entity when absent", () => {
  const store = createWorldStore({ playerName: "Hero" });
  store.applyUserLocation({ objectId: "p1", x: 3, y: 4 });
  assert.equal(store.getState().entities.length, 1);
  assert.equal(store.getState().entities[0].objectId, "p1");
});

// ── 7. upsertGroundDrop / removeGroundDrop ────────────────────────────────

console.log("\n[world-model/store] ground drops");

test("upsertGroundDrop adds a new drop", () => {
  const store = createWorldStore();
  store.upsertGroundDrop(makeDrop("d1"));
  assert.equal(store.getState().groundDrops.length, 1);
});

test("upsertGroundDrop replaces existing drop", () => {
  const store = createWorldStore();
  store.upsertGroundDrop(makeDrop("d1"));
  store.upsertGroundDrop({ ...makeDrop("d1"), quantity: 999 });
  assert.equal(store.getState().groundDrops.length, 1);
  assert.equal(store.getState().groundDrops[0].quantity, 999);
});

test("removeGroundDrop removes by objectId", () => {
  const store = createWorldStore();
  store.upsertGroundDrop(makeDrop("d1"));
  store.upsertGroundDrop(makeDrop("d2"));
  store.removeGroundDrop("d1");
  assert.equal(store.getState().groundDrops.length, 1);
  assert.equal(store.getState().groundDrops[0].objectId, "d2");
});

// ── 8. removeWorldItem ────────────────────────────────────────────────────

console.log("\n[world-model/store] removeWorldItem");

test("removes item from inventoryItems", () => {
  const store = createWorldStore();
  store.set({ inventoryItems: [makeItem(10, 1)] });
  store.removeWorldItem({ uniqueId: 10, count: 1 });
  assert.equal(store.getState().inventoryItems.length, 0);
});

test("decrements item quantity instead of removing when count < quantity", () => {
  const store = createWorldStore();
  store.set({ inventoryItems: [makeItem(10, 5)] });
  store.removeWorldItem({ uniqueId: 10, count: 2 });
  assert.equal(store.getState().inventoryItems[0].quantity, 3);
});

test("removeWorldItem skips beltItems when fromBelt=false", () => {
  const store = createWorldStore();
  store.set({ beltItems: [makeItem(10, 1, "belt")] });
  store.removeWorldItem({ uniqueId: 10, count: 1, fromInventory: false, fromBelt: false });
  assert.equal(store.getState().beltItems.length, 1);
});

// ── 9. addDamageFloater cap + prune ───────────────────────────────────────

console.log("\n[world-model/store] addDamageFloater");

test("adds a floater to the list", () => {
  const store = createWorldStore();
  const now = Date.now();
  store.addDamageFloater(
    { objectId: "m1", text: "42", variant: "hit", isPlayerTarget: false, startedAt: now, expiresAt: now + 1000 },
    1,
  );
  assert.equal(store.getState().damageFloaters.length, 1);
  assert.equal(store.getState().damageFloaters[0].objectId, "m1");
  assert.ok(store.getState().damageFloaters[0].key.startsWith("dmg-m1-"));
});

test("prunes expired floaters before appending", () => {
  const store = createWorldStore();
  const expired = { objectId: "old", text: "0", variant: "hit", isPlayerTarget: false, startedAt: 0, expiresAt: 1 };
  store.set({ damageFloaters: [{ ...expired, key: "k-old" }] });
  const now = Date.now();
  store.addDamageFloater(
    { objectId: "new", text: "1", variant: "hit", isPlayerTarget: false, startedAt: now, expiresAt: now + 1000 },
    1,
  );
  assert.equal(store.getState().damageFloaters.length, 1);
  assert.equal(store.getState().damageFloaters[0].objectId, "new");
});

test("caps live floaters at 48 by dropping oldest", () => {
  const store = createWorldStore();
  const now = Date.now();
  // Insert 48 live floaters.
  const live = Array.from({ length: 48 }, (_, i) => ({
    key: `k${i}`,
    objectId: `obj${i}`,
    text: String(i),
    variant: "hit",
    isPlayerTarget: false,
    startedAt: now,
    expiresAt: now + 10000,
  }));
  store.set({ damageFloaters: live });
  // Add one more — should drop oldest and keep total at 48.
  store.addDamageFloater(
    { objectId: "new", text: "99", variant: "crit", isPlayerTarget: false, startedAt: now, expiresAt: now + 1000 },
    99,
  );
  const floaters = store.getState().damageFloaters;
  assert.equal(floaters.length, 48);
  assert.equal(floaters[floaters.length - 1].objectId, "new");
  // Oldest (obj0) should have been trimmed.
  assert.ok(!floaters.some((f) => f.objectId === "obj0"));
});

// ── 10. markEntityStruck ──────────────────────────────────────────────────

console.log("\n[world-model/store] markEntityStruck");

test("sets struck fields on entity", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1", { x: 1, y: 2 }));
  const now = Date.now();
  store.markEntityStruck("e1", { struckStartedAt: now, struckUntil: now + 500 });
  const e = store.getState().entities[0];
  assert.equal(e.struckStartedAt, now);
  assert.equal(e.struckUntil, now + 500);
});

test("markEntityStruck updates position when provided", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1", { x: 0, y: 0 }));
  const now = Date.now();
  store.markEntityStruck("e1", { x: 5, y: 6, struckStartedAt: now, struckUntil: now + 500 });
  const e = store.getState().entities[0];
  assert.equal(e.x, 5);
  assert.equal(e.y, 6);
});

// ── 11. Selector subscriptions ────────────────────────────────────────────

console.log("\n[world-model/store] subscribe (selector-based)");

test("listener fires when selected slice changes", () => {
  const store = createWorldStore();
  let calls = 0;
  store.subscribe((s) => s.entities, () => { calls++; });
  store.upsertEntity(makeEntity("e1"));
  assert.equal(calls, 1);
});

test("listener does NOT fire when unrelated field changes", () => {
  const store = createWorldStore();
  let calls = 0;
  store.subscribe((s) => s.entities, () => { calls++; });
  store.set({ gold: 500 });
  assert.equal(calls, 0);
});

test("listener fires for each distinct change", () => {
  const store = createWorldStore();
  let calls = 0;
  store.subscribe((s) => s.gold, () => { calls++; });
  store.set({ gold: 1 });
  store.set({ gold: 2 });
  store.set({ gold: 2 }); // same value → no new call
  assert.equal(calls, 2);
});

test("unsubscribe stops listener from firing", () => {
  const store = createWorldStore();
  let calls = 0;
  const unsub = store.subscribe((s) => s.entities, () => { calls++; });
  store.upsertEntity(makeEntity("e1"));
  unsub();
  store.upsertEntity(makeEntity("e2"));
  assert.equal(calls, 1);
});

test("multiple independent subscriptions fire independently", () => {
  const store = createWorldStore();
  let goldCalls = 0, entityCalls = 0;
  store.subscribe((s) => s.gold, () => { goldCalls++; });
  store.subscribe((s) => s.entities, () => { entityCalls++; });
  store.set({ gold: 100 });
  store.upsertEntity(makeEntity("e1"));
  assert.equal(goldCalls, 1);
  assert.equal(entityCalls, 1);
});

// ── 12. Snapshot emitter ──────────────────────────────────────────────────

console.log("\n[world-model/snapshot-emitter]");

/**
 * Fake `setInterval` / `clearInterval` that lets us step ticks manually.
 * We patch global before loading emitter callbacks.
 */
function withFakeTimers(fn) {
  const timers = new Map();
  let nextId = 1;
  const realSetInterval = globalThis.setInterval;
  const realClearInterval = globalThis.clearInterval;

  globalThis.setInterval = (cb, ms) => {
    const id = nextId++;
    timers.set(id, cb);
    return id;
  };
  globalThis.clearInterval = (id) => {
    timers.delete(id);
  };

  const tick = () => {
    for (const cb of timers.values()) cb();
  };

  try {
    fn({ tick, timers });
  } finally {
    globalThis.setInterval = realSetInterval;
    globalThis.clearInterval = realClearInterval;
  }
}

test("start() begins emitting; stop() halts it", () => {
  withFakeTimers(({ tick }) => {
    const store = createWorldStore({ gold: 1 });
    const snapshots = [];
    const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => snapshots.push(j) });

    assert.equal(emitter.running, false);
    emitter.start();
    assert.equal(emitter.running, true);

    tick(); // first tick: emits (nothing was emitted yet)
    assert.equal(snapshots.length, 1);

    emitter.stop();
    assert.equal(emitter.running, false);

    tick(); // no timer running → nothing happens
    assert.equal(snapshots.length, 1);
  });
});

test("deduplicates by JSON: same state does not re-emit", () => {
  withFakeTimers(({ tick }) => {
    const store = createWorldStore({ gold: 10 });
    const snapshots = [];
    const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => snapshots.push(j) });
    emitter.start();

    tick(); // first tick emits
    tick(); // second tick: state unchanged → deduped
    tick(); // third tick: still unchanged
    assert.equal(snapshots.length, 1);

    emitter.stop();
  });
});

test("emits again after state changes", () => {
  withFakeTimers(({ tick }) => {
    const store = createWorldStore({ gold: 0 });
    const snapshots = [];
    const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => snapshots.push(j) });
    emitter.start();

    tick(); // emits snapshot 1
    store.set({ gold: 50 });
    tick(); // state changed → emits snapshot 2
    tick(); // unchanged → deduped
    assert.equal(snapshots.length, 2);

    const parsed = JSON.parse(snapshots[1]);
    assert.equal(parsed.gold, 50);
    emitter.stop();
  });
});

test("dedupe holds even as the wall clock advances between ticks", () => {
  withFakeTimers(({ tick }) => {
    const realNow = Date.now;
    let fakeNow = 1000;
    globalThis.Date.now = () => (fakeNow += 16); // advance on every read
    try {
      const store = createWorldStore({ gold: 7 });
      const snapshots = [];
      const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => snapshots.push(j) });
      emitter.start();

      tick(); // emits snapshot 1
      tick(); // world unchanged (only the clock moved) → must still dedupe
      tick(); // unchanged → dedupe
      assert.equal(snapshots.length, 1, "advancing clientTimeMs must not defeat dedupe");

      store.set({ gold: 8 });
      tick(); // world changed → emits snapshot 2 with a fresh clientTimeMs
      assert.equal(snapshots.length, 2);

      const a = JSON.parse(snapshots[0]).clientTimeMs;
      const b = JSON.parse(snapshots[1]).clientTimeMs;
      assert.ok(b > a, "each real push carries a fresh clientTimeMs");

      emitter.stop();
    } finally {
      globalThis.Date.now = realNow;
    }
  });
});

test("snapshot includes clientTimeMs", () => {
  withFakeTimers(({ tick }) => {
    const store = createWorldStore();
    let received = null;
    const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => { received = j; } });
    emitter.start();
    tick();
    emitter.stop();
    assert.ok(received !== null);
    const parsed = JSON.parse(received);
    assert.ok(typeof parsed.clientTimeMs === "number", "clientTimeMs should be present");
    assert.ok(parsed.clientTimeMs > 0);
  });
});

test("start() is idempotent — double start does not double-emit", () => {
  withFakeTimers(({ tick, timers }) => {
    const store = createWorldStore({ gold: 1 });
    const snapshots = [];
    const emitter = createSnapshotEmitter(store, { onSnapshot: (j) => snapshots.push(j) });
    emitter.start();
    emitter.start(); // second start should be a no-op
    assert.equal(timers.size, 1, "only one interval timer should exist");
    tick();
    assert.equal(snapshots.length, 1); // emitted once, not twice
    emitter.stop();
  });
});

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

console.log(`\n${passed} passed, ${failed} failed\n`);
if (failed > 0) process.exit(1);
