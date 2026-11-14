import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// Actual hook and Host sources; only React scheduling and the renderer ABI are
// fixtures. No browser, Core loader, WASM, socket or service is initialized.
function load(path, dependencies = {}) {
  const module = { exports: {} };
  const js = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("require", "exports", "module", js)(id => {
    assert(Object.hasOwn(dependencies, id), "unexpected pure dependency: " + id);
    return dependencies[id];
  }, module.exports, module);
  return module.exports;
}
const identity = load("../lib/world-model/item-identity.ts");
const belt = load("../lib/bag-belt-gesture.ts");
const bag = load("../lib/bevy-bag-ui.ts", { "./world-model/item-identity": identity, "./bag-belt-gesture": belt });
const npc = load("../lib/bevy-npc-shop-ui.ts", { "./bevy-bag-ui": bag });
const copy = value => JSON.parse(JSON.stringify(value));

function fixture() {
  const previousWindow = globalThis.window, previousDocument = globalThis.document;
  const listeners = new Map(), timers = new Map(), slots = [], pending = [];
  let cursor = 0, timer = 0, snapshot = null, status = null, sink = null;
  const effects = new Set(), snapshots = [], withdrawals = [];
  const react = {
    useRef(value) { return slots[cursor++] ??= { current: value }; },
    useState(initial) {
      const index = cursor++; slots[index] ??= { value: initial };
      return [slots[index].value, value => { slots[index].value = typeof value === "function" ? value(slots[index].value) : value; }];
    },
    useCallback(fn, deps) {
      const index = cursor++, previous = slots[index];
      if (!previous || deps.some((value, i) => !Object.is(value, previous.deps[i]))) slots[index] = { fn, deps };
      return slots[index].fn;
    },
    useEffect(fn, deps) {
      const index = cursor++, previous = slots[index];
      if (!previous || deps.some((value, i) => !Object.is(value, previous.deps[i]))) {
        const effect = { fn, deps, cleanup: null }; slots[index] = effect;
        pending.push(() => { previous?.cleanup?.(); if (previous) effects.delete(previous); effect.cleanup = fn(); effects.add(effect); });
      }
    },
  };
  globalThis.window = {
    setInterval(fn) { timers.set(++timer, fn); return timer; }, clearInterval(id) { timers.delete(id); },
    addEventListener(key, fn) { listeners.set(key, fn); }, removeEventListener(key, fn) { if (listeners.get(key) === fn) listeners.delete(key); },
  };
  globalThis.document = {
    addEventListener(key, fn) { listeners.set(key, fn); }, removeEventListener(key, fn) { if (listeners.get(key) === fn) listeners.delete(key); },
  };
  const hook = load("../lib/use-bevy-npc-shop-ui.ts", { react, "./bevy-npc-shop-ui": npc });
  const input = {
    connectionGeneration: 1, sessionGeneration: 1, ownerRevision: 1, playerObjectId: 1000,
    serviceRevision: 13, catalogRevision: 2, eligible: true, open: true, showBuy: true,
    presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false }, language: "en",
    shop: { goods: [{ unique_id: 7, name: "BaseDress(M)", price: 120, purchase_rate: 1,
      requires_gold_buy_plan: true, use_pearls: false, count: 0, stock: -1, panel_type: 0,
      icon: 1, icon_width: 0, icon_height: 0, description: "", tooltip_source: null }],
      selected_id: null, hide_added_stats: false, selected_bag_slot_for_sell: null, selected_bag_slot_for_repair: null,
      service_mode: "buy", supports_buy: true, supports_sell: true, repair_rate: null },
    inventory: { capacity: 46, gold: 150, items: [] },
    player: { hp: 53, maxHp: 60, mp: 35, maxMp: 35, gold: 150, credit: 0, level: 1,
      crystalStats: null, experience: 0, maxExperience: 1200, currentWeight: 0, maxWeight: 66,
      currentWeightKnown: false, weights: null, name: null, className: null, gender: null,
      hair: null, wingEffect: null, guildName: null, guildRankName: null, mapName: "0", inSafeZone: false },
    coreStatus: { authorityRevision: "4", canReserve: true, flight: null, lastPhase: null },
  };
  const runtime = {
    getMir2NpcShopUiCapabilities: () => JSON.stringify({ schemaVersion: 1, npcShopUiAbiVersion: 1, npcShopIntentAbiVersion: 1, compiled: true, startup: true }),
    setMir2NpcShopUiSnapshot(json) { snapshot = JSON.parse(json); snapshots.push(snapshot); if (status) status.ready = status.inputEnabled = false; return true; },
    getMir2NpcShopUiStatus: () => JSON.stringify(status ?? { runGeneration: 0 }),
    setMir2NpcShopUiIntentSink(fn) { sink = fn; }, clearMir2NpcShopUiIntentSink() { sink = null; },
    setMir2NpcShopUiPointerEdge: () => true,
    withdrawMir2NpcShopUiSnapshot(json) { withdrawals.push(JSON.parse(json)); if (status) status.ready = status.inputEnabled = false; return true; },
  };
  let options = { requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime },
    read: () => input, onOwner: () => {}, onIntent: () => false };
  function render(change = {}) { options = { ...options, ...change }; cursor = 0; return hook.useBevyNpcShopUi(options); }
  function flush() { while (pending.length) pending.shift()(); }
  function echo() {
    assert(snapshot);
    status = { ...Object.fromEntries(["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId"].map(key => [key, snapshot[key]])),
      frame: (status?.frame ?? 0) + 1, ready: true, inputEnabled: snapshot.inputEnabled,
      appliedRevision: snapshot.revision, appliedModelRevision: snapshot.modelRevision,
      appliedPresentationRevision: snapshot.presentationRevision, appliedServiceRevision: snapshot.serviceRevision,
      appliedCatalogRevision: snapshot.catalogRevision, coreAuthorityRevision: snapshot.coreAuthorityRevision,
      controlRevision: "7", selectedId: 7, quantity: 1, startIndex: 0, feedback: copy(snapshot.feedback),
      inputRegions: [{ left: 220, top: 217, width: 244, height: 334 }, { left: 474, top: 251, width: 330, height: 80 }], error: null };
  }
  function paint() { for (let n = 0; n < 3; n++) { echo(); for (const tick of timers.values()) tick(); } const ui = render(); assert(ui.active); return ui; }
  function cleanup() { for (const effect of effects) { effect.cleanup?.(); effect.cleanup = null; } }
  return { hook, input, render, flush, paint, snapshots, withdrawals, timers, listeners,
    get sink() { return sink; },
    strictReplay() { cleanup(); for (const effect of effects) effect.cleanup = effect.fn(); },
    dispose() {
      cleanup();
      if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
      if (previousDocument === undefined) delete globalThis.document; else globalThis.document = previousDocument;
    },
  };
}

