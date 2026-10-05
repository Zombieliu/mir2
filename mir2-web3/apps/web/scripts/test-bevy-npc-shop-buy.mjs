import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// The repository's existing source-test loader. Imports are type-only: no
// runtime/WASM initialization, product socket, browser or gateway dependency.
const compiled = ts.transpileModule(readFileSync(new URL("../lib/bevy-npc-shop-buy.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
const module = { exports: {} };
new Function("require", "exports", "module", compiled.outputText)(
  name => { throw new Error("Unexpected runtime dependency " + name); }, module.exports, module,
);
const { quoteNpcGoldBuy, NpcGoldBuyDispatcher } = module.exports;
const copy = value => JSON.parse(JSON.stringify(value));
function current() {
  const good = (unique_id, name) => ({ unique_id, name, price: 17.75, use_pearls: false,
    count: 1, stock: 100, panel_type: 0, icon: 10, icon_width: 32, icon_height: 32,
    description: "Raw catalogue", purchase_rate: 0.9, requires_gold_buy_plan: true,
    tooltip_source: { info: { index: 5, stack_size: 20, stats: { min_dc: 2 } },
      userItem: { unique_id, count: 1 }, fullraw: { unchanged: [1, 2, 3] } } });
  return { owner: { connectionGeneration: 1, sessionGeneration: 2, ownerRevision: 0, playerObjectId: 7 },
    serviceRevision: 1, catalogRevision: 1,
    presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false },
    shop: { goods: [good(77, "Potion"), good(78, "Other")], selected_id: 77,
      hide_added_stats: false, selected_bag_slot_for_sell: null, selected_bag_slot_for_repair: null,
      service_mode: "buy", supports_buy: true, supports_sell: true, repair_rate: null },
    inventory: { capacity: 40, gold: 100, items: [{ uniqueId: 100, key: "owned", name: "Owned",
      quantity: 1, slot: 0, container: 0, icon: 10, description: "Whole inventory",
      tooltipSource: { info: { index: 6 }, userItem: { unique_id: 100, count: 1 } } }] }, blocked: false };
}
// A fixed Rust response fixture, not a replica of pricing or capacity rules.
function accepted(request) {
  return { maxQuantity: 17, totalGold: 37, canBuy: true, blockReason: null,
    command: { type: "buyItem", itemIndex: request.shop.selected_id, count: request.quantity, panelType: 0 } };
}

// Extract complete production validators/facade declarations, never the Core
// loader, generated module, WASM constructor, React or a renderer instance.
const coreSource = readFileSync(new URL("../lib/client-core-runtime.ts", import.meta.url), "utf8");
const coreAst = ts.createSourceFile("actual-npc-attempt-core.ts", coreSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const coreNames = ["slotUtf8", "slotDecimal", "npcAttemptFields", "npcAttemptTicket",
  "npcAttemptRequest", "npcAttemptConnectionRequest", "npcAttemptResult", "persistentNpcGoldBuyAttemptSlot"];
const coreDeclarations = new Map();
function visitCore(node) {
  if (ts.isFunctionDeclaration(node) && node.name && coreNames.includes(node.name.text)) {
    assert(!coreDeclarations.has(node.name.text), "ambiguous actual Core facade declaration");
    coreDeclarations.set(node.name.text, node.getText(coreAst));
  }
  ts.forEachChild(node, visitCore);
}
visitCore(coreAst);
assert.deepEqual([...coreDeclarations.keys()].sort(), [...coreNames].sort());
const coreJavaScript = ts.transpileModule(coreNames.map(name => coreDeclarations.get(name)).join("\n"), {
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
function actualCoreFacade() {
  return new Function(coreJavaScript + "\nreturn {" + coreNames.join(",") + "};")();
}
const { persistentNpcGoldBuyAttemptSlot, npcAttemptRequest, npcAttemptConnectionRequest, npcAttemptResult } = actualCoreFacade();
const baseWire = Object.freeze({ type: "buyItem", itemIndex: 77, count: 2, panelType: 0 });
const baseTicket = Object.freeze({ transport: "1", body: JSON.stringify(baseWire) });
// These DTOs are explicit protocol responses, not a second attempt state
// machine. The oracle never compares authority, allocates a token, prices an
// item, or advances a phase. Tests install every response/queue themselves.
function decision(phase = null, { token = "9007199254740993", revision = "1",
  ticket = baseTicket, matched = true, canReserve = phase === null, flightRevision = revision } = {}) {
  return { ok: true, matched, state: { authorityRevision: revision, canReserve,
    flight: phase === null ? null : { token, authorityRevision: flightRevision,
      ticket: phase === "queued" ? null : copy(ticket), phase }, lastPhase: phase } };
}
function protocolOracle() {
  const responses = new Map(), queues = new Map(), transcripts = [], instances = [], attempts = [];
  const oracle = { transcripts, instances, attempts,
    set(key, value) { responses.set(key, value); queues.delete(key); },
    queue(key, values) { assert(values.length); queues.set(key, [...values]); },
    ready(token = "9007199254740993", revision = "1", ticket = baseTicket) {
      this.set("connection", decision(null, { revision, canReserve: false }));
      for (const op of ["status", "observe", "availability", "rejectUnpublished"]) this.set(op, decision(null, { revision }));
      this.queue("reserve", [decision("queued", { token, revision }), decision("bound", { token, revision, ticket, matched: false })]);
      for (const op of ["bind", "allows"]) this.set(op, decision("bound", { token, revision, ticket }));
      this.set("enter", decision("entered", { token, revision, ticket }));
      this.set("receipt:definitelyUnsent", decision(null, { revision }));
      this.set("receipt:flushed", decision("flushed", { token, revision, ticket }));
      this.set("receipt:unknown", decision("unknown", { token, revision, ticket }));
    },
    hold(phase, token = "9007199254740993", revision = "1", ticket = baseTicket) {
      this.set("connection", decision(phase, { token, revision, ticket }));
      this.set("status", decision(phase, { token, revision, ticket }));
      this.set("reserve", decision(phase, { token, revision, ticket, matched: false }));
      this.set("receipt:definitelyUnsent", decision(phase, { token, revision, ticket, matched: false }));
    },
  };
  class Bridge {
    constructor() { instances.push(this); }
    transact(inputJson) {
      attempts.push(inputJson);
      const input = JSON.parse(inputJson), key = input.op === "receipt" ? "receipt:" + input.outcome : input.op;
      const queue = queues.get(key);
      const scripted = queue?.length ? (queue.length > 1 ? queue.shift() : queue[0]) : responses.get(key);
      assert.notEqual(scripted, undefined, "unscripted protocol operation " + key);
      // An injected throw has an attempted raw input but no fabricated output.
      const output = typeof scripted === "function" ? scripted(input, inputJson) : scripted;
      const outputJson = typeof output === "string" ? output : JSON.stringify(output);
      transcripts.push({ inputJson, outputJson });
      return outputJson;
    }
  }
  oracle.ready();
  oracle.module = { npc_gold_buy_attempt_abi_version: () => 1, NpcGoldBuyAttemptBridge: Bridge };
  oracle.inputs = op => transcripts.map(row => JSON.parse(row.inputJson)).filter(input => input.op === op);
  return oracle;
}

function fixture() {
  let live = current(), answer = accepted, beforeRead = null;
  const calls = [], sent = [], protocol = protocolOracle(), documentOwner = {};
  let slot = persistentNpcGoldBuyAttemptSlot(protocol.module, documentOwner, "core-test-v1");
  let socket = {}, isOpen = true;
  const runtime = { getMir2NpcGoldBuyPlan(json) {
    const request = JSON.parse(json); calls.push({ json, request });
    const response = answer(request);
    return typeof response === "string" ? response : JSON.stringify(response);
  } };
  const api = new NpcGoldBuyDispatcher({ runtime, read() { beforeRead?.(); return live; },
    getAttemptSlot: () => slot, readSocket: () => socket ? { socket, isOpen } : null });
  return { api, runtime, calls, sent, protocol, documentOwner,
    get slot() { return slot; }, set slot(value) { slot = value; },
    get socket() { return socket; }, set socket(value) { socket = value; }, set isOpen(value) { isOpen = value; },
    get live() { return live; }, set live(value) { live = value; },
    set answer(value) { answer = value; }, set beforeRead(value) { beforeRead = value; },
    prepare(quantity = 2) { return api.prepare(live, quantity); },
    send(prepared, wire = prepared.wire, body = JSON.stringify(wire), target = socket) {
      if (!api.claim(prepared.proof, wire, body, target)) return false;
      sent.push(JSON.parse(body)); return true;
    } };
}

test("quote calls the actual runtime hook with exact full request and preserves Rust values", () => {
  const f = fixture();
  const quote = quoteNpcGoldBuy(f.runtime, f.live, 2);
  assert.deepEqual(quote, accepted({ shop: f.live.shop, quantity: 2 }));
  assert.deepEqual(f.calls[0].request, { shop: f.live.shop, inventory: f.live.inventory, quantity: 2 });
  assert.deepEqual(Object.keys(f.calls[0].request), ["shop", "inventory", "quantity"]);
  assert.equal(f.calls[0].request.shop.goods[0].price, 17.75);
  assert.equal(f.calls[0].request.shop.goods[0].purchase_rate, 0.9);
  assert.deepEqual(f.calls[0].request.shop.goods[1].tooltip_source.fullraw, { unchanged: [1, 2, 3] });
  assert.equal(quote.totalGold, 37); // Never TS price * count / rate.
  assert(Object.isFrozen(quote) && Object.isFrozen(quote.command));
  f.live.presentation = { input: "touch", layout: { mode: "single-pane", logicalWidth: 320 } };
  assert(quoteNpcGoldBuy(f.runtime, f.live, 2)); // No Bevy canvas/status measurement dependency.
});

test("blocked Rust quotes retain computed amounts and never create a sending proof", () => {
  const f = fixture();
  f.answer = () => ({ maxQuantity: 4, totalGold: 123, canBuy: false, blockReason: "inventoryFull", command: null });
  assert.deepEqual(f.api.preview(f.live, 2), { maxQuantity: 4, totalGold: 123,
    canBuy: false, blockReason: "inventoryFull", command: null });
  assert.equal(f.prepare(), null);
  assert.equal(f.sent.length, 0);
  const missing = fixture();
  missing.live.shop.goods[0].tooltip_source = null;
  missing.live.shop.goods[0].purchase_rate = null;
  missing.answer = request => {
    const good = request.shop.goods[0];
    assert.equal(good.requires_gold_buy_plan, true);
    assert.equal(good.tooltip_source, null); assert.equal(good.purchase_rate, null);
    return { maxQuantity: 0, totalGold: null, canBuy: false, blockReason: "invalidSource", command: null };
  };
  assert.equal(quoteNpcGoldBuy(missing.runtime, missing.live, 2).blockReason, "invalidSource");
  assert.equal(missing.prepare(), null); assert.equal(missing.sent.length, 0);
  const absentMarker = fixture(); delete absentMarker.live.shop.goods[0].requires_gold_buy_plan;
  absentMarker.answer = request => {
    assert.equal(Object.hasOwn(request.shop.goods[0], "requires_gold_buy_plan"), false);
    return { maxQuantity: 0, totalGold: null, canBuy: false, blockReason: "invalidInput", command: null };
  };
  assert.equal(absentMarker.prepare(), null); // No manufactured marker or legacy fallback.
});

test("response and wire shapes fail closed without rounding or transport extension", () => {
  const mutations = [
    v => { v.maxQuantity = 1.5; }, v => { v.totalGold = 37.5; }, v => { v.totalGold = 0x1_0000_0000; },
    v => { v.canBuy = 1; }, v => { v.command.itemIndex = 78; }, v => { v.command.itemIndex = Number.MAX_SAFE_INTEGER + 1; },
    v => { v.command.count = 3; }, v => { v.command.count = 2.5; }, v => { v.command.panelType = 1; },
    v => { v.command.requestId = "extra"; }, v => { v.extra = true; }, v => { v.blockReason = "blocked"; },
    v => { v.command = null; }, v => { v.totalGold = null; }, v => { v.canBuy = false; },
  ];
  for (const mutate of mutations) {
    const f = fixture(); f.answer = request => { const result = accepted(request); mutate(result); return result; };
    assert.equal(f.prepare(), null); assert.equal(f.sent.length, 0);
  }
  for (const value of [false, null, undefined, "not json", "{}", "[]"]) {
    const f = fixture(); f.answer = () => value; assert.equal(f.prepare(), null);
  }
});

test("caller capture must match the live snapshot before the first Rust call", () => {
  const f = fixture(), old = copy(f.live);
  f.live.shop.goods[0].unique_id = 900; f.live.shop.selected_id = 900;
  assert.equal(f.api.prepare(old, 2), null); assert.equal(f.calls.length, 0);
  for (const quantity of [-1, 2.1, 65_536, NaN, "2"]) {
    assert.equal(f.prepare(quantity), null);
  }
  assert.equal(f.calls.length, 0);
});

test("returned DTO and opaque token reject mutation, forgery and a foreign dispatcher", () => {
  const f = fixture(), p = f.prepare(); assert(p);
  assert(Object.isFrozen(p) && Object.isFrozen(p.proof) && Object.isFrozen(p.wire));
  assert.equal(Reflect.set(p.wire, "count", 99), false);
  assert.equal(f.api.claim({}, p.wire), false);
  assert.equal(fixture().api.claim(p.proof, p.wire), false);
  assert.equal(f.api.allows(p.proof, { panelType: 0, count: 2, itemIndex: 77, type: "buyItem" }), false); // Body order is part of the exact ticket.
  assert(f.api.allows(p.proof, { ...p.wire }));
  assert(f.send(p)); assert.deepEqual(f.sent, [{ type: "buyItem", itemIndex: 77, count: 2, panelType: 0 }]);
  assert.equal(f.send(p), false); assert.equal(f.sent.length, 1);
  assert.equal(f.calls.length, 3); // prepare, allows, final claim all use Rust.
});

test("final listener changes to every bound owner, model, raw item, wallet or layout reject the send", () => {
  const mutations = [
    s => { s.owner.connectionGeneration++; }, s => { s.owner.sessionGeneration++; }, s => { s.owner.ownerRevision++; }, s => { s.owner.playerObjectId++; },
    s => { s.serviceRevision++; }, s => { s.catalogRevision++; }, s => { s.inventory.gold--; },
    s => { s.inventory.capacity = 46; }, s => { s.inventory.items[0].quantity++; },
    s => { s.inventory.items[0].tooltipSource.userItem.count++; },
    s => { s.shop.goods[1].tooltip_source.fullraw.unchanged.push(4); },
    s => { s.shop.goods[0].unique_id = 99; s.shop.selected_id = 99; },
    s => { s.shop.goods[0].purchase_rate = 0.8; }, s => { s.shop.supports_buy = false; },
    s => { s.presentation.stageCssScale = 0.5; }, s => { s.blocked = true; },
  ];
  for (const mutate of mutations) {
    const f = fixture(), p = f.prepare(), old = copy(f.live); assert(p); mutate(f.live);
    assert.equal(f.send(p), false); assert.equal(f.sent.length, 0);
    f.live = old; assert.equal(f.send(p), false); // Never resurrect a failed claim.
  }
});

test("altered or extended mutable wire is rejected and consumes its token", () => {
  for (const patch of [{ itemIndex: 78 }, { count: 3 }, { panelType: 1 }, { requestId: "x" }, { count: 0 }]) {
    const f = fixture(), p = f.prepare(); assert(p);
    assert.equal(f.send(p, { ...p.wire, ...patch }), false);
    assert.equal(f.send(p), false); assert.equal(f.sent.length, 0);
  }
  const f = fixture(), p = f.prepare(); assert(p);
  const wire = { ...p.wire }; Object.defineProperty(wire, "count", { enumerable: true, get: () => 2 });
  assert.equal(f.send(p, wire), false);
  const g = fixture(), q = g.prepare(); assert(q); const mutable = { ...q.wire };
  g.answer = request => { mutable.count = 3; return accepted(request); };
  assert.equal(g.send(q, mutable), false); assert.equal(g.sent.length, 0);
});

test("runtime refusal, throw, replacement and unavailable source fail at the final claim", () => {
  for (const change of [
    f => { f.answer = () => false; }, f => { f.answer = () => { throw Error("runtime unavailable"); }; },
    f => { f.runtime.getMir2NpcGoldBuyPlan = () => JSON.stringify(accepted({ shop: f.live.shop, quantity: 2 })); },
    f => { f.live = null; }, f => { f.beforeRead = () => { throw Error("source unavailable"); }; },
  ]) {
    const f = fixture(), p = f.prepare(); assert(p); change(f);
    assert.equal(f.send(p), false); assert.equal(f.send(p), false); assert.equal(f.sent.length, 0);
  }
  assert.equal(quoteNpcGoldBuy({}, current(), 2), null);
});

test("runtime mutation during preparation or final revalidation cannot authorize a stale command", () => {
  const a = fixture(); a.answer = request => { a.live.inventory.gold--; return accepted(request); };
  assert.equal(a.prepare(), null);
  const b = fixture(), p = b.prepare(); assert(p);
  b.answer = request => { b.live.shop.goods[1].stock--; return accepted(request); };
  assert.equal(b.send(p), false); assert.equal(b.sent.length, 0);
});

test("read and Rust synchronous reentry cannot claim or prepare a second command", () => {
  const f = fixture(), p = f.prepare(); assert(p); let nested;
  f.beforeRead = () => { nested = f.send(p); assert.equal(f.prepare(), null); };
  assert(f.send(p)); assert.equal(nested, false); assert.equal(f.sent.length, 1);
  const g = fixture(), q = g.prepare(); assert(q);
  g.answer = request => { assert.equal(g.send(q), false); assert.equal(g.prepare(), null); return accepted(request); };
  assert(g.send(q)); assert.equal(g.sent.length, 1);
});

test("a recursive final listener cannot reserve a second proof before the first Core flight enters", () => {
  const f = fixture(), outer = f.prepare(2); assert(outer);
  f.live.shop.selected_id = 78;
  assert.equal(f.prepare(3), null); // Explicit scripted Core reserve refusal.
  f.live.shop.selected_id = 77;
  assert(f.send(outer)); assert.equal(f.sent.length, 1);
  assert.equal(f.prepare(1), null);
  assert.deepEqual(f.protocol.inputs("reserve"), [{ op: "reserve" }, { op: "reserve" }, { op: "reserve" }]);
});

test("spent unchanged authority stays disabled across selection, quantity, layout and source withdrawal", () => {
  const f = fixture(), p = f.prepare(); assert(p); assert(f.send(p)); f.protocol.hold("flushed");
  f.live.shop.selected_id = 78; f.live.presentation.logicalWidth = 900;
  assert.equal(f.prepare(4), null);
  const preview = f.api.preview(f.live, 4);
  assert.deepEqual(preview, { maxQuantity: 17, totalGold: 37, canBuy: false, blockReason: "localPending", command: null });
  const saved = f.live; f.live = null; assert.equal(f.api.preview(null, 4), null);
  f.live = saved; f.live.blocked = true; assert.equal(f.prepare(4), null);
  f.live.blocked = false; assert.equal(f.prepare(4), null); assert.equal(f.sent.length, 1);
});

test("same OPEN connection keeps entered flushed and unknown blocked through every authority change and ABA", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    for (const mutate of [s => { s.inventory.gold--; }, s => { s.shop.goods[1].stock--; },
      s => { s.owner.connectionGeneration++; }, s => { s.owner.sessionGeneration++; },
      s => { s.owner.playerObjectId++; }, s => { s.serviceRevision++; }, s => { s.catalogRevision++; }]) {
      const f = fixture(), p = f.prepare(); assert(p); assert(f.send(p)); const original = copy(f.live);
      f.protocol.hold(phase); mutate(f.live);
      // Explicit current-revision/old-flight DTOs supplied by the protocol oracle.
      // JS neither releases a barrier nor implements authority transitions.
      for (const op of ["connection", "observe", "availability", "status", "reserve"])
        f.protocol.set(op, decision(phase, { revision: "2", flightRevision: "1", matched: op !== "reserve" }));
      assert.equal(f.prepare(), null); assert.equal(f.api.preview(f.live, 2).blockReason, "localPending");
      const state = f.api.status(); assert.equal(state.authorityRevision, "2");
      assert.equal(state.flight.authorityRevision, "1"); assert.equal(state.flight.phase, phase);
      f.live = original;
      for (const op of ["connection", "observe", "availability", "status", "reserve"])
        f.protocol.set(op, decision(phase, { revision: "3", flightRevision: "1", matched: op !== "reserve" }));
      assert.equal(f.prepare(), null); assert.equal(f.sent.length, 1); assert.equal(f.send(p), false);
      assert(f.protocol.inputs("connection").every(input => input.run === "1" && input.connection === "1"));
      assert.equal(f.protocol.inputs("enter").length, 1);
    }
  }
});

test("an unclaimed old proof cannot revive after an observed model ABA", () => {
  const f = fixture(), old = copy(f.live), p = f.prepare(); assert(p);
  f.live.shop.goods[1].stock--; f.protocol.ready("9007199254740994", "2");
  assert(f.api.preview(f.live, 2));
  f.live = old; f.protocol.ready("9007199254740995", "3");
  assert(f.api.preview(f.live, 2));
  assert.equal(f.send(p), false); assert.equal(f.sent.length, 0);
  const fresh = f.prepare(); assert(fresh); assert(f.send(fresh));
  const authorities = f.protocol.inputs("observe").map(input => JSON.parse(input.authority));
  assert(authorities.some(input => input.shop.goods[1].stock === 99));
  assert.equal(authorities.at(-1).shop.goods[1].stock, 100);
  assert.equal(f.protocol.inputs("enter").at(-1).token, "9007199254740995");
});

test("socket throw after claim retains the Core pending barrier without replaying a command", () => {
  const f = fixture(), p = f.prepare(); assert(p); let calls = 0;
  assert.throws(() => { if (f.api.claim(p.proof, p.wire, JSON.stringify(p.wire), f.socket)) {
    calls++; f.api.transportResult(p.proof, "unknown"); throw Error("socket outcome unknown");
  } });
  f.protocol.hold("unknown");
  assert.equal(calls, 1); assert.equal(f.api.claim(p.proof, p.wire, JSON.stringify(p.wire), f.socket), false);
  f.api.rejectIfUnentered(p.proof);
  assert.equal(f.prepare(), null); assert.equal(f.api.preview(f.live, 2).blockReason, "localPending");
  assert.deepEqual(f.protocol.inputs("receipt").map(input => input.outcome), ["unknown", "definitelyUnsent"]);
});

test("non-JSON transport values are refused rather than silently coerced or invoking user callbacks", () => {
  for (const mutate of [s => { s.shop.goods[0].price = NaN; }, s => { s.shop.goods[0].purchase_rate = Infinity; },
    s => { s.shop.goods[0].tooltip_source.fullraw.value = undefined; },
    s => { s.shop.goods[0].tooltip_source.fullraw.value = 1n; },
    s => { s.shop.goods[0].tooltip_source.fullraw.self = s.shop.goods[0].tooltip_source.fullraw; },
    s => { Object.defineProperty(s.shop.goods[0], "price", { enumerable: true, get: () => { throw Error("must not invoke"); } }); },
  ]) {
    const f = fixture(); mutate(f.live); assert.equal(f.prepare(), null); assert.equal(f.calls.length, 0);
  }
});

test("missing raw purchase rate stays null and delegates invalidRate to Rust", () => {
  const f = fixture(); f.live.shop.goods[0].purchase_rate = null;
  f.answer = request => {
    assert.equal(request.shop.goods[0].purchase_rate, null);
    return { maxQuantity: 0, totalGold: null, canBuy: false, blockReason: "invalidRate", command: null };
  };
  assert.deepEqual(quoteNpcGoldBuy(f.runtime, f.live, 2), { maxQuantity: 0, totalGold: null,
    canBuy: false, blockReason: "invalidRate", command: null });
  assert.equal(f.prepare(), null); assert.equal(f.sent.length, 0);
});


// Complete current Page declarations and pure local projections only. This
// consumer harness never imports React, a runtime loader, or generated WASM.
const pagePureSources = {
  identity: "../lib/world-model/item-identity.ts",
  equipment: "../lib/equipment-gateway-adapter.ts",
  parcel: "../lib/mail-parcel-gateway-adapter.ts",
  storage: "../lib/storage-gateway-adapter.ts",
  bag: "../lib/bevy-bag-model.ts",
};
const pagePureRequires = {
  identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment",
    "./world-model/item-identity": "identity", "./mail-parcel-gateway-adapter": "parcel" },
  bag: { "./world-model/item-identity": "identity" },
};
const pagePureModules = new Map();
function loadPagePure(name) {
  assert(Object.hasOwn(pagePureSources, name), "outside pure Page dependency allowlist");
  if (pagePureModules.has(name)) return pagePureModules.get(name);
  const output = ts.transpileModule(readFileSync(new URL(pagePureSources[name], import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const local = { exports: {} };
  new Function("require", "exports", "module", output)(id => {
    assert(Object.hasOwn(pagePureRequires[name], id), "unexpected pure " + name + " dependency: " + id);
    return loadPagePure(pagePureRequires[name][id]);
  }, local.exports, local);
  pagePureModules.set(name, local.exports);
  return local.exports;
}
const actualPageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const actualPageAst = ts.createSourceFile("actual-npc-buy-page.tsx", actualPageSource,
  ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const npcPageNames = ["npcGoldBuyCurrent", "readNpcGoldBuyCurrent", "quoteNpcShopItem",
  "legacyNpcBuyAllowed", "buyNpcShopItem", "retireNpcShopService", "send", "sendRaw",
  "itemCommandRequiresOwner"];
const npcPageDeclarations = new Map();
const npcPageDispatcherCreations = [];
const npcResponseClauses = [];
function visitNpcPage(node) {
  if (ts.isFunctionDeclaration(node) && node.name && npcPageNames.includes(node.name.text)) {
    assert(!npcPageDeclarations.has(node.name.text), "ambiguous actual Page function: " + node.name.text);
    npcPageDeclarations.set(node.name.text, node.getText(actualPageAst));
  }
  if (ts.isNewExpression(node) && ts.isIdentifier(node.expression)
    && node.expression.text === "NpcGoldBuyDispatcher") npcPageDispatcherCreations.push(node.getText(actualPageAst));
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && node.expression.text === "NPCResponse")
    npcResponseClauses.push(node.getText(actualPageAst));
  ts.forEachChild(node, visitNpcPage);
}
visitNpcPage(actualPageAst);
assert.deepEqual([...npcPageDeclarations.keys()].sort(), [...npcPageNames].sort());
assert.equal(npcPageDispatcherCreations.length, 1, "sole actual Page dispatcher creation");
assert.equal(npcResponseClauses.length, 1, "sole actual NPCResponse clause");
const npcResponseJavaScript = ts.transpileModule('function applyNpcResponse(payload) { switch ("NPCResponse") {' + npcResponseClauses[0] + "} }", {
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
const npcPageJavaScript = ts.transpileModule(npcPageNames.map(name => npcPageDeclarations.get(name)).join("\n")
  + "\nfunction createActualNpcDispatcher() { return " + npcPageDispatcherCreations[0] + "; }", {
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
const npcPureBag = loadPagePure("bag");
const npcPureParcel = loadPagePure("parcel");
const npcPureStorage = loadPagePure("storage");

function npcPageWorld() {
  return { connected: true, playerObjectId: "1", playerHp: 100, mapFileName: "D000",
    entities: [{ objectId: "1", x: 4, y: 5, dead: false }],
    inventoryCapacity: 46, maxBagSlots: 40, gold: 100,
    inventoryItems: [{ uniqueId: 100, authoritativeUniqueId: 100, container: "bag1", slot: 0,
      key: "owned", name: "Owned", quantity: 1, icon: 10, description: "owned raw item",
      tooltipSource: { info: { index: 6, stack_size: 20 }, userItem: { unique_id: 100, count: 1 } } }],
    beltItems: [], equipmentItems: [], storageItems: [] };
}
function npcPageService() {
  return { supportsBuy: true, supportsSell: true, serviceRevision: 1, catalogRevision: 1,
    panelType: 0, hideAddedStats: false,
    buyItems: [77, 78].map(id => ({ id, name: "Potion", price: 17.75, purchaseRate: 0.9,
      requiresGoldBuyPlan: true, count: 1, stock: -1, icon: 10, description: "raw catalogue",
      tooltipSource: { info: { index: 5, stack_size: 20, stats: { min_dc: 2 } },
        userItem: { unique_id: id, count: 1, is_shop_item: true }, fullraw: { unchanged: [1, 2, 3] } } })) };
}
function legacyPageCatalogue(fixture) {
  for (const good of fixture.scope.npcShopService.buyItems) {
    good.requiresGoldBuyPlan = false;
    good.tooltipSource = null;
    good.purchaseRate = null;
  }
}

function npcPageHarness({ protocol = protocolOracle(), documentOwner = {}, coreVersion = "core-test-v1", socket: sharedSocket = null } = {}) {
  const sent = [], attempts = [], calls = [], trace = [], proofs = [], legacyProofs = [], retired = [];
  let listener = null, answer = accepted, socketThrows = false, runtimeThrows = false;
  const world = npcPageWorld(), service = npcPageService();
  const slot = persistentNpcGoldBuyAttemptSlot(protocol.module, documentOwner, coreVersion), errors = [];
  const owner = { connectionGeneration: 1, sessionGeneration: 2, ownerRevision: 0 };
  const runtime = { getMir2NpcGoldBuyPlan(json) {
    calls.push({ json, input: JSON.parse(json) }); trace.push("planner");
    if (runtimeThrows) throw Error("controlled missing planner outcome");
    return JSON.stringify(answer(JSON.parse(json)));
  } };
  const socket = sharedSocket ?? { readyState: 1, send(body) {
    trace.push("socket"); attempts.push(body);
    if (socketThrows) throw Error("controlled socket outcome unknown");
    sent.push(JSON.parse(body));
  } };
  const legacyMap = new WeakMap(), realSet = legacyMap.set.bind(legacyMap);
  legacyMap.set = (proof, lease) => { legacyProofs.push(proof); return realSet(proof, lease); };
  const scope = {
    NpcGoldBuyDispatcher, projectBevyBagModel: npcPureBag.projectBevyBagModel,
    npcGoldBuyInventoryRef: { current: new module.exports.NpcGoldBuyInventoryReadiness() },
    mailMutationAllowed: npcPureParcel.mailMutationAllowed,
    isMailItemMutation: npcPureParcel.isMailItemMutation,
    storageMutationAllowed: npcPureStorage.storageMutationAllowed,
    npcShopService: service, npcShopServiceRef: { current: service }, world, worldRef: { current: world },
    runtimeRef: { current: runtime }, questCoreRuntimeRef: { current: { getNpcGoldBuyAttemptSlot: () => slot } },
    console: { error: (...args) => errors.push(args) }, npcBuyDispatcherRef: { current: null },
    npcBuySelectedRef: { current: null }, npcLegacyBuyProofsRef: { current: legacyMap },
    npcShopClockRef: { current: { service: 1, catalog: 1 } },
    setNpcShopService: value => retired.push(value),
    equipmentRenderOwnerToken: owner,
    equipmentConnectionGenerationRef: { current: owner.connectionGeneration },
    equipmentSessionGenerationRef: { current: owner.sessionGeneration },
    equipmentBagOwnerRef: { current: { ownerRevision: owner.ownerRevision } },
    equipmentHostSuspendReasonRef: { current: null },
    equipmentStartGameRef: { current: { ...owner } },
    equipmentControllerRef: { current: { status: () => ({ ready: true, pending: 0 }) } },
    pendingStorageRequestsRef: { current: new Map() }, mailParcelRef: { current: null },
    screenRef: { current: "game" }, socketRef: { current: socket }, WebSocket: { OPEN: 1 },
    currentEquipmentOwner: token => token != null
      && token.connectionGeneration === scope.equipmentConnectionGenerationRef.current
      && token.sessionGeneration === scope.equipmentSessionGenerationRef.current
      && token.ownerRevision === scope.equipmentBagOwnerRef.current.ownerRevision,
    rejectEquipmentCommand: () => false,
    npcShopUiIngressRef: { current: null },
    combatIngressRef: { current: null }, mailDispatcherRef: { current: null },
    mailIngressRef: { current: null }, storageUiIngressRef: { current: null },
    syncMailParcel: () => { throw Error("unexpected Mail projection in idle NPC fixture"); },
    lastCommandRef: { current: null }, movementPredictionBlockedUntilRef: { current: 0 },
    movementDiagnosticsRef: { current: null }, MOVEMENT_ACTION_PREDICTION_BLOCK_MS: 0,
    isSpectatorBrowserMode: () => false,
    isMovementPredictionBlockingCommand: () => false, isMovementCommand: () => false,
    isMovementConsoleCommand: () => false, isCombatResolutionCommand: () => false,
    recordDebugEvent: () => {}, appendLog: () => {}, t: key => key,
    window: { __mir2CommandHistory: [], dispatchEvent(event) {
      assert.equal(event.type, "mir2:action"); trace.push("action"); listener?.(event);
    } },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
  };
  const readinessOwner = { ...owner, playerObjectId:1 };
  scope.npcGoldBuyInventoryRef.current.finish(scope.npcGoldBuyInventoryRef.current.begin(readinessOwner),
    readinessOwner, true, npcPureBag.projectBevyBagModel(world, {npcGoldTrade:true}).model);
  const keys = Object.keys(scope);
  const api = new Function(...keys, npcPageJavaScript + "\nreturn {" + npcPageNames.join(",") + ",createActualNpcDispatcher};")(
    ...keys.map(key => scope[key]));
  // Use the actual Page constructor expression, including its stable runtime
  // getter and read closure. Only trace wrappers surround real methods.
  const dispatcher = api.createActualNpcDispatcher();
  for (const name of ["prepare", "allows", "claim"]) {
    const actual = dispatcher[name].bind(dispatcher);
    dispatcher[name] = (...args) => {
      trace.push(name); const result = actual(...args);
      if (name === "prepare" && result) proofs.push(result);
      return result;
    };
  }
  scope.npcBuyDispatcherRef.current = dispatcher;
  const applyNpcResponse = new Function("retireNpcShopService", "appendLog", npcResponseJavaScript + "\nreturn applyNpcResponse;")(api.retireNpcShopService, scope.appendLog);
  return { ...api, applyNpcResponse, scope, dispatcher, socket, calls, sent, attempts, trace, proofs, legacyProofs, retired, protocol, documentOwner, slot, errors,
    onAction(value) { listener = value; },
    setAnswer(value) { answer = value; }, throwRuntime() { runtimeThrows = true; },
    throwSocket() { socketThrows = true; } };
}

test("actual Page quote delegates raw catalogue UID price rate and whole inventory to Rust", () => {
  const f = npcPageHarness();
  const quote = f.quoteNpcShopItem(77, 2);
  assert.deepEqual(quote, accepted({ shop: { selected_id: 77 }, quantity: 2 }));
  assert.equal(f.calls.length, 1);
  const request = f.calls[0].input;
  assert.deepEqual(Object.keys(request).sort(), ["inventory", "quantity", "shop"]);
  assert.equal(request.shop.selected_id, 77); assert.equal(request.quantity, 2);
  assert.equal(f.readNpcGoldBuyCurrent().owner.playerObjectId, 1);
  assert.equal(request.shop.goods[0].unique_id, 77);
  assert.equal(request.shop.goods[0].price, 17.75);
  assert.equal(request.shop.goods[0].purchase_rate, 0.9);
  assert.equal(request.shop.goods[0].requires_gold_buy_plan, true);
  assert.deepEqual(request.shop.goods[0].tooltip_source, f.scope.npcShopService.buyItems[0].tooltipSource);
  assert.deepEqual(request.shop.goods[1].tooltip_source, f.scope.npcShopService.buyItems[1].tooltipSource);
  assert.deepEqual(request.inventory, npcPureBag.projectBevyBagModel(f.scope.world).model);
  assert.equal(request.inventory.items[0].uniqueId, 100);
  assert.equal(request.inventory.gold, 100);
  assert.equal(f.sent.length, 0); assert.equal(f.proofs.length, 0);
});

test("actual Page Buy callback uses one opaque proof and sendRaw claims only after the action listener", () => {
  const f = npcPageHarness();
  f.onAction(event => {
    assert.equal(event.detail.type, "buyItem");
    assert.equal(f.trace.includes("claim"), false); assert.equal(f.attempts.length, 0);
  });
  f.buyNpcShopItem(77, 2, 0);
  assert.equal(f.proofs.length, 1); assert(Object.isFrozen(f.proofs[0].proof));
  assert.deepEqual(Object.keys(f.proofs[0].proof), []);
  assert.deepEqual(f.trace, ["prepare", "planner", "allows", "planner", "action", "claim", "planner", "socket"]);
  assert.deepEqual(f.sent, [{ type: "buyItem", itemIndex: 77, count: 2, panelType: 0 }]);
  assert.deepEqual(Object.keys(f.sent[0]).sort(), ["count", "itemIndex", "panelType", "type"]);
  assert.equal(f.calls.length, 3); assert.equal(f.legacyProofs.length, 0);
  assert.equal(f.sendRaw(f.proofs[0].wire, { npcBuyProof: f.proofs[0].proof }), false);
  f.protocol.hold("flushed");
  f.buyNpcShopItem(78, 3, 0); assert.equal(f.sent.length, 1);
  const pending = f.quoteNpcShopItem(78, 3);
  assert.equal(pending.canBuy, false); assert.equal(pending.command, null);
  assert.equal(pending.blockReason, "localPending");
});

test("actual Page final listener catalogue replacement rejects even a same-template selected UID", () => {
  for (const mutate of [service => { service.catalogRevision++; },
    service => { service.buyItems[0].id = 79; },
    service => { service.buyItems[0].tooltipSource.fullraw.unchanged[1] = 9; },
    service => { service.buyItems[1].price = 18.25; }]) {
    const f = npcPageHarness();
    f.onAction(() => { const next = copy(f.scope.npcShopServiceRef.current); mutate(next); f.scope.npcShopServiceRef.current = next; });
    f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.proofs.length, 1); assert.equal(f.trace.includes("claim"), true);
    assert.equal(f.attempts.length, 0); assert.equal(f.sent.length, 0);
    assert.equal(f.sendRaw(f.proofs[0].wire, { npcBuyProof: f.proofs[0].proof }), false);
  }
});

test("actual Page NPC service retirement during action withdraws current source before final claim", () => {
  const f = npcPageHarness();
  f.onAction(() => f.retireNpcShopService());
  f.buyNpcShopItem(77, 2, 0);
  assert.equal(f.scope.npcShopClockRef.current.service, 2);
  assert.equal(f.scope.npcShopServiceRef.current, null);
  assert.equal(f.scope.npcBuySelectedRef.current, null); assert.deepEqual(f.retired, [null]);
  assert.equal(f.readNpcGoldBuyCurrent(), null); assert.equal(f.attempts.length, 0);
  assert.equal(f.sendRaw(f.proofs[0].wire, { npcBuyProof: f.proofs[0].proof }), false);
  f.buyNpcShopItem(77, 2, 0); assert.equal(f.proofs.length, 1);
});

test("actual Page synchronous action reentry is refused by the existing reserved Core flight", () => {
  const f = npcPageHarness(); let entered = false;
  f.onAction(() => {
    if (entered) return; entered = true;
    f.buyNpcShopItem(78, 3, 0);
    f.scope.npcBuySelectedRef.current = 77;
  });
  f.buyNpcShopItem(77, 2, 0);
  assert.equal(f.proofs.length, 1);
  assert.equal(f.trace.filter(value => value === "action").length, 1);
  assert.deepEqual(f.sent, [baseWire]); assert.equal(f.attempts.length, 1);
  assert.equal(f.protocol.inputs("reserve").length, 2);
  assert.equal(f.protocol.inputs("bind").length, 1);
  assert.equal(f.sendRaw(f.proofs[0].wire, { npcBuyProof: f.proofs[0].proof }), false);
  f.buyNpcShopItem(77, 1, 0); assert.equal(f.attempts.length, 1);
});

test("actual Page marker with missing raw source or rate delegates refusal without a legacy downgrade", () => {
  for (const missing of ["tooltipSource", "purchaseRate"]) {
    const f = npcPageHarness(); f.scope.npcShopService.buyItems[0][missing] = null;
    f.setAnswer(request => {
      const good = request.shop.goods[0]; assert.equal(good.requires_gold_buy_plan, true);
      assert.equal(good[missing === "tooltipSource" ? "tooltip_source" : "purchase_rate"], null);
      return { maxQuantity: 0, totalGold: null, canBuy: false,
        blockReason: missing === "tooltipSource" ? "invalidSource" : "invalidRate", command: null };
    });
    assert.equal(f.quoteNpcShopItem(77, 2).canBuy, false);
    f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.calls.length, 2); assert.equal(f.proofs.length, 0);
    assert.equal(f.legacyProofs.length, 0); assert.equal(f.attempts.length, 0);
  }
});

test("actual Page unmarked legacy catalogue keeps its exact service proof and earlier quantity bound", () => {
  const f = npcPageHarness(); legacyPageCatalogue(f);
  f.scope.runtimeRef.current = null;
  f.buyNpcShopItem(77, 120, 0);
  assert.deepEqual(f.sent, [{ type: "buyItem", itemIndex: 77, count: 99, panelType: 0 }]);
  assert.equal(f.calls.length, 0); assert.equal(f.proofs.length, 0); assert.equal(f.legacyProofs.length, 1);
  assert.equal(f.scope.npcLegacyBuyProofsRef.current.get(f.legacyProofs[0]).used, true);
  assert.equal(f.sendRaw(f.sent[0], { npcLegacyBuyProof: f.legacyProofs[0] }), false);
  assert.equal(f.attempts.length, 1);
});

test("actual Page unmarked legacy final listener cannot send through changed catalogue service or owner", () => {
  for (const mutate of [f => { const next = copy(f.scope.npcShopServiceRef.current); next.catalogRevision++; f.scope.npcShopServiceRef.current = next; },
    f => f.retireNpcShopService(),
    f => { f.scope.equipmentSessionGenerationRef.current++; }]) {
    const f = npcPageHarness(); legacyPageCatalogue(f);
    f.onAction(() => mutate(f)); f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.legacyProofs.length, 1); assert.equal(f.attempts.length, 0); assert.equal(f.calls.length, 0);
    const proof = f.legacyProofs[0];
    assert.equal(f.scope.npcLegacyBuyProofsRef.current.get(proof).used, true);
    assert.equal(f.sendRaw({ type: "buyItem", itemIndex: 77, count: 2, panelType: 0 }, { npcLegacyBuyProof: proof }), false);
  }
  const stale = npcPageHarness(); stale.scope.npcShopService.buyItems[0].requiresGoldBuyPlan = false;
  const next = copy(stale.scope.npcShopService); next.serviceRevision++; stale.scope.npcShopServiceRef.current = next;
  stale.buyNpcShopItem(77, 2, 0); assert.equal(stale.legacyProofs.length, 0); assert.equal(stale.attempts.length, 0);
});

test("actual Page send and sendRaw deny naked and forged buyItem commands before any event or socket", () => {
  const f = npcPageHarness(), wire = { type: "buyItem", itemIndex: 77, count: 2, panelType: 0 };
  assert.equal(f.sendRaw(wire), false); assert.equal(f.send(wire), false);
  assert.equal(f.sendRaw(wire, { npcBuyProof: Object.freeze({}) }), false);
  assert.equal(f.sendRaw(wire, { npcLegacyBuyProof: Object.freeze({}) }), false);
  assert.equal(f.sendRaw(wire, { npcBuyProof: Object.freeze({}), npcLegacyBuyProof: Object.freeze({}) }), false);
  assert.equal(f.calls.length, 0); assert.equal(f.trace.length, 1); // Forged proof reaches real allows only.
  assert.deepEqual(f.trace, ["allows"]); assert.equal(f.attempts.length, 0);
});

test("actual Page final listener rejects wallet owner socket and runtime changes and retains an unknown-send fence", () => {
  for (const mutate of [f => { const next = copy(f.scope.worldRef.current); next.gold--; f.scope.worldRef.current = next; },
    f => { f.scope.equipmentConnectionGenerationRef.current++; },
    f => { f.scope.equipmentBagOwnerRef.current.ownerRevision++; },
    f => { f.scope.socketRef.current = { readyState: 1, send: () => assert.fail("replacement socket must not send") }; },
    f => { f.scope.runtimeRef.current = null; }, f => f.throwRuntime(),
    f => { f.setAnswer(() => ({ maxQuantity: 0, totalGold: null, canBuy: false, blockReason: "serviceUnavailable", command: null })); }]) {
    const f = npcPageHarness(); f.onAction(() => mutate(f)); f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.proofs.length, 1); assert.equal(f.attempts.length, 0); assert.equal(f.sent.length, 0);
    assert.equal(f.sendRaw(f.proofs[0].wire, { npcBuyProof: f.proofs[0].proof }), false);
  }
  const unknown = npcPageHarness(); unknown.throwSocket();
  unknown.protocol.set("receipt:definitelyUnsent", decision("unknown", { matched: false }));
  assert.doesNotThrow(() => unknown.buyNpcShopItem(77, 2, 0));
  unknown.protocol.hold("unknown");
  assert.equal(unknown.errors.length, 1);
  assert.match(unknown.errors[0][0], /outcome is unknown/);
  assert.deepEqual(unknown.protocol.inputs("receipt").map(input => input.outcome), ["unknown", "definitelyUnsent"]);
  assert.equal(unknown.attempts.length, 1); assert.equal(unknown.sent.length, 0);
  assert.equal(unknown.dispatcher.claim(unknown.proofs[0].proof, unknown.proofs[0].wire), false);
  unknown.buyNpcShopItem(78, 3, 0); assert.equal(unknown.attempts.length, 1);
  assert.equal(unknown.quoteNpcShopItem(77, 2).blockReason, "localPending");
});


// Facade/scripted protocol tests prove JS wiring and refusal, not the Rust
// state transitions. Raw pairs remain in memory; no replay fixture is emitted.
test("actual Core facade forwards all nine exact operations and retains u64 decimal precision", () => {
  const p = protocolOracle(), token = "18446744073709551615", revision = "9007199254740993";
  p.ready(token, revision); const slot = persistentNpcGoldBuyAttemptSlot(p.module, {}, "v1"), producer = {};
  assert(slot.attachProducer(producer)); const socket = {};
  assert.equal(slot.transportFor(producer, socket, socket, true), "1");
  const requests = [{ op: "status" }, { op: "observe", authority: "whole authority" },
    { op: "availability", available: true }, { op: "reserve" },
    { op: "bind", token, ticket: baseTicket }, { op: "allows", token, ticket: baseTicket },
    { op: "enter", token, ticket: baseTicket },
    { op: "receipt", token, ticket: baseTicket, outcome: "unknown" }, { op: "rejectUnpublished", token }];
  for (const request of requests) assert.equal(slot.transact(producer, request).ok, true);
  const actual = p.transcripts.slice(-9);
  assert.deepEqual(actual.map(row => row.inputJson), requests.map(request => JSON.stringify(request)));
  const bound = JSON.parse(actual[4].outputJson);
  assert.equal(bound.state.flight.token, token); assert.equal(bound.state.authorityRevision, revision);
  assert.equal(typeof bound.state.flight.token, "string");
  assert.deepEqual(Object.keys(JSON.parse(baseTicket.body)).sort(), ["count", "itemIndex", "panelType", "type"]);
});

test("actual Core request validators refuse getters extra fields malformed u64 and UTF8 before bridge entry", () => {
  const p = protocolOracle(), slot = persistentNpcGoldBuyAttemptSlot(p.module, {}, "v1"), producer = {};
  assert(slot.attachProducer(producer)); let reads = 0;
  const accessor = {}; Object.defineProperty(accessor, "op", { enumerable: true, get: () => { reads++; return "status"; } });
  const invalid = [accessor, { op: "status", extra: true }, { op: "unknown" }, { op: "availability", available: 1 },
    { op: "observe", authority: "" }, { op: "observe", authority: "\ud800" },
    { op: "observe", authority: "a".repeat(1048577) },
    { op: "bind", token: "1", ticket: { transport: "1", body: "a".repeat(4097) } },
    { op: "receipt", token: "1", ticket: baseTicket, outcome: "ack" }];
  for (const token of [0, 1, "0", "01", "-1", "18446744073709551616", "9007199254740993.0"])
    invalid.push({ op: "rejectUnpublished", token });
  const before = p.transcripts.length;
  for (const request of invalid) assert.throws(() => slot.transact(producer, request), /Invalid NPC purchase request/);
  assert.equal(p.transcripts.length, before); assert.equal(reads, 0);
  assert(npcAttemptRequest({ op: "observe", authority: "😀".repeat(262144) }));
  assert(!npcAttemptRequest({ op: "observe", authority: "😀".repeat(262145) }));
});

test("actual Core result validator rejects noncanonical malformed contradictory or imprecise responses", () => {
  const mutations = [r => { r.extra = true; }, r => { r.state.authorityRevision = 1; },
    r => { r.state.flight.token = 9007199254740992; }, r => { r.state.flight.token = "01"; },
    r => { r.state.flight.token = "18446744073709551616"; }, r => { r.state.canReserve = true; },
    r => { r.state.flight.authorityRevision = "2"; }, r => { r.state.lastPhase = "entered"; },
    r => { r.state.flight.ticket.transport = "0"; }, r => { r.state.flight.ticket.extra = true; },
    r => { r.state.flight.phase = "queued"; }, r => { r.state.flight.ticket.body = "\ud800"; }];
  for (const mutate of mutations) { const response = decision("bound"); mutate(response);
    assert.throws(() => npcAttemptResult(JSON.stringify(response)), /Invalid NPC purchase/); }
  assert.throws(() => npcAttemptResult(" " + JSON.stringify(decision())), /Noncanonical/);
  assert.throws(() => npcAttemptResult('{"ok":true,"ok":true,"matched":true,"state":{}}'), /Noncanonical/);
  const result = npcAttemptResult(JSON.stringify(decision("bound")));
  assert(Object.isFrozen(result.state) && Object.isFrozen(result.state.flight) && Object.isFrozen(result.state.flight.ticket));
});

test("persistent document version facade survives module reevaluation without a second Core bridge", () => {
  const p = protocolOracle(), documentOwner = {}, first = persistentNpcGoldBuyAttemptSlot(p.module, documentOwner, "b17");
  const reevaluated = actualCoreFacade().persistentNpcGoldBuyAttemptSlot(p.module, documentOwner, "b17");
  assert.strictEqual(reevaluated, first); assert.equal(p.instances.length, 1);
  const descriptor = Object.getOwnPropertyDescriptor(documentOwner, Symbol.for("mir2.clientCore.npcGoldBuyAttempt.v1"));
  assert.equal(descriptor.enumerable, false); assert.equal(descriptor.configurable, false); assert.equal(descriptor.writable, false);
  const producer = {}; assert(first.attachProducer(producer));
  const count = p.transcripts.length;
  assert.strictEqual(actualCoreFacade().persistentNpcGoldBuyAttemptSlot(p.module, documentOwner, "different-content"), first);
  assert.deepEqual(p.transcripts.slice(count).map(row => JSON.parse(row.inputJson)), [{ op: "availability", available: false }]);
  assert.equal(first.attachProducer({}), false); assert.equal(first.transact(producer, { op: "status" }).ok, false);
  assert.equal(p.instances.length, 1); // No fresh slot to bypass the existing barrier.
  persistentNpcGoldBuyAttemptSlot(p.module, {}, "b17"); assert.equal(p.instances.length, 2);
});

test("missing incompatible or throwing Core capability closes ordinary Buy while legacy Page remains usable", () => {
  for (const module of [{}, { npc_gold_buy_attempt_abi_version: () => 2, NpcGoldBuyAttemptBridge: class { constructor() { assert.fail("ABI mismatch must not construct"); } } },
    { npc_gold_buy_attempt_abi_version: () => 1, NpcGoldBuyAttemptBridge: class { constructor() { throw Error("controlled constructor failure"); } } }]) {
    const f = fixture(); f.slot = persistentNpcGoldBuyAttemptSlot(module, {}, "v1");
    assert.equal(f.prepare(), null); assert.equal(f.api.preview(f.live, 2), null); assert.equal(f.sent.length, 0);
  }
  const page = npcPageHarness(); page.scope.questCoreRuntimeRef.current = null;
  page.buyNpcShopItem(77, 2, 0); assert.equal(page.proofs.length, 0); assert.equal(page.attempts.length, 0);
  legacyPageCatalogue(page); page.scope.runtimeRef.current = null;
  page.buyNpcShopItem(77, 2, 0); assert.deepEqual(page.sent, [baseWire]);
});

test("producer handoff prevents stale cleanup commands while forwarding only exact old receipts", () => {
  const p = protocolOracle(), slot = persistentNpcGoldBuyAttemptSlot(p.module, {}, "v1"), old = {}, current = {};
  assert(slot.attachProducer(old)); assert(slot.attachProducer(current)); const count = p.transcripts.length;
  assert.equal(slot.withdrawProducer(old), false); assert.equal(slot.attachProducer(old), false);
  assert.equal(slot.transact(old, { op: "reserve" }).ok, false);
  assert.equal(slot.transact(old, { op: "availability", available: false }).ok, false);
  assert.equal(p.transcripts.length, count);
  const nextToken = "9007199254740994";
  p.set("receipt:definitelyUnsent", decision("bound", { token: nextToken, revision: "2", matched: false }));
  p.set("status", decision("bound", { token: nextToken, revision: "2" }));
  const stale = { op: "receipt", token: "9007199254740993", ticket: baseTicket, outcome: "definitelyUnsent" };
  const result = slot.transact(old, stale);
  assert.equal(result.matched, false); assert.equal(result.state.flight.token, nextToken);
  assert.deepEqual(JSON.parse(p.transcripts.at(-1).inputJson), stale);
  assert.equal(slot.transact(current, { op: "status" }).state.flight.token, nextToken);
  assert.equal(slot.transact({}, stale).ok, false);
});

test("actual facade transport identities bind current open object and retain decimal socket tickets", () => {
  const p = protocolOracle(), slot = persistentNpcGoldBuyAttemptSlot(p.module, {}, "v1"), source = {}, a = {}, b = {};
  assert(slot.attachProducer(source));
  assert.equal(slot.transportFor(source, a, b, true), null); assert.equal(slot.transportFor(source, a, a, false), null);
  assert.equal(slot.transportFor(source, a, a, true), "1"); assert.equal(slot.transportFor(source, a, a, true), "1");
  assert.equal(slot.transportFor(source, b, b, true), "2"); assert.equal(slot.transportFor({}, a, a, true), null);
  const count = p.transcripts.length;
  assert.equal(slot.transportFor(source, a, a, true), null);
  assert.equal(p.transcripts.length, count, "old known socket refusal cannot reach or mutate Core");
  assert.deepEqual(p.inputs("connection"), [{ op:"connection",run:"1",connection:"1" },
    { op:"connection",run:"1",connection:"1" }, { op:"connection",run:"1",connection:"2" }]);
  const f = fixture(), proof = f.prepare(); assert(proof);
  f.socket = {}; assert.equal(f.send(proof), false); assert.equal(f.protocol.inputs("enter").length, 0);
});

test("dispatcher requires exact body and exact socket at entry and never exposes a token on the wire", () => {
  for (const alter of [(body, socket) => [body + " ", socket], (body) => [body, {}]]) {
    const f = fixture(), prepared = f.prepare(); assert(prepared);
    const [body, socket] = alter(JSON.stringify(prepared.wire), f.socket);
    assert.equal(f.api.claim(prepared.proof, prepared.wire, body, socket), false);
    assert.equal(f.protocol.inputs("enter").length, 0); assert.equal(f.send(prepared), false);
    f.api.rejectIfUnentered(prepared.proof);
    assert.deepEqual(f.protocol.inputs("receipt").at(-1), { op: "receipt", token: "9007199254740993", ticket: baseTicket, outcome: "definitelyUnsent" });
  }
  const f = fixture(), prepared = f.prepare(); assert(prepared); assert(f.send(prepared));
  assert.deepEqual(f.protocol.inputs("enter"), [{ op: "enter", token: "9007199254740993", ticket: baseTicket }]);
  assert.deepEqual(Object.keys(f.sent[0]), ["type", "itemIndex", "count", "panelType"]);
});

test("failed bind retires only the exact unpublished token and a later manual attempt gets a new token", () => {
  const f = fixture(); f.protocol.set("bind", decision("queued", { matched: false }));
  assert.equal(f.prepare(), null); assert.equal(f.sent.length, 0);
  assert.deepEqual(f.protocol.inputs("rejectUnpublished"), [{ op: "rejectUnpublished", token: "9007199254740993" }]);
  f.protocol.ready("9007199254740994", "1"); const fresh = f.prepare(); assert(fresh); assert(f.send(fresh));
  assert.equal(f.protocol.inputs("enter").at(-1).token, "9007199254740994");
});

test("ownerRevision invalidates a gesture proof but cannot change the entered Core authority", () => {
  const f = fixture(), prepared = f.prepare(); assert(prepared); const before = f.protocol.inputs("observe").at(-1).authority;
  assert.deepEqual(JSON.parse(before).owner, { connectionGeneration: 1, sessionGeneration: 2, playerObjectId: 7 });
  f.live.owner.ownerRevision++; assert.equal(f.send(prepared), false); assert.equal(f.protocol.inputs("enter").length, 0);
  assert.equal(f.protocol.inputs("observe").at(-1).authority, before);
  const g = fixture(), entered = g.prepare(); assert(entered); assert(g.send(entered)); g.protocol.hold("entered");
  const authority = g.protocol.inputs("observe").at(-1).authority;
  g.live.owner.ownerRevision++; g.live.presentation.logicalWidth = 800;
  assert.equal(g.api.preview(g.live, 4).blockReason, "localPending"); assert.equal(g.prepare(4), null);
  assert.equal(g.protocol.inputs("observe").at(-1).authority, authority);
  g.api.dispose(); g.api.activate();
  assert.equal(g.api.preview(g.live, 2).blockReason, "localPending"); assert.equal(g.send(entered), false);
  assert.equal(g.protocol.instances.length, 1);
});

test("player object identity is required and authority changes bind the actual selected player", () => {
  for (const value of [undefined, null, 0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    const f = fixture(); f.live.owner.playerObjectId = value;
    assert.equal(f.prepare(), null); assert.equal(f.calls.length, 0);
  }
  const f = fixture(), old = f.prepare(); assert(old); f.live.owner.playerObjectId = 8;
  assert.equal(f.send(old), false);
  assert.equal(JSON.parse(f.protocol.inputs("observe").at(-1).authority).owner.playerObjectId, 8);
  const page = npcPageHarness(); delete page.scope.world.playerObjectId;
  assert.equal(page.quoteNpcShopItem(77, 2), null); page.buyNpcShopItem(77, 2, 0);
  assert.equal(page.attempts.length, 0); assert.equal(page.calls.length, 0);
});

test("two dispatchers share one document bridge and stale dispose cannot withdraw the newer producer", () => {
  const p = protocolOracle(), documentOwner = {}, first = npcPageHarness({ protocol: p, documentOwner });
  first.buyNpcShopItem(77, 2, 0); p.hold("flushed");
  const next = npcPageHarness({ protocol: p, documentOwner, socket: first.socket }); next.dispatcher.observe();
  const count = p.transcripts.length; first.dispatcher.dispose();
  assert.equal(p.transcripts.length, count); assert.equal(p.instances.length, 1);
  next.buyNpcShopItem(77, 2, 0); assert.equal(next.attempts.length, 0);
  assert.equal(next.quoteNpcShopItem(77, 2).blockReason, "localPending");
  assert.equal(first.sendRaw(first.proofs[0].wire, { npcBuyProof: first.proofs[0].proof }), false);
  assert.equal(first.attempts.length, 1);
  assert(p.inputs("connection").every(input => input.connection === "1"));
});

test("actual Page rejected preentry listener finally submits exact Unsent and permits only a fresh manual token", () => {
  const f = npcPageHarness(); f.onAction(() => { throw Error("controlled listener failure"); });
  assert.doesNotThrow(() => f.buyNpcShopItem(77, 2, 0));
  assert.equal(f.errors.length, 1); assert.match(f.errors[0][0], /before transport/);
  assert.equal(f.protocol.inputs("enter").length, 0); assert.equal(f.attempts.length, 0);
  assert.deepEqual(f.protocol.inputs("receipt"), [{ op: "receipt", token: "9007199254740993", ticket: baseTicket, outcome: "definitelyUnsent" }]);
  const old = f.proofs[0]; f.protocol.ready("9007199254740994", "1"); f.onAction(null);
  assert.equal(f.sendRaw(old.wire, { npcBuyProof: old.proof }), false);
  f.buyNpcShopItem(77, 2, 0); assert.equal(f.attempts.length, 1);
  assert.equal(f.protocol.inputs("enter").at(-1).token, "9007199254740994");
});

test("actual Page Core enter refusal or throw produces zero socket calls and exact final cleanup", () => {
  for (const response of [decision("bound", { matched: false }), () => { throw Error("controlled Core enter failure"); }]) {
    const f = npcPageHarness(); f.protocol.set("enter", response);
    f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.attempts.length, 0); assert.equal(f.sent.length, 0);
    assert.equal(f.protocol.attempts.map(json => JSON.parse(json)).filter(input => input.op === "enter").length, 1);
    // A throwing bridge poisons the facade; no socket fallback/retry is allowed.
    if (typeof response !== "function") assert.equal(f.protocol.inputs("receipt").at(-1).outcome, "definitelyUnsent");
    f.buyNpcShopItem(77, 2, 0); assert.equal(f.attempts.length, 0);
  }
});

test("actual Page enter is directly before socket send and all terminal receipts keep exact token ticket", () => {
  const f = npcPageHarness();
  f.protocol.set("receipt:definitelyUnsent", decision("flushed", { matched: false }));
  f.onAction(() => { assert.equal(f.protocol.inputs("enter").length, 0); });
  f.buyNpcShopItem(77, 2, 0); assert.deepEqual(f.sent, [baseWire]);
  const requests = f.protocol.transcripts.map(row => JSON.parse(row.inputJson));
  const enter = requests.findIndex(input => input.op === "enter");
  assert.deepEqual(requests.slice(enter), [{ op: "enter", token: "9007199254740993", ticket: baseTicket },
    { op: "receipt", token: "9007199254740993", ticket: baseTicket, outcome: "flushed" },
    { op: "receipt", token: "9007199254740993", ticket: baseTicket, outcome: "definitelyUnsent" }]);
  assert.equal(f.trace.at(-2), "planner"); assert.equal(f.trace.at(-1), "socket");
  f.protocol.hold("flushed"); f.dispatcher.withdraw();
  f.buyNpcShopItem(78, 3, 0); assert.equal(f.attempts.length, 1);
});

test("actual Page malformed receipt result poisons the slot without replaying an already sent purchase", () => {
  const f = npcPageHarness(); f.protocol.set("receipt:flushed", "{}");
  f.buyNpcShopItem(77, 2, 0); assert.equal(f.attempts.length, 1); assert.equal(f.sent.length, 1);
  assert.equal(f.protocol.inputs("receipt").filter(input => input.outcome === "flushed").length, 1);
  f.buyNpcShopItem(77, 2, 0); assert.equal(f.attempts.length, 1);
  assert.equal(f.quoteNpcShopItem(77, 2), null);
});


test("actual NPCResponse clause synchronously retires old preentry proof before any later rendered service", () => {
  const p = protocolOracle(), documentOwner = {}, old = npcPageHarness({ protocol: p, documentOwner });
  old.onAction(() => old.applyNpcResponse({ page: ["new service", 1, ""] }));
  old.buyNpcShopItem(77, 2, 0);
  assert.equal(old.scope.npcShopServiceRef.current, null); assert.equal(old.scope.npcShopClockRef.current.service, 2);
  assert.equal(old.attempts.length, 0); assert.equal(p.inputs("enter").length, 0);
  assert.equal(p.inputs("receipt").at(-1).outcome, "definitelyUnsent");
  p.ready("9007199254740994", "2");
  // A subsequent rendered/current service, not a synthetic NPCGoods handler
  // or server exchange. Only the actual Response retirement is proven here.
  const next = npcPageHarness({ protocol: p, documentOwner });
  next.scope.npcShopService.serviceRevision = 2; next.scope.npcShopService.catalogRevision = 2;
  next.buyNpcShopItem(77, 2, 0); assert.equal(next.attempts.length, 1);
  assert.equal(p.inputs("enter").at(-1).token, "9007199254740994");
  assert.equal(old.sendRaw(old.proofs[0].wire, { npcBuyProof: old.proofs[0].proof }), false);
  assert.equal(p.instances.length, 1);
});

test("actual Page player IDs reject noncanonical zero missing fractional and unsafe raw strings", () => {
  for (const id of [null, undefined, 0, 1, "0", "01", "+1", "1.0", "-1", "9007199254740992"]) {
    const f = npcPageHarness(); f.scope.world.playerObjectId = id;
    assert.equal(f.quoteNpcShopItem(77, 2), null); f.buyNpcShopItem(77, 2, 0);
    assert.equal(f.calls.length, 0); assert.equal(f.attempts.length, 0);
  }
  const f = npcPageHarness(); assert(f.quoteNpcShopItem(77, 2));
  assert.equal(f.readNpcGoldBuyCurrent().owner.playerObjectId, 1);
  assert.equal(f.scope.world.playerObjectId, "1");
});

test("temporary missing Core slot recovers with a fresh producer but cannot resurrect an old proof", () => {
  const f = fixture(), slot = f.slot, old = f.prepare(); assert(old);
  f.slot = null; assert.equal(f.api.preview(f.live, 2), null);
  f.slot = slot; f.protocol.ready("9007199254740994", "1");
  const fresh = f.prepare(); assert(fresh); assert.equal(f.send(old), false); assert(f.send(fresh));
  assert.equal(f.protocol.inputs("enter").at(-1).token, "9007199254740994");
  assert.equal(f.protocol.instances.length, 1);
  for (const phase of ["entered", "unknown"]) {
    const pending = fixture(), proof = pending.prepare(); assert(proof); assert(pending.send(proof)); pending.protocol.hold(phase);
    const prior = pending.slot; pending.slot = null; assert.equal(pending.api.preview(pending.live, 2), null);
    pending.slot = prior; assert.equal(pending.api.preview(pending.live, 2).blockReason, "localPending");
    assert.equal(pending.prepare(), null); assert.equal(pending.send(proof), false);
    assert.equal(pending.protocol.instances.length, 1);
  }
});


// Progress66 adds readonly host seams; Progress73 updates connection fixtures
// and the earlier authority-release case to the retained barrier contract. Responses are protocol fixtures,
// never an implementation of the Rust attempt machine or pricing authority.
test("shared UI status reads only an already attached producer and cannot discover or observe a Core", () => {
  const f = fixture(); let gets = 0, reads = 0, sockets = 0;
  const api = new NpcGoldBuyDispatcher({ runtime: f.runtime,
    read: () => { reads++; return f.live; },
    getAttemptSlot: () => { gets++; return f.slot; },
    readSocket: () => { sockets++; return { socket: f.socket, isOpen: true }; },
  });
  const initial = f.protocol.transcripts.length;
  assert.equal(api.status(), null); assert.equal(api.withdrawAt("1"), false);
  assert.deepEqual([gets, reads, sockets], [0, 0, 0]); assert.equal(f.protocol.transcripts.length, initial);
  api.observe(); const before = [gets, reads, sockets, f.calls.length], start = f.protocol.transcripts.length;
  const state = api.status(); assert.equal(state.authorityRevision, "1");
  assert.deepEqual([gets, reads, sockets, f.calls.length], before);
  assert.deepEqual(f.protocol.transcripts.slice(start).map(r => JSON.parse(r.inputJson)), [{ op: "status" }]);
  api.dispose(); const closed = f.protocol.transcripts.length;
  assert.equal(api.status(), null); assert.equal(api.withdrawAt("1"), false);
  assert.equal(f.protocol.transcripts.length, closed);
});

test("shared UI status preserves canonical full-u64 data and does not manufacture receipt history", () => {
  const f = fixture(); f.api.observe();
  for (const revision of ["9007199254740993", "18446744073709551615"]) {
    f.protocol.set("status", decision("unknown", { revision, canReserve: false }));
    const start = f.protocol.transcripts.length, status = f.api.status();
    assert.equal(status.authorityRevision, revision); assert.equal(typeof status.authorityRevision, "string");
    assert.equal(status.flight.phase, "unknown"); assert.equal(status.flight.token, "9007199254740993");
    assert.deepEqual(Object.keys(status).sort(), ["authorityRevision", "canReserve", "flight", "lastPhase"]);
    assert.deepEqual(f.protocol.transcripts.slice(start).map(r => JSON.parse(r.inputJson)), [{ op: "status" }]);
  }
});

test("shared UI exact revision withdrawal rejects malformed and stale strings before availability", () => {
  const f = fixture(); f.api.observe();
  for (const invalid of [undefined, null, 1, "", "0", "01", "+1", "1.0", "18446744073709551616"]) {
    const start = f.protocol.transcripts.length; assert.equal(f.api.withdrawAt(invalid), false);
    assert.equal(f.protocol.transcripts.length, start, String(invalid));
  }
  const start = f.protocol.transcripts.length; assert.equal(f.api.withdrawAt("2"), false);
  assert.deepEqual(f.protocol.transcripts.slice(start).map(r => JSON.parse(r.inputJson)), [{ op: "status" }]);
  const revision = "18446744073709551615";
  f.protocol.queue("status", [decision(null, { revision }), decision(null, { revision, canReserve: false })]);
  f.protocol.set("availability", decision(null, { revision, canReserve: false }));
  const valid = f.protocol.transcripts.length; assert.equal(f.api.withdrawAt(revision), true);
  assert.deepEqual(f.protocol.transcripts.slice(valid).map(r => JSON.parse(r.inputJson)),
    [{ op: "status" }, { op: "availability", available: false }, { op: "status" }]);
});

test("shared UI withdrawal preserves Entered Flushed and Unknown rather than asserting a Buy ACK", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    const f = fixture(); const old = f.prepare(); assert(old); assert(f.send(old));
    f.protocol.hold(phase); f.protocol.set("availability", decision(phase, { canReserve: false }));
    const start = f.protocol.transcripts.length; assert.equal(f.api.withdrawAt("1"), true);
    assert.deepEqual(f.protocol.transcripts.slice(start).map(r => JSON.parse(r.inputJson)),
      [{ op: "status" }, { op: "availability", available: false }, { op: "status" }]);
    const status = f.api.status(); assert.equal(status.flight.phase, phase);
    assert.equal(status.flight.token, "9007199254740993"); assert.equal(f.prepare(3), null);
    assert.equal(f.sent.length, 1); assert.equal(f.protocol.inputs("receipt").length, 0);
  }
});

test("shared UI status and withdrawal fail closed on synchronous producer reentry without cleaning the new lease", () => {
  for (const operation of ["status", "withdrawAt"]) {
    const f = fixture(); f.api.observe(); let once = false;
    f.protocol.set("status", () => {
      if (!once) {
        once = true;
        assert.equal(f.api.status(), null); assert.equal(f.api.withdrawAt("1"), false);
        f.api.dispose(); f.api.activate();
      }
      return decision(null);
    });
    const availabilityBefore = f.protocol.inputs("availability").length;
    const result = operation === "status" ? f.api.status() : f.api.withdrawAt("1");
    assert.equal(result, operation === "status" ? null : false);
    assert.equal(f.protocol.inputs("availability").length - availabilityBefore, 1, "only explicit old-producer disposal, no catch withdrawal of the replacement");
    const after = f.protocol.transcripts.length;
    f.protocol.set("status", decision(null)); f.api.observe(); assert(f.api.status());
    assert(f.protocol.transcripts.length > after); assert.equal(f.protocol.instances.length, 1);
    // The only availability in the rejected call belongs to explicit dispose;
    // the readonly operation must not invoke a catch-withdraw on the new host.
    assert.equal(f.protocol.attempts.map(JSON.parse).filter(i => i.op === "enter").length, 0);
  }
});

test("shared UI availability refusal throw and mismatched post-status cannot certify withdrawal", () => {
  for (const fault of ["refuse", "throw", "wrongRevision", "postAvailable", "postChanged"]) {
    const f = fixture(); f.api.observe();
    if (fault === "refuse") f.protocol.set("availability", decision(null, { matched: false, canReserve: false }));
    if (fault === "throw") f.protocol.set("availability", () => { throw Error("controlled Core unavailable"); });
    if (fault === "wrongRevision") f.protocol.set("availability", decision(null, { revision: "2", canReserve: false }));
    if (fault === "postAvailable" || fault === "postChanged") {
      f.protocol.set("availability", decision(null, { canReserve: false }));
      f.protocol.queue("status", [decision(null), decision(null, {
        revision: fault === "postChanged" ? "2" : "1", canReserve: fault === "postAvailable" })]);
    }
    assert.equal(f.api.withdrawAt("1"), false, fault); assert.equal(f.sent.length, 0);
    assert.equal(f.protocol.inputs("enter").length, 0); assert.equal(f.protocol.inputs("reserve").length, 0);
  }
});


test("final shared UI beforeEntry veto or throw consumes only the unentered proof and never calls Core enter", () => {
  for (const veto of [() => false, () => { throw Error("controlled retired UI checkpoint"); }]) {
    const f = fixture(), prepared = f.prepare(); assert(prepared); let checkpoints = 0;
    const body = JSON.stringify(prepared.wire), before = f.protocol.inputs("enter").length;
    assert.equal(f.api.claim(prepared.proof, prepared.wire, body, f.socket, () => {
      checkpoints++; assert.equal(f.protocol.inputs("enter").length, before); return veto();
    }), false);
    assert.equal(checkpoints, 1); assert.equal(f.protocol.inputs("enter").length, before);
    assert.equal(f.api.claim(prepared.proof, prepared.wire, body, f.socket, () => true), false);
    f.api.rejectIfUnentered(prepared.proof); assert.equal(f.sent.length, 0);
  }
});

test("last shared UI checkpoint follows all callbacks and cannot reenter or revive a consumed proof", () => {
  const f = fixture(), prepared = f.prepare(); assert(prepared); const body = JSON.stringify(prepared.wire);
  const at = f.protocol.transcripts.length; let called = 0;
  assert.equal(f.api.claim(prepared.proof, prepared.wire, body, f.socket, () => {
    called++; assert.equal(f.protocol.inputs("enter").length, 0);
    assert.equal(JSON.parse(f.protocol.transcripts.at(-1).inputJson).op, "allows");
    assert.equal(f.api.claim(prepared.proof, prepared.wire, body, f.socket, () => assert.fail("nested checkpoint")), false);
    assert.equal(f.api.status(), null); assert.equal(f.api.withdrawAt("1"), false); return true;
  }), true);
  assert.equal(called, 1); const operations = f.protocol.transcripts.slice(at).map(r => JSON.parse(r.inputJson).op);
  assert.equal(operations.at(-1), "enter"); assert.equal(operations.filter(op => op === "enter").length, 1);
  assert.equal(f.api.claim(prepared.proof, prepared.wire, body, f.socket, () => assert.fail("reused checkpoint")), false);
});

test("capacity evidence remains in the Rust request and live proof but leaves persistent authority unchanged", () => {
  const f = fixture();
  f.live.inventory.npcGoldTradeCapacity = { rosterValid:true, freshCompatibleUniqueIds:[100] };
  const prepared = f.prepare(); assert(prepared);
  const firstAuthority = f.protocol.inputs("observe").at(-1).authority;
  const decoded = JSON.parse(firstAuthority);
  assert.equal(Object.hasOwn(decoded.inventory, "npcGoldTradeCapacity"), false);
  const { npcGoldTradeCapacity, ...inventory } = f.live.inventory;
  assert.deepEqual(decoded.inventory, inventory);
  assert.deepEqual(f.calls.at(-1).request.inventory.npcGoldTradeCapacity, npcGoldTradeCapacity);
  f.live.inventory.npcGoldTradeCapacity = { rosterValid:false, freshCompatibleUniqueIds:[] };
  assert.equal(f.api.allows(prepared.proof), false, "evidence change retires the exact live proof");
  assert.equal(f.protocol.inputs("observe").at(-1).authority, firstAuthority);
  f.live.inventory.items[0].tooltipSource.userItem.count = 2;
  f.api.observe();
  assert.notEqual(f.protocol.inputs("observe").at(-1).authority, firstAuthority, "actual item metadata stays authoritative");
});

test("evidence-only availability recovery presents the same authority to a held Entered or Unknown Core response", () => {
  for (const phase of ["entered", "unknown"]) {
    const f = fixture();
    f.live.inventory.npcGoldTradeCapacity = { rosterValid:true, freshCompatibleUniqueIds:[100] };
    f.api.observe(); const authority = f.protocol.inputs("observe").at(-1).authority;
    f.protocol.hold(phase);
    f.live.blocked = true;
    f.live.inventory.npcGoldTradeCapacity = { rosterValid:false, freshCompatibleUniqueIds:[] };
    f.api.observe();
    assert.equal(f.protocol.inputs("availability").at(-1).available, false);
    assert.equal(f.prepare(), null);
    f.live.blocked = false;
    f.live.inventory.npcGoldTradeCapacity = { rosterValid:true, freshCompatibleUniqueIds:[100] };
    f.api.observe(); assert.equal(f.protocol.inputs("availability").at(-1).available, true);
    assert.equal(f.prepare(), null); assert.equal(f.api.status().flight.phase, phase);
    assert(f.protocol.inputs("observe").every(input => input.authority === authority));
    assert.equal(f.sent.length, 0);
  }
});

test("inventory readiness retires stale projection stages and owner replacements synchronously", () => {
  const { NpcGoldBuyInventoryReadiness } = module.exports;
  const gate = new NpcGoldBuyInventoryReadiness(), owner = { connectionGeneration:1, sessionGeneration:2, playerObjectId:7 };
  const inventory = {capacity:46,gold:100,items:[]};
  assert.equal(gate.matches(owner, inventory), false);
  const old = gate.begin(owner); gate.invalidate();
  assert.equal(gate.finish(old, owner, true, inventory), false); assert.equal(gate.matches(owner, inventory), false);
  const fresh = gate.begin(owner);
  assert.equal(gate.finish(fresh, {...owner, sessionGeneration:3}, true, inventory), false);
  assert.equal(gate.finish(fresh, owner, false, inventory), false); assert.equal(gate.matches(owner, inventory), false);
  const newest = gate.begin(owner);
  assert(gate.finish(newest, owner, true, inventory)); assert(gate.matches(owner, inventory));
  assert.equal(gate.finish(fresh, owner, true, inventory), false); assert(gate.matches(owner, inventory));
  for (const changed of [{...owner,connectionGeneration:2},{...owner,sessionGeneration:3},{...owner,playerObjectId:8},null]) {
    assert.equal(gate.matches(changed, inventory), false);
  }
  gate.invalidate(); assert.equal(gate.matches(owner, inventory), false);
});

// Exercise the real gateway/parser and Bag projection before the existing NPC
// transport. The fixed blocked response records input only; it proves no Rust
// price, capacity or full-carrier decision and never creates a sending proof.
function loadExpiryPipelineModule(relative, modules = {}, json = JSON) {
  const result = { exports: {} };
  const output = ts.transpileModule(readFileSync(new URL(relative, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("require", "exports", "module", "JSON", output)(name => {
    assert(Object.hasOwn(modules, name), "Unexpected expiry pipeline import " + name);
    return modules[name];
  }, result.exports, result, json);
  return result.exports;
}
const expiryPackets = loadExpiryPipelineModule("../lib/extended-server-packets.ts");
const expiryIdentity = loadExpiryPipelineModule("../lib/world-model/item-identity.ts");
const expiryPresentation = loadExpiryPipelineModule("../lib/world-model/item-presentation.ts");
const expiryBag = loadExpiryPipelineModule("../lib/bevy-bag-model.ts", { "./world-model/item-identity": expiryIdentity });
const blockedExpiryQuote = { maxQuantity: 0, totalGold: null, canBuy: false, blockReason: "invalidSource", command: null };
function expirySnapshotWire(literal, rawChanges = {}) {
  const userItem = { unique_id: 100, item_index: 6, count: 1, current_dura: 3, max_dura: 4,
    expire_info: { expiry_binary_datetime: "__expiry__" }, otherRaw: { untouched: [true, 17, "A"] }, ...rawChanges };
  const snapshot = { type: "worldSnapshot", payload: { inventoryCapacity: 46, maxBagSlots: 40, gold: 100,
    inventoryItems: [{ uniqueId: 100, key: "owned", name: "Owned", quantity: 1, slot: 0, container: "bag1",
      icon: 10, description: "Whole inventory", durabilityCurrent: 3, durabilityMax: 4,
      tooltipSource: { info: { index: 6, stack_size: 20 }, userItem } }], beltItems: [], equipmentItems: [] } };
  return JSON.stringify(snapshot).replace('"__expiry__"', literal).replaceAll('"__unsafe__"', "9007199254740993");
}
function expirySnapshotProjection(parsed) {
  const source = parsed.payload;
  return expiryBag.projectBevyBagModel({ ...source, inventoryItems: source.inventoryItems.map(row => ({
    ...row, ...expiryIdentity.projectInventoryItemIdentity(row.uniqueId, row.container, row.slot),
    ...expiryPresentation.projectItemPresentation(row),
  })) });
}
function recordExpiryRequest(parsed) {
  const projection = expirySnapshotProjection(parsed); assert.equal(projection.ok, true);
  const f = fixture(); f.live.inventory = projection.model;
  const raw = projection.model.items[0].tooltipSource.userItem;
  f.live.shop.goods[0].tooltip_source.userItem.expire_info = copy(raw.expire_info);
  f.answer = () => blockedExpiryQuote;
  assert.deepEqual(quoteNpcGoldBuy(f.runtime, f.live, 2), blockedExpiryQuote);
  f.api.observe();
  return { f, projection, request: f.calls.at(-1).request,
    authority: JSON.parse(f.protocol.inputs("observe").at(-1).authority) };
}

test("canonical expiry strings survive actual WorldSnapshot Bag projection and NPC full request authority", () => {
  for (const expected of ["-9223372036854775808", "9007199254740992", "9007199254740993", "9223372036854775807"]) {
    const parsed = expiryPackets.parseGatewayMailDates(expirySnapshotWire(JSON.stringify(expected)));
    const { f, projection, request, authority } = recordExpiryRequest(parsed);
    assert.equal(projection.model.items[0].tooltipSource, parsed.payload.inventoryItems[0].tooltipSource);
    assert.equal(request.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, expected);
    assert.equal(request.shop.goods[0].tooltip_source.userItem.expire_info.expiry_binary_datetime, expected);
    assert.equal(authority.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, expected);
    assert.equal(authority.shop.goods[0].tooltip_source.userItem.expire_info.expiry_binary_datetime, expected);
    assert.deepEqual(request.inventory.items[0].tooltipSource.userItem.otherRaw, { untouched: [true, 17, "A"] });
    assert.equal(request.inventory.items[0].uniqueId, 100);
    assert.equal(request.inventory.items[0].quantity, 1);
    assert.equal(request.inventory.items[0].durabilityCurrent, 3);
    assert.equal(f.protocol.inputs("reserve").length, 0); assert.equal(f.sent.length, 0);
  }
  const first = recordExpiryRequest(expiryPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740992"')));
  const second = recordExpiryRequest(expiryPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740993"')));
  assert.notEqual(JSON.stringify(first.authority), JSON.stringify(second.authority), "adjacent expiry remains in whole authority");
});

test("old unsafe expiry uses actual source while no-source null is retained through the NPC transport", () => {
  const noSourceJson = { parse(text, reviver) {
    return JSON.parse(text, reviver ? function(key, value) { return reviver.call(this, key, value); } : undefined);
  } };
  const oldPackets = loadExpiryPipelineModule("../lib/extended-server-packets.ts", {}, noSourceJson);
  for (const literal of ["9007199254740992", "9007199254740993"]) {
    const actual = recordExpiryRequest(expiryPackets.parseGatewayMailDates(expirySnapshotWire(literal)));
    assert.equal(actual.request.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, literal);
    const unsupported = recordExpiryRequest(oldPackets.parseGatewayMailDates(expirySnapshotWire(literal)));
    assert.equal(unsupported.request.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, null);
    assert.equal(unsupported.authority.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, null);
    assert.equal(unsupported.f.protocol.inputs("reserve").length, 0);
  }
  const newString = recordExpiryRequest(oldPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740993"')));
  assert.equal(newString.request.inventory.items[0].tooltipSource.userItem.expire_info.expiry_binary_datetime, "9007199254740993");
});

test("expiry transport never rescues adjacent unsafe UID count or dura or grants their item authority", () => {
  for (const field of ["unique_id", "count", "current_dura", "max_dura"]) {
    const parsed = expiryPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740993"', { [field]: "__unsafe__" }));
    const ordinary = JSON.parse(expirySnapshotWire('"9007199254740993"', { [field]: "__unsafe__" }));
    assert.equal(typeof parsed.payload.inventoryItems[0].tooltipSource.userItem[field], "number");
    assert.equal(parsed.payload.inventoryItems[0].tooltipSource.userItem[field], ordinary.payload.inventoryItems[0].tooltipSource.userItem[field]);
    const { f, request } = recordExpiryRequest(parsed);
    assert.equal(typeof request.inventory.items[0].tooltipSource.userItem[field], "number");
    assert.equal(request.inventory.items[0].tooltipSource.userItem[field], ordinary.payload.inventoryItems[0].tooltipSource.userItem[field]);
    assert.equal(f.protocol.inputs("reserve").length, 0);
  }
  const invalidUid = expiryPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740993"'));
  invalidUid.payload.inventoryItems[0].uniqueId = Number("9007199254740993");
  const projection = expirySnapshotProjection(invalidUid); assert.equal(projection.ok, true);
  assert.equal(projection.model.items[0].uniqueId, null, "unsafe outer UID has no concrete Bag authority");
  for (const field of ["quantity", "durabilityCurrent", "durabilityMax"]) {
    const invalid = expiryPackets.parseGatewayMailDates(expirySnapshotWire('"9007199254740993"'));
    invalid.payload.inventoryItems[0][field] = Number("9007199254740993");
    const rejected = expirySnapshotProjection(invalid); assert.equal(rejected.ok, false, field);
    assert.equal(rejected.error.code, "invalidField");
  }
});


test("trusted socket connection is observed before complete authority availability and reserve", () => {
  const f = fixture(), prepared = f.prepare(); assert(prepared);
  const inputs = f.protocol.transcripts.map(row => JSON.parse(row.inputJson));
  const connection = inputs.findIndex(input => input.op === "connection");
  const observed = inputs.findIndex(input => input.op === "observe");
  const available = inputs.findIndex((input, index) => index > observed && input.op === "availability");
  const reserve = inputs.findIndex(input => input.op === "reserve");
  assert(connection >= 0 && connection < observed && observed < available && available < reserve);
  assert.deepEqual(inputs[connection], { op:"connection",run:"1",connection:"1" });
  assert.deepEqual(prepared.wire, baseWire);
  const closed = fixture(); closed.isOpen = false;
  assert.equal(closed.prepare(), null); assert.equal(closed.calls.length, 0);
  for (const op of ["connection", "observe", "reserve", "enter"]) assert.equal(closed.protocol.inputs(op).length, 0);
});

test("public connection forgery and malformed trusted epoch fields never reach the Core bridge", () => {
  const p = protocolOracle(), slot = persistentNpcGoldBuyAttemptSlot(p.module, {}, "v1"), producer = {};
  assert(slot.attachProducer(producer)); const before = p.transcripts.length;
  const valid = { op:"connection",run:"1",connection:"18446744073709551615" };
  assert.equal(npcAttemptConnectionRequest(valid), true); assert.equal(npcAttemptRequest(valid), false);
  for (const request of [valid, { ...valid, extra:true }, { ...valid, run:"0" }, { ...valid, connection:1 }])
    assert.throws(() => slot.transact(producer, request), /Invalid NPC purchase request/);
  for (const value of [0, 1, "0", "01", "+1", "-1", "1.0", "18446744073709551616"])
    for (const key of ["run", "connection"]) assert.equal(npcAttemptConnectionRequest({ ...valid,[key]:value }), false);
  let reads = 0; const getter = { run:"1",connection:"1" };
  Object.defineProperty(getter,"op",{enumerable:true,get(){reads++;return "connection";}});
  assert.equal(npcAttemptConnectionRequest(getter), false);
  assert.equal(p.transcripts.length, before); assert.equal(reads, 0);
});

test("only entered flushed and unknown results may retain a canonical old authority revision", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    const response = decision(phase, { revision:"18446744073709551615",flightRevision:"9007199254740993" });
    const parsed = npcAttemptResult(JSON.stringify(response));
    assert.equal(parsed.state.authorityRevision,"18446744073709551615");
    assert.equal(parsed.state.flight.authorityRevision,"9007199254740993");
    assert(Object.isFrozen(parsed.state.flight));
    assert.throws(() => npcAttemptResult(JSON.stringify(decision(phase, { revision:"1",flightRevision:"2" }))), /Invalid NPC purchase/);
  }
  for (const phase of ["queued", "bound", "definitelyUnsent"])
    assert.throws(() => npcAttemptResult(JSON.stringify(decision(phase, { revision:"2",flightRevision:"1" }))), /Invalid NPC purchase/);
});

test("connection refusal and unavailable old ABI1 operation cannot fall through to authority or reserve", () => {
  for (const answer of [{ok:false,error:"Invalid request"}, decision(null, {matched:false}), "{}"] ) {
    const f = fixture(); f.protocol.set("connection", answer);
    assert.equal(f.prepare(),null); assert.equal(f.calls.length,0);
    for (const op of ["observe","reserve","enter"]) assert.equal(f.protocol.inputs(op).length,0);
    assert.equal(f.protocol.attempts.map(JSON.parse).filter(input=>input.op==="connection").length,1);
    assert.equal(f.sent.length,0);
  }
});

test("preparation read and planner socket changes cannot publish a stale or replacement-socket proof", () => {
  for (const point of ["read", "planner", "observe", "availability", "bind"]) {
    const f = fixture(); let once = false;
    const swap = () => { if (!once) { once = true; f.socket = {}; } };
    if (point === "read") f.beforeRead = swap;
    if (point === "planner") f.answer = request => { swap(); return accepted(request); };
    if (["observe","availability","bind"].includes(point))
      f.protocol.set(point, input => { if (point !== "availability" || input.available) swap();
        return decision(point === "bind" ? "bound" : null); });
    assert.equal(f.prepare(),null,point); assert.equal(f.sent.length,0);
    assert.equal(f.protocol.inputs("enter").length,0);
    if (point === "read" || point === "observe" || point === "availability") assert.equal(f.calls.length,0);
    if (point !== "bind") assert.equal(f.protocol.inputs("reserve").length,0);
    else assert.deepEqual(f.protocol.inputs("receipt"),[{op:"receipt",token:"9007199254740993",ticket:baseTicket,outcome:"definitelyUnsent"}]);
  }
});

test("postproof planner Core allows and final callback socket changes produce zero entry", () => {
  for (const point of ["planner", "allows", "beforeEntry"]) {
    const f = fixture(), prepared = f.prepare(); assert(prepared); const originalSocket = f.socket;
    if (point === "planner") f.answer = request => { f.socket = {}; return accepted(request); };
    if (point === "allows") f.protocol.set("allows", () => { f.socket = {}; return decision("bound"); });
    const checkpoint = point === "beforeEntry" ? () => { f.socket = {}; return f.socket === originalSocket; } : () => true;
    assert.equal(f.api.claim(prepared.proof,prepared.wire,JSON.stringify(prepared.wire),originalSocket,checkpoint),false,point);
    assert.equal(f.protocol.inputs("enter").length,0); assert.equal(f.sent.length,0);
    assert.equal(f.api.claim(prepared.proof,prepared.wire,JSON.stringify(prepared.wire),f.socket),false);
  }
});

test("same socket close reopen HMR and dual producers retain every unresolved phase", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    const f = fixture(), prepared = f.prepare(); assert(prepared); assert(f.send(prepared)); f.protocol.hold(phase);
    f.isOpen = false; assert.equal(f.prepare(),null);
    f.isOpen = true; f.api.dispose(); f.api.activate();
    assert.equal(f.prepare(),null); assert.equal(f.api.preview(f.live,2).blockReason,"localPending");
    const reevaluated = actualCoreFacade().persistentNpcGoldBuyAttemptSlot(f.protocol.module,f.documentOwner,"core-test-v1");
    assert.strictEqual(reevaluated,f.slot);
    const replacement = new NpcGoldBuyDispatcher({runtime:f.runtime,read:()=>f.live,
      getAttemptSlot:()=>reevaluated,readSocket:()=>({socket:f.socket,isOpen:true})});
    replacement.observe(); const after = f.protocol.transcripts.length; f.api.dispose();
    assert.equal(f.protocol.transcripts.length,after,"obsolete producer cleanup cannot withdraw current work");
    assert.equal(replacement.prepare(f.live,2),null); assert.equal(replacement.preview(f.live,2).blockReason,"localPending");
    assert(f.protocol.inputs("connection").every(input=>input.run==="1"&&input.connection==="1"));
    assert.equal(f.protocol.instances.length,1); assert.equal(f.protocol.inputs("enter").length,1);
  }
});

test("new actual socket requires fresh complete observation and a new explicit manual token", () => {
  const f = fixture(), old = f.prepare(); assert(old); assert(f.send(old)); f.protocol.hold("unknown");
  const oldSocket = f.socket, nextSocket = {}; f.socket = nextSocket;
  const nextTicket = {transport:"2",body:JSON.stringify(baseWire)};
  f.protocol.ready("9007199254740994","3",nextTicket);
  f.protocol.set("connection",decision(null,{revision:"2",canReserve:false}));
  const start = f.protocol.transcripts.length;
  f.api.observe(); assert.equal(f.sent.length,1,"connection observation never retries automatically");
  const observed = f.protocol.transcripts.slice(start).map(row=>JSON.parse(row.inputJson));
  assert.deepEqual(observed[0],{op:"connection",run:"1",connection:"2"});
  assert.equal(observed[1].op,"observe"); assert.equal(observed[2].op,"availability");
  const fresh = f.prepare(); assert(fresh); assert.equal(f.sent.length,1);
  assert.equal(f.send(old,old.wire,JSON.stringify(old.wire),oldSocket),false);
  f.protocol.set("receipt:unknown",decision("bound",{token:"9007199254740994",revision:"3",ticket:nextTicket,matched:false}));
  f.protocol.set("status",decision("bound",{token:"9007199254740994",revision:"3",ticket:nextTicket}));
  f.api.transportResult(old.proof,"unknown");
  assert.equal(f.api.status().flight.token,"9007199254740994");
  assert(f.send(fresh)); assert.equal(f.sent.length,2);
  assert.deepEqual(f.protocol.inputs("enter").at(-1),{op:"enter",token:"9007199254740994",ticket:nextTicket});
});

test("returning to a known older socket refuses without any Core operation on the new flight", () => {
  const f = fixture(), originalSocket = f.socket, old = f.prepare(); assert(old);
  f.socket = {}; const nextTicket = {transport:"2",body:JSON.stringify(baseWire)};
  f.protocol.ready("9007199254740994","3",nextTicket);
  f.protocol.set("connection",decision(null,{revision:"2",canReserve:false}));
  const current = f.prepare(); assert(current);
  const before = f.protocol.transcripts.length; f.socket = originalSocket;
  f.api.observe(); assert.equal(f.prepare(),null);
  assert.equal(f.api.claim(current.proof,current.wire,JSON.stringify(current.wire),f.socket),false);
  assert.equal(f.protocol.transcripts.length,before,"old known socket cannot clear or mutate newer Core flight");
  assert.equal(f.protocol.inputs("enter").length,0); assert.equal(f.sent.length,0);
  assert.equal(f.send(old),false);
});


test("connection and Core model callbacks cannot admit an old full model on the current socket", () => {
  for (const point of ["connection", "observe", "availability"]) {
    const f = fixture(); let changed = false;
    f.protocol.set(point,input=>{
      if (!changed && (point !== "availability" || input.available)) {
        changed = true; f.live.inventory.gold--; f.live.shop.goods[1].tooltip_source.fullraw.unchanged.push(4);
      }
      return decision(null,{canReserve:point!=="connection"});
    });
    assert.equal(f.prepare(),null,point); assert.equal(f.sent.length,0);
    assert.equal(f.protocol.inputs("reserve").length,0); assert.equal(f.protocol.inputs("enter").length,0);
    if (point === "connection" || point === "observe") assert.equal(f.calls.length,0);
    if (point === "observe") assert(f.protocol.inputs("availability").every(input=>!input.available));
  }
});


test("final beforeEntry sees same-socket last-read source changes before any Core entry", () => {
  for (const kind of ["inventory", "owner", "sharedHost"]) {
    const f = fixture(); let stage = "prepare", afterAllowsReads = 0, changed = false, hostCurrent = true;
    const originalSocket = f.socket, source = JSON.stringify(f.live);
    const api = new NpcGoldBuyDispatcher({ runtime:f.runtime, read:()=>f.live, getAttemptSlot:()=>f.slot,
      readSocket:()=>{
        // The read after allows plus the final socket read are both actual
        // production callbacks. Inject only in that final read, with the same
        // OPEN socket; the final trusted checkpoint must see the changed source.
        if (stage === "claim" && JSON.parse(f.protocol.transcripts.at(-1).inputJson).op === "allows"
          && ++afterAllowsReads === 2) {
          changed = true;
          if (kind === "inventory") f.live.inventory.gold--;
          if (kind === "owner") f.live.owner.playerObjectId++;
          if (kind === "sharedHost") hostCurrent = false;
        }
        return {socket:originalSocket,isOpen:true};
      },
    });
    const prepared = api.prepare(f.live,2); assert(prepared); stage = "claim";
    let checkpoints = 0, checkpointSawChange = false;
    assert.equal(api.claim(prepared.proof,prepared.wire,JSON.stringify(prepared.wire),originalSocket,()=>{
      checkpoints++; checkpointSawChange = changed;
      return JSON.stringify(f.live) === source && hostCurrent;
    }),false,kind);
    assert.equal(changed,true); assert.equal(checkpointSawChange,true); assert.equal(checkpoints,1);
    assert.equal(f.protocol.inputs("enter").length,0); assert.equal(f.sent.length,0);
    assert.equal(api.claim(prepared.proof,prepared.wire,JSON.stringify(prepared.wire),originalSocket,()=>true),false);
  }
});


test("same-content old72 holder is poisoned and rejected without returning its pre-connection facade", () => {
  for (const phase of ["entered", "flushed", "unknown"]) {
    for (const shape of ["old72", "wrongContract"]) {
      const p = protocolOracle(); p.hold(phase);
      p.set("availability",decision(phase));
      // One existing pure-memory bridge and an immutable old holder. The
      // response is scripted; this fixture does not implement Core phases.
      const bridge = new p.module.NpcGoldBuyAttemptBridge();
      const held = npcAttemptResult(bridge.transact(JSON.stringify({op:"status"}))).state.flight;
      const calls = { transportFor:0,observe:0,reserve:0,attach:0,poison:0 };
      let poisoned = false;
      const oldRuntime = Object.freeze({
        attachProducer(){calls.attach++;return !poisoned;},
        withdrawProducer(){return false;},
        transportFor(){calls.transportFor++;return "1";},
        transact(_source,input){
          if (input.op === "observe") calls.observe++;
          if (input.op === "reserve") calls.reserve++;
          return npcAttemptResult(bridge.transact(JSON.stringify(input)));
        },
      });
      const poison = ()=>{
        calls.poison++; if (poisoned) return; poisoned = true;
        bridge.transact(JSON.stringify({op:"availability",available:false}));
      };
      const documentOwner = {}, key = Symbol.for("mir2.clientCore.npcGoldBuyAttempt.v1");
      const holder = Object.freeze(shape === "old72" ? {version:"same-content",runtime:oldRuntime,poison}
        : {version:"same-content",runtime:oldRuntime,poison,facadeContract:"pre-connection-contract"});
      Object.defineProperty(documentOwner,key,{value:holder,enumerable:false,writable:false,configurable:false});
      const at = p.transcripts.length;
      assert.throws(()=>persistentNpcGoldBuyAttemptSlot(p.module,documentOwner,"same-content"),/Incompatible persistent NPC purchase facade/);
      assert.throws(()=>actualCoreFacade().persistentNpcGoldBuyAttemptSlot(p.module,documentOwner,"same-content"),/Incompatible persistent NPC purchase facade/);
      assert.equal(poisoned,true); assert.equal(calls.poison,2);
      assert.deepEqual({transportFor:calls.transportFor,observe:calls.observe,reserve:calls.reserve,attach:calls.attach},
        {transportFor:0,observe:0,reserve:0,attach:0});
      assert.deepEqual(p.transcripts.slice(at).map(row=>JSON.parse(row.inputJson)),[{op:"availability",available:false}]);
      const descriptor = Object.getOwnPropertyDescriptor(documentOwner,key);
      assert.strictEqual(descriptor.value,holder); assert.strictEqual(descriptor.value.runtime,oldRuntime);
      assert.equal(descriptor.writable,false); assert.equal(descriptor.configurable,false);
      assert.equal(p.instances.length,1,"incompatible holder cannot create a replacement Core");
      assert.deepEqual(npcAttemptResult(bridge.transact(JSON.stringify({op:"status"}))).state.flight,held);
      assert.equal(p.inputs("connection").length,0); assert.equal(p.inputs("observe").length,0);
      assert.equal(p.inputs("reserve").length,0); assert.equal(p.inputs("receipt").length,0);
    }
  }
});

test("matching connection facade contract reuses the immutable holder and socket epoch across HMR", () => {
  const p = protocolOracle(), documentOwner = {}, key = Symbol.for("mir2.clientCore.npcGoldBuyAttempt.v1");
  const first = persistentNpcGoldBuyAttemptSlot(p.module,documentOwner,"same-content"), socket = {}, owner = {};
  assert(first.attachProducer(owner)); assert.equal(first.transportFor(owner,socket,socket,true),"1");
  const holder = Object.getOwnPropertyDescriptor(documentOwner,key).value;
  assert.deepEqual(Object.keys(holder).sort(),["facadeContract","poison","runtime","version"]);
  assert.equal(holder.facadeContract,"npcGoldBuyConnectionBarrier.v1"); assert(Object.isFrozen(holder));
  p.hold("unknown"); p.set("availability",decision("unknown"));
  const at = p.transcripts.length;
  const next = actualCoreFacade().persistentNpcGoldBuyAttemptSlot(p.module,documentOwner,"same-content");
  assert.strictEqual(next,first); assert.equal(p.transcripts.length,at);
  const replacement = {}; assert(next.attachProducer(replacement));
  assert.equal(next.transportFor(replacement,socket,socket,true),"1");
  const count = p.transcripts.length; assert.equal(first.withdrawProducer(owner),false);
  assert.equal(first.transportFor(owner,socket,socket,true),null); assert.equal(p.transcripts.length,count);
  assert.equal(next.transact(replacement,{op:"status"}).state.flight.phase,"unknown");
  assert.equal(p.instances.length,1);
  assert(p.inputs("connection").every(input=>input.run==="1"&&input.connection==="1"));
  assert.strictEqual(Object.getOwnPropertyDescriptor(documentOwner,key).value,holder);
});
