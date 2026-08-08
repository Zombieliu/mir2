// Behavioral tests for lib/world-model/use-world-selector.ts (render-perf Stage 5a).
//
// `useWorldSelector` itself is a React hook, but its load-bearing core is the
// framework-free `createSelectorMemo` cache (the useSyncExternalStoreWithSelector
// algorithm) plus the store's additive `subscribeFull`. This file tests that core
// with plain `node` — no DOM, no React — by composing exactly what the hook does:
//   subscribeFull(onStoreChange) -> on each change, recompute the selection via
//   the memo against store.getSnapshot(); the "component" re-renders only when the
//   selection reference actually changes (which is what React's
//   useSyncExternalStore does when getSnapshot returns a new reference).
//
// Coverage (the three cases the roadmap requires, Stage 5a):
//   1. A direct-slice selector fires ONCE per matching change.
//   2. An unrelated-slice change does NOT fire.
//   3. A derived selector (fresh object each call) with `isEqual` does NOT loop:
//      its getSnapshot is stable across reads, and an equal recompute keeps the
//      previous reference so React would not re-render.
//
// Loader mirrors the other test-*.mjs scripts: the .ts sources are transpiled
// in-memory via the `typescript` devDependency.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

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

const sceneTypes = loadTsModule(new URL("../lib/scene-types.ts", import.meta.url));
const types = loadTsModule(new URL("types.ts", BASE), {
  "../scene-types": sceneTypes,
});
const storeModule = loadTsModule(new URL("store.ts", BASE), {
  "./types": types,
});
// use-world-selector imports "react" only inside the hook; the pure helpers
// (createSelectorMemo / shallowEqual) never touch it. Stub `react` with throwing
// members so any accidental React use in the tested path is caught loudly.
const reactStub = new Proxy(
  {},
  {
    get(_t, prop) {
      throw new Error(`react.${String(prop)} used in a non-hook code path under test`);
    },
  },
);
const hookModule = loadTsModule(new URL("use-world-selector.ts", BASE), {
  react: reactStub,
  "./types": types,
  "./store": storeModule,
});

const { createWorldStore } = storeModule;
const { createSelectorMemo, shallowEqual } = hookModule;

// ---------------------------------------------------------------------------
// Test harness
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

/**
 * Faithfully simulate `useWorldSelector(store, selector, isEqual)`:
 *   - one memo per subscriber (createSelectorMemo),
 *   - subscribe via the store's additive `subscribeFull`,
 *   - on every store change, recompute the selection via the memo against the
 *     live snapshot; only when the selection REFERENCE changes does a real
 *     React render happen (useSyncExternalStore bails when getSnapshot is `Object.is`-equal).
 * Returns { renders, lastValue, getSnapshot, unsubscribe }.
 */
function mountSelector(store, selector, isEqual) {
  const memo = createSelectorMemo(selector, isEqual);
  const getSnapshot = () => memo(store.getSnapshot());
  let lastValue = getSnapshot();
  let renders = 0;
  const unsubscribe = store.subscribeFull(() => {
    const next = getSnapshot();
    if (!Object.is(next, lastValue)) {
      lastValue = next;
      renders += 1;
    }
  });
  return {
    get renders() {
      return renders;
    },
    get lastValue() {
      return lastValue;
    },
    getSnapshot,
    unsubscribe,
  };
}

// ── 1. Direct-slice selector: fires once per matching change ────────────────

console.log("\n[use-world-selector] direct-slice selector");

test("direct-slice selector re-renders once per matching change", () => {
  const store = createWorldStore();
  const sub = mountSelector(store, (s) => s.entities);
  assert.equal(sub.renders, 0);
  store.upsertEntity(makeEntity("e1"));
  assert.equal(sub.renders, 1, "one matching change → one render");
  store.upsertEntity(makeEntity("e2"));
  assert.equal(sub.renders, 2, "second matching change → second render");
});

test("direct-slice selector keeps a stable getSnapshot between reads", () => {
  const store = createWorldStore();
  store.upsertEntity(makeEntity("e1"));
  const sub = mountSelector(store, (s) => s.entities);
  const a = sub.getSnapshot();
  const b = sub.getSnapshot();
  assert.ok(Object.is(a, b), "repeat reads return the same reference (no churn)");
  assert.ok(Object.is(a, store.getState().entities), "selection is the live slice reference");
});

// ── 2. Unrelated-slice change: does NOT fire ────────────────────────────────

console.log("\n[use-world-selector] unrelated-slice isolation");

test("unrelated-slice change does NOT re-render an entities selector", () => {
  const store = createWorldStore();
  const sub = mountSelector(store, (s) => s.entities);
  store.set({ gold: 500 }); // unrelated slice
  assert.equal(sub.renders, 0, "gold change must not re-render an entities subscriber");
});

test("two subscribers track only their own slice", () => {
  const store = createWorldStore();
  const goldSub = mountSelector(store, (s) => s.gold);
  const entitySub = mountSelector(store, (s) => s.entities);
  store.set({ gold: 100 });
  assert.equal(goldSub.renders, 1, "gold subscriber sees the gold change");
  assert.equal(entitySub.renders, 0, "entities subscriber ignores the gold change");
  store.upsertEntity(makeEntity("e1"));
  assert.equal(goldSub.renders, 1, "gold subscriber ignores the entity change");
  assert.equal(entitySub.renders, 1, "entities subscriber sees the entity change");
});