test("actual NPC hook identity is readonly and stable across React state wrappers and positive HP renders", () => {
  const f = fixture();
  try {
    const unmounted = f.render(); assert.equal(unmounted.readHostIdentity(), null); f.flush();
    const first = f.paint(), host = first.readHostIdentity(); assert(host instanceof npc.NpcShopHost);
    const count = f.snapshots.length;
    f.input.player.hp--;
    const next = f.render(); assert.notEqual(next, first); assert.equal(next.readHostIdentity, first.readHostIdentity);
    assert.equal(next.readHostIdentity(), host); assert.equal(first.readHostIdentity(), host);
    assert.equal(f.snapshots.length, count, "render and identity reads cannot publish or arm input");
    assert.equal(f.timers.size, 1); assert.equal(f.listeners.size, 2);
  } finally { f.dispose(); }
});

test("actual NPC hook cleanup StrictMode restart runtime replacement and disabled effect retire the captured Host", () => {
  const f = fixture();
  try {
    const first = f.render(); f.flush(); f.paint(); const original = first.readHostIdentity(), retiredSink = f.sink;
    f.strictReplay(); const replayed = first.readHostIdentity();
    assert(replayed instanceof npc.NpcShopHost); assert.notEqual(replayed, original);
    assert.equal(retiredSink("{}"), false); assert.equal(original.readStatus(), null);
    assert.equal(f.timers.size, 1); assert.equal(f.listeners.size, 2);
    f.paint(); const secondSink = f.sink;
    f.render({ runtimeGeneration: 2 }); f.flush(); const replaced = first.readHostIdentity();
    assert(replaced instanceof npc.NpcShopHost); assert.notEqual(replaced, replayed);
    assert.equal(secondSink("{}"), false); assert.equal(replayed.readStatus(), null);
    f.render({ requested: false }); f.flush();
    assert.equal(first.readHostIdentity(), null); assert.equal(f.timers.size, 0); assert.equal(f.listeners.size, 0); assert.equal(f.sink, null);
    assert.equal(replaced.readStatus(), null); assert(f.withdrawals.length >= 3);
  } finally { f.dispose(); }
});

