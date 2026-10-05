import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// Only these pure TS sources are evaluated by the eventual Node test runner.
// No client-core loader, generated WASM, React, DOM or renderer is initialized.
const modules = new Map();
const sources = {
  identity: "../lib/world-model/item-identity.ts",
  bag: "../lib/bevy-bag-ui.ts",
  npc: "../lib/bevy-npc-shop-ui.ts",
};
const dependencies = { identity: {}, bag: { "./world-model/item-identity": "identity" },
  npc: { "./bevy-bag-ui": "bag" } };
function loadPure(name, fresh = false) {
  assert(Object.hasOwn(sources, name), "source outside explicit pure allowlist: " + name);
  if (!fresh && modules.has(name)) return modules.get(name);
  const compiled = ts.transpileModule(readFileSync(new URL(sources[name], import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const module = { exports: {} };
  new Function("require", "exports", "module", compiled)(id => {
    assert(Object.hasOwn(dependencies[name], id), "unexpected pure " + name + " dependency: " + id);
    return loadPure(dependencies[name][id]);
  }, module.exports, module);
  if (!fresh) modules.set(name, module.exports);
  return module.exports;
}
const npc = loadPure("npc");
const clone = v => JSON.parse(JSON.stringify(v));
const capabilities = Object.freeze({ schemaVersion: 1, npcShopUiAbiVersion: 1,
  npcShopIntentAbiVersion: 1, compiled: true, startup: true });
const presentation = Object.freeze({ logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false });
function player() {
  return { hp: 100, maxHp: 100, mp: 50, maxMp: 50, gold: 100, credit: 0,
    crystalStats: null, level: 1, experience: 0, maxExperience: 0,
    currentWeight: 0, currentWeightKnown: false, weights: null, maxWeight: 0,
    name: null, className: null, gender: null, hair: null, wingEffect: null,
    guildName: null, guildRankName: null, mapName: "D000", inSafeZone: false };
}
function input() {
  const good = id => ({ unique_id: id, name: "Potion " + id, price: 17,
    use_pearls: false, count: 1, stock: -1, panel_type: 0, icon: 10,
    icon_width: 32, icon_height: 32, description: "Raw catalogue", purchase_rate: 0.9,
    requires_gold_buy_plan: true, tooltip_source: { info: { item_index: 658, price: 1,
      stack_size: 99, item_type: 13, stats: [] }, realInfo: null,
      userItem: { unique_id: id, item_index: 658, count: 1, is_shop_item: true },
      socketInfos: [], realSocketInfos: [] } });
  return { connectionGeneration: 1, sessionGeneration: 2, ownerRevision: 0, playerObjectId: 7,
    serviceRevision: 1, catalogRevision: 1, eligible: true, open: true, showBuy: true,
    presentation: clone(presentation), language: "en", player: player(),
    shop: { goods: Array.from({ length: 10 }, (_, i) => good(i)), selected_id: null,
      hide_added_stats: false, selected_bag_slot_for_sell: null, selected_bag_slot_for_repair: null,
      service_mode: "buy", supports_buy: true, supports_sell: true, repair_rate: null },
    inventory: { capacity: 46, gold: 100, items: [] },
    coreStatus: { authorityRevision: "9007199254740993", canReserve: true, flight: null, lastPhase: null } };
}
function fixture(overrides = {}) {
  let now = 100, live = input(), status = null, sink = null, state = null, host;
  let behavior = () => false, reads = 0, statusReads = 0;
  const snapshots = [], owners = [], intents = [], edges = [], withdrawals = [];
  const runtime = {
    getMir2NpcShopUiCapabilities: () => JSON.stringify(capabilities),
    setMir2NpcShopUiSnapshot(json) { snapshots.push(JSON.parse(json)); return true; },
    getMir2NpcShopUiStatus: () => { statusReads++; return JSON.stringify(status ?? { runGeneration: 0 }); },
    setMir2NpcShopUiIntentSink(callback) { sink = callback; },
    clearMir2NpcShopUiIntentSink() { sink = null; },
    setMir2NpcShopUiPointerEdge(json) { edges.push(JSON.parse(json)); return true; },
    withdrawMir2NpcShopUiSnapshot(json) { withdrawals.push(JSON.parse(json)); return true; },
    ...overrides,
  };
  function replace(lib = npc) {
    host = new lib.NpcShopHost({ runtime, read: () => { reads++; return live; }, now: () => now,
      onOwner(owner, runGeneration, ownerRevision) { owners.push({ owner, runGeneration, ownerRevision }); },
      onState(next) { state = next; },
      onIntent(intent) { intents.push(intent); return behavior(intent); },
    }); return host;
  }
  replace();
  const router = new npc.NpcShopPointerRouter();
  function echo(extra = {}) {
    const s = snapshots.at(-1); assert(s, "memory runtime echoes only an actually published snapshot");
    const scale = s.presentation.touch ? 1.28 / s.presentation.stageCssScale : 1;
    const left = (s.presentation.logicalWidth - 584 * scale) / 2;
    const top = (s.presentation.logicalHeight - 334 * scale) / 2;
    status = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision, playerObjectId: s.playerObjectId,
      frame: (status?.frame ?? 0) + 1, ready: true, inputEnabled: s.inputEnabled,
      appliedRevision: s.revision, appliedModelRevision: s.modelRevision,
      appliedPresentationRevision: s.presentationRevision, appliedServiceRevision: s.serviceRevision,
      appliedCatalogRevision: s.catalogRevision, coreAuthorityRevision: s.coreAuthorityRevision,
      controlRevision: "9007199254740995", selectedId: null, quantity: 1, startIndex: 0,
      feedback: clone(s.feedback), inputRegions: [
        { left, top, width: 244 * scale, height: 334 * scale },
        { left: left + 254 * scale, top: top + 34 * scale, width: 330 * scale, height: 80 * scale },
      ], error: null, ...extra };
    return status;
  }
  function activate() {
    host.tick(); echo(); host.tick(); echo(); host.tick();
    assert.equal(state.active, true); assert.equal(state.transitioning, false);
  }
  function intent(extra = {}) {
    const s = snapshots.at(-1), current = status;
    assert(s && current);
    return { proof: { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision, playerObjectId: s.playerObjectId,
      revision: s.revision, modelRevision: s.modelRevision, presentationRevision: s.presentationRevision,
      serviceRevision: s.serviceRevision, catalogRevision: s.catalogRevision,
      coreAuthorityRevision: s.coreAuthorityRevision, controlRevision: current.controlRevision },
      gesture: { pointerId: 1, downSequence: 1, sequence: 2, origin: "shop", button: 0 },
      intentSequence: 1, action: { type: "select", uniqueId: 0 },
      selectedId: current.selectedId, quantity: current.quantity, startIndex: current.startIndex,
      preentryWithdraw: true, command: null, ...extra };
  }
  function tapIntent(extra = {}) {
    const c = host.pointerContext(); assert(c, "requires actual admitted current pointer context");
    const region = c.inputRegions[0], x = region.left + 20, y = region.top + 40;
    const down = router.down(c, 1, 0, x, y); assert(down); assert.equal(host.pointer(down), true);
    const up = router.edge("up", 1, x, y); assert(up); assert.equal(host.pointer(up), true);
    return intent({ ...extra, gesture: { pointerId: up.pointerId, downSequence: up.downSequence,
      sequence: up.sequence, origin: up.origin, button: up.button } });
  }
  return { runtime, snapshots, owners, intents, edges, withdrawals, replace, echo, activate, intent, tapIntent, router,
    get host() { return host; }, get reads() { return reads; }, get statusReads() { return statusReads; }, get input() { return live; }, set input(v) { live = v; },
    get state() { return state; }, get sink() { return sink; }, get status() { return status; }, set status(v) { status = v; },
    set behavior(callback) { behavior = callback; }, time(ms) { now += ms; },
    emit(i) { assert.equal(typeof sink, "function"); return sink(JSON.stringify(i)); },
  };
}


test("independent NPC ABI requires seven hooks and compiled startup without extending old capabilities", () => {
  assert.equal(npc.supportsNpcShopUi(fixture().runtime), true);
  for (const patch of [{ schemaVersion: 0 }, { npcShopUiAbiVersion: 0 }, { npcShopIntentAbiVersion: 0 },
    { compiled: false }, { startup: false }]) {
    const f = fixture({ getMir2NpcShopUiCapabilities: () => JSON.stringify({ ...capabilities, ...patch }) });
    f.host.tick(); assert.equal(npc.supportsNpcShopUi(f.runtime), false);
    assert.equal(f.state.active, false); assert.equal(f.state.transitioning, false);
    assert.equal(f.snapshots.length, 0); assert.equal(f.sink, null);
  }
  for (const name of ["setMir2NpcShopUiSnapshot", "getMir2NpcShopUiStatus", "setMir2NpcShopUiIntentSink",
    "clearMir2NpcShopUiIntentSink", "setMir2NpcShopUiPointerEdge", "withdrawMir2NpcShopUiSnapshot"]) {
    const f = fixture({ [name]: undefined }); f.host.tick();
    assert.equal(npc.supportsNpcShopUi(f.runtime), false, name); assert.equal(f.snapshots.length, 0, name);
  }
});

test("whole ordinary directory handoff preserves typed raw models and all twenty-three player fields", () => {
  const f = fixture(); f.host.tick(); const snapshot = f.snapshots.at(-1);
  assert.equal(Object.keys(snapshot.player).length, 23); assert.deepEqual(snapshot.player, f.input.player);
  assert.deepEqual(snapshot.inventory, f.input.inventory); assert.deepEqual(snapshot.shop.goods, f.input.shop.goods);
  assert.equal(snapshot.shop.goods[0].unique_id, 0); assert.equal(snapshot.shop.goods[0].price, 17);
  assert.equal(snapshot.shop.goods[0].purchase_rate, 0.9);
  assert.equal(snapshot.coreAuthorityRevision, "9007199254740993");
  assert.deepEqual(snapshot.feedback, { phase: null, pending: false, canReserve: true, previousUnknown: 0 });
  assert.equal(snapshot.inputEnabled, false); assert.equal(Object.hasOwn(snapshot, "eligible"), false);
  assert.equal(Object.hasOwn(snapshot, "coreStatus"), false); assert.equal(f.input.ownerRevision, 0, "readonly owner notification");
});

test("off-page Pearl finite panel and unmarked catalogs preserve whole legacy while marked missing raw remains explicit", () => {
  for (const change of [i => { i.shop.goods[9].use_pearls = true; }, i => { i.shop.goods[9].stock = 0; },
    i => { i.shop.goods[9].panel_type = 1; }, i => { i.showBuy = false; },
    i => { i.shop.goods[9].requires_gold_buy_plan = false; i.shop.goods[9].purchase_rate = null; i.shop.goods[9].tooltip_source = null; }]) {
    const f = fixture(); change(f.input); f.host.tick();
    assert.equal(f.state.active, false); assert.equal(f.state.transitioning, false); assert.equal(f.host.blocksInput(), false);
    assert.equal(f.snapshots.length, 0); assert.equal(f.host.pointerContext(), null);
  }
  const f = fixture(); f.input.shop.goods[0].tooltip_source = null; f.input.shop.goods[0].purchase_rate = null;
  f.host.tick(); assert.equal(f.snapshots.at(-1).shop.goods[0].tooltip_source, null);
  assert.equal(f.snapshots.at(-1).shop.goods[0].purchase_rate, null);
  assert.equal(f.snapshots.at(-1).shop.goods[0].requires_gold_buy_plan, true);
});

test("prepare and arming use two actual echoes before sole ownership and synchronous input blocking", () => {
  const f = fixture(); f.host.tick();
  assert.equal(f.state.active, false); assert.equal(f.host.blocksInput(), false);
  assert.equal(f.host.pointerContext(), null); assert.equal(f.snapshots.at(-1).inputEnabled, false);
  f.echo(); f.host.tick(); assert.equal(f.state.transitioning, true); assert.equal(f.state.active, false);
  assert.equal(f.host.blocksInput(), true); assert.equal(f.host.pointerContext(), null);
  const counts = [f.snapshots.length, f.owners.length]; f.host.tick();
  assert.deepEqual([f.snapshots.length, f.owners.length], counts, "no repeated ownership transition without enabled applied proof");
  f.echo(); f.host.tick(); assert.equal(f.state.active, true); assert.equal(f.host.blocksInput(), true);
  assert(f.host.readStatus()); assert(f.host.pointerContext()); assert.equal(f.input.ownerRevision, 0);
});

test("status full-u64 and every applied clock identity or control mismatch rejects final input", () => {
  for (const field of ["runGeneration", "connectionGeneration", "sessionGeneration", "ownerRevision", "playerObjectId",
    "appliedRevision", "appliedModelRevision", "appliedPresentationRevision", "appliedServiceRevision", "appliedCatalogRevision",
    "coreAuthorityRevision"]) {
    const f = fixture(); f.activate(); const raw = f.tapIntent(); assert.equal(f.emit(raw), false);
    const i = f.intents.at(-1); assert(f.host.allows(i));
    const wrong = typeof f.status[field] === "string" ? "18446744073709551615" : f.status[field] + 1;
    f.echo({ [field]: wrong }); assert.equal(f.host.allows(i), false, field); assert.equal(f.host.claim(i), false, field);
    assert.equal(f.emit(i), false, field); assert.equal(f.intents.length, 1, field);
  }
  for (const invalid of [1, "0", "01", "+1", "18446744073709551616"]) {
    const f = fixture(); f.activate(); f.echo({ controlRevision: invalid }); assert.equal(f.host.readStatus(), null);
  }
  const f = fixture(); f.input.coreStatus.authorityRevision = "18446744073709551615"; f.activate();
  f.echo({ controlRevision: "18446744073709551615" });
  const i = f.tapIntent(); assert.equal(i.proof.coreAuthorityRevision, "18446744073709551615");
  assert.equal(i.proof.controlRevision, "18446744073709551615"); assert(npc.parseNpcShopIntent(JSON.stringify(i)));
});

test("missing player raw fields malformed Core and unsafe DTOs do not fabricate a prepared surface", () => {
  for (const change of [i => { delete i.player.crystalStats; }, i => { delete i.player.guildName; },
    i => { i.player.extra = true; }, i => { i.player.hp = Number.NaN; }, i => { i.player.hair = 256; },
    i => { i.playerObjectId = 0; }, i => { i.serviceRevision = 0; }, i => { i.inventory.capacity = 47; },
    i => { i.coreStatus.authorityRevision = 9007199254740992; }, i => { i.coreStatus.authorityRevision = "01"; },
    i => { i.coreStatus = null; }, i => { i.shop.goods[1].unique_id = 0; },
    i => { delete i.shop.selected_id; }, i => { delete i.shop.selected_bag_slot_for_sell; },
    i => { delete i.shop.repair_rate; }, i => { i.shop.goods[0].price = 17.75; }]) {
    const f = fixture(); change(f.input); f.host.tick(); assert.equal(f.state.active, false);
    assert.equal(f.host.pointerContext(), null); assert.equal(f.host.blocksInput(), false);
  }
});

test("whole 584px notice and current status clip are required while narrow touch falls back", () => {
  const narrow = fixture(); narrow.input.presentation = { logicalWidth: 640, logicalHeight: 480, stageCssScale: 1, touch: true };
  assert.equal(npc.fitsNpcShop(narrow.input.presentation), false); narrow.host.tick();
  assert.equal(narrow.snapshots.length, 0); assert.equal(narrow.host.blocksInput(), false);
  for (const patch of [{ ready: false }, { inputEnabled: false }, { error: "font or layout unavailable" },
    { inputRegions: [{ left: 220, top: 217, width: 244, height: 334 }] },
    { inputRegions: [{ left: -1, top: 10, width: 244, height: 334 }, { left: 253, top: 44, width: 330, height: 80 }] }]) {
    const f = fixture(); f.activate(); const raw = f.tapIntent(); f.emit(raw);
    const i = f.intents.at(-1); assert(f.host.allows(i)); f.echo(patch);
    assert.equal(f.host.pointerContext(), null); assert.equal(f.host.allows(i), false); assert.equal(f.host.claim(i), false);
  }
  const touch = fixture(); touch.input.presentation.touch = true; touch.activate(); assert(touch.host.pointerContext());
});

test("runtime intents require actual ordered pointer provenance and exact nested DTOs", () => {
  const f = fixture(); f.activate(); const naked = f.intent();
  assert.equal(f.emit(naked), false); assert.equal(f.intents.length, 0);
  const raw = f.tapIntent(); assert.equal(f.emit(raw), false); assert.equal(f.intents.length, 1);
  const i = f.intents.at(-1); assert.equal(f.host.allows(i), true); assert.notEqual(i, raw); assert(Object.isFrozen(i));
  for (const alter of [v => { v.extra = true; }, v => { v.proof.controlRevision = 1; },
    v => { v.gesture.downSequence++; }, v => { v.action.uniqueId = 1; },
    v => { v.preentryWithdraw = false; }, v => { v.command = { type: "buyItem", itemIndex: 0, count: 1, panelType: 0 }; }]) {
    const changed = clone(i); alter(changed); assert.equal(f.host.claim(changed), false);
  }
  assert.equal(f.host.claim(i), true); assert.equal(f.host.claim(i), false); assert.equal(f.emit(i), false);
});

test("false and throw retain common locals without replaying a callback sequence", () => {
  for (const response of [() => false, () => { throw Error("controlled callback failure"); }]) {
    const f = fixture(); f.activate(); const i = f.tapIntent(); f.behavior = response;
    const before = clone(f.status); assert.equal(f.emit(i), false); assert.equal(f.emit(i), false);
    assert.equal(f.intents.length, 1); assert.equal(f.status.selectedId, before.selectedId);
    assert.equal(f.status.quantity, before.quantity); assert.equal(f.status.startIndex, before.startIndex);
    const admitted = f.intents.at(-1); assert.equal(f.host.allows(admitted), true);
    assert.equal(f.host.claim(admitted), true); assert.equal(f.host.claim(admitted), false);
  }
});

test("live owner raw model service catalog player or presentation mutation fences admitted proof synchronously", () => {
  for (const change of [i => { i.ownerRevision++; }, i => { i.sessionGeneration++; }, i => { i.connectionGeneration++; },
    i => { i.playerObjectId++; }, i => { i.serviceRevision++; }, i => { i.catalogRevision++; },
    i => { i.inventory.gold++; }, i => { i.shop.goods[0].tooltip_source.info.price++; },
    i => { i.shop.goods[9].description += " replaced"; }, i => { i.player.level++; },
    i => { i.presentation.logicalWidth = 1200; }, i => { i.language = "zh-CN"; }, i => { i.open = false; }]) {
    const f = fixture(); f.activate(); const raw = f.tapIntent(); f.emit(raw);
    const i = f.intents.at(-1); assert(f.host.allows(i));
    change(f.input); assert.equal(f.host.allows(i), false); assert.equal(f.host.claim(i), false);
    assert.equal(f.emit({ ...i, intentSequence: 2 }), false); assert.equal(f.intents.length, 1);
  }
});

test("readonly transport feedback changes its publication without inventing model change history or Buy ACK", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    const f = fixture(); f.activate(); const raw = f.tapIntent(); f.emit(raw);
    const i = f.intents.at(-1); assert(f.host.allows(i)); const before = clone(f.snapshots.at(-1));
    f.input.coreStatus = { authorityRevision: before.coreAuthorityRevision, canReserve: false,
      flight: { token: "9007199254740997", authorityRevision: before.coreAuthorityRevision,
        ticket: { transport: "1", body: '{"type":"buyItem","itemIndex":0,"count":1,"panelType":0}' }, phase }, lastPhase: phase };
    assert.equal(f.host.allows(i), false); f.host.tick(); const after = f.snapshots.at(-1);
    assert.equal(after.modelRevision, before.modelRevision); assert.deepEqual(after.inventory, before.inventory);
    assert.deepEqual(after.feedback, { phase, pending: true, canReserve: false, previousUnknown: 0 });
    assert.equal(after.coreAuthorityRevision, before.coreAuthorityRevision); assert.equal(Object.hasOwn(after.feedback, "ack"), false);
  }
});