test("setting a slice to an equal scalar value does not re-render", () => {
  const store = createWorldStore({ gold: 7 });
  const sub = mountSelector(store, (s) => s.gold);
  store.set({ gold: 7 }); // store.set returns a new {...} but gold is unchanged
  assert.equal(sub.renders, 0, "same scalar value → no render");
});

// ── 3. Derived selector with isEqual: does NOT loop ─────────────────────────

console.log("\n[use-world-selector] derived selector loop-guard");

test("derived selector (fresh object) has a STABLE getSnapshot across reads", () => {
  const store = createWorldStore({ playerHp: 50, playerMaxHp: 100, playerMp: 10 });
  // A derived selector that builds a new object every call. WITHOUT the cache
  // this would return a fresh reference every read and loop React forever.
  const sub = mountSelector(store, (s) => ({ hp: s.playerHp, mp: s.playerMp }), shallowEqual);
  const a = sub.getSnapshot();
  const b = sub.getSnapshot();
  const c = sub.getSnapshot();
  assert.ok(Object.is(a, b) && Object.is(b, c), "repeat reads return one stable reference (no loop)");
});

test("derived selector does not re-render when an unrelated field changes", () => {
  const store = createWorldStore({ playerHp: 50, playerMp: 10 });
  const sub = mountSelector(store, (s) => ({ hp: s.playerHp, mp: s.playerMp }), shallowEqual);
  store.set({ gold: 999 }); // unrelated to hp/mp
  assert.equal(sub.renders, 0, "isEqual holds the previous reference → no render");
});

test("derived selector re-renders exactly once when a selected field changes", () => {
  const store = createWorldStore({ playerHp: 50, playerMp: 10 });
  const sub = mountSelector(store, (s) => ({ hp: s.playerHp, mp: s.playerMp }), shallowEqual);
  store.set({ playerHp: 49 });
  assert.equal(sub.renders, 1, "hp change → one render");
  assert.deepEqual(sub.lastValue, { hp: 49, mp: 10 });
  // A no-op change to an already-equal projection must not re-render again.
  store.set({ gold: 1 });
  assert.equal(sub.renders, 1, "subsequent unrelated change → still one render");
});

test("createSelectorMemo recomputes only when the snapshot reference changes", () => {
  let computeCount = 0;
  const memo = createSelectorMemo((s) => {
    computeCount += 1;
    return s.gold;
  });
  const snapA = { gold: 1 };
  const snapB = { gold: 2 };
  memo(snapA); // compute #1
  memo(snapA); // same ref → cached, no recompute
  memo(snapA);
  assert.equal(computeCount, 1, "same snapshot reference is not recomputed");
  memo(snapB); // new ref → compute #2
  assert.equal(computeCount, 2);
});

test("createSelectorMemo without isEqual returns each new value (Object.is semantics)", () => {
  const memo = createSelectorMemo((s) => s.value);
  const v1 = { id: 1 };
  const v2 = { id: 1 }; // structurally equal but a different reference
  assert.ok(Object.is(memo({ value: v1 }), v1));
  // No isEqual → a new snapshot with a new value reference yields the new value.
  assert.ok(Object.is(memo({ value: v2 }), v2));
});

// ── 4. shallowEqual helper ──────────────────────────────────────────────────

console.log("\n[use-world-selector] shallowEqual");

test("shallowEqual: same reference", () => {
  const o = { a: 1 };
  assert.equal(shallowEqual(o, o), true);
});

test("shallowEqual: equal one level deep", () => {
  assert.equal(shallowEqual({ a: 1, b: 2 }, { a: 1, b: 2 }), true);
});

test("shallowEqual: differing value", () => {
  assert.equal(shallowEqual({ a: 1, b: 2 }, { a: 1, b: 3 }), false);
});

test("shallowEqual: differing key count", () => {
  assert.equal(shallowEqual({ a: 1 }, { a: 1, b: 2 }), false);
});

test("shallowEqual: nested objects compared by reference (shallow)", () => {
  const inner = { x: 1 };
  assert.equal(shallowEqual({ inner }, { inner }), true);
  assert.equal(shallowEqual({ inner: { x: 1 } }, { inner: { x: 1 } }), false);
});

// ── 5. subscribeFull store contract ─────────────────────────────────────────

console.log("\n[world-model/store] subscribeFull");

test("subscribeFull fires on every state change", () => {
  const store = createWorldStore();
  let calls = 0;
  store.subscribeFull(() => {
    calls += 1;
  });
  store.set({ gold: 1 });
  store.upsertEntity(makeEntity("e1"));
  assert.equal(calls, 2, "fires for each distinct mutation regardless of slice");
});

test("subscribeFull does NOT fire when the updater returns the same state", () => {
  const store = createWorldStore();
  let calls = 0;
  store.subscribeFull(() => {
    calls += 1;
  });
  store.set((current) => current); // no-op updater → same reference
  assert.equal(calls, 0, "identical state reference must not notify");
});

test("subscribeFull unsubscribe stops the listener", () => {
  const store = createWorldStore();
  let calls = 0;
  const unsub = store.subscribeFull(() => {
    calls += 1;
  });
  store.set({ gold: 1 });
  unsub();
  store.set({ gold: 2 });
  assert.equal(calls, 1, "no further calls after unsubscribe");
});

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

console.log(`\n${passed} passed, ${failed} failed\n`);
if (failed > 0) process.exit(1);