test("actual NPC hook error projection and Page public diagnostic expose only fixed codes", () => {
  const f = fixture();
  try {
    const codes = new Map([
      [null, null], ["Shared NPC host run exhausted", "hostRunExhausted"], ["Shared NPC host clock exhausted", "hostClockExhausted"],
      ["Shared NPC service clock regressed", "serviceClockRegressed"], ["Shared NPC cleanup failed", "cleanupFailed"],
      ["Shared NPC host clock unavailable", "clockUnavailable"], ["Shared NPC ABI 1 is unavailable", "abiUnavailable"],
      ["Shared NPC rejected the snapshot", "snapshotRejected"], ["Shared NPC is waiting for painted controls", "waitingForPaint"],
      ["Shared NPC input arming failed", "inputArmingFailed"], ["Shared NPC input handoff expired", "inputHandoffExpired"],
      ["Shared NPC unavailable", "hostUnavailable"], ["PRIVATE_EXCEPTION_CANARY", "hostError"],
    ]);
    const page = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8"), ast = ts.createSourceFile("page.tsx", page, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
    const expressions = [];
    function visit(node) { if (ts.isJsxAttribute(node) && node.name.getText(ast) === "npcShopInputDiagnostics") expressions.push(node.initializer.expression.getText(ast)); ts.forEachChild(node, visit); }
    visit(ast); assert.equal(expressions.length, 1);
    const read = new Function("scope", "with(scope){return (" + expressions[0] + ");}");
    const service = { supportsBuy: true };
    for (const [error, expected] of codes) {
      const errorCode = f.hook.npcShopHostErrorCode(error); assert.equal(errorCode, expected);
      const json = read({ npcShopService: service, npcShopServiceRef: { current: service }, npcGoldBuySourceFailureRef: { current: null },
        npcBuyDispatcherRef: { current: null }, runtimeRef: { current: {} }, questCoreRuntimeRef: { current: {} },
        bevyNpcShopUi: { active: false, transitioning: false, blocksInput: () => false, error, errorCode } });
      assert.equal(JSON.parse(json).host.errorCode, expected); assert.equal(JSON.parse(json).host.errorPresent, error !== null);
      assert.deepEqual(Object.keys(JSON.parse(json).host).sort(), ["active", "blocksInput", "errorCode", "errorPresent", "transitioning"]);
      assert(!json.includes("PRIVATE_EXCEPTION_CANARY"));
    }
    let reads = 0; assert.equal(f.hook.npcShopHostErrorCode({ toString() { reads++; throw Error("must not coerce"); } }), "hostError"); assert.equal(reads, 0);
  } finally { f.dispose(); }
});