test("deferred Close retires after accepted synchronous callback rather than withdrawing inside it", () => {
  const f = fixture(); f.activate(); const i = f.tapIntent({ action: { type: "close" } });
  const before = [f.snapshots.length, f.withdrawals.length];
  f.behavior = intent => {
    assert(f.host.allows(intent)); assert.equal(f.host.deferRetirement(intent), true);
    assert.deepEqual([f.snapshots.length, f.withdrawals.length], before);
    assert.equal(f.host.blocksInput(), true); return true;
  };
  assert.equal(f.emit(i), true); assert.deepEqual([f.snapshots.length, f.withdrawals.length], before);
  assert.equal(f.host.claim(f.intents.at(-1)), false); f.host.tick(); assert.equal(f.state.active, false);
});

test("synchronous callback reentry or current source replacement cannot accept old proof", () => {
  for (const change of [f => f.host.withdraw(), f => { f.input.serviceRevision++; }, f => { f.input.inventory.gold++; },
    f => { f.replace(loadPure("npc", true)); }]) {
    const f = fixture(); f.activate(); const i = f.tapIntent(); f.behavior = intent => {
      assert.equal(f.emit(intent), false, "same callback sequence cannot recurse"); change(f); return f.host.claim(intent);
    };
    assert.equal(f.emit(i), false); assert.equal(f.intents.length, 1); assert.equal(f.status.selectedId, null);
  }
});

test("same-runtime HMR allocates above actual prior run and stale cleanup cannot clear new host", () => {
  const f = fixture(); f.activate(); const old = f.host, oldSink = f.sink, oldIntent = f.tapIntent();
  f.replace(loadPure("npc", true)); f.activate();
  assert(f.snapshots.at(-1).runGeneration > oldIntent.proof.runGeneration);
  const currentSink = f.sink, before = [f.snapshots.length, f.withdrawals.length, f.owners.length];
  old.withdraw(); old.stop(); old.tick();
  assert.equal(f.sink, currentSink); assert.deepEqual([f.snapshots.length, f.withdrawals.length, f.owners.length], before);
  assert.equal(oldSink(JSON.stringify(oldIntent)), false); const current = f.tapIntent({ intentSequence: 2 });
  assert.equal(f.emit(current), false); assert(f.host.allows(f.intents.at(-1)));
});

test("stopped frames and permanent run exhaustion fail closed without replacing a Core", () => {
  const f = fixture(); f.activate(); const raw = f.tapIntent(); f.emit(raw); const i = f.intents.at(-1); assert(f.host.allows(i)); f.time(2001);
  assert.equal(f.host.allows(i), false); assert.equal(f.host.claim(i), false); f.host.tick(); assert.equal(f.state.active, false);
  const seed = fixture(); seed.activate(); const high = clone(seed.status); high.runGeneration = Number.MAX_SAFE_INTEGER;
  const exhausted = fixture({ getMir2NpcShopUiStatus: () => JSON.stringify(high) }); exhausted.host.tick();
  assert.equal(exhausted.snapshots.length, 0); assert.equal(exhausted.host.pointerContext(), null);
  assert.equal(exhausted.host.blocksInput(), false);
});

test("world origin never becomes shop and quarantined old nonce cannot clear a new lease", () => {
  const f = fixture(); f.activate(); const context = f.host.pointerContext();
  const down = f.router.down(context, 1, 0, 900, 700); assert.equal(down.origin, "world"); assert.equal(f.host.pointer(down), true);
  const move = f.router.edge("move", 1, 250, 260); assert.equal(move.origin, "world"); assert.equal(f.host.pointer(move), true);
  const up = f.router.edge("up", 1, 250, 260); assert.equal(up.origin, "world"); assert.equal(f.host.pointer(up), true);
  const forged = f.intent({ gesture: { pointerId: up.pointerId, downSequence: up.downSequence, sequence: up.sequence,
    origin: "shop", button: up.button } }); assert.equal(f.emit(forged), false); assert.equal(f.intents.length, 0);
  const old = f.router.down(context, 1, 0, 240, 260); assert.equal(f.host.pointer(old), true);
  const cancel = f.router.cancel("blur"); assert.equal(f.host.pointer(cancel), true);
  f.host.withdraw(); f.activate(); const fresh = f.router.down(f.host.pointerContext(), 1, 0, 240, 260);
  assert.equal(f.host.pointer(fresh), true); const late = f.router.edge("up", 1, 240, 260);
  assert.equal(late.runGeneration, old.runGeneration); assert.equal(f.router.held, fresh);
  assert.equal(f.host.pointer(late), true); assert.equal(f.router.held, fresh);
  assert.equal(f.edges.at(-1).phase, "cancel"); assert.equal(f.edges.at(-1).downSequence, old.downSequence);
  const currentUp = f.router.edge("up", 1, 240, 260); assert.equal(currentUp.downSequence, fresh.downSequence);
  assert.equal(f.host.pointer(currentUp), true); assert.equal(f.router.held, null);
});


test("readStatus is readonly and cannot read input Core or attach another producer", () => {
  const f = fixture(); f.activate(); const reads = f.reads, before = [f.snapshots.length, f.edges.length, f.owners.length];
  const status = f.host.readStatus(); assert(status); assert.equal(status.coreAuthorityRevision, "9007199254740993");
  assert.equal(f.reads, reads); assert.deepEqual([f.snapshots.length, f.edges.length, f.owners.length], before);
});

test("only current synchronous sink may finish after expected preentry feedback changes", () => {
  const f = fixture(); f.activate(); const i = f.tapIntent(); const model = f.snapshots.at(-1).modelRevision;
  f.behavior = admitted => {
    assert(f.host.allows(admitted));
    // Scripted readonly post-withdraw status, not a Core transition oracle.
    f.input.coreStatus = { authorityRevision: "9007199254740993", canReserve: false, flight: null, lastPhase: null };
    assert(f.host.allows(admitted), "current local-action callback can finish after exact Core withdrawal");
    assert(f.host.claim(admitted)); return true;
  };
  assert.equal(f.emit(i), true); assert.equal(f.intents.length, 1);
  assert.equal(f.status.selectedId, null, "JS host never applies common selection itself");
  assert.equal(f.snapshots.at(-1).modelRevision, model); assert.equal(f.host.claim(f.intents[0]), false);
});

test("callback withdraw and unmount stop clean up only after the synchronous Last checkpoint", async () => {
  for (const stop of [false, true]) {
    const f = fixture(); f.activate(); const raw = f.tapIntent(); const before = f.withdrawals.length;
    f.behavior = admitted => {
      assert(f.host.allows(admitted));
      if (stop) f.host.stop(); else f.host.withdraw();
      assert.equal(f.withdrawals.length, before, "no runtime mutation inside the current sink callback");
      assert.equal(f.host.blocksInput(), true); assert.equal(f.host.claim(admitted), false); return true;
    };
    assert.equal(f.emit(raw), false); assert.equal(f.withdrawals.length, before);
    await Promise.resolve();
    assert.equal(f.withdrawals.length, before + 1, "cleanup works even with no next timer tick");
    assert.equal(f.host.pointerContext(), null); assert.equal(f.host.blocksInput(), false);
    if (stop) assert.equal(f.sink, null);
  }
});

test("a refused current Up RPC restores its exact Down lease and cannot manufacture an admitted intent", () => {
  // Keep the captured method identity unchanged: the memory RPC outcome alone changes.
  const rejected = fixture({ setMir2NpcShopUiPointerEdge(json) {
    const edge = JSON.parse(json); return edge.phase !== "up";
  } });
  rejected.activate(); const context = rejected.host.pointerContext(), region = context.inputRegions[0];
  const d = rejected.router.down(context, 1, 0, region.left + 20, region.top + 40);
  assert(rejected.host.pointer(d)); const u = rejected.router.edge("up", 1, region.left + 20, region.top + 40);
  assert.equal(rejected.host.pointer(u), false);
  assert.equal(rejected.emit(rejected.intent({ gesture: { pointerId:u.pointerId, downSequence:u.downSequence,
    sequence:u.sequence, origin:u.origin, button:u.button } })), false);
  assert.equal(rejected.intents.length, 0);
  // The refused terminal did not clear the admitted lease: a matching cancel can retire it.
  assert.equal(rejected.host.pointer({ ...u, phase:"cancel", sequence:u.sequence + 1 }), true);
  assert.equal(rejected.host.pointer({ ...d, sequence:u.sequence + 2, downSequence:u.sequence + 2 }), true, "after exact cancellation a fresh Down can be admitted");
});

test("actual intent decoder rejects unknown unsafe and noncanonical precision fields", () => {
  const f = fixture(); f.activate(); const raw = f.tapIntent(); assert(npc.parseNpcShopIntent(JSON.stringify(raw)));
  for (const mutate of [i => { i.extra = true; }, i => { i.proof.extra = true; },
    i => { i.gesture.origin = "storage"; }, i => { i.gesture.downSequence = 0; },
    i => { i.proof.coreAuthorityRevision = 9007199254740993; },
    i => { i.proof.controlRevision = "01"; }, i => { i.proof.controlRevision = "18446744073709551616"; },
    i => { i.proof.modelRevision = Number.MAX_SAFE_INTEGER + 1; }]) {
    const bad = clone(raw); mutate(bad); assert.equal(npc.parseNpcShopIntent(JSON.stringify(bad)), null);
  }
});


test("pure final UI claimCurrent is only the current claimed Buy callback and rejects deferred stop or withdrawal", async () => {
  for (const retirement of [null,"withdraw","stop"]) {
    const f=fixture();f.activate();f.echo({selectedId:0,quantity:2});
    const raw=f.tapIntent({action:{type:"buy"},preentryWithdraw:false,command:{type:"buyItem",itemIndex:0,count:2,panelType:0}});
    f.behavior=i=>{
      assert.equal(f.host.claimCurrent(i),false);assert(f.host.claim(i));
      const counts=[f.reads,f.statusReads,f.snapshots.length,f.edges.length,f.withdrawals.length];
      assert.equal(f.host.claimCurrent(i),true);assert.equal(f.host.claimCurrent(clone(i)),false);
      assert.deepEqual([f.reads,f.statusReads,f.snapshots.length,f.edges.length,f.withdrawals.length],counts,"final checkpoint is pure and cannot read Core/source/status or mutate ABI");
      if(retirement){f.host[retirement]();assert.equal(f.host.claimCurrent(i),false);
        assert.equal(f.withdrawals.length,counts[4],"deferred cleanup has not changed same renderer status yet");}
      return f.host.claimCurrent(i);
    };
    assert.equal(f.emit(raw),retirement===null);assert.equal(f.host.claimCurrent(f.intents[0]),false,"callback completion burns final UI checkpoint");
    await Promise.resolve();
  }
});


test("actual Router exact one-time terminal cancel retires a refused Host Up and permits the next manual Down",()=>{
  const edges=[];const f=fixture({setMir2NpcShopUiPointerEdge(json){const e=JSON.parse(json);edges.push(e);return e.phase!=="up";}});
  f.activate();const c=f.host.pointerContext(),r=c.inputRegions[0],x=r.left+20,y=r.top+40;
  const down=f.router.down(c,1,0,x,y);assert(f.host.pointer(down));const up=f.router.edge("up",1,x,y);
  assert.equal(f.host.pointer(up),false);assert.equal(f.router.held,null);
  assert.equal(f.router.cancelTerminal(clone(up)),null,"structural clone is not the consumed terminal");
  const cancel=f.router.cancelTerminal(up);assert(cancel);assert.equal(cancel.sequence,up.sequence+1);assert.equal(cancel.phase,"cancel");
  for(const key of ["runGeneration","connectionGeneration","sessionGeneration","ownerRevision","playerObjectId","revision","modelRevision",
    "presentationRevision","serviceRevision","catalogRevision","coreAuthorityRevision","controlRevision","pointerId","downSequence","origin","button"])
    assert.equal(cancel[key],up[key],key);
  assert.equal(f.router.cancelTerminal(up),null);assert(f.host.pointer(cancel));
  const next=f.router.down(f.host.pointerContext(),1,0,x,y);assert(next);assert(f.host.pointer(next));assert.equal(f.router.held,next);
  assert.deepEqual(edges.map(e=>e.phase),["down","up","cancel","down"]);
});

test("actual terminal recovery refuses newer live lease and quarantined old Up without advancing or clearing it",()=>{
  const f=fixture();f.activate();const c=f.host.pointerContext(),r=c.inputRegions[0],x=r.left+20,y=r.top+40;
  const first=f.router.down(c,1,0,x,y),terminal=f.router.edge("up",1,x,y);const second=f.router.down(c,1,0,x,y);
  assert.equal(f.router.cancelTerminal(terminal),null);assert.equal(f.router.held,second);
  const retired=f.router.cancel();assert(retired);const fresh=f.router.down(c,1,0,x,y);
  const oldUp=f.router.edge("up",1,x,y);assert.equal(oldUp.downSequence,second.downSequence);assert.equal(f.router.held,fresh);
  assert.equal(f.router.cancelTerminal(oldUp),null);assert.equal(f.router.cancelTerminal(retired),null);assert.equal(f.router.held,fresh);
  const currentUp=f.router.edge("up",1,x,y);assert.equal(currentUp.downSequence,fresh.downSequence);
  assert.equal(currentUp.sequence,oldUp.sequence+1,"refused recovery calls do not advance the current sequence");
  const exact=f.router.cancelTerminal(currentUp);assert(exact);assert.equal(exact.downSequence,fresh.downSequence);assert.equal(exact.sequence,currentUp.sequence+1);
  assert.equal(f.router.cancelTerminal(currentUp),null);
});

test("NPC host alone admits current attested grid aliases and withdraws malformed or ambiguous listed roots", () => {
  const f=fixture();
  f.input.inventory.items=[{container:0,slot:0,uniqueId:0},{container:1,slot:0,uniqueId:0}];
  f.input.inventory.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[]};
  f.activate();assert.equal(f.state.active,true);
  assert.deepEqual(f.snapshots.at(-1).inventory.npcGoldTradeCapacity,f.input.inventory.npcGoldTradeCapacity);
  f.input.inventory.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[0]};
  f.host.tick();assert.equal(f.state.active,false);
  assert.equal(f.owners.at(-1).owner,"react");
});
